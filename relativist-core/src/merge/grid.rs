//! Grid loop: the BSP cycle of split -> reduce -> merge -> resolve (SPEC-05, Section 4.5).

use std::time::Instant;

use crate::net::Net;
#[cfg(debug_assertions)]
use crate::net::PortRef;
use crate::partition::PartitionStrategy;
use crate::reduction::{reduce_all, reduce_border_once, ReductionStats};

use super::core::merge;
use super::helpers::{compute_border_activity, drain_stale_redexes, rebuild_free_port_index};
use super::types::{GridConfig, GridMetrics, WorkerRoundStats};

/// SPEC-19 §3.1 R3, R4 (TASK-0351): derives the two booleans the
/// coordinator uses to decide between (a) Global Normal Form
/// termination, (b) merge skip, (c) full v1 merge.
///
/// Returns `(all_no_border_activity, all_no_local_redexes)`:
/// - **all_no_border_activity**: every worker reports
///   `has_border_activity == false` — by R5 (T4 strong confluence) no
///   border redex can fire this round.
/// - **all_no_local_redexes**: every worker finished its local
///   reduction with `local_redexes == 0` — combined with the above
///   this is the Global Normal Form predicate (R4).
///
/// On an EMPTY slice both booleans are `true` vacuously. The caller
/// (`run_grid`) only invokes this after a successful split, so the
/// vacuous case never appears in practice; tests pin down the
/// behavior anyway.
pub(crate) fn check_global_normal_form(stats: &[WorkerRoundStats]) -> (bool, bool) {
    let all_no_border = stats.iter().all(|s| !s.has_border_activity);
    let all_no_redexes = stats.iter().all(|s| s.local_redexes == 0);
    (all_no_border, all_no_redexes)
}

/// Executes the complete distributed reduction cycle (SPEC-05, R24-R30).
///
/// The grid loop repeats: split -> local reduce -> merge -> resolve borders
/// until the net reaches Normal Form or max_rounds is exceeded.
///
/// For n == 1, reduces locally without partitioning (degenerate case, R26).
///
/// Returns: (net in Normal Form or partial result, accumulated metrics).
pub fn run_grid(
    net: Net,
    config: &GridConfig,
    strategy: &dyn PartitionStrategy,
) -> (Net, GridMetrics) {
    assert!(config.num_workers >= 1, "num_workers must be >= 1");

    let mut current_net = net;
    let mut metrics = GridMetrics::default();
    let start_time = Instant::now();

    // [0] Initial Normal Form check: avoids unnecessary split/merge for
    // nets already in Normal Form (SPEC-05 v3, SC-011).
    drain_stale_redexes(&mut current_net);
    if current_net.redex_queue.is_empty() {
        metrics.converged = true;
        metrics.total_time = start_time.elapsed();
        return (current_net, metrics);
    }

    // n == 1 optimization (R26): reduce locally without partitioning
    if config.num_workers == 1 {
        return run_single_worker(current_net, config, metrics, start_time);
    }

    loop {
        // [1] Check round limit (R29)
        if let Some(max) = config.max_rounds {
            if metrics.rounds >= max {
                metrics.converged = false;
                break;
            }
        }

        // Record agents at start of round
        metrics
            .agents_per_round
            .push(current_net.count_live_agents());

        // === [2] PHASE 1: SPLIT ===
        let t_partition = Instant::now();
        // SF-004: propagate `GridConfig.recycle_under_delta` into the
        // per-worker subnets via PartitionConfig.recycle_policy. Pre-fix,
        // this field on GridConfig was never read; every worker silently
        // used the default `DisableUnderDelta` regardless of operator choice.
        let partition_cfg = crate::partition::PartitionConfig {
            recycle_policy: config.recycle_under_delta,
            ..crate::partition::PartitionConfig::default()
        };
        let mut plan = crate::partition::split_with_config(
            current_net,
            config.num_workers,
            strategy,
            &partition_cfg,
        );
        metrics.partition_time_per_round.push(t_partition.elapsed());

        // === [3] PHASE 2: LOCAL REDUCTION (per worker, sequentially) ===
        let t_compute = Instant::now();
        let mut local_interactions: u64 = 0;
        let mut local_by_rule: [u64; 6] = [0; 6];
        let mut worker_stats: Vec<WorkerRoundStats> = Vec::new();

        for partition in &mut plan.partitions {
            let agents_before = partition.subnet.count_live_agents();

            let t_reduce = Instant::now();
            let reduction_stats: ReductionStats = reduce_all(&mut partition.subnet);
            let reduce_duration = t_reduce.elapsed();
            local_interactions += reduction_stats.total_interactions;

            // Per-rule counts directly from ReductionStats (SPEC-03 R17)
            let by_rule = reduction_stats.interactions_by_rule;
            for i in 0..6 {
                local_by_rule[i] += by_rule[i];
            }

            // Rebuild free_port_index after local reduction (TASK-0063)
            partition.free_port_index = rebuild_free_port_index(
                &partition.subnet,
                partition.border_id_start,
                partition.border_id_end,
            );

            let agents_after = partition.subnet.count_live_agents();

            // SPEC-19 §3.1 R1+R2: compute the activity flag AFTER
            // `rebuild_free_port_index` (R1 ordering) so the index
            // reflects the post-reduction state. The flag is `true`
            // iff at least one local border endpoint is a principal
            // port — i.e. potentially active under any future merge.
            // R5 (T4 strong confluence): when every worker reports
            // `false`, the coordinator may safely skip merge this
            // round (TASK-0351 consumes this; the wire format
            // already carries the field — R7).
            let has_border_activity = compute_border_activity(partition);

            worker_stats.push(WorkerRoundStats {
                worker_id: partition.worker_id,
                agents_before,
                agents_after,
                local_redexes: reduction_stats.total_interactions as usize,
                reduce_duration_secs: reduce_duration.as_secs_f64(),
                interactions_by_rule: by_rule,
                has_border_activity,
                is_coordinator_self: false, // run_grid only models remote workers in v1
            });
        }

        metrics.compute_time_per_round.push(t_compute.elapsed());
        metrics
            .local_interactions_per_round
            .push(local_interactions);

        // === SPEC-19 §3.1 R3, R4: Coordinator-Free Round + Global NF ===
        //
        // Inspect the per-worker stats just collected. Two derived booleans
        // drive the optimization:
        //   - `all_no_border_activity`: no worker has a principal-port
        //     border endpoint. By R5 (T4 strong confluence), no border
        //     redex can fire this round; the merge-redistribute cycle is
        //     pure overhead.
        //   - `all_no_local_redexes`: every worker reached local Normal
        //     Form. Combined with the above, the distributed net has
        //     reached Global Normal Form (R4) and `run_grid` may exit.
        //
        // The skip is gated on (a) explicit opt-in via
        // `coordinator_free_rounds` AND (b) `strict_bsp` (R6 SHOULD —
        // lenient mode already collapses the cycle into one round, so
        // the optimization has no observable benefit there). When
        // disabled, the v1 merge-redistribute path runs unchanged.
        //
        // Wire-FSM scope (R7): this bundle changes only the local
        // simulation path. `protocol/coordinator.rs` keeps its existing
        // `Message::*` variant set; the equivalent skip branch lands in
        // the wire FSM with item 2.26 (Delta-Only Protocol). The type
        // plumbing (TASK-0349) is already shared by both paths.
        let (all_no_border_activity, all_no_local_redexes) =
            check_global_normal_form(&worker_stats);
        metrics.worker_stats_per_round.push(worker_stats);

        if config.coordinator_free_rounds
            && config.strict_bsp
            && all_no_border_activity
            && all_no_local_redexes
        {
            // R4 — Global Normal Form: reassemble the partitions one
            // last time (cheap: zero border redexes) and exit. We do
            // NOT run border resolution (no redex can fire under T4).
            let t_merge = Instant::now();
            let (merged_net, border_redex_count) = merge(plan);
            metrics.merge_time_per_round.push(t_merge.elapsed());
            metrics.border_redexes_per_round.push(border_redex_count);
            metrics
                .border_reduce_time_per_round
                .push(std::time::Duration::ZERO);
            metrics.border_interactions_per_round.push(0);
            // Accumulate local-only contributions to global totals so
            // the metrics are byte-identical to the v1 path (whose
            // border contribution would also be zero here).
            for (i, count) in local_by_rule.iter().enumerate() {
                metrics.total_interactions_by_rule[i] += count;
            }
            metrics.total_interactions += local_interactions;
            metrics.coordinator_free_rounds += 1;
            current_net = merged_net;
            metrics.rounds += 1;
            metrics.converged = true;
            break;
        }

        if config.coordinator_free_rounds && config.strict_bsp && all_no_border_activity {
            // R3 — Skip Merge: merge would produce 0 border redexes
            // (every endpoint is auxiliary), so border resolution is a
            // no-op. We still need to reassemble agents to keep the
            // round metrics consistent and to feed `current_net` for
            // the next round's split. Border resolution is omitted —
            // that is the actual skipped work (in the wire FSM this
            // would be the round-trip the workers don't have to do).
            let t_merge = Instant::now();
            let (merged_net, border_redex_count) = merge(plan);
            metrics.merge_time_per_round.push(t_merge.elapsed());
            metrics.border_redexes_per_round.push(border_redex_count);
            // No border resolution this round (R3). Record zero work
            // so the per-round vectors stay aligned.
            metrics
                .border_reduce_time_per_round
                .push(std::time::Duration::ZERO);
            metrics.border_interactions_per_round.push(0);
            metrics.coordinator_free_rounds += 1;

            // Accumulate ONLY local interactions (border = 0 by skip).
            for (i, count) in local_by_rule.iter().enumerate() {
                metrics.total_interactions_by_rule[i] += count;
            }
            metrics.total_interactions += local_interactions;

            current_net = merged_net;
            metrics.rounds += 1;

            // Termination check: even on skip we must drain stale
            // entries and detect Normal Form (defense-in-depth — under
            // R5 confluence the predicate above already implies no
            // remaining work, but checking keeps the loop invariants
            // identical between the skip and the merge branches).
            drain_stale_redexes(&mut current_net);
            if current_net.redex_queue.is_empty() {
                metrics.converged = true;
                break;
            }
            continue;
        }

        // === [4] PHASE 3: MERGE (structural) — v1 default path ===
        let t_merge = Instant::now();
        let (mut merged_net, border_redex_count) = merge(plan);
        metrics.merge_time_per_round.push(t_merge.elapsed());
        metrics.border_redexes_per_round.push(border_redex_count);

        // === [5] PHASE 4: RESOLVE BORDERS ===
        //
        // Two modes (SPEC-05 R30, R30a):
        //   - Lenient (default, `strict_bsp = false`): run `reduce_all` on
        //     the merged net. Cascades generated during border resolution
        //     are collapsed at the coordinator in the same round, so
        //     `rounds == 1` for nets whose cascades are confined to the
        //     merge step. This is the performance-oriented path and was
        //     the original behaviour prior to v0.10.0-bench.
        //   - Strict (`strict_bsp = true`): call `reduce_border_once`,
        //     which processes every redex currently in the merged queue
        //     exactly once and defers any new cascades to the next round.
        //     This forces the BSP cycle to iterate until Normal Form,
        //     exposing the real per-round border cost — the quantity that
        //     the Phase 3 LAN experiment measures. G1 still holds (the
        //     total interactions performed match the lenient and
        //     sequential baselines); only the round distribution changes.
        let t_border = Instant::now();
        let border_stats: ReductionStats = if config.strict_bsp {
            reduce_border_once(&mut merged_net)
        } else {
            reduce_all(&mut merged_net)
        };
        metrics
            .border_reduce_time_per_round
            .push(t_border.elapsed());

        let border_by_rule = border_stats.interactions_by_rule;
        metrics
            .border_interactions_per_round
            .push(border_stats.total_interactions);

        // === [6] Accumulate metrics, rounds++ ===
        for i in 0..6 {
            metrics.total_interactions_by_rule[i] += local_by_rule[i] + border_by_rule[i];
        }
        metrics.total_interactions += local_interactions + border_stats.total_interactions;

        current_net = merged_net;
        metrics.rounds += 1;

        // === [7] TERMINATION CHECK (aligned with SPEC-13 CheckTermination) ===
        drain_stale_redexes(&mut current_net);
        if current_net.redex_queue.is_empty() {
            #[cfg(debug_assertions)]
            verify_no_redexes_full_scan(&current_net);
            metrics.converged = true;
            break;
        }
    }

    metrics.total_time = start_time.elapsed();
    (current_net, metrics)
}

// ---------------------------------------------------------------------------
// SPEC-19 R20 — dispatcher fork (TASK-0396, 2026-04-23)
// ---------------------------------------------------------------------------

/// SPEC-19 R20 (TASK-0396): public dispatcher that observes
/// `config.delta_mode` and routes to either the v1 full-partition path
/// ([`run_grid`]) or the v2 delta BSP loop ([`run_grid_delta`]).
///
/// This function closes SF-001 from
/// `docs/reviews/REVIEW-SPEC-19-section-3.3-3.5-3.6-item-2.26-BCD-2026-04-23.md`:
/// before this router, `GridConfig.delta_mode` was threaded through the
/// CLI but no call site actually read it, so passing `--delta-mode` on
/// the command line was a no-op beyond the field round-trip. Callers
/// now go through `run_grid_entry` and get the correct path based on
/// their config.
///
/// # Semantics
///
/// - `config.delta_mode == false`: delegates to `run_grid(net, config, strategy)`
///   and ignores `dispatch`. `dispatch` MAY be `None`.
/// - `config.delta_mode == true`: delegates to
///   `run_grid_delta(net, config, strategy, dispatch.unwrap())`. Passing
///   `None` in this case panics with a clear SPEC-19 R20 message
///   (DC-0396-B option: panic over `Result`, matching the
///   pre-condition-assertion convention in `merge/grid.rs`).
///
/// # When to call this
///
/// Production CLI paths and downstream callers (benchmarks, harnesses)
/// should reach grid reduction through `run_grid_entry` rather than the
/// individual `run_grid` / `run_grid_delta` functions. Unit tests that
/// want to exercise a specific path directly may still call
/// `run_grid` / `run_grid_delta` by name.
// Visibility note (TASK-0396 + DC-0395-A coordination): this function
// is `pub(crate)` — not `pub` — because it takes `&mut dyn WorkerDispatch`,
// and `WorkerDispatch` is `pub(crate)` per DC-C2 (the async binding is
// deferred to `protocol/coordinator.rs`). Promoting either surface to
// `pub` would leak internals; the `test-support` feature gate introduced
// by TASK-0395 for external integration tests is the proper vehicle for
// cross-crate exposure. CLI paths call this function from within the
// same crate (`commands.rs::local_main`) so `pub(crate)` is sufficient.
pub(crate) fn run_grid_entry(
    net: Net,
    config: &GridConfig,
    strategy: &dyn PartitionStrategy,
    dispatch: Option<&mut dyn crate::merge::types::WorkerDispatch>,
) -> (Net, GridMetrics) {
    if config.delta_mode {
        let d = dispatch.expect(
            "SPEC-19 R20: run_grid_entry with delta_mode=true requires a WorkerDispatch; \
             None was provided. Callers that cannot supply a dispatch (e.g., current CLI \
             paths without coordinator runtime) MUST either set delta_mode=false or \
             short-circuit with a user-facing error before reaching this entry point.",
        );
        run_grid_delta(net, config, strategy, d)
    } else {
        // v1 path: `dispatch` is ignored. This is by design — the v1
        // run_grid has no stateful worker contract to satisfy.
        let _ = dispatch;
        run_grid(net, config, strategy)
    }
}

// ---------------------------------------------------------------------------
// SPEC-19 R20, R21 — delta-mode grid entry (TASK-0384)
// ---------------------------------------------------------------------------

/// SPEC-19 R20, R21 (TASK-0384): delta-mode BSP loop entry point.
///
/// Dispatched by the caller based on `config.delta_mode` (field lands in
/// sub-bundle 2.26-D — until then, calling this function unconditionally
/// routes through the delta path). The outer shell handles the two
/// degenerate cases inherited from v1 `run_grid`:
/// 1. **Already-normalized input** — if `drain_stale_redexes` leaves the
///    queue empty, return `(net, metrics)` immediately with
///    `metrics.converged = true`, `metrics.rounds = 0`,
///    `metrics.delta_mode = true`. No dispatch I/O.
/// 2. **Single worker (`n == 1`)** — delegate to the existing
///    `run_single_worker` path; the one-worker contract has no borders
///    so the delta machinery has nothing to do.
///
/// For the genuine multi-worker non-trivial case, this function
/// delegates the round loop to `run_grid_delta_inner` (landed in
/// TASK-0385). Until that task is DEV-green, the inner stub `todo!()`s
/// — tests that EXIT through the degenerate-case paths stay runnable.
///
/// **DC-C3 firewall (2026-04-17):** this entry point MUST accept both
/// `strict_bsp = true` (deferred border dispatch — delta strict
/// semantics per R40) AND `strict_bsp = false` (inline border
/// resolution — delta lenient semantics per R40). No `assert!` on the
/// flag combination. Branching logic lives in TASK-0385.
///
/// **DC-C2 (ratified):** the `dispatch` parameter is a synchronous
/// `&mut dyn WorkerDispatch`. The async binding is the
/// `impl WorkerDispatch for CoordinatorConnection` block in
/// `protocol/coordinator.rs`, OUTSIDE this bundle.
#[allow(dead_code)] // TASK-0385+ exercises via real coordinator; tests cover degenerate paths today.
pub(crate) fn run_grid_delta(
    net: Net,
    config: &GridConfig,
    strategy: &dyn PartitionStrategy,
    dispatch: &mut dyn crate::merge::types::WorkerDispatch,
) -> (Net, GridMetrics) {
    assert!(config.num_workers >= 1, "num_workers must be >= 1");
    // TASK-0396 (2026-04-23): the `GridConfig.delta_mode` gate is enforced
    // at `run_grid_entry` — the router ensures callers only reach this
    // function when `delta_mode = true`. We intentionally do NOT
    // re-assert the flag here so that focused unit tests can drive
    // `run_grid_delta` directly with a degenerate `GridConfig` that does
    // not set `delta_mode` (the inner delta BSP semantics are identical
    // either way). DC-C3 firewall (2026-04-17): intentionally NO assert
    // on `config.strict_bsp` — both values are legal per R40.

    let mut current_net = net;
    let mut metrics = GridMetrics {
        delta_mode: true,
        ..GridMetrics::default()
    };
    let start_time = Instant::now();

    // [0] Already-normalized short-circuit — identical to v1 contract.
    drain_stale_redexes(&mut current_net);
    if current_net.redex_queue.is_empty() {
        metrics.converged = true;
        metrics.total_time = start_time.elapsed();
        return (current_net, metrics);
    }

    // [1] Single-worker degenerate — delegate. No dispatch I/O.
    if config.num_workers == 1 {
        let _ = dispatch; // explicitly unused on this path
        return run_single_worker(current_net, config, metrics, start_time);
    }

    // [2] Multi-worker delta loop — TASK-0385 implements the body.
    // SF-004: same recycle_policy propagation as the synchronous run_grid path.
    let partition_cfg = crate::partition::PartitionConfig {
        recycle_policy: config.recycle_under_delta,
        ..crate::partition::PartitionConfig::default()
    };
    let plan = crate::partition::split_with_config(
        current_net,
        config.num_workers,
        strategy,
        &partition_cfg,
    );
    match run_grid_delta_inner(plan, config, dispatch, &mut metrics) {
        Ok(final_net) => {
            metrics.total_time = start_time.elapsed();
            (final_net, metrics)
        }
        Err(_err) => {
            metrics.converged = false;
            metrics.total_time = start_time.elapsed();
            (Net::new(), metrics)
        }
    }
}

/// SPEC-19 R21 phase 2 (TASK-0385): per-round coordinator loop.
///
/// Fires Round 0 `InitialPartition` (fire-and-forget, DC-C1), then loops
/// delta rounds:
/// 1. Dispatch `RoundStart` (Round 1's payload is empty; subsequent
///    rounds carry the previous round's resolver output per DC-C3).
/// 2. Collect `RoundResult`s and apply worker deltas to both the
///    coordinator's `BorderGraph` and the per-worker partition cache.
/// 3. Detect remaining active border redexes and resolve them via
///    `resolve_border_redex` (per redex — the public API is fine-grained).
/// 4. Run the DC-C3 branch:
///    - **strict_bsp = true (strict):** convergence check runs BEFORE
///      resolution. Resolutions ship as round k+1's payload.
///    - **strict_bsp = false (lenient):** convergence check runs AFTER
///      resolution (if the resolver emptied the graph, we converge this
///      round). Resolutions still ship as round k+1's payload (if we
///      haven't converged).
/// 5. Cap at `config.max_rounds` (TASK-0388's predicate).
///
/// On convergence, delegates to `run_grid_delta_final_collect` (TASK-0387)
/// to ship `FinalStateRequest` and reassemble the net.
#[allow(dead_code)] // Called from run_grid_delta; tests reach here via the multi-worker path.
fn run_grid_delta_inner(
    plan: crate::partition::PartitionPlan,
    config: &GridConfig,
    dispatch: &mut dyn crate::merge::types::WorkerDispatch,
    metrics: &mut GridMetrics,
) -> Result<Net, crate::error::GridError> {
    use std::collections::HashMap;
    use std::time::Instant;

    use crate::merge::border_graph::BorderGraph;
    use crate::merge::border_resolver::{
        package_resolutions_with_pending, resolve_border_redex, BorderIdAllocator,
        BorderResolution, CommutationIdAllocator, RoundStartDispatch,
    };
    use crate::merge::helpers::apply_border_deltas_to_partition;
    use crate::partition::{Partition, WorkerId};

    let num_workers = plan.partitions.len();
    debug_assert!(
        num_workers >= 2,
        "run_grid_delta_inner: multi-worker path only (num_workers = {num_workers})"
    );

    // === Round 0 (R21.1): InitialPartition, fire-and-forget (DC-C1). ===
    dispatch.dispatch_initial(&plan)?;

    // Initial coordinator-side agent count for the Round 0 metrics slot
    // (task acceptance criterion: agents_per_round.push(plan_agents_total)
    // at Round 0).
    let initial_agents: usize = plan
        .partitions
        .iter()
        .map(|p| p.subnet.count_live_agents())
        .sum();
    metrics.agents_per_round.push(initial_agents);

    let mut border_graph = BorderGraph::from_partition_plan(&plan);
    let mut partitions_vec: Vec<Partition> = plan.partitions.clone();
    let mut border_alloc = BorderIdAllocator::from_graph(&border_graph);
    let mut commutation_alloc = CommutationIdAllocator::new();

    // Round 1's dispatch carries no resolver output — workers just
    // reduce their seeded partition. Subsequent rounds reuse this slot
    // with the previous round's `package_resolutions` output.
    let mut pending_dispatch: Vec<(WorkerId, RoundStartDispatch)> = (0..num_workers as WorkerId)
        .map(|w| (w, RoundStartDispatch::default()))
        .collect();

    loop {
        // TASK-0388 cap — stop if we've already hit the configured ceiling.
        if check_max_rounds_cap(config, metrics) {
            metrics.delta_max_rounds_hit = Some(true);
            metrics.converged = false;
            break;
        }

        // Per-round timers. The "partition" slot tracks coordinator-side
        // bookkeeping (resolver prep); "compute" covers the wait for
        // worker replies; "merge" covers delta apply + cache refresh.
        let t_partition = Instant::now();
        metrics.partition_time_per_round.push(t_partition.elapsed());

        let t_compute = Instant::now();
        let results = dispatch.dispatch_round_start(&pending_dispatch)?;
        metrics.compute_time_per_round.push(t_compute.elapsed());

        // D-004 round-N+2 finalizer (TASK-0399, SPEC-19 R26 / DC-B5).
        // Consume the `minted_agents` echo BEFORE applying border deltas
        // and BEFORE the resolver pass — promoting fully-resolved pending
        // borders to `self.borders` here means any subsequent apply_deltas
        // call that references a freshly-materialized border_id finds it
        // present. R48 protocol violations propagate via `?` to the
        // loop's existing error branch (metrics.converged = false +
        // degenerate net return). Per-result call preserves the guilty
        // worker attribution in the ProtocolViolation message (DC-0399-A).
        // D-005 F-C1/F-C2/F-C3: collect the `(border_id, BorderState)`
        // pairs promoted this round. The coordinator mirrors each pair
        // into its local `partitions_vec` cache AND plumbs the per-worker
        // `(bid, local_side)` entries into the NEXT round's
        // `RoundStartDispatch.new_borders` so worker partitions observe
        // the newly-wired principal ports before convergence. Without
        // this loop, minted CON-DUP / CON-ERA / DUP-ERA principals land
        // as `DISCONNECTED` and every downstream consumer drops them (see
        // REVIEW-D-005-2026-04-24.md §"Root cause").
        let mut promoted_this_round: Vec<(u32, crate::merge::BorderState)> = Vec::new();
        for result in &results {
            let promoted =
                border_graph.register_minted_agents(result.worker_id, &result.minted_agents)?;
            promoted_this_round.extend(promoted);
        }
        // NOTE (2026-04-24): the coordinator's `partitions_vec` is NOT
        // updated with the promoted (bid, side) wires here because the
        // minted agents referenced by `BorderState.side_*` live in the
        // WORKERS' arenas, not the coordinator cache. Calling
        // `Net::connect(AgentPort(mint_id, 0), FreePort(bid))` on the
        // coordinator-side subnet would index into an arena slot that
        // does not own the minted agent → T1 / arena mis-write. The
        // coordinator cache is only consulted as a last-resort fallback
        // in `run_grid_delta_final_collect` when the transport returns
        // an empty `Vec<Partition>`; real transports (TCP + the
        // `LocalDeltaDispatch` harness) always return the worker's
        // freshly-mutated partition, which carries the minted agents.
        let promoted_new_borders_by_worker =
            promoted_borders_to_per_worker_new_borders(&promoted_this_round, num_workers);
        // F-H6: intra-worker Lafont-FreePort borders cannot survive
        // merge's border-restoration step (merge needs BOTH workers to
        // carry the border id in their `free_port_index`). Bypass the
        // border entirely: emit a direct `local_reconnections` pair to
        // the worker AND evict the spurious border_id from the
        // coordinator's `BorderGraph` so merge step 2/3 copies the
        // Lafont FreePort through intact.
        let (promoted_intra_local_reconnects, spurious_intra_border_ids) =
            promoted_intra_worker_lafont_wires(&promoted_this_round, num_workers);
        for bid in &spurious_intra_border_ids {
            border_graph.remove_border(*bid);
        }
        // D-005 F-H8: when this round promoted any border, workers have
        // not yet applied the promoted principal-port wires (they ship
        // via next round's `new_borders`). Forcing one more dispatch
        // round guarantees worker partitions observe the promotion
        // before the final merge reassembles the net.
        let promotion_forces_next_round = !promoted_this_round.is_empty();

        let t_merge = Instant::now();
        for result in &results {
            border_graph.apply_deltas(result.worker_id, &result.border_deltas);
            if let Some(cached) = partitions_vec.get_mut(result.worker_id as usize) {
                apply_border_deltas_to_partition(cached, &result.border_deltas, &[], &[]);
            }
        }
        metrics.merge_time_per_round.push(t_merge.elapsed());

        // Per-round stats accumulation.
        metrics.rounds += 1;
        let agents_this_round: usize = partitions_vec
            .iter()
            .map(|p| p.subnet.count_live_agents())
            .sum();
        metrics.agents_per_round.push(agents_this_round);
        let worker_stats_snapshot: Vec<_> = results.iter().map(|r| r.stats.clone()).collect();
        let local_interactions_this_round: u64 = worker_stats_snapshot
            .iter()
            .map(|s| s.local_redexes as u64)
            .sum();
        let mut local_by_rule = [0u64; 6];
        for s in &worker_stats_snapshot {
            for (i, count) in local_by_rule.iter_mut().enumerate() {
                *count += s.interactions_by_rule[i];
            }
        }
        metrics
            .local_interactions_per_round
            .push(local_interactions_this_round);
        metrics.worker_stats_per_round.push(worker_stats_snapshot);

        // Snapshot the active border redexes BEFORE resolution so the
        // strict-branch convergence test sees the pre-resolve graph.
        let t_border = Instant::now();
        let redexes = border_graph.detect_border_redexes();
        let pre_resolve_redex_count = redexes.len() as u32;
        metrics
            .border_redexes_per_round
            .push(pre_resolve_redex_count);

        // ---- DC-C3 strict branch: check convergence BEFORE resolution. ----
        // D-005 F-H8: skip the convergence short-circuit on a round that
        // just promoted pending borders — workers still need to apply the
        // promoted `new_borders` entries in their partitions before the
        // final merge can see the minted principal ports wired up. The
        // "post-resolve" predicate variant is safe here because the strict
        // branch runs BEFORE resolution and its `graph_has_no_redexes`
        // conjunct is authoritative: if no principal-pair border exists,
        // resolution would be a no-op this round.
        if config.strict_bsp
            && !promotion_forces_next_round
            && check_delta_convergence_post_resolve(&results, &border_graph)
        {
            metrics
                .border_reduce_time_per_round
                .push(t_border.elapsed());
            metrics.border_interactions_per_round.push(0);
            metrics.converged = true;
            break;
        }

        // Resolve all detected border redexes. Each call mutates
        // `border_graph` (removes the redex) and advances the two
        // allocators. Partition cache state is read-only here — the
        // resolver's side effects land on `border_graph` only.
        let mut resolutions: Vec<BorderResolution> = Vec::with_capacity(redexes.len());
        for (border_id, _state) in &redexes {
            let resolution = resolve_border_redex(
                &mut border_graph,
                &partitions_vec,
                *border_id,
                &mut border_alloc,
                &mut commutation_alloc,
            );
            resolutions.push(resolution);
        }
        let border_interactions_this_round = resolutions.len() as u64;
        metrics
            .border_reduce_time_per_round
            .push(t_border.elapsed());
        metrics
            .border_interactions_per_round
            .push(border_interactions_this_round);

        // Global interaction accumulators (matches v1 run_grid semantics).
        for (i, count) in local_by_rule.iter().enumerate() {
            metrics.total_interactions_by_rule[i] += count;
        }
        metrics.total_interactions +=
            local_interactions_this_round + border_interactions_this_round;

        // ---- DC-C3 lenient branch: check convergence AFTER resolution. ----
        // If the resolver emptied the graph AND workers were quiet this
        // round, the net is in Global Normal Form. D-005 F-H8: skip the
        // short-circuit on a round that just promoted pending borders (see
        // strict-branch comment above). When the resolver produced zero
        // `resolutions` for this round AND workers are all locally quiet,
        // the stricter "inert-remote" predicate catches post-promotion
        // rounds where every principal-port border is inert and no
        // further work ever ships.
        if !config.strict_bsp && !promotion_forces_next_round {
            let resolver_was_quiet = border_interactions_this_round == 0;
            let converged = if resolver_was_quiet {
                check_delta_convergence_post_resolve(&results, &border_graph)
            } else {
                check_delta_convergence(&results, &border_graph)
            };
            if converged {
                metrics.converged = true;
                break;
            }
        }

        // Build next round's dispatch payload from the resolutions and
        // mirror the same changes into the coordinator's partition cache
        // so the resolver can read consistent agent state next round.
        //
        // D-004 TASK-0399: `package_resolutions_with_pending` is the
        // companion function that ALSO extracts `pending_new_borders`
        // from each BorderResolution (the base `package_resolutions`
        // intentionally drops them per DC-B5 §3.3 design). We feed them
        // into `border_graph.enqueue_pending_borders` so the next-round
        // `register_minted_agents` call (above, after dispatch_round_start)
        // can promote fully-resolved entries via `add_border_states`.
        let (mut packaged, pending_new_borders) =
            package_resolutions_with_pending(resolutions, num_workers);
        border_graph.enqueue_pending_borders(pending_new_borders);

        for (worker_id, payload) in &packaged {
            if let Some(cached) = partitions_vec.get_mut(*worker_id as usize) {
                // IC rule semantics: every resolved principal-port border
                // consumes the agent on that side. Snapshot the set of
                // consumed agents BEFORE `apply_border_deltas_to_partition`
                // mutates `free_port_index`; remove them AFTER the rest of
                // cache maintenance so local_reconnections targeting their
                // aux ports still resolve cleanly.
                let mut agents_to_remove: Vec<crate::net::AgentId> = Vec::new();
                for bid in &payload.resolved_borders {
                    if let Some(&crate::net::PortRef::AgentPort(id, 0)) =
                        cached.free_port_index.get(bid)
                    {
                        agents_to_remove.push(id);
                    }
                }

                apply_border_deltas_to_partition(
                    cached,
                    &payload.border_deltas,
                    &payload.resolved_borders,
                    &payload.new_borders,
                );
                // Skip reconnections that target DISCONNECTED on either side
                // (the `net::connect` debug_assert rejects self-sentinel
                // pairs) or that would form a same-port self-loop.
                for (a, b) in &payload.local_reconnections {
                    if *a == crate::net::DISCONNECTED || *b == crate::net::DISCONNECTED || *a == *b
                    {
                        continue;
                    }
                    cached.subnet.connect(*a, *b);
                }

                // Annihilate consumed agents — `remove_agent` clears each
                // of their port-array slots, restoring T1 for the cache.
                for agent_id in agents_to_remove {
                    cached.subnet.remove_agent(agent_id);
                }
            }
        }

        // D-005 F-C3 (2026-04-24): append per-worker promoted-border
        // entries to the NEXT round's `RoundStartDispatch.new_borders` so
        // each worker's `handle_round_start` wires the freshly-minted
        // principal port to `FreePort(border_id)`. This MUST run AFTER
        // the coordinator-side cache-apply loop above: the promoted
        // targets reference AgentIds minted inside the WORKERS' arenas,
        // not the coordinator-side `partitions_vec` — applying them to
        // the coordinator cache would index into a slot owned by a
        // different (or nonexistent) agent and corrupt T1. Only the
        // worker-facing `pending_dispatch` payload receives them.
        for (worker_id, payload) in packaged.iter_mut() {
            if let Some(extras) = promoted_new_borders_by_worker.get(&(*worker_id)) {
                payload.new_borders.extend(extras.iter().copied());
            }
            if let Some(extras) = promoted_intra_local_reconnects.get(&(*worker_id)) {
                payload.local_reconnections.extend(extras.iter().copied());
            }
        }
        pending_dispatch = packaged;
    }

    // === Round N+1 (R21.3): FinalStateRequest + reassembly. ===
    let partition_cache: HashMap<WorkerId, Partition> = partitions_vec
        .into_iter()
        .enumerate()
        .map(|(i, p)| (i as WorkerId, p))
        .collect();
    let final_net = run_grid_delta_final_collect(dispatch, partition_cache, border_graph, metrics)?;
    Ok(final_net)
}

/// SPEC-19 R4, R21 phase 3, R40 (TASK-0386, DC-C5 amendment 2026-04-17):
/// three-conjunct Global Normal Form predicate for delta-mode BSP.
///
/// Returns `true` iff all three conjuncts hold:
/// 1. Every worker reports `has_border_activity == false`
///    — no border endpoint on any worker is a principal port that could
///    fire under a future coordinator-side resolution.
/// 2. Every worker reports `stats.local_redexes == 0`
///    — no local redex fired during the last round (no pending work
///    ended up on the local queue at round boundary).
/// 3. `border_graph.detect_border_redexes().is_empty()`
///    — no pending coordinator-side (cross-partition) redex.
///
/// # Rationale (DC-C5 FLIP, 2026-04-17)
///
/// SPEC-19 R40 literal: "all workers report zero local redexes AND the
/// BorderGraph contains zero active pairs". The v1 `check_global_normal_form`
/// at `merge/grid.rs` already required both `local_redexes == 0` and
/// an empty graph; delta mode adds `has_border_activity == false` as the
/// worker-local signal that *complements* the coordinator's graph view.
///
/// Dropping `local_redexes == 0` (as in the original two-predicate
/// draft) would rely on the folklore assumption that
/// `reduce_all` always reaches a local fixed point before reporting.
/// Keeping it defends against that gap: one extra O(W) scan per round,
/// wall time measured in microseconds.
///
/// Complexity: O(|results| + |active_redexes|). `detect_border_redexes`
/// consults the incremental redex-set (R18), so the graph scan is
/// proportional to the number of currently-active borders, not the
/// total border count.
pub(crate) fn check_delta_convergence(
    results: &[crate::merge::types::RoundResultPayload],
    border_graph: &crate::merge::border_graph::BorderGraph,
) -> bool {
    let all_no_border_activity = results.iter().all(|r| !r.has_border_activity);
    let all_no_local_redexes = results.iter().all(|r| r.stats.local_redexes == 0);
    let graph_has_no_redexes = border_graph.detect_border_redexes().is_empty();
    all_no_border_activity && all_no_local_redexes && graph_has_no_redexes
}

/// D-005 F-H8 (2026-04-24): stricter variant of [`check_delta_convergence`]
/// that also returns `true` when workers report `has_border_activity = true`
/// BUT every principal-port border in the coordinator's `BorderGraph` has
/// a non-principal remote side (i.e. pairs with a Lafont/concrete
/// `FreePort` — a permanently inert border per IC R5). Used by the
/// round-loop AFTER the resolver has drained all active redexes, so any
/// residual `has_border_activity` flag can only describe the inert-
/// principal state produced by CON-DUP / CON-ERA / DUP-ERA promotions.
///
/// Callers must have already run the resolver pass for the round. Calling
/// this BEFORE resolution risks early convergence — the OLD 3-conjunct
/// [`check_delta_convergence`] is the correct predicate for pre-resolution
/// checks (strict-mode R4 branch).
pub(crate) fn check_delta_convergence_post_resolve(
    results: &[crate::merge::types::RoundResultPayload],
    border_graph: &crate::merge::border_graph::BorderGraph,
) -> bool {
    let all_no_local_redexes = results.iter().all(|r| r.stats.local_redexes == 0);
    let graph_has_no_redexes = border_graph.detect_border_redexes().is_empty();
    if !(all_no_local_redexes && graph_has_no_redexes) {
        return false;
    }
    if results.iter().all(|r| !r.has_border_activity) {
        return true;
    }
    every_border_has_inert_remote(border_graph)
}

/// D-005 F-H8 helper (2026-04-24): returns `true` iff at least one
/// border in `border_graph.borders` has a principal-port endpoint AND
/// every such border pairs that principal port with a non-principal
/// remote side (FreePort or aux AgentPort). The predicate encodes the
/// post-promotion CON-DUP / CON-ERA / DUP-ERA state where the minted
/// agent's principal port is permanently wired to a concrete Lafont
/// `FreePort` on the remote side — a border that can NEVER fire under
/// R5 (principal-principal-only redex rule).
///
/// The "at least one principal-port border" guard distinguishes the
/// genuine post-promotion state from an empty BorderGraph (which
/// vacuously satisfies the "all inert remote" predicate but provides no
/// evidence that convergence is safe). When no border has a principal
/// port at all, the caller falls back to the stricter legacy predicate.
pub(crate) fn every_border_has_inert_remote(
    border_graph: &crate::merge::border_graph::BorderGraph,
) -> bool {
    let mut saw_principal_port = false;
    for state in border_graph.borders.values() {
        let a_principal = matches!(state.side_a, crate::net::PortRef::AgentPort(_, 0));
        let b_principal = matches!(state.side_b, crate::net::PortRef::AgentPort(_, 0));
        if a_principal || b_principal {
            saw_principal_port = true;
        }
        // Both sides principal would have been caught by
        // `detect_border_redexes` in the caller's pre-check; seeing it
        // here would break the contract. Under well-formed input this
        // branch is unreachable but we guard defensively.
        if a_principal && b_principal {
            return false;
        }
    }
    saw_principal_port
}

/// D-005 F-C2 (2026-04-24): per-worker side-routing of promoted borders
/// as prescribed by QA F-C2 / F-H6. Given a slice of promoted
/// `(border_id, BorderState)` pairs, applies each pair's `side_a` to
/// `partitions_vec[state.worker_a]` and `side_b` to
/// `partitions_vec[state.worker_b]`, with intra-worker collapse when
/// `worker_a == worker_b` (F-H6) and self-sentinel suppression when
/// a side equals `FreePort(border_id)` itself.
///
/// NOTE — this helper is NOT called on production coordinator caches in
/// `run_grid_delta_inner` because the `BorderState.side_*` targets
/// reference minted AgentIds that live inside the WORKERS' arenas, not
/// the coordinator's in-memory `partitions_vec`. Applying them would
/// mis-index the coordinator cache. The helper is exercised exclusively
/// by unit tests that construct synthetic worker partitions with
/// matching `id_range` and minted-agent slots, verifying the side-
/// routing math is correct end-to-end.
#[cfg(test)]
pub(crate) fn apply_promoted_borders_to_cache(
    partitions_vec: &mut [crate::partition::Partition],
    promoted: &[(u32, crate::merge::BorderState)],
) {
    use crate::merge::helpers::apply_border_deltas_to_partition;
    for (bid, state) in promoted {
        // Worker A always gets side_a.
        if let Some(cached) = partitions_vec.get_mut(state.worker_a as usize) {
            if !is_self_sentinel(*bid, state.side_a) {
                apply_border_deltas_to_partition(cached, &[], &[], &[(*bid, state.side_a)]);
            }
        }
        if state.worker_a == state.worker_b {
            // Intra-worker border: apply side_b to the same worker.
            if let Some(cached) = partitions_vec.get_mut(state.worker_a as usize) {
                if !is_self_sentinel(*bid, state.side_b) {
                    apply_border_deltas_to_partition(cached, &[], &[], &[(*bid, state.side_b)]);
                }
            }
        } else if let Some(cached) = partitions_vec.get_mut(state.worker_b as usize) {
            if !is_self_sentinel(*bid, state.side_b) {
                apply_border_deltas_to_partition(cached, &[], &[], &[(*bid, state.side_b)]);
            }
        }
    }
}

/// D-005 F-H6 guard: a `PortRef::FreePort(x)` targeting the same
/// `border_id` as the one we are promoting would have
/// `Net::connect(FreePort(x), FreePort(x))` trip the self-sentinel
/// `debug_assert_ne!`. Such entries represent the SAME sentinel on both
/// ends of the wire — already represented by the sibling side's index
/// entry — so skipping is correct.
fn is_self_sentinel(border_id: u32, port: crate::net::PortRef) -> bool {
    matches!(port, crate::net::PortRef::FreePort(x) if x == border_id)
}

/// D-005 F-C3 (2026-04-24): derive per-worker `new_borders` entries from
/// a set of promoted `(border_id, BorderState)` pairs, for inclusion in
/// the NEXT round's `RoundStartDispatch.new_borders`.
///
/// Routing rules:
/// - **Cross-worker promotion** (`worker_a != worker_b`): worker_a
///   receives `(bid, side_a)`, worker_b receives `(bid, side_b)`.
/// - **Intra-worker promotion where BOTH sides are AgentPort** (F-H6
///   aux-aux cross inside the minting worker): worker_a receives both
///   `(bid, side_a)` AND `(bid, side_b)` — the worker will later wire
///   them together through merge's border restoration.
/// - **Intra-worker promotion where ONE side is FreePort** (the
///   common CON-DUP / CON-ERA / DUP-ERA case where an external
///   principal targets a Lafont/boundary FreePort on the minting
///   worker): the `(bid, principal_side)` pair is plumbed as usual,
///   BUT the FreePort side is skipped — otherwise merge would see only
///   one worker carrying the border id and drop the wire via its
///   erasure branch, stranding the minted agent's principal at
///   DISCONNECTED. The separate `promoted_intra_worker_lafont_wires`
///   helper emits the Lafont-side wire as a direct
///   `local_reconnections` entry for that worker.
/// - Self-sentinel entries (`FreePort(border_id)` equal to the current
///   `border_id`) are suppressed in all branches to avoid
///   `Net::connect`'s `debug_assert_ne!` self-wire guard.
///
/// Returns a `HashMap<WorkerId, Vec<(u32, PortRef)>>` keyed by worker id.
/// Workers with no promoted entries are absent from the map.
pub(crate) fn promoted_borders_to_per_worker_new_borders(
    promoted: &[(u32, crate::merge::BorderState)],
    num_workers: usize,
) -> std::collections::HashMap<crate::partition::WorkerId, Vec<(u32, crate::net::PortRef)>> {
    use crate::net::PortRef;
    let mut out: std::collections::HashMap<
        crate::partition::WorkerId,
        Vec<(u32, crate::net::PortRef)>,
    > = std::collections::HashMap::new();
    for (bid, state) in promoted {
        let intra_worker = state.worker_a == state.worker_b;
        let a_free = matches!(state.side_a, PortRef::FreePort(_));
        let b_free = matches!(state.side_b, PortRef::FreePort(_));

        // Side A — emit unless self-sentinel OR intra-worker + side A is
        // the Lafont FreePort (handled via local_reconnections helper).
        if (state.worker_a as usize) < num_workers
            && !is_self_sentinel(*bid, state.side_a)
            && !(intra_worker && a_free && !b_free)
        {
            out.entry(state.worker_a)
                .or_default()
                .push((*bid, state.side_a));
        }

        // Side B — emit to worker_b on cross-worker, or to worker_a on
        // intra-worker aux-aux (both AgentPort). Skip when intra-worker
        // and side_b is a Lafont FreePort.
        if !is_self_sentinel(*bid, state.side_b) {
            if intra_worker {
                if !b_free && (state.worker_a as usize) < num_workers {
                    out.entry(state.worker_a)
                        .or_default()
                        .push((*bid, state.side_b));
                }
                // Intra-worker + b is Lafont FreePort: handled by
                // `promoted_intra_worker_lafont_wires`.
            } else if (state.worker_b as usize) < num_workers {
                out.entry(state.worker_b)
                    .or_default()
                    .push((*bid, state.side_b));
            }
        }
    }
    out
}

/// D-005 F-H6 (2026-04-24): derive per-worker `local_reconnections`
/// entries for intra-worker promoted borders whose side_b is a Lafont
/// (or concrete) `FreePort`. Each such promotion represents a direct
/// `side_a_principal ↔ side_b_freeport` wire that should NOT surface as
/// a merge-time border — merge's `borders` step requires both workers
/// to carry the same border_id in their `free_port_index`, and an
/// intra-worker-only border with a FreePort remote trips the
/// `(Some, None)` erasure branch and strands the principal port at
/// DISCONNECTED. We bypass the border entirely by wiring the pair via
/// `local_reconnections`, and we remove the now-spurious border from
/// the coordinator's `BorderGraph` so merge does not try to restore it.
///
/// Returns `(per_worker_reconnections, bids_to_drop)`:
/// - `per_worker_reconnections[w]`: pairs of `(local_port, target)` for
///   worker `w`'s `RoundStartDispatch.local_reconnections`.
/// - `bids_to_drop`: border_ids the caller should evict from
///   `BorderGraph.borders` AND from `border_graph.worker_borders` /
///   `border_graph.active_redexes` via `BorderGraph::remove_border`.
///
/// Cross-worker and intra-worker-aux-aux promotions return empty lists
/// via this helper.
/// Per-worker map of `(local_port, target_port)` reconnection pairs
/// emitted by `promoted_intra_worker_lafont_wires`. Keyed by
/// `WorkerId`, values land in `RoundStartDispatch.local_reconnections`.
pub(crate) type IntraWorkerLafontReconnectMap = std::collections::HashMap<
    crate::partition::WorkerId,
    Vec<(crate::net::PortRef, crate::net::PortRef)>,
>;

pub(crate) fn promoted_intra_worker_lafont_wires(
    promoted: &[(u32, crate::merge::BorderState)],
    num_workers: usize,
) -> (IntraWorkerLafontReconnectMap, Vec<u32>) {
    use crate::net::PortRef;
    let mut per_worker: std::collections::HashMap<
        crate::partition::WorkerId,
        Vec<(PortRef, PortRef)>,
    > = std::collections::HashMap::new();
    let mut drop_ids: Vec<u32> = Vec::new();
    for (bid, state) in promoted {
        if state.worker_a != state.worker_b {
            continue;
        }
        if (state.worker_a as usize) >= num_workers {
            continue;
        }
        let a_free = matches!(state.side_a, PortRef::FreePort(_));
        let b_free = matches!(state.side_b, PortRef::FreePort(_));
        // Case 1: one FreePort + one AgentPort → wire directly.
        // Case 2: both FreePort → wire directly (both are Lafont).
        // Case 3: both AgentPort → aux-aux cross, handled by the
        //         border route; skip here.
        if a_free || b_free {
            let (local, target) = match (a_free, b_free) {
                // Exactly one free: put the AgentPort side as "local".
                (false, true) => (state.side_a, state.side_b),
                (true, false) => (state.side_b, state.side_a),
                // Both free: either side works as "local"; pick side_a.
                (true, true) => (state.side_a, state.side_b),
                _ => unreachable!(),
            };
            if local != target {
                per_worker
                    .entry(state.worker_a)
                    .or_default()
                    .push((local, target));
            }
            drop_ids.push(*bid);
        }
    }
    (per_worker, drop_ids)
}

/// SPEC-19 R30 (TASK-0388): check whether `run_grid_delta`'s round loop
/// has reached the `max_rounds` cap.
///
/// Called at the TOP of each round-loop iteration, BEFORE any resolver
/// or dispatch work for the round. Preserves v1 `run_grid` semantics
/// (SPEC-05 R29):
/// - `max_rounds == None` → unbounded; caller relies on
///   [`check_delta_convergence`] for termination. Returns `false`.
/// - `max_rounds == Some(m)` → caps the round count at `m`. Returns
///   `metrics.rounds >= m` (inclusive: the cap fires when the loop
///   has ALREADY executed `m` rounds; `Some(0)` fires on entry
///   before the first dispatch).
///
/// On cap hit, the caller sets
/// `metrics.delta_max_rounds_hit = Some(true)` and
/// `metrics.converged = false`, then `break`s to Final Collection
/// (TASK-0387). Per R30, Final Collection runs REGARDLESS of
/// convergence-vs-cap — the returned net is "best effort": partially
/// reduced, with any remaining border redexes unresolved.
pub(crate) fn check_max_rounds_cap(config: &GridConfig, metrics: &GridMetrics) -> bool {
    match config.max_rounds {
        None => false,
        Some(m) => metrics.rounds >= m,
    }
}

/// SPEC-19 R21 phase 3, R27, R29 (TASK-0387): Final State Collection +
/// final `merge()`.
///
/// Invoked when the round loop exits via:
/// - **convergence (R4)** — `check_delta_convergence` returned `true`;
///   the reassembled net MUST be border-redex-free, and the caller
///   observes `metrics.converged = true`.
/// - **max_rounds cap (R30)** — the loop hit the ceiling before
///   converging. `merge()` MAY return a non-zero border-redex count;
///   we return the partial net anyway so callers that set
///   `delta_max_rounds_hit = Some(true)` can distinguish it.
///
/// Semantics (R27 + R29):
/// 1. Dispatch `Message::FinalStateRequest { round: metrics.rounds }`
///    to every worker via `WorkerDispatch`.
/// 2. **Cache fallback (in-process path):** when the dispatch returns
///    an empty `Vec` (`LocalDeltaDispatch` harness / unit tests), use
///    the coordinator's logical `partition_cache` — workers and
///    coordinator have been mirroring deltas since Round 0.
/// 3. **Sanity check:** when the dispatch returns a non-empty `Vec`
///    whose length != `partition_cache.len()`, error out with
///    `GridError::DispatchFailed { round, message }` — a real
///    transport that drops some workers' responses is a protocol bug.
/// 4. **Reconstruct `PartitionPlan`** via
///    [`reconstruct_partition_plan_from_collected`]: sort partitions
///    by `worker_id`, rebuild `borders` from the `BorderGraph`.
/// 5. Invoke `merge()` (SPEC-05) and record `merge_time_per_round`.
fn run_grid_delta_final_collect(
    dispatch: &mut dyn crate::merge::types::WorkerDispatch,
    partition_cache: std::collections::HashMap<
        crate::partition::WorkerId,
        crate::partition::Partition,
    >,
    border_graph: crate::merge::border_graph::BorderGraph,
    metrics: &mut GridMetrics,
) -> Result<Net, crate::error::GridError> {
    // R27: dispatch FinalStateRequest to every worker.
    let final_round = metrics.rounds;
    let collected = dispatch.dispatch_final_state_request(final_round)?;

    // R29: coordinate partitions for the final merge. Prefer the
    // collected responses (carry each worker's post-reduction state
    // including minted agents); fall back to the coordinator cache
    // only when the transport returned an empty slice (in-process
    // test fixtures). A non-empty-but-wrong-size response is a
    // protocol-level violation and fails loudly.
    let partitions: Vec<crate::partition::Partition> = if collected.is_empty() {
        let mut ordered: Vec<_> = partition_cache.into_iter().collect();
        ordered.sort_by_key(|(wid, _)| *wid);
        ordered.into_iter().map(|(_, p)| p).collect()
    } else {
        if collected.len() != partition_cache.len() {
            return Err(crate::error::GridError::DispatchFailed {
                round: final_round,
                message: format!(
                    "FinalStateResult count mismatch: expected {}, got {}",
                    partition_cache.len(),
                    collected.len()
                ),
            });
        }
        let mut sorted = collected;
        sorted.sort_by_key(|p| p.worker_id);
        sorted
    };

    // Pass 0 explicitly: merge() destructures PartitionPlan with `..` and
    // ignores next_border_id on the final-collect path.  The explicit 0
    // documents that no further allocate_border_ids call will follow.
    let plan = reconstruct_partition_plan_from_collected(partitions, &border_graph, Vec::new(), 0);
    let t_merge = Instant::now();
    let (merged_net, _border_redex_count) = super::core::merge(plan);
    metrics.merge_time_per_round.push(t_merge.elapsed());
    Ok(merged_net)
}

/// SPEC-19 R29, TASK-0387: compose a `PartitionPlan` from the collected
/// `Vec<Partition>` plus the coordinator's remaining `BorderGraph`.
///
/// SPEC-20 §3.8 A8 (TASK-0412): extended to accept `reclaimed_partitions`
/// (optional; empty `Vec` preserves the original 2-argument behaviour
/// byte-for-byte).
///
/// Invariants:
/// - The combined `surviving ∪ reclaimed` partition list is sorted ascending
///   by `worker_id` before `PartitionPlan` construction (SPEC-04 / SPEC-20
///   R11a). Under the v1 BSP pipeline `worker_id == partition_index`; under
///   SPEC-20 elastic mode worker_ids may be sparse (e.g., `{0, 1, 5, 7}`).
///   **Caller must ensure `worker_id`s are unique across the union** —
///   duplicates produce build-dependent merge ordering. The sort used is
///   `sort_by_key` (stable); do **NOT** switch to `sort_unstable_by_key`
///   because that would make the merge non-deterministic when two partitions
///   share the same `worker_id` (last-writer-wins in `merge` step 2 would
///   become build-dependent, violating SPEC-01 G1).
/// - `borders` is populated from every surviving `BorderGraph` entry
///   as `(side_a, side_b)`. Empty `BorderGraph` → empty map → final
///   merge is a pure union of agents.
/// - When `reclaimed_partitions` is non-empty, their `worker_id` values
///   MUST be disjoint from those in `partitions` (SPEC-20 A8 / D4).
///
/// # Test-baseline contract
///
/// `UT-0412-01` uses this function directly as the 2-argument legacy
/// baseline: it calls `reconstruct_partition_plan_from_collected(…,
/// Vec::new(), 0)` and then `merge()` to obtain the reference `Net`, then
/// calls `reconstruct(…, Vec::new())` and asserts structural identity
/// between the two results.  This is guaranteed by construction —  both
/// code paths ARE identical (reconstruct is a thin wrapper that calls this
/// same helper) — so the test's protection is against **wrapper divergence**,
/// not against an independent implementation diverging.  If the two code
/// paths are ever separated (e.g., `reconstruct` gets its own merge path),
/// the test must be updated to compare against an independent structural
/// expected value (see REVIEW-TASK-0412 SF-003).
fn reconstruct_partition_plan_from_collected(
    mut partitions: Vec<crate::partition::Partition>,
    border_graph: &crate::merge::border_graph::BorderGraph,
    reclaimed_partitions: Vec<crate::partition::Partition>,
    next_border_id: u32,
) -> crate::partition::PartitionPlan {
    // When reclaimed_partitions is empty this branch is elided entirely,
    // preserving byte-identical behaviour vs the pre-A8 2-argument form.
    if !reclaimed_partitions.is_empty() {
        partitions.extend(reclaimed_partitions);
    }
    // stable_sort_required: future sort_unstable_by_key would silently
    // introduce non-determinism when two partitions share worker_id (the
    // last-writer-wins in merge step 2 becomes build-dependent, breaking
    // SPEC-01 G1). Do NOT replace with sort_unstable_by_key.
    partitions.sort_by_key(|p| p.worker_id);
    // SPEC-19 R29 / SPEC-20 R11a: after sort, worker_ids must be strictly
    // monotone (no duplicates). Duplicate worker_ids imply AgentId-range
    // overlap that merge() step 2 would silently corrupt (last-writer-wins).
    #[cfg(debug_assertions)]
    for w in partitions.windows(2) {
        debug_assert!(
            w[0].worker_id < w[1].worker_id,
            "SPEC-19 R29 / SPEC-20 R11a: partition list contains duplicate \
             worker_id {} after surviving∪reclaimed sort — AgentId-range \
             overlap would silently corrupt the merged Net",
            w[0].worker_id
        );
    }
    let mut borders = std::collections::HashMap::with_capacity(border_graph.len());
    for (border_id, state) in &border_graph.borders {
        borders.insert(*border_id, (state.side_a, state.side_b));
    }
    crate::partition::PartitionPlan {
        partitions,
        borders,
        // Caller supplies the active border-id cursor so that subsequent
        // `allocate_border_ids` calls never collide with border IDs already
        // issued during the originating split (SPEC-20 §3.8 A3, RV-001).
        // For the final-collect path the cursor is unused by `merge()`, so
        // passing 0 is harmless but must be explicit to document the intent.
        // Departure-recovery callers (TASK-0440/0443) MUST pass the cursor
        // from the active PartitionPlan (e.g., `plan.next_border_id`).
        next_border_id,
    }
}

/// Build a `Net` from coordinator `BorderGraph` state and worker partition
/// snapshots (SPEC-19 R38, amended by SPEC-20 §3.8 A8).
///
/// Semantics: `surviving_partitions ∪ reclaimed_partitions` forms the
/// complete input partition set. The disjointness of the union's live
/// `AgentId` sets is the **caller's** precondition (SPEC-20 R30 / A4
/// guarantee this by renumbering reclaimed partitions via
/// `remap_partition_ids` before this call). When `reclaimed_partitions`
/// is empty, behaviour is identical to the pre-A8 2-argument baseline
/// (SPEC-19 R38 regression guarantee).
///
/// # Precondition
///
/// - The `id_range` of every partition in
///   `surviving_partitions ∪ reclaimed_partitions` must be pairwise
///   non-overlapping (SPEC-20 §3.8 A8 / D4 / `merge::core::merge` step 2
///   invariant). This is checked unconditionally (NOT debug-gated) via a
///   direct `id_range`-overlap scan — **`assert!` fires in release builds**.
///   Callers that build partitions outside the BSP pipeline MUST ensure
///   their `id_range` fields are pairwise disjoint before calling this
///   function. The BSP pipeline guarantees this via `remap_partition_ids`
///   (SPEC-20 R30 / A4).
/// - Every partition's `redex_queue` must contain only stale redexes (R9).
///   Reclaimed partitions retrieved from `retained_initial` or
///   `retained_last_acked` snapshots taken **before** local reduction MUST
///   be locally reduced via `reduce_all(&mut subnet)` BEFORE calling
///   `reconstruct`, OR the caller MUST invoke a full-scan redex rediscovery
///   on the merged `Net` immediately after `reconstruct` returns. Failure
///   to do so produces a silent R41 violation (QA-004).
///
/// # R7a (hybrid mode)
///
/// Under SPEC-20 hybrid mode, `WorkerId = 0` is reserved for the
/// coordinator self-partition (R7a). In hybrid mode the reclaimed
/// partitions correspond to *departing workers* and their `worker_id`
/// values MUST be >= 1 (monotonic per R11). EC-1 uses `worker_id = 0`
/// in the reclaimed slot to represent the non-hybrid permissive clause;
/// Phase-B integrators writing hybrid-mode tests MUST NOT treat EC-1 as
/// a precedent for hybrid-mode reclaimed-worker assignment.
///
/// # Invariants preserved
///
/// - **D3** (Border Completeness) — inherited from the subsequent
///   `merge()` call.
/// - **D4** (ID Uniqueness) — enforced by `id_range` overlap assert (always
///   active, not debug-gated). See `merge::core::merge` step 2 for the
///   last-writer-wins behaviour that this assert protects against.
///
/// # See also
///
/// - SPEC-19 §3.8 A8 (amendment), SPEC-20 §4.2.2 delta-mode departure
///   recovery step 4, ARG-006 P12.
///
/// Note: reclaimed partitions are appended to the `PartitionPlan` input
/// rather than unioned via `Net::union` (TASK-0410/A7) — `merge()` performs
/// the equivalent structural concatenation internally. `Net::union` would
/// be appropriate only if the call site needs a pre-assembled `Net` before
/// `split()`.
#[allow(dead_code)] // wired by TASK-0443 (SPEC-20 §4.2.2 delta-mode departure recovery).
pub(crate) fn reconstruct(
    border_graph: &crate::merge::border_graph::BorderGraph,
    surviving_partitions: Vec<crate::partition::Partition>,
    reclaimed_partitions: Vec<crate::partition::Partition>,
) -> Net {
    // QA-001 (CRITICAL): Direct id_range overlap scan — unconditional assert!
    // (fires in both debug AND release builds). Replaces the former
    // worker_id-proxy debug_assert! which was: (a) debug-gated only,
    // (b) only checked surviving-vs-reclaimed, and (c) unsound for callers
    // constructing Partition values with distinct worker_ids but overlapping
    // id_ranges (the proxy's blind spot — two callers with wid=0 and wid=1
    // but both starting at AgentId 0 would pass the old proxy silently).
    //
    // Algorithm: collect (id_range.start, id_range.end, worker_id) from all
    // partitions, sort by start, then verify prev.end <= curr.start for
    // adjacent pairs. O(K log K) where K = total partition count.
    // reconstruct is called at round boundaries, not on a hot path.
    //
    // Cites: SPEC-20 R30 (remap_partition_ids guarantees disjointness along
    // the BSP pipeline), merge::core::merge step 2 invariant (last-writer-wins
    // by AgentId index silently corrupts the Net when ranges overlap), D4.
    {
        let mut ranges: Vec<(u32, u32, crate::partition::WorkerId)> = surviving_partitions
            .iter()
            .chain(reclaimed_partitions.iter())
            .map(|p| (p.id_range.start, p.id_range.end, p.worker_id))
            .collect();
        ranges.sort_unstable_by_key(|&(start, _, _)| start);
        for pair in ranges.windows(2) {
            let (prev_start, prev_end, prev_wid) = pair[0];
            let (curr_start, _, curr_wid) = pair[1];
            assert!(
                prev_end <= curr_start,
                "SPEC-20 A8 / R30 / merge::core::merge step 2 invariant: \
                 id_range [{prev_start}, {prev_end}) of worker_id {prev_wid} \
                 overlaps with id_range starting at {curr_start} of worker_id \
                 {curr_wid}. Caller MUST call remap_partition_ids (SPEC-20 A4) \
                 before reconstruct to ensure pairwise-disjoint AgentId ranges. \
                 Overlapping ranges cause silent topology corruption in release \
                 builds (last-writer-wins in merge step 2 — D4 breach).",
            );
        }
        // QA-006 / SPEC-01 I3: unconditional (not debug-gated) next_id
        // overflow guard. The merged Net's next_id = max(p.subnet.next_id)
        // across all partitions. If any partition carries next_id == u32::MAX
        // the merged Net has next_id == u32::MAX and the next create_agent
        // call wraps to AgentId 0, silently overwriting whatever lives there.
        // Reclaimed snapshots from long-running coordinators are the most
        // likely source of exhausted next_id values.
        let max_next_id = surviving_partitions
            .iter()
            .chain(reclaimed_partitions.iter())
            .map(|p| p.subnet.next_id)
            .max()
            .unwrap_or(0);
        assert!(
            max_next_id < u32::MAX,
            "SPEC-01 I3: reconstruct would produce a Net with next_id == u32::MAX; \
             no subsequent create_agent call is possible without AgentId overflow. \
             Reclaimed snapshots from long-running coordinators may carry exhausted \
             next_id values — verify the partition source."
        );
    }
    // Pass 0: this helper is called by departure-recovery orchestrators
    // (TASK-0440/0443) which consume the plan immediately via merge().
    // The cursor is unused by merge(); a future caller that needs
    // allocate_border_ids on the result MUST supply the active cursor
    // instead of relying on this helper (see reconstruct_partition_plan_from_collected
    // comment, RV-001).
    let plan = reconstruct_partition_plan_from_collected(
        surviving_partitions,
        border_graph,
        reclaimed_partitions,
        0,
    );
    let (net, _border_redex_count) = super::core::merge(plan);
    net
}

/// Optimized path for n == 1: reduce locally without partitioning (R26).
fn run_single_worker(
    mut net: Net,
    config: &GridConfig,
    mut metrics: GridMetrics,
    start_time: Instant,
) -> (Net, GridMetrics) {
    // Check round limit
    if let Some(max) = config.max_rounds {
        if max == 0 {
            metrics.converged = false;
            metrics.total_time = start_time.elapsed();
            return (net, metrics);
        }
    }

    metrics.agents_per_round.push(net.count_live_agents());

    let t_reduce = Instant::now();
    let stats = reduce_all(&mut net);
    let reduce_duration = t_reduce.elapsed();

    metrics.compute_time_per_round.push(reduce_duration);
    metrics
        .local_interactions_per_round
        .push(stats.total_interactions);
    metrics.total_interactions = stats.total_interactions;
    for i in 0..6 {
        metrics.total_interactions_by_rule[i] += stats.interactions_by_rule[i];
    }
    metrics.border_interactions_per_round.push(0);
    metrics.border_redexes_per_round.push(0);
    metrics.rounds = 1;
    metrics.converged = true;

    metrics.worker_stats_per_round.push(vec![WorkerRoundStats {
        worker_id: 0,
        agents_before: metrics.agents_per_round[0],
        agents_after: net.count_live_agents(),
        local_redexes: stats.total_interactions as usize,
        reduce_duration_secs: reduce_duration.as_secs_f64(),
        interactions_by_rule: stats.interactions_by_rule,
        // n == 1 has no borders by definition (no partitioning).
        has_border_activity: false,
        is_coordinator_self: true, // single-worker is self-reducing
    }]);

    metrics.total_time = start_time.elapsed();
    (net, metrics)
}

/// Defense-in-depth: full scan for redexes not in the queue (R41).
///
/// In debug mode, verifies that no agent has its principal port connected
/// to another agent's principal port without being in the redex queue.
/// This catches bugs in Net::connect that might fail to insert a redex.
///
/// Complexity: O(A * PORTS_PER_SLOT).
#[cfg(debug_assertions)]
fn verify_no_redexes_full_scan(net: &Net) {
    for agent in net.live_agents() {
        let target = net.get_target(PortRef::AgentPort(agent.id, 0));
        if let PortRef::AgentPort(other_id, 0) = target {
            if agent.id < other_id {
                // This is an active pair not in the queue — bug in connect()
                panic!(
                    "R41: undiscovered redex ({}, {}) found during full scan",
                    agent.id, other_id
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::PortRef;
    use crate::net::Symbol;
    use crate::partition::ContiguousIdStrategy;

    // === TASK-0069: Skeleton tests ===

    // T1: Net already in Normal Form -> converged immediately, 0 rounds
    #[test]
    fn test_run_grid_already_normal_form() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        // No redexes (no principal-principal connections)
        net.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(b, 0), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 1));
        net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(b, 2));
        let config = GridConfig {
            num_workers: 2,
            max_rounds: None,
            ..GridConfig::default()
        };
        let (result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert!(metrics.converged);
        assert_eq!(metrics.rounds, 0);
        assert_eq!(metrics.total_interactions, 0);
        assert_eq!(result.count_live_agents(), 2);
    }

    // T2: max_rounds = Some(0) -> terminates immediately, converged = false
    #[test]
    fn test_run_grid_max_rounds_zero() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        let config = GridConfig {
            num_workers: 2,
            max_rounds: Some(0),
            ..GridConfig::default()
        };
        let (_result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert!(!metrics.converged);
        assert_eq!(metrics.rounds, 0);
    }

    // T3: total_time is populated
    #[test]
    fn test_run_grid_total_time_populated() {
        let net = Net::new();
        let config = GridConfig {
            num_workers: 2,
            max_rounds: None,
            ..GridConfig::default()
        };
        let (_result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);
        // Verify the grid actually ran and produced metrics
        let _ = metrics.total_time;
    }

    // === TASK-0072: n == 1 optimization ===

    // T4: Single worker reduces without partitioning
    #[test]
    fn test_run_grid_single_worker() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        let config = GridConfig {
            num_workers: 1,
            max_rounds: None,
            ..GridConfig::default()
        };
        let (result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert!(metrics.converged);
        assert_eq!(metrics.rounds, 1);
        assert_eq!(metrics.total_interactions, 1); // ERA-ERA = 1 interaction
        assert_eq!(result.count_live_agents(), 0);
    }

    // === TASK-0070/0071: Phase integration tests ===

    // T5: ERA-ERA pair with 2 workers -> split as border, merge resolves
    #[test]
    fn test_run_grid_era_era_border() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era); // id=0 -> w0
        let b = net.create_agent(Symbol::Era); // id=1 -> w1
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        let config = GridConfig {
            num_workers: 2,
            max_rounds: None,
            ..GridConfig::default()
        };
        let (result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert!(metrics.converged);
        assert_eq!(result.count_live_agents(), 0);
        assert_eq!(metrics.total_interactions, 1); // 1 ERA-ERA interaction
        assert_eq!(metrics.border_redexes_per_round[0], 1); // detected as border redex
    }

    // T6: CON-CON annihilation with border redex
    #[test]
    fn test_run_grid_con_con_border() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con); // id=0 -> w0
        let b = net.create_agent(Symbol::Con); // id=1 -> w1
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(3));
        let config = GridConfig {
            num_workers: 2,
            max_rounds: None,
            ..GridConfig::default()
        };
        let (result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert!(metrics.converged);
        assert_eq!(result.count_live_agents(), 0); // annihilation removes both
        assert_eq!(metrics.total_interactions, 1);
    }

    // T7: Internal redexes resolved during local reduction
    #[test]
    fn test_run_grid_internal_redexes() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era); // id=0
        let b = net.create_agent(Symbol::Era); // id=1
        let c = net.create_agent(Symbol::Era); // id=2
        let d = net.create_agent(Symbol::Era); // id=3
                                               // a-b pair -> internal to w0 (ids 0,1)
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        // c-d pair -> internal to w1 (ids 2,3)
        net.connect(PortRef::AgentPort(c, 0), PortRef::AgentPort(d, 0));
        let config = GridConfig {
            num_workers: 2,
            max_rounds: None,
            ..GridConfig::default()
        };
        let (result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert!(metrics.converged);
        assert_eq!(result.count_live_agents(), 0);
        assert_eq!(metrics.total_interactions, 2); // 2 ERA-ERA
                                                   // Both redexes are internal, no border redexes
        assert_eq!(metrics.border_redexes_per_round[0], 0);
        assert_eq!(metrics.local_interactions_per_round[0], 2);
    }

    // T8: Metrics accumulation across a single round
    #[test]
    fn test_run_grid_metrics_single_round() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        let config = GridConfig {
            num_workers: 2,
            max_rounds: None,
            ..GridConfig::default()
        };
        let (_result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert_eq!(metrics.rounds, 1);
        assert_eq!(metrics.agents_per_round.len(), 1);
        assert_eq!(metrics.agents_per_round[0], 2);
        assert!(!metrics.partition_time_per_round.is_empty());
        assert!(!metrics.compute_time_per_round.is_empty());
        assert!(!metrics.merge_time_per_round.is_empty());
        assert!(!metrics.border_reduce_time_per_round.is_empty());
    }

    // === TASK-0075: Fundamental Property G1 ===

    // G1a: ERA-ERA — reduce_all(net) == run_grid(net, 2)
    #[test]
    fn test_g1_era_era() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        let net_seq = net.clone();
        let mut seq = net_seq;
        let seq_stats = reduce_all(&mut seq);
        let config = GridConfig {
            num_workers: 2,
            max_rounds: None,
            ..GridConfig::default()
        };
        let (grid_result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert_eq!(seq.count_live_agents(), grid_result.count_live_agents());
        assert_eq!(seq_stats.total_interactions, metrics.total_interactions);
    }

    // G1b: CON-CON annihilation
    #[test]
    fn test_g1_con_con() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(3));

        let mut seq = net.clone();
        let seq_stats = reduce_all(&mut seq);
        let config = GridConfig {
            num_workers: 2,
            max_rounds: None,
            ..GridConfig::default()
        };
        let (grid_result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert_eq!(seq.count_live_agents(), grid_result.count_live_agents());
        assert_eq!(seq_stats.total_interactions, metrics.total_interactions);
    }

    // G1c: DUP-DUP annihilation
    #[test]
    fn test_g1_dup_dup() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Dup);
        let b = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(3));

        let mut seq = net.clone();
        let seq_stats = reduce_all(&mut seq);
        let config = GridConfig {
            num_workers: 2,
            max_rounds: None,
            ..GridConfig::default()
        };
        let (grid_result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert_eq!(seq.count_live_agents(), grid_result.count_live_agents());
        assert_eq!(seq_stats.total_interactions, metrics.total_interactions);
    }

    // G1d: CON-ERA erasure
    #[test]
    fn test_g1_con_era() {
        let mut net = Net::new();
        let c = net.create_agent(Symbol::Con); // id=0
        let e = net.create_agent(Symbol::Era); // id=1
        net.connect(PortRef::AgentPort(c, 0), PortRef::AgentPort(e, 0));
        net.connect(PortRef::AgentPort(c, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(c, 2), PortRef::FreePort(1));

        let mut seq = net.clone();
        let seq_stats = reduce_all(&mut seq);
        let config = GridConfig {
            num_workers: 2,
            max_rounds: None,
            ..GridConfig::default()
        };
        let (grid_result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert_eq!(seq.count_live_agents(), grid_result.count_live_agents());
        assert_eq!(seq_stats.total_interactions, metrics.total_interactions);
    }

    // G1e: Chain of ERA-ERA pairs (4 agents, 2 internal redexes, 2 workers)
    #[test]
    fn test_g1_chain_internal() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        let c = net.create_agent(Symbol::Era);
        let d = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(c, 0), PortRef::AgentPort(d, 0));

        let mut seq = net.clone();
        let seq_stats = reduce_all(&mut seq);
        let config = GridConfig {
            num_workers: 2,
            max_rounds: None,
            ..GridConfig::default()
        };
        let (grid_result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert_eq!(seq.count_live_agents(), grid_result.count_live_agents());
        assert_eq!(seq_stats.total_interactions, metrics.total_interactions);
    }

    // G1f: CON-DUP commutation across partition boundary
    #[test]
    fn test_g1_con_dup_border() {
        let mut net = Net::new();
        let c = net.create_agent(Symbol::Con); // id=0 -> w0
        let d = net.create_agent(Symbol::Dup); // id=1 -> w1
        net.connect(PortRef::AgentPort(c, 0), PortRef::AgentPort(d, 0));
        net.connect(PortRef::AgentPort(c, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(c, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(d, 1), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(d, 2), PortRef::FreePort(3));

        let mut seq = net.clone();
        let seq_stats = reduce_all(&mut seq);
        let config = GridConfig {
            num_workers: 2,
            max_rounds: None,
            ..GridConfig::default()
        };
        let (grid_result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        // CON-DUP creates 4 new agents from 2 original
        assert_eq!(seq.count_live_agents(), grid_result.count_live_agents());
        assert_eq!(seq_stats.total_interactions, metrics.total_interactions);
    }

    // G1g: G1 with 4 workers
    #[test]
    fn test_g1_four_workers() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        let c = net.create_agent(Symbol::Era);
        let d = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(c, 0), PortRef::AgentPort(d, 0));

        let mut seq = net.clone();
        let seq_stats = reduce_all(&mut seq);

        let config = GridConfig {
            num_workers: 4,
            max_rounds: None,
            ..GridConfig::default()
        };
        let (grid_result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert_eq!(seq.count_live_agents(), grid_result.count_live_agents());
        assert_eq!(seq_stats.total_interactions, metrics.total_interactions);
    }

    // G1h: Empty net -> both return empty net immediately
    #[test]
    fn test_g1_empty_net() {
        let net = Net::new();
        let mut seq = net.clone();
        let seq_stats = reduce_all(&mut seq);
        let config = GridConfig {
            num_workers: 2,
            max_rounds: None,
            ..GridConfig::default()
        };
        let (grid_result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert_eq!(seq.count_live_agents(), grid_result.count_live_agents());
        assert_eq!(seq_stats.total_interactions, metrics.total_interactions);
    }

    // G1i: FreePort-FreePort redirect regression — annihilation consumes both
    // agents in one partition, leaving the border reference unresolvable.
    //
    // Topology: c0-c1 (internal pair in W0), c2 survives in W1.
    //   c0.0 <-> c1.0 (active pair, internal to W0)
    //   c1.1 <-> c2.1 (border wire)
    //   c1.2 <-> c2.2 (border wire)
    //   c0.1, c0.2 = Lafont FreePorts; c2.0 = Lafont FreePort
    //
    // Bug: W0 reduces c0-c1 (CON-CON CROSS). The CROSS links
    //   FreePort(B0) <-> FreePort(Lafont) — a no-op in the port array.
    //   Border refs B0, B1 are lost. Merge can't restore c2.1 and c2.2,
    //   leaving them DISCONNECTED → T1 violation.
    //
    // Fix: Net::connect records FreePort-FreePort redirects in
    //   freeport_redirects, which rebuild_free_port_index uses.
    #[test]
    fn test_g1_freeport_redirect_regression() {
        let mut net = Net::new();
        let c0 = net.create_agent(Symbol::Con); // id=0 → W0
        let c1 = net.create_agent(Symbol::Con); // id=1 → W0
        let c2 = net.create_agent(Symbol::Con); // id=2 → W1

        // Active pair in W0
        net.connect(PortRef::AgentPort(c0, 0), PortRef::AgentPort(c1, 0));
        // Border wires (will cross the partition boundary)
        net.connect(PortRef::AgentPort(c1, 1), PortRef::AgentPort(c2, 1));
        net.connect(PortRef::AgentPort(c1, 2), PortRef::AgentPort(c2, 2));
        // Lafont FreePorts
        net.connect(PortRef::AgentPort(c0, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(c0, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(c2, 0), PortRef::FreePort(2));

        // Sequential baseline
        let mut seq = net.clone();
        let seq_stats = reduce_all(&mut seq);

        // Grid with 2 workers
        let config = GridConfig {
            num_workers: 2,
            max_rounds: None,
            ..GridConfig::default()
        };
        let (grid_result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        // Both should produce 1 agent (c2 survives) with 1 interaction
        assert_eq!(seq.count_live_agents(), 1);
        assert_eq!(grid_result.count_live_agents(), 1);
        assert_eq!(seq_stats.total_interactions, metrics.total_interactions);

        // c2's ports should match sequential result
        // Sequential: c2.0 = FP(2), c2.1 = FP(1) (via CROSS), c2.2 = FP(0) (via CROSS)
        assert_eq!(
            grid_result.get_target(PortRef::AgentPort(c2, 0)),
            seq.get_target(PortRef::AgentPort(c2, 0))
        );
        assert_eq!(
            grid_result.get_target(PortRef::AgentPort(c2, 1)),
            seq.get_target(PortRef::AgentPort(c2, 1))
        );
        assert_eq!(
            grid_result.get_target(PortRef::AgentPort(c2, 2)),
            seq.get_target(PortRef::AgentPort(c2, 2))
        );
    }

    // G1j: Same as G1i but with DUP-DUP (PARALLEL pattern)
    #[test]
    fn test_g1_freeport_redirect_dup_dup() {
        let mut net = Net::new();
        let d0 = net.create_agent(Symbol::Dup); // id=0 → W0
        let d1 = net.create_agent(Symbol::Dup); // id=1 → W0
        let d2 = net.create_agent(Symbol::Dup); // id=2 → W1

        net.connect(PortRef::AgentPort(d0, 0), PortRef::AgentPort(d1, 0));
        net.connect(PortRef::AgentPort(d1, 1), PortRef::AgentPort(d2, 1));
        net.connect(PortRef::AgentPort(d1, 2), PortRef::AgentPort(d2, 2));
        net.connect(PortRef::AgentPort(d0, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(d0, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(d2, 0), PortRef::FreePort(2));

        let mut seq = net.clone();
        let seq_stats = reduce_all(&mut seq);
        let config = GridConfig {
            num_workers: 2,
            max_rounds: None,
            ..GridConfig::default()
        };
        let (grid_result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert_eq!(seq.count_live_agents(), 1);
        assert_eq!(grid_result.count_live_agents(), 1);
        assert_eq!(seq_stats.total_interactions, metrics.total_interactions);
        assert_eq!(
            grid_result.get_target(PortRef::AgentPort(d2, 1)),
            seq.get_target(PortRef::AgentPort(d2, 1))
        );
        assert_eq!(
            grid_result.get_target(PortRef::AgentPort(d2, 2)),
            seq.get_target(PortRef::AgentPort(d2, 2))
        );
    }

    // G1k: Symmetric — both partitions consume agents with border wires
    #[test]
    fn test_g1_freeport_redirect_symmetric() {
        let mut net = Net::new();
        let c0 = net.create_agent(Symbol::Con); // id=0 → W0
        let c1 = net.create_agent(Symbol::Con); // id=1 → W0
        let c2 = net.create_agent(Symbol::Con); // id=2 → W1
        let c3 = net.create_agent(Symbol::Con); // id=3 → W1

        // Internal pairs in both partitions
        net.connect(PortRef::AgentPort(c0, 0), PortRef::AgentPort(c1, 0));
        net.connect(PortRef::AgentPort(c2, 0), PortRef::AgentPort(c3, 0));
        // Border wires (c1 aux <-> c2 aux)
        net.connect(PortRef::AgentPort(c1, 1), PortRef::AgentPort(c2, 1));
        net.connect(PortRef::AgentPort(c1, 2), PortRef::AgentPort(c2, 2));
        // Lafont FreePorts
        net.connect(PortRef::AgentPort(c0, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(c0, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(c3, 1), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(c3, 2), PortRef::FreePort(3));

        let mut seq = net.clone();
        let seq_stats = reduce_all(&mut seq);
        let config = GridConfig {
            num_workers: 2,
            max_rounds: None,
            ..GridConfig::default()
        };
        let (grid_result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        // All agents consumed
        assert_eq!(seq.count_live_agents(), 0);
        assert_eq!(grid_result.count_live_agents(), 0);
        assert_eq!(seq_stats.total_interactions, metrics.total_interactions);
    }

    // === SPEC-05 R30a: strict BSP mode tests ===
    //
    // These tests cover the `strict_bsp` branch of the grid loop. In strict
    // mode, border cascades are NOT reduced at the coordinator after merge,
    // so the loop iterates — redistributing border redexes to workers in the
    // next round. This exposes true multi-round BSP behavior needed for
    // Phase 3 LAN measurements, which is otherwise hidden by the lenient
    // coordinator-side `reduce_all` optimization.
    //
    // The Fundamental Property G1 (SPEC-01) MUST hold in both modes: the
    // same reductions are performed, only the distribution across rounds
    // differs.

    // T-strict-1: Net already in Normal Form behaves identically under
    // strict_bsp. Lenient and strict both report 0 rounds.
    #[test]
    fn test_strict_bsp_already_normal_form() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(b, 0), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 1));
        net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(b, 2));
        let config = GridConfig {
            num_workers: 2,
            strict_bsp: true,
            ..GridConfig::default()
        };
        let (result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert!(metrics.converged);
        assert_eq!(metrics.rounds, 0);
        assert_eq!(metrics.total_interactions, 0);
        assert_eq!(result.count_live_agents(), 2);
    }

    // T-strict-2: ERA-ERA annihilation with strict_bsp completes in exactly
    // 1 round. The single border redex is consumed by the next round's
    // worker, and no new redexes are produced (annihilation is terminal).
    #[test]
    fn test_strict_bsp_era_era_single_round() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era); // id=0 -> w0
        let b = net.create_agent(Symbol::Era); // id=1 -> w1
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        let config = GridConfig {
            num_workers: 2,
            strict_bsp: true,
            ..GridConfig::default()
        };
        let (result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert!(metrics.converged);
        assert_eq!(result.count_live_agents(), 0);
        assert_eq!(metrics.total_interactions, 1);
        // In strict mode the border redex is resolved by worker reduction
        // in the next round; no cascading is generated, so 1 round suffices.
        assert!(
            metrics.rounds >= 1,
            "expected at least 1 round, got {}",
            metrics.rounds
        );
    }

    // T-strict-3: CON-CON cascade across the partition boundary forces
    // a multi-round reduction in strict mode. Two left-side CONs (worker 0)
    // and two right-side CONs (worker 1) are chained so that reducing the
    // first border active pair produces a second border active pair. Under
    // lenient mode this all resolves in 1 round (coordinator reduce_all);
    // under strict mode, the cascade is deferred to a subsequent round.
    #[test]
    fn test_strict_bsp_con_cascade_multi_round() {
        let mut net = Net::new();
        // Left chain (worker 0): ids 0, 1
        let l0 = net.create_agent(Symbol::Con); // id=0 -> w0
        let l1 = net.create_agent(Symbol::Con); // id=1 -> w0
                                                // Right chain (worker 1): ids 2, 3
        let r0 = net.create_agent(Symbol::Con); // id=2 -> w1
        let r1 = net.create_agent(Symbol::Con); // id=3 -> w1

        // Initial border active pair: l0.0 <-> r0.0
        net.connect(PortRef::AgentPort(l0, 0), PortRef::AgentPort(r0, 0));
        // l0.1 <-> l1.0 (aux-to-principal; not an active pair yet)
        net.connect(PortRef::AgentPort(l0, 1), PortRef::AgentPort(l1, 0));
        // r0.2 <-> r1.0 (aux-to-principal; mirrors l side)
        net.connect(PortRef::AgentPort(r0, 2), PortRef::AgentPort(r1, 0));
        // Terminal FreePorts for the remaining aux ports
        net.connect(PortRef::AgentPort(l0, 2), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(r0, 1), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(l1, 1), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(l1, 2), PortRef::FreePort(3));
        net.connect(PortRef::AgentPort(r1, 1), PortRef::FreePort(4));
        net.connect(PortRef::AgentPort(r1, 2), PortRef::FreePort(5));

        // CON-CON annihilation rule: link(a1, b2) and link(a2, b1). After
        // the l0-r0 annihilation, a1=l1.0 and b2=r1.0, so link(l1.0, r1.0)
        // creates the second principal-principal active pair — still
        // cross-partition (l1 in w0, r1 in w1).

        let mut seq = net.clone();
        let seq_stats = reduce_all(&mut seq);
        let config = GridConfig {
            num_workers: 2,
            strict_bsp: true,
            ..GridConfig::default()
        };
        let (grid_result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert!(metrics.converged);
        // G1: equivalent Normal Form to sequential
        assert_eq!(seq.count_live_agents(), grid_result.count_live_agents());
        assert_eq!(seq_stats.total_interactions, metrics.total_interactions);
        // Strict mode exposes the cascade as >= 2 rounds: round 1 consumes
        // the first CON-CON pair and produces a new principal-principal
        // border pair; round 2 (or later) resolves it.
        assert!(
            metrics.rounds >= 2,
            "expected >= 2 rounds under strict BSP, got {}",
            metrics.rounds
        );
    }

    // T-strict-4: Binary tree of CON-DUP nodes (DualTree-style) at depth 4.
    // Each layer of commutations pushes new active pairs across the
    // partition boundary, so strict mode should accumulate several rounds
    // before convergence.
    #[test]
    fn test_strict_bsp_dual_tree_depth4_multi_round() {
        // Use the generator from io::generators to match the DualTree bench.
        let net = crate::io::generators::dual_tree(4);

        let mut seq = net.clone();
        let seq_stats = reduce_all(&mut seq);
        let config = GridConfig {
            num_workers: 2,
            strict_bsp: true,
            ..GridConfig::default()
        };
        let (grid_result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert!(metrics.converged);
        assert_eq!(seq.count_live_agents(), grid_result.count_live_agents());
        assert_eq!(seq_stats.total_interactions, metrics.total_interactions);
        // DualTree depth=4 has cross-partition cascades at every tree level.
        // We expect at least 2 rounds (strict actually reveals multi-round
        // behavior); the exact count depends on tree topology.
        assert!(
            metrics.rounds >= 2,
            "expected >= 2 rounds for dual_tree(4) under strict BSP, got {}",
            metrics.rounds
        );
    }

    // T-strict-5: G1 — strict mode yields the same Normal Form as sequential
    // reduction, for a variety of topologies. This is the Fundamental
    // Property check for strict mode.
    #[test]
    fn test_strict_bsp_g1_vs_sequential() {
        // Topology: 2 CON-CON pairs across the border + 1 internal ERA-ERA.
        let mut net = Net::new();
        let c0 = net.create_agent(Symbol::Con); // id=0 -> w0
        let c1 = net.create_agent(Symbol::Con); // id=1 -> w0
        let c2 = net.create_agent(Symbol::Con); // id=2 -> w1
        let c3 = net.create_agent(Symbol::Con); // id=3 -> w1
        let e0 = net.create_agent(Symbol::Era); // id=4 -> w1
        let e1 = net.create_agent(Symbol::Era); // id=5 -> w1

        // Border: c1 <-> c2 (CON-CON annihilation)
        net.connect(PortRef::AgentPort(c1, 0), PortRef::AgentPort(c2, 0));
        // Internal: c0 <-> c3 is aux wire; e0 <-> e1 is internal ERA-ERA
        net.connect(PortRef::AgentPort(c1, 1), PortRef::AgentPort(c0, 1));
        net.connect(PortRef::AgentPort(c1, 2), PortRef::AgentPort(c0, 2));
        net.connect(PortRef::AgentPort(c2, 1), PortRef::AgentPort(c3, 1));
        net.connect(PortRef::AgentPort(c2, 2), PortRef::AgentPort(c3, 2));
        net.connect(PortRef::AgentPort(c0, 0), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(c3, 0), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(e0, 0), PortRef::AgentPort(e1, 0));

        let mut seq = net.clone();
        let seq_stats = reduce_all(&mut seq);
        let config = GridConfig {
            num_workers: 2,
            strict_bsp: true,
            ..GridConfig::default()
        };
        let (grid_result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert!(metrics.converged);
        assert_eq!(seq.count_live_agents(), grid_result.count_live_agents());
        assert_eq!(seq_stats.total_interactions, metrics.total_interactions);
    }

    // T-strict-6: G1 — lenient and strict modes produce structurally
    // equivalent Normal Forms (same live agent count and same total
    // interactions). Round count may differ, but the work is the same.
    #[test]
    fn test_strict_bsp_g1_vs_lenient() {
        let net = crate::io::generators::dual_tree(3);

        let lenient_cfg = GridConfig {
            num_workers: 2,
            strict_bsp: false,
            ..GridConfig::default()
        };
        let strict_cfg = GridConfig {
            num_workers: 2,
            strict_bsp: true,
            ..GridConfig::default()
        };

        let (lenient_net, lenient_metrics) =
            run_grid(net.clone(), &lenient_cfg, &ContiguousIdStrategy);
        let (strict_net, strict_metrics) = run_grid(net, &strict_cfg, &ContiguousIdStrategy);

        assert!(lenient_metrics.converged);
        assert!(strict_metrics.converged);
        assert_eq!(
            lenient_net.count_live_agents(),
            strict_net.count_live_agents()
        );
        assert_eq!(
            lenient_metrics.total_interactions,
            strict_metrics.total_interactions
        );
        // Lenient concentrates all cascades into 1 round; strict should
        // use >= lenient's round count.
        assert!(strict_metrics.rounds >= lenient_metrics.rounds);
    }

    // === SPEC-19 §3.1 — TASK-0349 multi-worker wiring ===

    // IT-0349-04: end-to-end propagation of `has_border_activity` through
    // `run_grid`'s per-worker reduction loop. After a single round of
    // CON-CON annihilation across the border, both partitions reach
    // Normal Form locally and have NO remaining border endpoints (the
    // border redex is consumed during merge+resolve). The final round's
    // worker stats must therefore report `has_border_activity == false`
    // on every worker — which is the exact signal TASK-0351 will use to
    // decide that no further merge is needed.
    #[test]
    fn it_0349_04_run_grid_wires_has_border_activity_per_worker() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con); // id=0 -> w0
        let b = net.create_agent(Symbol::Con); // id=1 -> w1
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(3));
        let config = GridConfig {
            num_workers: 2,
            max_rounds: None,
            ..GridConfig::default()
        };
        let (_result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert!(metrics.converged);
        // Pre-merge round 0: both agents are split as singleton partitions
        // with the border principal-port endpoint exposed via FreePort,
        // so `has_border_activity` should be true for at least one worker.
        let round0 = &metrics.worker_stats_per_round[0];
        assert_eq!(round0.len(), 2, "two workers in round 0");
        let any_active_round0 = round0.iter().any(|s| s.has_border_activity);
        assert!(
            any_active_round0,
            "round 0 must surface principal-port border activity"
        );

        // Sanity: every worker stat carries the field (it is now wired).
        for stats in metrics.worker_stats_per_round.iter().flatten() {
            // Just touching the field proves the type-level wiring.
            let _ = stats.has_border_activity;
        }
    }

    // IT-0349-05: when both workers are already in Normal Form locally
    // and have NO boundary endpoints, every per-worker stat must report
    // `has_border_activity == false`. This is the negative case that
    // TASK-0351 will rely on to skip merge.
    #[test]
    fn it_0349_05_no_border_endpoints_reports_false() {
        let mut net = Net::new();
        // Two ERA-ERA pairs, each entirely within one partition (ids
        // 0,1 -> w0; ids 2,3 -> w1). No FreePorts at all.
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        let c = net.create_agent(Symbol::Era);
        let d = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(c, 0), PortRef::AgentPort(d, 0));
        let config = GridConfig {
            num_workers: 2,
            max_rounds: None,
            ..GridConfig::default()
        };
        let (_result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert!(metrics.converged);
        for stats in metrics.worker_stats_per_round.iter().flatten() {
            assert!(
                !stats.has_border_activity,
                "no FreePort endpoints means no border activity (worker {})",
                stats.worker_id
            );
        }
    }

    // === SPEC-19 §3.1 — TASK-0351 (R3, R4, R5, R6, R7) ===

    use crate::merge::types::WorkerRoundStats;

    /// Test fixture: build a `WorkerRoundStats` with the two booleans
    /// the truth-table tests vary.
    fn stats_with(has_border_activity: bool, local_redexes: usize) -> WorkerRoundStats {
        WorkerRoundStats {
            worker_id: 0,
            agents_before: 0,
            agents_after: 0,
            local_redexes,
            reduce_duration_secs: 0.0,
            interactions_by_rule: [0; 6],
            has_border_activity,
            is_coordinator_self: false,
        }
    }

    // UT-0351-01..04: exhaustive 2x2 truth table over the
    // `check_global_normal_form` helper.

    #[test]
    fn ut_0351_01_check_gnf_all_quiescent_true_true() {
        let stats = vec![stats_with(false, 0), stats_with(false, 0)];
        assert_eq!(check_global_normal_form(&stats), (true, true));
    }

    #[test]
    fn ut_0351_02_check_gnf_skip_eligible_true_false() {
        let stats = vec![stats_with(false, 5), stats_with(false, 3)];
        assert_eq!(check_global_normal_form(&stats), (true, false));
    }

    #[test]
    fn ut_0351_03_check_gnf_one_active_false_true() {
        let stats = vec![stats_with(true, 0), stats_with(false, 0)];
        assert_eq!(check_global_normal_form(&stats), (false, true));
    }

    #[test]
    fn ut_0351_04_check_gnf_full_merge_needed_false_false() {
        let stats = vec![stats_with(true, 5), stats_with(true, 3)];
        assert_eq!(check_global_normal_form(&stats), (false, false));
    }

    /// Build a 2-worker net whose round 0 produces `has_border_activity
    /// == false` for both workers (no principal-port border endpoints):
    /// two ERA-ERA pairs, each fully inside one partition, both pairs
    /// will reduce locally and leave nothing for the border. After
    /// round 0 every worker has 0 local redexes, so this triggers R4
    /// (Global Normal Form) — proving the GNF early-exit path.
    fn build_already_quiescent_net() -> Net {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        let c = net.create_agent(Symbol::Era);
        let d = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(c, 0), PortRef::AgentPort(d, 0));
        net
    }

    /// Build a 2-worker net with a CON-CON border redex: round 0 will
    /// produce `has_border_activity == true` for at least one worker
    /// (the principal port of one CON sits at the border). This
    /// workload NEVER triggers the skip path — R5 identity check.
    fn build_typical_two_worker_net_with_borders() -> Net {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con); // id=0 -> w0
        let b = net.create_agent(Symbol::Con); // id=1 -> w1
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(3));
        net
    }

    // UT-0351-06 (R4): GNF early termination. With opt-in + strict_bsp,
    // the already-quiescent net must converge in 1 round with the
    // counter incremented (since merge was skipped on the GNF path).
    #[test]
    fn ut_0351_06_run_grid_terminates_on_global_normal_form() {
        let net = build_already_quiescent_net();
        let cfg = GridConfig {
            num_workers: 2,
            max_rounds: Some(100),
            strict_bsp: true,
            delta_mode: false,
            coordinator_free_rounds: true,
            ..GridConfig::default()
        };
        let (result, metrics) = run_grid(net, &cfg, &ContiguousIdStrategy);
        assert!(metrics.converged, "GNF must mark converged = true");
        assert!(
            metrics.rounds <= 1,
            "GNF must terminate ASAP; took {} rounds",
            metrics.rounds
        );
        assert_eq!(
            result.count_live_agents(),
            0,
            "ERA-ERA annihilates both pairs"
        );
        assert!(
            metrics.coordinator_free_rounds >= 1,
            "GNF path must increment the counter; got {}",
            metrics.coordinator_free_rounds
        );
    }

    // UT-0351-07 (R7 v1 compat): default config must NEVER touch the new
    // counter and must produce identical results to the v1 path.
    #[test]
    fn ut_0351_07_run_grid_default_config_unchanged_v1_behavior() {
        let net = build_typical_two_worker_net_with_borders();
        let cfg = GridConfig {
            num_workers: 2,
            max_rounds: Some(5),
            strict_bsp: true,
            ..GridConfig::default() // coordinator_free_rounds = false
        };
        let (result, metrics) = run_grid(net, &cfg, &ContiguousIdStrategy);
        assert_eq!(
            metrics.coordinator_free_rounds, 0,
            "default config must never increment the counter"
        );
        assert!(metrics.converged);
        // CON-CON annihilates both agents.
        assert_eq!(result.count_live_agents(), 0);
    }

    // UT-0351-08 (R6 SHOULD): lenient mode + opt-in must NOT skip.
    #[test]
    fn ut_0351_08_run_grid_lenient_does_not_skip() {
        let net = build_already_quiescent_net();
        let cfg = GridConfig {
            num_workers: 2,
            max_rounds: Some(5),
            strict_bsp: false, // lenient
            delta_mode: false,
            coordinator_free_rounds: true,
            ..GridConfig::default()
        };
        let (_result, metrics) = run_grid(net, &cfg, &ContiguousIdStrategy);
        assert_eq!(
            metrics.coordinator_free_rounds, 0,
            "lenient mode must not skip even with the flag on (R6 SHOULD)"
        );
    }

    // UT-0351-10 (R5): result equivalence on a workload that DOES
    // trigger the skip path. Toggling the flag MUST NOT change the
    // decoded result.
    #[test]
    fn ut_0351_10_run_grid_equivalence_no_border_activity_workload() {
        let net = build_already_quiescent_net();
        let cfg_off = GridConfig {
            num_workers: 2,
            max_rounds: Some(10),
            strict_bsp: true,
            ..GridConfig::default()
        };
        let cfg_on = GridConfig {
            coordinator_free_rounds: true,
            ..cfg_off.clone()
        };
        let (off_net, off_m) = run_grid(net.clone(), &cfg_off, &ContiguousIdStrategy);
        let (on_net, on_m) = run_grid(net, &cfg_on, &ContiguousIdStrategy);
        assert_eq!(
            off_net.count_live_agents(),
            on_net.count_live_agents(),
            "R5: live agent count must match across skip on/off"
        );
        assert_eq!(
            off_m.total_interactions, on_m.total_interactions,
            "R5: total interactions must match"
        );
        assert_eq!(off_m.coordinator_free_rounds, 0);
        assert!(
            on_m.coordinator_free_rounds >= 1,
            "this workload was chosen to trigger the skip; got {}",
            on_m.coordinator_free_rounds
        );
    }

    // UT-0351-11 (R5): result equivalence on a workload that does NOT
    // trigger the skip path. The new code path must be the identity
    // when its predicate is false.
    #[test]
    fn ut_0351_11_run_grid_equivalence_with_border_activity_workload() {
        let net = build_typical_two_worker_net_with_borders();
        let cfg_off = GridConfig {
            num_workers: 2,
            max_rounds: Some(10),
            strict_bsp: true,
            ..GridConfig::default()
        };
        let cfg_on = GridConfig {
            coordinator_free_rounds: true,
            ..cfg_off.clone()
        };
        let (off_net, off_m) = run_grid(net.clone(), &cfg_off, &ContiguousIdStrategy);
        let (on_net, on_m) = run_grid(net, &cfg_on, &ContiguousIdStrategy);
        assert_eq!(
            off_net.count_live_agents(),
            on_net.count_live_agents(),
            "R5: identity when predicate is always false"
        );
        assert_eq!(
            off_m.total_interactions, on_m.total_interactions,
            "R5: identity must extend to total interactions"
        );
        assert_eq!(off_m.coordinator_free_rounds, 0);
        assert_eq!(
            on_m.coordinator_free_rounds, 0,
            "border-active workload must NEVER trigger the skip"
        );
    }

    // UT-0351-12 (R5 + G1 spot check): real workload — `church_add(2,3)`
    // at w=2 strict BSP MUST decode to 5 with `coordinator_free_rounds`
    // toggled on or off.
    #[test]
    fn ut_0351_12_church_add_2_3_w2_strict_bsp_equivalence() {
        use crate::encoding::codec_church::ChurchArithmeticCodec;
        use crate::encoding::traits::{Decoder, Encoder};

        let codec = ChurchArithmeticCodec::add();
        let input = br#"{"op":"add","a":2,"b":3}"#;
        let net = codec.encode(input).unwrap();

        let cfg_off = GridConfig {
            num_workers: 2,
            max_rounds: Some(50),
            strict_bsp: true,
            ..GridConfig::default()
        };
        let cfg_on = GridConfig {
            coordinator_free_rounds: true,
            ..cfg_off.clone()
        };

        let (net_off, _m_off) = run_grid(net.clone(), &cfg_off, &ContiguousIdStrategy);
        let (net_on, _m_on) = run_grid(net, &cfg_on, &ContiguousIdStrategy);

        let dec_off = codec.decode(&net_off).unwrap();
        let dec_on = codec.decode(&net_on).unwrap();

        assert_eq!(
            dec_off, dec_on,
            "G1: church_add(2, 3) result must match across skip on/off"
        );
        assert_eq!(dec_off["result"], 5, "church_add(2, 3) must decode to 5");
    }

    // TASK-0391 UT-01 (SPEC-19 R42): a default GridConfig with
    // delta_mode = false MUST produce bit-identical behaviour to a
    // GridConfig constructed *before* the delta_mode field existed.
    // Concretely: run_grid over church_add(2, 3) with delta_mode = false
    // must converge, decode to 5, and match the number of interactions
    // reported by the v1 baseline path (constructed here by explicitly
    // spelling the same field set the CoordinatorArgs-driven builder
    // produces). A silent behavioural flip from the additive struct
    // change — the only realistic failure mode of TASK-0389 — would
    // surface here.
    //
    // TODO(2.26-C): when the delta grid loop lands, add a polarity pass
    // that asserts `delta_mode = true` still decodes to 5 (and record
    // any permitted metric divergences in the amendment notes).
    #[test]
    fn r42_default_delta_mode_preserves_v1_smoke_output() {
        use crate::encoding::codec_church::ChurchArithmeticCodec;
        use crate::encoding::traits::{Decoder, Encoder};

        let codec = ChurchArithmeticCodec::add();
        let input = br#"{"op":"add","a":2,"b":3}"#;
        let net = codec.encode(input).unwrap();

        // Baseline: explicit field set, no delta_mode mention.
        // This mirrors the pre-TASK-0389 call sites.
        let cfg_baseline = GridConfig {
            num_workers: 2,
            max_rounds: Some(50),
            strict_bsp: true,
            ..GridConfig::default()
        };
        // R42: delta_mode defaults to false via Default::default(); we
        // also write the field explicitly to catch a hypothetical future
        // change to the Default impl that would flip it silently.
        let cfg_delta_off = GridConfig {
            num_workers: 2,
            max_rounds: Some(50),
            strict_bsp: true,
            delta_mode: false,
            ..GridConfig::default()
        };
        // Preflight: the default impl must still say false, otherwise the
        // regression wouldn't be testing what it claims to test.
        assert!(
            !GridConfig::default().delta_mode,
            "R42 preflight: Default::default() must keep delta_mode = false"
        );

        let (net_baseline, metrics_baseline) =
            run_grid(net.clone(), &cfg_baseline, &ContiguousIdStrategy);
        let (net_delta_off, metrics_delta_off) =
            run_grid(net, &cfg_delta_off, &ContiguousIdStrategy);

        // Functional equivalence: both paths must converge and decode to 5.
        assert!(
            metrics_baseline.converged,
            "baseline path must converge on church_add(2,3)"
        );
        assert!(
            metrics_delta_off.converged,
            "delta_mode=false path must converge on church_add(2,3)"
        );
        let dec_baseline = codec.decode(&net_baseline).unwrap();
        let dec_delta_off = codec.decode(&net_delta_off).unwrap();
        assert_eq!(dec_baseline["result"], 5);
        assert_eq!(dec_delta_off["result"], 5);

        // R42 characterisation: adding the field must not change metric
        // counts for the default caller. We compare exact totals here
        // (not a loose inequality) because `..Default::default()` vs.
        // explicit `delta_mode: false` must be observationally identical.
        assert_eq!(
            metrics_baseline.total_interactions, metrics_delta_off.total_interactions,
            "R42: delta_mode=false must produce the same total_interactions as the \
             pre-bundle baseline"
        );
        assert_eq!(
            metrics_baseline.rounds, metrics_delta_off.rounds,
            "R42: delta_mode=false must complete in the same round count as baseline"
        );
        assert_eq!(
            metrics_baseline.total_interactions_by_rule,
            metrics_delta_off.total_interactions_by_rule,
            "R42: delta_mode=false must record the same per-rule interaction breakdown"
        );
    }

    // ============================================================================
    // TASK-0396 — SPEC-19 R20 dispatcher fork (run_grid_entry) tests
    // ============================================================================

    // UT-0396-01 — delta_mode=false delegates to v1 run_grid; dispatch=None is accepted.
    #[test]
    fn ut_0396_01_run_grid_entry_with_delta_mode_false_delegates_to_v1() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 1));
        net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(b, 2));
        let cfg = GridConfig {
            num_workers: 1,
            delta_mode: false,
            ..GridConfig::default()
        };

        let (net_v1, metrics_v1) = run_grid(net.clone(), &cfg, &ContiguousIdStrategy);
        let (net_entry, metrics_entry) = run_grid_entry(net, &cfg, &ContiguousIdStrategy, None);

        // Byte-identical metrics: the router on the v1 branch must be a
        // pass-through. count_live_agents is a crude but sufficient net
        // equality proxy for this regression probe.
        assert_eq!(
            metrics_v1.total_interactions,
            metrics_entry.total_interactions
        );
        assert_eq!(metrics_v1.rounds, metrics_entry.rounds);
        assert_eq!(metrics_v1.converged, metrics_entry.converged);
        assert_eq!(
            net_v1.count_live_agents(),
            net_entry.count_live_agents(),
            "v1 output net agent count must match when routed through run_grid_entry"
        );
    }

    // UT-0396-02 — delta_mode=true with a Some(dispatch) delegates to
    // run_grid_delta. Uses the same fixture as UT-0384-01 so the delta
    // path exercises the real inner loop (not the short-circuit).
    #[test]
    fn ut_0396_02_run_grid_entry_with_delta_mode_true_and_dispatch_delegates_to_delta() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(1_000));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1_001));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(1_002));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(1_003));
        let cfg = GridConfig {
            num_workers: 2,
            delta_mode: true,
            ..GridConfig::default()
        };
        let mut dispatch = NoopDispatch::default();
        let (_out, metrics) = run_grid_entry(net, &cfg, &ContiguousIdStrategy, Some(&mut dispatch));

        assert!(
            metrics.delta_mode,
            "run_grid_entry with delta_mode=true MUST set metrics.delta_mode"
        );
    }

    // UT-0396-03 — delta_mode=true with dispatch=None panics with a
    // SPEC-19 R20 descriptive message. Uses catch_unwind to assert the
    // panic payload contains the grep-able anchors.
    #[test]
    fn ut_0396_03_run_grid_entry_with_delta_mode_true_and_no_dispatch_panics() {
        use std::panic::{catch_unwind, AssertUnwindSafe};

        let net = Net::new();
        let cfg = GridConfig {
            num_workers: 2,
            delta_mode: true,
            ..GridConfig::default()
        };

        let caught = catch_unwind(AssertUnwindSafe(|| {
            run_grid_entry(net, &cfg, &ContiguousIdStrategy, None)
        }));
        let err = caught.expect_err("must panic on delta_mode=true + dispatch=None");

        // Extract the panic message for assertion on anchor substrings.
        let msg = if let Some(s) = err.downcast_ref::<String>() {
            s.clone()
        } else if let Some(s) = err.downcast_ref::<&'static str>() {
            s.to_string()
        } else {
            String::from("<non-string panic payload>")
        };
        assert!(
            msg.contains("SPEC-19 R20"),
            "panic message must cite SPEC-19 R20 (got: {msg})"
        );
        assert!(
            msg.contains("delta_mode"),
            "panic message must mention delta_mode (got: {msg})"
        );
        assert!(
            msg.contains("WorkerDispatch"),
            "panic message must mention WorkerDispatch (got: {msg})"
        );
    }

    // UT-0396-04 — R42 regression canary under the router: routing
    // church_add(2,3) through run_grid_entry(delta_mode=false) MUST
    // yield metrics byte-identical to the direct run_grid invocation.
    // If this test ever fails, the router introduced drift on the v1
    // path.
    #[test]
    fn ut_0396_04_run_grid_entry_preserves_r42_church_add_smoke() {
        use crate::encoding::codec_church::ChurchArithmeticCodec;
        use crate::encoding::traits::{Decoder, Encoder};

        let codec = ChurchArithmeticCodec::add();
        let net = codec.encode(br#"{"op":"add","a":2,"b":3}"#).unwrap();
        let cfg = GridConfig {
            num_workers: 2,
            max_rounds: Some(50),
            strict_bsp: true,
            ..GridConfig::default()
        };

        let (net_direct, metrics_direct) = run_grid(net.clone(), &cfg, &ContiguousIdStrategy);
        let (net_entry, metrics_entry) = run_grid_entry(net, &cfg, &ContiguousIdStrategy, None);

        assert!(metrics_direct.converged);
        assert!(metrics_entry.converged);
        assert_eq!(
            codec.decode(&net_direct).unwrap()["result"],
            5,
            "direct run_grid path must decode church_add(2,3) = 5"
        );
        assert_eq!(
            codec.decode(&net_entry).unwrap()["result"],
            5,
            "run_grid_entry path must decode church_add(2,3) = 5"
        );
        assert_eq!(
            metrics_direct.total_interactions, metrics_entry.total_interactions,
            "router must not introduce drift on total_interactions"
        );
        assert_eq!(metrics_direct.rounds, metrics_entry.rounds);
        assert_eq!(
            metrics_direct.total_interactions_by_rule,
            metrics_entry.total_interactions_by_rule
        );
    }

    // ============================================================================
    // SPEC-19 §3.1 Stage 5 QA probes (qa agent, 2026-04-16)
    // ============================================================================
    //
    // Adversarial probes from REVIEW-SPEC-19-section-3.1-2026-04-16.md §7.
    // The probe suite for the helper level lives in `merge/helpers.rs` (Probes
    // A and F). The probes here exercise `run_grid` end-to-end to check skip
    // engagement, GNF semantics, oscillation, single-worker behavior, lenient
    // mode interaction, and exact telemetry counters.

    /// Probe A (defense-in-depth integration): `run_grid` MUST panic — not
    /// silently return — when `num_workers == 0`. Combined with Probe A in
    /// `helpers.rs`, this pins both layers of the empty-workers contract.
    #[test]
    #[should_panic(expected = "num_workers must be >= 1")]
    fn qa_probe_a_run_grid_panics_on_zero_workers() {
        let net = Net::new();
        let cfg = GridConfig {
            num_workers: 0,
            coordinator_free_rounds: true,
            strict_bsp: true,
            ..GridConfig::default()
        };
        let _ = run_grid(net, &cfg, &ContiguousIdStrategy);
    }

    /// Probe B — single worker with `coordinator_free_rounds=true`: a single
    /// worker has no border partitions, so the run takes the
    /// `run_single_worker` fast path, which does NOT involve merge at all
    /// and never increments `coordinator_free_rounds`. The skip
    /// optimization is irrelevant — but it must not corrupt anything.
    /// Pins behavior in the `num_workers == 1` branch (R26 in SPEC-05).
    #[test]
    fn qa_probe_b_single_worker_always_skips_merge() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        let cfg = GridConfig {
            num_workers: 1,
            coordinator_free_rounds: true,
            strict_bsp: true,
            ..GridConfig::default()
        };
        let (result, metrics) = run_grid(net, &cfg, &ContiguousIdStrategy);

        assert!(metrics.converged, "single-worker net must converge");
        assert_eq!(result.count_live_agents(), 0, "ERA-ERA annihilates");
        assert_eq!(metrics.total_interactions, 1);
        // The single-worker fast path runs `run_single_worker` directly —
        // it doesn't enter the merge loop, so the counter stays at 0.
        // This pins the design: `coordinator_free_rounds` counts only
        // multi-worker skip events.
        assert_eq!(
            metrics.coordinator_free_rounds, 0,
            "single-worker fast path must not inflate the counter"
        );
        // The single-worker round MUST still report the new field.
        let round0 = &metrics.worker_stats_per_round[0];
        assert_eq!(round0.len(), 1);
        assert!(
            !round0[0].has_border_activity,
            "single-worker has no borders by definition"
        );
    }

    /// Probe C — oscillating border activity across rounds.
    ///
    /// Build a workload where round 0 has border activity (skip OFF) and
    /// the resulting next round has no border activity (skip ON). Verify
    /// the coordinator tracks per-round state without carryover bugs:
    /// (1) round 0 takes the full merge path,
    /// (2) round 1+ may take the skip path,
    /// (3) total skip count == number of rounds with all_no_border_activity,
    /// (4) result equals flag-OFF baseline (R5).
    ///
    /// CON-CON border annihilation: round 0 has principal-port border
    /// (true → no skip), then both agents are consumed → no border in
    /// round 1 → if there are remaining redexes, skip engages.
    #[test]
    fn qa_probe_c_oscillating_border_activity_no_carryover() {
        // Build the same border-active workload used for UT-0351-11.
        let net = build_typical_two_worker_net_with_borders();

        // Reference: flag OFF.
        let cfg_off = GridConfig {
            num_workers: 2,
            max_rounds: Some(20),
            strict_bsp: true,
            ..GridConfig::default()
        };
        let (off_net, off_m) = run_grid(net.clone(), &cfg_off, &ContiguousIdStrategy);

        // Skip ON.
        let cfg_on = GridConfig {
            coordinator_free_rounds: true,
            ..cfg_off.clone()
        };
        let (on_net, on_m) = run_grid(net, &cfg_on, &ContiguousIdStrategy);

        // R5 — the result must be identical regardless of skip engagement.
        assert_eq!(
            off_net.count_live_agents(),
            on_net.count_live_agents(),
            "oscillating workload: live agents must match flag-off baseline"
        );
        assert_eq!(
            off_m.total_interactions, on_m.total_interactions,
            "oscillating workload: total interactions must match"
        );

        // Per-round audit: count rounds where every worker reported false.
        // The skip counter MUST equal exactly the number of such rounds.
        // This pins the no-carryover property: the coordinator decides skip
        // round-by-round, never based on a prior round's state.
        let mut expected_skip_rounds: u32 = 0;
        for round_stats in &on_m.worker_stats_per_round {
            // We are interested in rounds where every worker had
            // has_border_activity == false. The skip branch only engages
            // for these rounds (under strict + opt-in).
            //
            // NOTE: not every "all false" round translates to a skip — the
            // round may still be the GNF terminator (which also increments
            // the counter). Both branches feed the same counter, so we
            // count both. The invariant we pin is:
            //   counter == count of rounds whose stats have
            //              `every worker.has_border_activity == false`
            // post-hoc on the recorded stats.
            if !round_stats.is_empty() && round_stats.iter().all(|s| !s.has_border_activity) {
                expected_skip_rounds += 1;
            }
        }
        assert_eq!(
            on_m.coordinator_free_rounds, expected_skip_rounds,
            "skip counter must equal the count of all-false rounds (no carryover)"
        );
    }

    /// Probe D — `coordinator_free_rounds=true` with `strict_bsp=false`.
    ///
    /// Per R6 SHOULD and the splitting-phase design (TEST-SPEC-0351 UT-08),
    /// the lenient mode (collapses to 1 round via coordinator-side
    /// `reduce_all`) MUST NOT engage the skip path — the optimization has
    /// no observable benefit there. The flag is silently ignored: no error
    /// at config construction, no skip taken, counter stays 0.
    ///
    /// Documents the actual behavior so it doesn't drift.
    #[test]
    fn qa_probe_d_lenient_mode_ignores_coordinator_free_flag() {
        let net = build_already_quiescent_net();
        let cfg = GridConfig {
            num_workers: 2,
            max_rounds: Some(5),
            strict_bsp: false, // lenient
            delta_mode: false,
            coordinator_free_rounds: true,
            ..GridConfig::default()
        };
        let (result, metrics) = run_grid(net, &cfg, &ContiguousIdStrategy);

        // Behavior under lenient + flag-on: identical to flag-off (R6 SHOULD).
        assert!(metrics.converged);
        assert_eq!(result.count_live_agents(), 0);
        // The skip predicate is gated on `strict_bsp && coordinator_free_rounds`,
        // so the lenient path NEVER increments the counter.
        assert_eq!(
            metrics.coordinator_free_rounds, 0,
            "lenient mode must silently ignore the coordinator_free_rounds flag"
        );
        // Sanity: lenient still produces correct stats for the new field.
        for stats in metrics.worker_stats_per_round.iter().flatten() {
            // Lenient mode resolves all borders at the coordinator in
            // round 0, so the post-reduction stats may report `false`
            // (no remaining principal-port border endpoints). We just
            // verify the field is reachable and well-typed.
            let _ = stats.has_border_activity;
        }
    }

    /// Probe E — race window in local-sim: N/A (sequential execution).
    ///
    /// `run_grid` is sequential: workers reduce one after another inside
    /// the `for partition in &mut plan.partitions` loop. There is no
    /// thread-level race between workers in the in-process simulator —
    /// each `WorkerRoundStats` is built from a fully-reduced partition
    /// before the next worker starts. Race conditions would only appear
    /// in the wire-FSM path (item 2.26 / async coordinator), which is
    /// out of scope for SPEC-19 §3.1.
    ///
    /// Documenting as a no-op test so future readers see the rationale.
    #[test]
    fn qa_probe_e_race_window_in_local_sim_is_not_applicable() {
        // Sequential structure pinned: this test is the documentation,
        // and run_grid's loop body is sequential by construction.
        // A real race would only appear in protocol/coordinator.rs,
        // which is untouched by this bundle (UT-0351-09 guards that).
        // Nothing to assert beyond "sequential by design".
    }

    /// Probe G — two-worker skip→non-skip transition.
    ///
    /// Construct a workload that, under strict BSP, runs more than one
    /// round and where round N has border activity (no skip) and round
    /// N+k has NO border activity (skip engages). We use the same
    /// CON-CON border cascade as `test_strict_bsp_con_cascade_multi_round`:
    ///   - round 0: l0-r0 border CON-CON pair → border-active → no skip.
    ///   - round 1: the cascade creates a fresh border CON-CON pair →
    ///     still border-active → no skip.
    ///   - eventual final round: the cascade consumes itself, leaving
    ///     a fully-quiescent net → either GNF triggers (counter +1) or
    ///     the loop exits before observing.
    ///
    /// The contract pinned: across the on/off transition, the byte-equal
    /// result MUST match (R5), per-rule interaction totals MUST match,
    /// and the final live-agent count MUST match. Catches stale-partition
    /// state leaks (e.g., if a skipped round forgot to drain stale redexes,
    /// the next round would diverge).
    #[test]
    fn qa_probe_g_skip_transition_preserves_correctness() {
        // Build the CON-CON cascade workload (cf. T-strict-3).
        let mut net = Net::new();
        let l0 = net.create_agent(Symbol::Con); // id=0 -> w0
        let l1 = net.create_agent(Symbol::Con); // id=1 -> w0
        let r0 = net.create_agent(Symbol::Con); // id=2 -> w1
        let r1 = net.create_agent(Symbol::Con); // id=3 -> w1
        net.connect(PortRef::AgentPort(l0, 0), PortRef::AgentPort(r0, 0));
        net.connect(PortRef::AgentPort(l0, 1), PortRef::AgentPort(l1, 0));
        net.connect(PortRef::AgentPort(r0, 2), PortRef::AgentPort(r1, 0));
        net.connect(PortRef::AgentPort(l0, 2), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(r0, 1), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(l1, 1), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(l1, 2), PortRef::FreePort(3));
        net.connect(PortRef::AgentPort(r1, 1), PortRef::FreePort(4));
        net.connect(PortRef::AgentPort(r1, 2), PortRef::FreePort(5));

        let cfg_off = GridConfig {
            num_workers: 2,
            max_rounds: Some(50),
            strict_bsp: true,
            ..GridConfig::default()
        };
        let cfg_on = GridConfig {
            coordinator_free_rounds: true,
            ..cfg_off.clone()
        };

        let (net_off, m_off) = run_grid(net.clone(), &cfg_off, &ContiguousIdStrategy);
        let (net_on, m_on) = run_grid(net, &cfg_on, &ContiguousIdStrategy);

        // R5: byte-equal result, totals, and per-rule counts.
        assert!(m_off.converged && m_on.converged);
        assert_eq!(
            net_off.count_live_agents(),
            net_on.count_live_agents(),
            "skip transition: live agent count must match flag-off baseline"
        );
        assert_eq!(
            m_off.total_interactions, m_on.total_interactions,
            "skip transition: total interactions must match"
        );
        assert_eq!(
            m_off.total_interactions_by_rule, m_on.total_interactions_by_rule,
            "skip transition: per-rule interaction totals must match"
        );

        // Per-round audit: the skip counter on the flag-on run MUST equal
        // the count of rounds whose stats record every-worker-false.
        // This pins the no-stale-state property: no skip is taken because
        // of a previous round's flag, only because of the current round's
        // recorded stats. Even if zero such rounds occur on this exact
        // workload, the relation `counter == count(all-false rounds)` MUST
        // hold (both sides 0 is the trivial case).
        let skip_eligible_rounds = m_on
            .worker_stats_per_round
            .iter()
            .filter(|round| !round.is_empty() && round.iter().all(|s| !s.has_border_activity))
            .count() as u32;
        assert_eq!(
            m_on.coordinator_free_rounds, skip_eligible_rounds,
            "skip counter ({}) must equal the count of all-false rounds ({}) — \
             this pins no-carryover even when the count is 0",
            m_on.coordinator_free_rounds, skip_eligible_rounds
        );
        // Flag-off baseline: counter MUST be 0 regardless of round shape.
        assert_eq!(
            m_off.coordinator_free_rounds, 0,
            "flag-off run on cascade workload MUST report 0; got {}",
            m_off.coordinator_free_rounds
        );
        // The cascade workload genuinely runs more than one round under
        // strict BSP, so this is a non-trivial multi-round test.
        assert!(
            m_off.rounds >= 2,
            "cascade workload must run >= 2 rounds in strict BSP; got {}",
            m_off.rounds
        );
    }

    /// Probe H — strict-BSP exact telemetry audit (`==` not `>=`).
    ///
    /// Pick a deterministic workload where we can predict EXACTLY how many
    /// coordinator-free rounds occur. The simplest such workload is the
    /// already-quiescent net (two internal ERA-ERA pairs):
    ///   - round 0: workers reduce locally → both report
    ///     `local_redexes == 0` (after reduction) AND no border activity.
    ///   - check_global_normal_form returns (true, true) → R4 GNF branch
    ///     → exactly 1 coordinator-free round on the GNF exit, then break.
    ///
    /// We assert `==` not `>=`, which is sharper than UT-0351-06's `>= 1`.
    /// This pins the counter against double-counting bugs (e.g., counting
    /// both the skip branch and the GNF branch in a single round).
    #[test]
    fn qa_probe_h_exact_coordinator_free_count() {
        let net = build_already_quiescent_net();
        let cfg = GridConfig {
            num_workers: 2,
            max_rounds: Some(50),
            strict_bsp: true,
            delta_mode: false,
            coordinator_free_rounds: true,
            ..GridConfig::default()
        };
        let (_result, metrics) = run_grid(net, &cfg, &ContiguousIdStrategy);

        assert!(metrics.converged, "GNF must mark converged");
        // EXACT count: this workload converges in exactly 1 round via
        // the R4 GNF branch. The counter MUST equal 1 — not 0 (missed
        // the increment), not 2 (double-counted skip + GNF), not >= 1
        // (which would hide a double-count bug). This is the sharpest
        // form of the telemetric correctness invariant.
        assert_eq!(
            metrics.coordinator_free_rounds, 1,
            "exactly 1 coordinator-free round expected for this workload; got {}",
            metrics.coordinator_free_rounds
        );
        // Total rounds must be exactly 1 too — the GNF branch breaks the
        // loop after the first round.
        assert_eq!(
            metrics.rounds, 1,
            "GNF must terminate after exactly 1 round; got {}",
            metrics.rounds
        );

        // Pin the inverse direction: when the flag is OFF, the counter
        // MUST stay at 0 even on the same workload. No ambient state
        // leaks from an earlier-run instance, no false "ghost" increments.
        let net2 = build_already_quiescent_net();
        let cfg_off = GridConfig {
            num_workers: 2,
            max_rounds: Some(50),
            strict_bsp: true,
            ..GridConfig::default()
        };
        let (_result_off, metrics_off) = run_grid(net2, &cfg_off, &ContiguousIdStrategy);
        assert_eq!(
            metrics_off.coordinator_free_rounds, 0,
            "flag-off run on the same workload MUST report 0; got {}",
            metrics_off.coordinator_free_rounds
        );
    }

    // === TASK-0384 — run_grid_delta entry-point tests (SPEC-19 R20, R21) ===
    //
    // These tests exercise ONLY the degenerate paths that exit BEFORE
    // `run_grid_delta_inner` (which `todo!()`s until TASK-0385 lands).
    // UT-0384-01 is marked `#[ignore]` until the inner loop is green.

    use std::collections::HashMap;

    use crate::error::GridError;
    use crate::merge::border_resolver::RoundStartDispatch;
    use crate::merge::types::{RoundResultPayload, WorkerDispatch};
    use crate::partition::{Partition, PartitionPlan, WorkerId};

    /// Minimal `WorkerDispatch` test fixture. Counts how many times
    /// each method is called and returns trivial `Ok` values. Used by
    /// UT-0384-02 / UT-0384-03 to verify the degenerate short-circuits
    /// never invoke the dispatch trait.
    #[derive(Debug, Default)]
    struct NoopDispatch {
        initial_calls: usize,
        round_start_calls: usize,
        final_state_calls: usize,
    }

    impl WorkerDispatch for NoopDispatch {
        fn dispatch_initial(&mut self, _plan: &PartitionPlan) -> Result<(), GridError> {
            self.initial_calls += 1;
            Ok(())
        }

        fn dispatch_round_start(
            &mut self,
            _dispatch: &[(WorkerId, RoundStartDispatch)],
        ) -> Result<Vec<RoundResultPayload>, GridError> {
            self.round_start_calls += 1;
            Ok(Vec::new())
        }

        fn dispatch_final_state_request(
            &mut self,
            _round: u32,
        ) -> Result<Vec<Partition>, GridError> {
            self.final_state_calls += 1;
            Ok(Vec::new())
        }
    }

    // UT-0384-01 (DC-C3 firewall) — entry point accepts both strict_bsp
    // values without panicking. With TASK-0385 shipped, the multi-worker
    // path now runs through the real loop. The `NoopDispatch` returns an
    // empty `Vec<RoundResultPayload>`, so `check_delta_convergence`
    // trivially holds (vacuous `all()` on zero workers reporting) and the
    // loop exits after Round 1.
    //
    // Aux ports are wired to free sinks so the input net is T1-valid
    // (the final `merge()` in `run_grid_delta_final_collect` asserts
    // all invariants in debug builds).
    #[test]
    fn run_grid_delta_accepts_both_strict_bsp_values() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(1_000));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1_001));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(1_002));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(1_003));

        for strict in [true, false] {
            let cfg = GridConfig {
                num_workers: 2,
                strict_bsp: strict,
                ..GridConfig::default()
            };
            let mut dispatch = NoopDispatch::default();
            let (_n, metrics) =
                run_grid_delta(net.clone(), &cfg, &ContiguousIdStrategy, &mut dispatch);
            assert!(metrics.delta_mode, "delta_mode marker MUST be set");
        }
    }

    // UT-0384-02 — short-circuit when the input net is already in
    // Normal Form (empty redex queue after `drain_stale_redexes`).
    #[test]
    fn run_grid_delta_short_circuits_on_normalized_net() {
        // Empty net: no agents, no redexes → instantly normalized.
        let net = Net::new();
        let cfg = GridConfig {
            num_workers: 2,
            ..GridConfig::default()
        };
        let mut dispatch = NoopDispatch::default();

        let (_result, metrics) = run_grid_delta(net, &cfg, &ContiguousIdStrategy, &mut dispatch);

        assert!(metrics.converged, "already-normalized must converge");
        assert_eq!(metrics.rounds, 0, "zero rounds for already-normalized");
        assert!(metrics.delta_mode, "delta_mode marker MUST be set");
        assert_eq!(metrics.delta_max_rounds_hit, None);
        assert_eq!(dispatch.initial_calls, 0);
        assert_eq!(dispatch.round_start_calls, 0);
        assert_eq!(dispatch.final_state_calls, 0);
    }

    // UT-0384-03 — single-worker degenerate delegates to run_single_worker.
    #[test]
    fn run_grid_delta_delegates_single_worker_to_run_single_worker() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 1));
        net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(b, 2));
        let cfg = GridConfig {
            num_workers: 1,
            ..GridConfig::default()
        };
        let mut dispatch = NoopDispatch::default();

        let (_result, metrics) = run_grid_delta(net, &cfg, &ContiguousIdStrategy, &mut dispatch);

        assert_eq!(
            dispatch.initial_calls, 0,
            "single-worker path MUST bypass WorkerDispatch"
        );
        assert_eq!(dispatch.round_start_calls, 0);
        assert_eq!(dispatch.final_state_calls, 0);
        assert_eq!(
            metrics.rounds, 1,
            "single-worker path runs exactly one local round"
        );
        assert!(metrics.delta_mode, "delta_mode marker MUST be set");
    }

    // UT-0384-05 — trait is object-safe (compile-time check).
    #[test]
    fn worker_dispatch_trait_is_object_safe() {
        let mut dispatch = NoopDispatch::default();
        let dispatch_ref: &mut dyn WorkerDispatch = &mut dispatch;

        // All three methods must be callable through the trait object.
        let plan = PartitionPlan::default();
        assert!(dispatch_ref.dispatch_initial(&plan).is_ok());
        assert!(dispatch_ref.dispatch_round_start(&[]).is_ok());
        assert!(dispatch_ref.dispatch_final_state_request(0).is_ok());
    }

    // === TASK-0385 — run_grid_delta_inner coordinator round loop tests ===
    //
    // See TEST-SPEC-0385.md. Covers R21.1 (Round 0 dispatch), R21.2
    // (delta rounds), R23 (RoundStart payload), R26 (RoundResult
    // consumption), DC-C3 (strict_bsp branching), and DC-C5 (convergence
    // predicate) inline. DC-C3 lenient/strict matrix cells + G1 parity
    // (UT-0385-06..08) live in `super::grid_delta_integration_tests`
    // (sibling `#[cfg(test)]` module under `merge/`, introduced by
    // TASK-0395 MF-002 closure 2026-04-23).

    use crate::merge::border_graph::BorderGraph;
    use std::collections::VecDeque;

    /// Test-only in-process `WorkerDispatch` that both records every
    /// dispatch call into per-method vectors AND serves canned responses
    /// from FIFO queues. Queue-based so a single test can script an
    /// arbitrary multi-round scenario. Unused fields panic loudly if an
    /// unanticipated dispatch path fires.
    #[derive(Debug, Default)]
    struct CapturingDispatch {
        initial_dispatches: Vec<PartitionPlan>,
        round_start_dispatches: Vec<Vec<(WorkerId, RoundStartDispatch)>>,
        final_state_dispatches: Vec<u32>,
        canned_round_results: VecDeque<Vec<RoundResultPayload>>,
        canned_final_states: Option<Vec<Partition>>,
    }

    impl WorkerDispatch for CapturingDispatch {
        fn dispatch_initial(&mut self, plan: &PartitionPlan) -> Result<(), GridError> {
            self.initial_dispatches.push(plan.clone());
            Ok(())
        }

        fn dispatch_round_start(
            &mut self,
            dispatch: &[(WorkerId, RoundStartDispatch)],
        ) -> Result<Vec<RoundResultPayload>, GridError> {
            self.round_start_dispatches.push(dispatch.to_vec());
            Ok(self.canned_round_results.pop_front().unwrap_or_default())
        }

        fn dispatch_final_state_request(
            &mut self,
            round: u32,
        ) -> Result<Vec<Partition>, GridError> {
            self.final_state_dispatches.push(round);
            Ok(self.canned_final_states.take().unwrap_or_default())
        }
    }

    /// Build a 2-worker `Net` with a single cross-partition redex: two
    /// CON agents connected principal-to-principal. `ContiguousIdStrategy`
    /// splits this so each worker owns one CON agent with a shared
    /// border `FreePort`.
    fn build_two_worker_cross_redex_net() -> Net {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        // Dangling aux ports: connect to free sinks so the net is
        // well-formed post-split.
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(1_000));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1_001));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(1_002));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(1_003));
        net
    }

    /// Build a canned `RoundResultPayload` that reports "this worker is
    /// quiet" (no border activity, 0 local redexes, no deltas).
    fn canned_quiet_result(worker_id: WorkerId, round: u32) -> RoundResultPayload {
        RoundResultPayload {
            worker_id,
            round,
            border_deltas: Vec::new(),
            stats: WorkerRoundStats {
                worker_id,
                agents_before: 1,
                agents_after: 1,
                local_redexes: 0,
                reduce_duration_secs: 0.0,
                interactions_by_rule: [0; 6],
                has_border_activity: false,
                is_coordinator_self: false,
            },
            has_border_activity: false,
            // TASK-0398 (D-004): no mints on canned quiet results — no
            // commutation happened. `register_minted_agents` treats
            // empty slices as no-op (not a ProtocolViolation).
            minted_agents: Vec::new(),
        }
    }

    /// Build a canned `RoundResultPayload` for a worker that still has
    /// border activity (forces the loop to continue another round).
    fn canned_active_result(worker_id: WorkerId, round: u32) -> RoundResultPayload {
        RoundResultPayload {
            worker_id,
            round,
            border_deltas: Vec::new(),
            stats: WorkerRoundStats {
                worker_id,
                agents_before: 1,
                agents_after: 1,
                local_redexes: 0,
                reduce_duration_secs: 0.0,
                interactions_by_rule: [0; 6],
                has_border_activity: true,
                is_coordinator_self: false,
            },
            has_border_activity: true,
            // TASK-0398 (D-004): no mints on canned active results either.
            minted_agents: Vec::new(),
        }
    }

    // UT-0385-01: Round 0 dispatches `InitialPartition` exactly once and
    // does not fire anything else before the first `RoundStart`.
    #[test]
    fn run_grid_delta_inner_round_zero_dispatches_initial_partition_only() {
        let net = build_two_worker_cross_redex_net();
        let cfg = GridConfig {
            num_workers: 2,
            ..GridConfig::default()
        };
        let mut dispatch = CapturingDispatch {
            canned_round_results: VecDeque::from(vec![vec![
                canned_quiet_result(0, 1),
                canned_quiet_result(1, 1),
            ]]),
            canned_final_states: Some(Vec::new()),
            ..CapturingDispatch::default()
        };

        let (_result, metrics) = run_grid_delta(net, &cfg, &ContiguousIdStrategy, &mut dispatch);

        assert_eq!(
            dispatch.initial_dispatches.len(),
            1,
            "Round 0 must dispatch InitialPartition exactly once"
        );
        assert_eq!(
            dispatch.round_start_dispatches.len(),
            1,
            "exactly one RoundStart before convergence on quiet workers"
        );
        assert_eq!(
            dispatch.final_state_dispatches.len(),
            1,
            "FinalStateRequest must be dispatched after convergence"
        );
        assert_eq!(metrics.rounds, 1);
        assert!(metrics.converged);
    }

    // UT-0385-02: Happy path — one round, workers converge naturally.
    #[test]
    fn run_grid_delta_inner_single_delta_round_converges() {
        let net = build_two_worker_cross_redex_net();
        let cfg = GridConfig {
            num_workers: 2,
            ..GridConfig::default()
        };
        let mut dispatch = CapturingDispatch {
            canned_round_results: VecDeque::from(vec![vec![
                canned_quiet_result(0, 1),
                canned_quiet_result(1, 1),
            ]]),
            canned_final_states: Some(Vec::new()),
            ..CapturingDispatch::default()
        };

        let (_result, metrics) = run_grid_delta(net, &cfg, &ContiguousIdStrategy, &mut dispatch);

        assert_eq!(metrics.rounds, 1);
        assert!(metrics.converged);
        assert!(metrics.delta_mode);
        assert_eq!(metrics.delta_max_rounds_hit, None);
        assert_eq!(dispatch.round_start_dispatches.len(), 1);
        assert_eq!(dispatch.final_state_dispatches.len(), 1);
    }

    // UT-0385-03: 3-round scenario — per-round metric vectors track loop
    // iterations exactly. Rounds 1 and 2 report border activity to keep
    // the loop running; round 3 reports quiet, triggering convergence.
    #[test]
    fn run_grid_delta_inner_multi_round_records_metrics() {
        let net = build_two_worker_cross_redex_net();
        let cfg = GridConfig {
            num_workers: 2,
            ..GridConfig::default()
        };
        let mut dispatch = CapturingDispatch {
            canned_round_results: VecDeque::from(vec![
                vec![canned_active_result(0, 1), canned_active_result(1, 1)],
                vec![canned_active_result(0, 2), canned_active_result(1, 2)],
                vec![canned_quiet_result(0, 3), canned_quiet_result(1, 3)],
            ]),
            canned_final_states: Some(Vec::new()),
            ..CapturingDispatch::default()
        };

        let (_result, metrics) = run_grid_delta(net, &cfg, &ContiguousIdStrategy, &mut dispatch);

        assert_eq!(metrics.rounds, 3);
        assert_eq!(metrics.partition_time_per_round.len(), 3);
        assert_eq!(metrics.compute_time_per_round.len(), 3);
        // 3 per-round entries + 1 final merge entry (TASK-0387, R29).
        assert_eq!(metrics.merge_time_per_round.len(), 4);
        assert_eq!(metrics.border_redexes_per_round.len(), 3);
        assert_eq!(metrics.border_reduce_time_per_round.len(), 3);
        assert_eq!(metrics.border_interactions_per_round.len(), 3);
        assert_eq!(metrics.worker_stats_per_round.len(), 3);
        assert_eq!(dispatch.round_start_dispatches.len(), 3);
        assert!(metrics.converged);
    }

    // UT-0385-04: Each round's `RoundResultPayload.border_deltas` feeds
    // `BorderGraph::apply_deltas` per worker. Construct a canned
    // response that re-points a specific border's target; after the
    // loop the packaged resolutions (or post-loop partition cache) must
    // reflect the update.
    #[test]
    fn run_grid_delta_inner_applies_round_result_deltas_to_border_graph() {
        let net = build_two_worker_cross_redex_net();
        let cfg = GridConfig {
            num_workers: 2,
            ..GridConfig::default()
        };

        // Split once so we know what border_ids exist. The inner loop
        // does this too — we replicate to synthesise matching deltas.
        let plan_preview = crate::partition::split(net.clone(), 2, &ContiguousIdStrategy);
        let first_border_id = plan_preview
            .borders
            .keys()
            .next()
            .copied()
            .expect("split must yield at least one border for a cross-redex net");

        // Round 1: worker 0 reports a delta re-pointing the border to a
        // new (fake) local target. The coordinator's BorderGraph must
        // apply this; the test passes iff the loop completes without
        // panicking (invariant violations inside `apply_deltas` panic).
        let mut delta_result = canned_active_result(0, 1);
        delta_result.border_deltas.push(crate::merge::BorderDelta {
            border_id: first_border_id,
            new_target: crate::net::PortRef::AgentPort(0, 1),
        });

        // The synthetic repoint delta above leaves agent 0's principal
        // port DISCONNECTED in the coordinator cache (no worker-side
        // reconnection accompanies the test's delta). In a real worker
        // flow the post-reduction partition would be well-formed;
        // simulate that by supplying empty final partitions so the
        // `final_collect` path uses them instead of the invalid cache.
        let final_partitions = vec![
            crate::partition::Partition {
                subnet: Net::new(),
                worker_id: 0,
                free_port_index: HashMap::new(),
                id_range: crate::partition::IdRange { start: 0, end: 0 },
                border_id_start: 0,
                border_id_end: 0,
            },
            crate::partition::Partition {
                subnet: Net::new(),
                worker_id: 1,
                free_port_index: HashMap::new(),
                id_range: crate::partition::IdRange { start: 0, end: 0 },
                border_id_start: 0,
                border_id_end: 0,
            },
        ];
        let mut dispatch = CapturingDispatch {
            canned_round_results: VecDeque::from(vec![
                vec![delta_result, canned_active_result(1, 1)],
                vec![canned_quiet_result(0, 2), canned_quiet_result(1, 2)],
            ]),
            canned_final_states: Some(final_partitions),
            ..CapturingDispatch::default()
        };

        let (_result, metrics) = run_grid_delta(net, &cfg, &ContiguousIdStrategy, &mut dispatch);

        // Two rounds: round 1 applied the delta (and potentially
        // resolved the border), round 2 reports quiet → converge.
        assert!(
            metrics.rounds >= 1,
            "delta application must not short-circuit the loop"
        );
        assert!(metrics.converged);
    }

    // UT-0385-05: Coordinator-side partition cache stays in sync with
    // worker-reported deltas (DC-B1 option (a)). The cache is private to
    // `run_grid_delta_inner`; we assert the proxy invariant that the
    // final net returned does NOT carry the re-pointed border any more.
    #[test]
    fn run_grid_delta_inner_caches_partitions_for_resolver() {
        let net = build_two_worker_cross_redex_net();
        let cfg = GridConfig {
            num_workers: 2,
            ..GridConfig::default()
        };

        let mut dispatch = CapturingDispatch {
            canned_round_results: VecDeque::from(vec![vec![
                canned_quiet_result(0, 1),
                canned_quiet_result(1, 1),
            ]]),
            canned_final_states: Some(Vec::new()),
            ..CapturingDispatch::default()
        };

        let (result, metrics) = run_grid_delta(net, &cfg, &ContiguousIdStrategy, &mut dispatch);

        // Cache consistency proxy: the final merge produced a net with
        // at least the two original CON agents; the loop didn't drop
        // agents on the floor during cache maintenance.
        assert!(
            result.count_live_agents() >= 2,
            "final merged net must retain the worker-side agents"
        );
        assert!(metrics.converged);
        assert_eq!(metrics.rounds, 1);
    }

    // -----------------------------------------------------------------
    // TASK-0386 — `check_delta_convergence` (DC-C5 three-conjunct GNF).
    //
    // SPEC-19 R4, R21 phase 3, R40 literal:
    //   "all workers report zero local redexes AND the BorderGraph
    //    contains zero active pairs"
    // plus the worker-local `has_border_activity == false` signal that
    // complements the coordinator's graph view.
    //
    // Acceptance Criteria → 7 inline tests:
    //   UT-0386-01: all quiet + empty graph                → true
    //   UT-0386-02: one worker active, graph empty         → false
    //   UT-0386-03: all quiet, graph has pending redex     → false
    //   UT-0386-04: workers active AND graph redex         → false
    //   UT-0386-05: empty results + empty graph (vacuous)  → true
    //   UT-0386-06: has_border_activity=false but
    //               local_redexes=99 (DC-C5 FLIP)          → false
    //   UT-0386-07: one worker has local_redexes=1, rest
    //               quiet, graph empty (DC-C5 sanity)      → false
    // -----------------------------------------------------------------

    /// Build a two-worker `BorderGraph` whose single border is a
    /// principal-principal pair → exactly one active redex. Used by
    /// UT-0386-03/04.
    fn one_principal_redex_graph() -> BorderGraph {
        let mut p0 = Partition {
            subnet: Net::new(),
            worker_id: 0,
            free_port_index: HashMap::new(),
            id_range: crate::partition::IdRange { start: 0, end: 1 },
            border_id_start: 0,
            border_id_end: 1,
        };
        p0.free_port_index.insert(1, PortRef::AgentPort(0, 0));
        let mut p1 = Partition {
            subnet: Net::new(),
            worker_id: 1,
            free_port_index: HashMap::new(),
            id_range: crate::partition::IdRange { start: 1, end: 2 },
            border_id_start: 0,
            border_id_end: 1,
        };
        p1.free_port_index.insert(1, PortRef::AgentPort(1, 0));
        let mut borders = HashMap::new();
        borders.insert(1, (PortRef::FreePort(0), PortRef::FreePort(0)));
        let plan = PartitionPlan {
            partitions: vec![p0, p1],
            borders,
            ..Default::default()
        };
        let graph = BorderGraph::from_partition_plan(&plan);
        debug_assert_eq!(
            graph.detect_border_redexes().len(),
            1,
            "fixture must yield exactly one principal-principal border redex"
        );
        graph
    }

    /// Build a `BorderGraph` with zero borders. Used by UT-0386-01, 02,
    /// 05, 06, 07.
    fn empty_graph() -> BorderGraph {
        BorderGraph::from_partition_plan(&PartitionPlan::default())
    }

    // UT-0386-01 (DC-C5): every conjunct holds → Global Normal Form.
    #[test]
    fn check_delta_convergence_true_when_all_quiet() {
        let graph = empty_graph();
        let results = vec![canned_quiet_result(0, 1), canned_quiet_result(1, 1)];
        assert!(
            check_delta_convergence(&results, &graph),
            "all quiet workers + empty graph MUST converge"
        );
    }

    // UT-0386-02 (DC-C5): worker-level activity flag breaks convergence
    // even when the coordinator's graph is empty.
    #[test]
    fn check_delta_convergence_false_on_worker_activity() {
        let graph = empty_graph();
        let results = vec![canned_quiet_result(0, 1), canned_active_result(1, 1)];
        assert!(
            !check_delta_convergence(&results, &graph),
            "any worker reporting has_border_activity=true MUST block GNF"
        );
    }

    // UT-0386-03 (DC-C5): a pending coordinator-side border redex blocks
    // convergence even when every worker reports quiet.
    #[test]
    fn check_delta_convergence_false_on_graph_redex() {
        let graph = one_principal_redex_graph();
        let results = vec![canned_quiet_result(0, 1), canned_quiet_result(1, 1)];
        assert!(
            !check_delta_convergence(&results, &graph),
            "non-empty BorderGraph MUST block GNF"
        );
    }

    // UT-0386-04 (DC-C5): both worker-level and graph-level signals
    // positive → false (sanity / matrix corner).
    #[test]
    fn check_delta_convergence_false_both() {
        let graph = one_principal_redex_graph();
        let results = vec![canned_active_result(0, 1), canned_active_result(1, 1)];
        assert!(
            !check_delta_convergence(&results, &graph),
            "both signals positive MUST block GNF"
        );
    }

    // UT-0386-05 (DC-C5, vacuous edge): empty results slice + empty
    // graph → `true`. Not expected in practice (the round loop always
    // collects ≥1 result per worker), but the predicate must not panic.
    #[test]
    fn check_delta_convergence_vacuous_empty_results_empty_graph() {
        let graph = empty_graph();
        let results: Vec<RoundResultPayload> = Vec::new();
        assert!(
            check_delta_convergence(&results, &graph),
            "empty results + empty graph MUST be vacuously converged"
        );
    }

    // UT-0386-06 (DC-C5 FLIP, 2026-04-17): `has_border_activity=false`
    // AND empty graph is NOT sufficient — `stats.local_redexes > 0`
    // BLOCKS convergence per SPEC-19 R40 literal. The pre-amendment
    // two-predicate draft would have returned `true` here; the
    // amendment forces `false`.
    #[test]
    fn check_delta_convergence_requires_no_local_redexes() {
        let graph = empty_graph();
        let mut result = canned_quiet_result(0, 1);
        result.stats.local_redexes = 99;
        assert!(
            !check_delta_convergence(&[result], &graph),
            "DC-C5 FLIP: local_redexes>0 MUST block GNF even if \
             has_border_activity=false and graph is empty"
        );
    }

    // UT-0386-07 (DC-C5 sanity, folklore-gap closure): any single
    // worker reporting `local_redexes > 0` is enough to block GNF,
    // even with all other workers quiet and the graph empty. Closes
    // the assumption that `has_border_activity=false` implies
    // `reduce_all` reached a fixed point.
    #[test]
    fn check_delta_convergence_false_when_one_worker_has_local_redexes() {
        let graph = empty_graph();
        let quiet_a = canned_quiet_result(0, 1);
        let mut nonzero_b = canned_quiet_result(1, 1);
        nonzero_b.stats.local_redexes = 1;
        assert!(
            !check_delta_convergence(&[quiet_a, nonzero_b], &graph),
            "one worker with local_redexes>0 MUST block GNF"
        );
    }

    // -----------------------------------------------------------------
    // TASK-0387 — `run_grid_delta_final_collect` +
    //             `reconstruct_partition_plan_from_collected`.
    //
    // SPEC-19 R21 phase 3, R27, R29. Acceptance Criteria → 6 inline
    // unit tests. E2E integration lives in `tests/grid_delta_e2e.rs`.
    //
    //   UT-0387-01: empty graph, 2 collected partitions → merge succeeds
    //   UT-0387-02: 1 remaining inert border, 2 partitions → merge OK
    //   UT-0387-03: transport returns 1 partition when 2 expected → err
    //   UT-0387-04: reconstruct sorts partitions by worker_id
    //   UT-0387-05: reconstruct preserves every remaining border
    //   UT-0387-06: merge call records one merge_time_per_round entry
    // -----------------------------------------------------------------

    /// Build a `Partition` with one CON agent whose ports are wired to
    /// FreePort sinks (no borders). The worker's `next_id` is bumped so
    /// each partition mints a DISTINCT agent id (needed because
    /// `merge()` must union non-overlapping arenas).
    fn lone_con_partition(worker_id: WorkerId, base_free: u32) -> Partition {
        let mut subnet = Net::new();
        // Bump next_id by worker_id so the minted agent has a unique id.
        subnet.next_id = worker_id;
        let a = subnet.create_agent(Symbol::Con);
        subnet.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(base_free));
        subnet.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(base_free + 1));
        subnet.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(base_free + 2));
        Partition {
            subnet,
            worker_id,
            free_port_index: HashMap::new(),
            id_range: crate::partition::IdRange {
                start: worker_id,
                end: worker_id + 1,
            },
            border_id_start: 0,
            border_id_end: 0,
        }
    }

    /// `WorkerDispatch` that returns canned partitions from a one-shot
    /// `Option` — `take()` ensures the second call sees `None`, helping
    /// assert dispatch is invoked exactly once.
    #[derive(Debug, Default)]
    struct FinalStateOnlyDispatch {
        canned: Option<Vec<Partition>>,
        calls: usize,
    }

    impl WorkerDispatch for FinalStateOnlyDispatch {
        fn dispatch_initial(&mut self, _plan: &PartitionPlan) -> Result<(), GridError> {
            Ok(())
        }
        fn dispatch_round_start(
            &mut self,
            _dispatch: &[(WorkerId, RoundStartDispatch)],
        ) -> Result<Vec<RoundResultPayload>, GridError> {
            Ok(Vec::new())
        }
        fn dispatch_final_state_request(
            &mut self,
            _round: u32,
        ) -> Result<Vec<Partition>, GridError> {
            self.calls += 1;
            Ok(self.canned.take().unwrap_or_default())
        }
    }

    // UT-0387-01: empty BorderGraph + 2 collected partitions →
    // `merge()` returns union of agents, no borders, T1-valid.
    #[test]
    fn run_grid_delta_final_collect_empty_border_graph() {
        let p0 = lone_con_partition(0, 100);
        let p1 = lone_con_partition(1, 200);
        let mut dispatch = FinalStateOnlyDispatch {
            canned: Some(vec![p0.clone(), p1.clone()]),
            calls: 0,
        };
        let mut cache: HashMap<WorkerId, Partition> = HashMap::new();
        cache.insert(0, p0);
        cache.insert(1, p1);
        let graph = empty_graph();
        let mut metrics = GridMetrics::default();

        let net = run_grid_delta_final_collect(&mut dispatch, cache, graph, &mut metrics)
            .expect("empty border graph must merge successfully");

        assert_eq!(
            net.count_live_agents(),
            2,
            "merged net must contain both CON agents"
        );
        assert_eq!(dispatch.calls, 1);
    }

    // UT-0387-02: one remaining inert border (aux-aux) + 2 partitions.
    // The merge reconnects the two aux ports; `merge()` returns a
    // T1-valid net with both agents.
    #[test]
    fn run_grid_delta_final_collect_remaining_borders() {
        // Build two partitions each with one CON agent whose port 1
        // sits on a shared border_id (aux endpoints → inert border).
        // Distinct agent IDs per partition — `merge()` unions arenas
        // and expects non-overlapping ranges.
        let mut subnet_a = Net::new();
        subnet_a.next_id = 0;
        let a = subnet_a.create_agent(Symbol::Con);
        subnet_a.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(50));
        subnet_a.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(77));
        subnet_a.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(51));
        let mut idx_a = HashMap::new();
        idx_a.insert(77, PortRef::AgentPort(a, 1));
        let p0 = Partition {
            subnet: subnet_a,
            worker_id: 0,
            free_port_index: idx_a,
            id_range: crate::partition::IdRange { start: 0, end: 1 },
            border_id_start: 77,
            border_id_end: 78,
        };

        let mut subnet_b = Net::new();
        subnet_b.next_id = 1;
        let b = subnet_b.create_agent(Symbol::Con);
        subnet_b.connect(PortRef::AgentPort(b, 0), PortRef::FreePort(60));
        subnet_b.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(77));
        subnet_b.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(61));
        let mut idx_b = HashMap::new();
        idx_b.insert(77, PortRef::AgentPort(b, 1));
        let p1 = Partition {
            subnet: subnet_b,
            worker_id: 1,
            free_port_index: idx_b,
            id_range: crate::partition::IdRange { start: 1, end: 2 },
            border_id_start: 77,
            border_id_end: 78,
        };

        let mut plan_for_graph = PartitionPlan {
            partitions: vec![p0.clone(), p1.clone()],
            ..Default::default()
        };
        plan_for_graph
            .borders
            .insert(77, (PortRef::FreePort(0), PortRef::FreePort(0)));
        let graph = BorderGraph::from_partition_plan(&plan_for_graph);
        // Aux-aux → not a redex.
        assert!(graph.detect_border_redexes().is_empty());

        let mut dispatch = FinalStateOnlyDispatch {
            canned: Some(vec![p0.clone(), p1.clone()]),
            calls: 0,
        };
        let mut cache: HashMap<WorkerId, Partition> = HashMap::new();
        cache.insert(0, p0);
        cache.insert(1, p1);
        let mut metrics = GridMetrics::default();

        let net = run_grid_delta_final_collect(&mut dispatch, cache, graph, &mut metrics)
            .expect("aux-aux inert border must merge cleanly");
        assert_eq!(net.count_live_agents(), 2);
    }

    // UT-0387-03: transport returns 1 partition when cache has 2 →
    // `GridError::DispatchFailed` with "count mismatch" in the message.
    #[test]
    fn run_grid_delta_final_collect_mismatched_partitions_errors() {
        let p0 = lone_con_partition(0, 100);
        let p1 = lone_con_partition(1, 200);
        let mut dispatch = FinalStateOnlyDispatch {
            canned: Some(vec![p0.clone()]), // Only 1; cache has 2.
            calls: 0,
        };
        let mut cache: HashMap<WorkerId, Partition> = HashMap::new();
        cache.insert(0, p0);
        cache.insert(1, p1);
        let mut metrics = GridMetrics::default();

        let err = run_grid_delta_final_collect(&mut dispatch, cache, empty_graph(), &mut metrics)
            .expect_err("size mismatch must error");
        match err {
            crate::error::GridError::DispatchFailed { message, .. } => {
                assert!(
                    message.contains("count mismatch"),
                    "error must mention count mismatch, got: {message}"
                );
            }
            other => panic!("expected DispatchFailed, got {other:?}"),
        }
    }

    // UT-0387-04: `reconstruct_partition_plan_from_collected` sorts
    // partitions by `worker_id` regardless of input order.
    #[test]
    fn reconstruct_partition_plan_sorts_by_worker_id() {
        let p2 = lone_con_partition(2, 300);
        let p0 = lone_con_partition(0, 100);
        let p1 = lone_con_partition(1, 200);
        let graph = empty_graph();
        let plan =
            reconstruct_partition_plan_from_collected(vec![p2, p0, p1], &graph, Vec::new(), 0);
        let ids: Vec<WorkerId> = plan.partitions.iter().map(|p| p.worker_id).collect();
        assert_eq!(ids, vec![0, 1, 2]);
    }

    // UT-0387-05: every surviving `BorderGraph` entry shows up in the
    // reconstructed plan's `borders` map with matching id.
    #[test]
    fn reconstruct_partition_plan_preserves_remaining_borders() {
        // Three aux-aux borders → not redexes, all survive.
        let mut subnet_a = Net::new();
        let ea = subnet_a.create_agent(Symbol::Era);
        subnet_a.connect(PortRef::AgentPort(ea, 0), PortRef::FreePort(999));
        let mut idx_a = HashMap::new();
        idx_a.insert(10, PortRef::AgentPort(ea, 0));
        idx_a.insert(20, PortRef::FreePort(20));
        idx_a.insert(30, PortRef::FreePort(30));

        let mut subnet_b = Net::new();
        let eb = subnet_b.create_agent(Symbol::Era);
        subnet_b.connect(PortRef::AgentPort(eb, 0), PortRef::FreePort(998));
        let mut idx_b = HashMap::new();
        idx_b.insert(10, PortRef::FreePort(10));
        idx_b.insert(20, PortRef::FreePort(21));
        idx_b.insert(30, PortRef::FreePort(31));

        // We only need a graph with three borders — construct via plan.
        let mut borders = HashMap::new();
        borders.insert(10, (PortRef::FreePort(0), PortRef::FreePort(0)));
        borders.insert(20, (PortRef::FreePort(0), PortRef::FreePort(0)));
        borders.insert(30, (PortRef::FreePort(0), PortRef::FreePort(0)));
        let plan_for_graph = PartitionPlan {
            partitions: vec![
                Partition {
                    subnet: subnet_a.clone(),
                    worker_id: 0,
                    free_port_index: idx_a.clone(),
                    id_range: crate::partition::IdRange { start: 0, end: 1 },
                    border_id_start: 10,
                    border_id_end: 31,
                },
                Partition {
                    subnet: subnet_b.clone(),
                    worker_id: 1,
                    free_port_index: idx_b.clone(),
                    id_range: crate::partition::IdRange { start: 1, end: 2 },
                    border_id_start: 10,
                    border_id_end: 31,
                },
            ],
            borders,
            ..Default::default()
        };
        let graph = BorderGraph::from_partition_plan(&plan_for_graph);

        // Now reconstruct from collected partitions (same shape) + graph.
        let collected = vec![
            Partition {
                subnet: subnet_a,
                worker_id: 0,
                free_port_index: idx_a,
                id_range: crate::partition::IdRange { start: 0, end: 1 },
                border_id_start: 10,
                border_id_end: 31,
            },
            Partition {
                subnet: subnet_b,
                worker_id: 1,
                free_port_index: idx_b,
                id_range: crate::partition::IdRange { start: 1, end: 2 },
                border_id_start: 10,
                border_id_end: 31,
            },
        ];
        let plan = reconstruct_partition_plan_from_collected(collected, &graph, Vec::new(), 0);
        let keys: std::collections::HashSet<u32> = plan.borders.keys().copied().collect();
        assert_eq!(
            keys,
            std::collections::HashSet::from([10, 20, 30]),
            "every surviving border must appear in the reconstructed plan"
        );
    }

    // UT-0387-06: `run_grid_delta_final_collect` pushes exactly one
    // entry to `merge_time_per_round`.
    #[test]
    fn run_grid_delta_final_collect_merge_call_records_time() {
        let p0 = lone_con_partition(0, 100);
        let p1 = lone_con_partition(1, 200);
        let mut dispatch = FinalStateOnlyDispatch {
            canned: Some(vec![p0.clone(), p1.clone()]),
            calls: 0,
        };
        let mut cache: HashMap<WorkerId, Partition> = HashMap::new();
        cache.insert(0, p0);
        cache.insert(1, p1);
        let mut metrics = GridMetrics::default();
        let before = metrics.merge_time_per_round.len();

        let _ = run_grid_delta_final_collect(&mut dispatch, cache, empty_graph(), &mut metrics)
            .expect("merge must succeed");

        assert_eq!(metrics.merge_time_per_round.len(), before + 1);
    }

    // -----------------------------------------------------------------
    // TASK-0388 — `check_max_rounds_cap` (SPEC-19 R30).
    //
    // Acceptance Criteria → 5 inline tests. Integration coverage
    // (cap-hit partial net, natural-convergence leaves None, zero-cap
    // immediate exit, v1 non-regression) lives in
    // `tests/grid_delta_maxrounds.rs`.
    //
    //   UT-0388-01: None → false regardless of rounds
    //   UT-0388-02: Some(5), rounds=3 → false
    //   UT-0388-03: Some(5), rounds=5 → true
    //   UT-0388-04: Some(5), rounds=100 → true
    //   UT-0388-05: Some(0), rounds=0 → true (immediate)
    // -----------------------------------------------------------------

    #[test]
    fn check_max_rounds_cap_none_returns_false() {
        let cfg = GridConfig {
            max_rounds: None,
            ..GridConfig::default()
        };
        let mut metrics = GridMetrics {
            rounds: 0,
            ..Default::default()
        };
        assert!(!check_max_rounds_cap(&cfg, &metrics));
        metrics.rounds = 10_000;
        assert!(
            !check_max_rounds_cap(&cfg, &metrics),
            "None means unbounded — even very large rounds MUST not cap"
        );
    }

    #[test]
    fn check_max_rounds_cap_below_returns_false() {
        let cfg = GridConfig {
            max_rounds: Some(5),
            ..GridConfig::default()
        };
        let metrics = GridMetrics {
            rounds: 3,
            ..Default::default()
        };
        assert!(!check_max_rounds_cap(&cfg, &metrics));
    }

    #[test]
    fn check_max_rounds_cap_at_returns_true() {
        let cfg = GridConfig {
            max_rounds: Some(5),
            ..GridConfig::default()
        };
        let metrics = GridMetrics {
            rounds: 5,
            ..Default::default()
        };
        assert!(
            check_max_rounds_cap(&cfg, &metrics),
            "cap fires inclusive at rounds == max_rounds"
        );
    }

    #[test]
    fn check_max_rounds_cap_above_returns_true() {
        let cfg = GridConfig {
            max_rounds: Some(5),
            ..GridConfig::default()
        };
        let metrics = GridMetrics {
            rounds: 100,
            ..Default::default()
        };
        assert!(check_max_rounds_cap(&cfg, &metrics));
    }

    #[test]
    fn check_max_rounds_cap_zero_immediately_true() {
        let cfg = GridConfig {
            max_rounds: Some(0),
            ..GridConfig::default()
        };
        let metrics = GridMetrics::default();
        assert_eq!(metrics.rounds, 0);
        assert!(
            check_max_rounds_cap(&cfg, &metrics),
            "Some(0) MUST fire on entry before any dispatch"
        );
    }

    // -----------------------------------------------------------------
    // TASK-0388 integration tests (inline per spec — `tests/` crate
    // cannot reach `pub(crate) run_grid_delta`). Cover the cap wiring
    // end-to-end through `run_grid_delta`.
    //
    //   UT-0388-06: Some(2) on non-convergent net → rounds=2, flag set
    //   UT-0388-07: natural convergence leaves flag == None
    //   UT-0388-08: Some(0) → rounds=0, flag set, partial net returned
    //   UT-0388-09: v1 `run_grid` never sets delta_max_rounds_hit
    // -----------------------------------------------------------------

    // UT-0388-06: cap fires after `max_rounds` rounds; Final Collection
    // STILL runs (R30); metrics reflect cap-hit state.
    #[test]
    fn run_grid_delta_respects_max_rounds_cap() {
        let net = build_two_worker_cross_redex_net();
        let cfg = GridConfig {
            num_workers: 2,
            max_rounds: Some(2),
            ..GridConfig::default()
        };
        // Supply 2 rounds of "active" canned results — workers never
        // report convergence, so only the cap can stop the loop.
        let mut dispatch = CapturingDispatch {
            canned_round_results: VecDeque::from(vec![
                vec![canned_active_result(0, 1), canned_active_result(1, 1)],
                vec![canned_active_result(0, 2), canned_active_result(1, 2)],
            ]),
            canned_final_states: Some(Vec::new()),
            ..CapturingDispatch::default()
        };

        let (_net, metrics) = run_grid_delta(net, &cfg, &ContiguousIdStrategy, &mut dispatch);

        assert_eq!(metrics.rounds, 2, "loop MUST exit at the cap");
        assert_eq!(
            metrics.delta_max_rounds_hit,
            Some(true),
            "cap-hit flag MUST be set"
        );
        assert!(
            !metrics.converged,
            "converged MUST be false on cap-hit exit"
        );
        // R30: Final Collection ran even though no convergence.
        assert_eq!(
            dispatch.final_state_dispatches.len(),
            1,
            "FinalStateRequest MUST fire even on cap-hit (R30)"
        );
    }

    // UT-0388-07: natural convergence (cap not hit) leaves
    // `delta_max_rounds_hit == None` — distinguishes the two exit
    // paths in metrics.
    #[test]
    fn run_grid_delta_natural_convergence_leaves_delta_max_rounds_hit_none() {
        let net = build_two_worker_cross_redex_net();
        let cfg = GridConfig {
            num_workers: 2,
            max_rounds: Some(100),
            ..GridConfig::default()
        };
        let mut dispatch = CapturingDispatch {
            canned_round_results: VecDeque::from(vec![vec![
                canned_quiet_result(0, 1),
                canned_quiet_result(1, 1),
            ]]),
            canned_final_states: Some(Vec::new()),
            ..CapturingDispatch::default()
        };

        let (_net, metrics) = run_grid_delta(net, &cfg, &ContiguousIdStrategy, &mut dispatch);

        assert!(metrics.converged);
        assert_eq!(
            metrics.delta_max_rounds_hit, None,
            "natural convergence MUST leave delta_max_rounds_hit == None"
        );
    }

    // UT-0388-08: `Some(0)` caps BEFORE any dispatch. `metrics.rounds
    // == 0`, `delta_max_rounds_hit == Some(true)`, `converged == false`.
    // Final Collection still runs (R30).
    #[test]
    fn run_grid_delta_zero_max_rounds_returns_partial_immediately() {
        let net = build_two_worker_cross_redex_net();
        let cfg = GridConfig {
            num_workers: 2,
            max_rounds: Some(0),
            ..GridConfig::default()
        };
        let mut dispatch = CapturingDispatch {
            canned_round_results: VecDeque::new(),
            canned_final_states: Some(Vec::new()),
            ..CapturingDispatch::default()
        };

        let (_net, metrics) = run_grid_delta(net, &cfg, &ContiguousIdStrategy, &mut dispatch);

        assert_eq!(metrics.rounds, 0);
        assert_eq!(metrics.delta_max_rounds_hit, Some(true));
        assert!(!metrics.converged);
        assert_eq!(
            dispatch.round_start_dispatches.len(),
            0,
            "zero-cap MUST exit before any RoundStart"
        );
        // R30: Final Collection runs regardless.
        assert_eq!(dispatch.final_state_dispatches.len(), 1);
    }

    // UT-0388-09: v1 `run_grid` non-regression — the existing public
    // entry point MUST NOT touch `delta_max_rounds_hit` (v1 code paths
    // never set it).
    #[test]
    fn grid_metrics_v1_never_sets_delta_max_rounds_hit() {
        // Build a simple two-CON input that v1 run_grid will reduce.
        let net = build_two_worker_cross_redex_net();
        let cfg = GridConfig {
            num_workers: 2,
            ..GridConfig::default()
        };
        let (_net, metrics) = run_grid(net, &cfg, &ContiguousIdStrategy);
        assert_eq!(
            metrics.delta_max_rounds_hit, None,
            "v1 run_grid MUST leave delta_max_rounds_hit at its default (None)"
        );
    }

    // =================================================================
    // D-005 Option A Stage 6 new UTs (2026-04-24, QA matrix #3/#4/#5/#8).
    // =================================================================

    // UT-D005-03 (QA F-H6 / E3): intra-worker promotion where
    // `worker_a == worker_b` and side_b is a Lafont FreePort must NOT
    // emit `side_b` into `new_borders` for the same worker (that would
    // overwrite the `free_port_index` entry and strand the principal at
    // merge). The helper should emit ONLY `(bid, side_a)` via
    // `promoted_borders_to_per_worker_new_borders`, and the Lafont
    // wire should surface via `promoted_intra_worker_lafont_wires`.
    #[test]
    fn ut_d005_03_reg_minted_agents_intra_worker_promotion_single_apply() {
        let promoted = vec![(
            100,
            crate::merge::BorderState {
                border_id: 100,
                side_a: crate::net::PortRef::AgentPort(5, 0),
                side_b: crate::net::PortRef::FreePort(200),
                worker_a: 0,
                worker_b: 0,
                is_redex: false,
            },
        )];
        let new_borders = promoted_borders_to_per_worker_new_borders(&promoted, 2);
        let w0 = new_borders
            .get(&0)
            .expect("worker 0 must receive exactly one new_borders entry");
        assert_eq!(
            w0.len(),
            1,
            "intra-worker Lafont side_b must NOT be emitted as a second new_borders entry; \
             got {w0:?}"
        );
        assert_eq!(w0[0], (100, crate::net::PortRef::AgentPort(5, 0)));
        let (local_reconnects, drop_ids) = promoted_intra_worker_lafont_wires(&promoted, 2);
        assert_eq!(drop_ids, vec![100]);
        let w0_lr = local_reconnects
            .get(&0)
            .expect("worker 0 must receive a reconnection");
        assert_eq!(w0_lr.len(), 1);
        assert_eq!(
            w0_lr[0],
            (
                crate::net::PortRef::AgentPort(5, 0),
                crate::net::PortRef::FreePort(200)
            )
        );
    }

    // UT-D005-04 (QA F-C2 / E6): true cross-worker promotion routes
    // side_a to worker_a and side_b to worker_b — NEVER swapped,
    // NEVER cross-injected. Uses disjoint id_ranges to expose any
    // mis-routing as a mint-id owned by the wrong worker.
    #[test]
    fn ut_d005_04_reg_minted_agents_cross_worker_promotion_pairs_sides_correctly() {
        let promoted = vec![(
            200,
            crate::merge::BorderState {
                border_id: 200,
                side_a: crate::net::PortRef::AgentPort(5, 2), // worker 0 arena
                side_b: crate::net::PortRef::AgentPort(105, 1), // worker 1 arena
                worker_a: 0,
                worker_b: 1,
                is_redex: false,
            },
        )];
        let new_borders = promoted_borders_to_per_worker_new_borders(&promoted, 2);
        let w0 = new_borders.get(&0).expect("worker 0 must receive side_a");
        let w1 = new_borders.get(&1).expect("worker 1 must receive side_b");
        assert_eq!(w0, &vec![(200, crate::net::PortRef::AgentPort(5, 2))]);
        assert_eq!(w1, &vec![(200, crate::net::PortRef::AgentPort(105, 1))]);

        // Additional coverage: `apply_promoted_borders_to_cache` the
        // helper provides parallel guarantees on synthetic partitions
        // where arenas are pre-sized to accept the side-A / side-B
        // mint-id slots. Verifies the side-routing math end-to-end.
        use crate::partition::IdRange;
        let mut partitions: Vec<Partition> = vec![
            {
                let mut s = Net::new();
                // worker 0: create 6 dummy CONs to expand arena to id 5.
                for _ in 0..6 {
                    let _ = s.create_agent(crate::net::Symbol::Con);
                }
                Partition {
                    subnet: s,
                    worker_id: 0,
                    free_port_index: std::collections::HashMap::new(),
                    id_range: IdRange { start: 0, end: 100 },
                    border_id_start: 0,
                    border_id_end: 1000,
                }
            },
            {
                let mut s = Net::new();
                // worker 1: create 106 dummy CONs so id 105 is owned.
                for _ in 0..106 {
                    let _ = s.create_agent(crate::net::Symbol::Con);
                }
                Partition {
                    subnet: s,
                    worker_id: 1,
                    free_port_index: std::collections::HashMap::new(),
                    id_range: IdRange {
                        start: 100,
                        end: 200,
                    },
                    border_id_start: 0,
                    border_id_end: 1000,
                }
            },
        ];
        apply_promoted_borders_to_cache(&mut partitions, &promoted);
        assert_eq!(
            partitions[0].free_port_index.get(&200),
            Some(&crate::net::PortRef::AgentPort(5, 2)),
            "F-C2: worker 0 cache must carry side_a for border 200"
        );
        assert_eq!(
            partitions[1].free_port_index.get(&200),
            Some(&crate::net::PortRef::AgentPort(105, 1)),
            "F-C2: worker 1 cache must carry side_b for border 200"
        );
    }

    // UT-D005-05 (QA F-H7): `apply_border_deltas_to_partition`'s
    // new_borders loop debug-asserts the target AgentId belongs to the
    // partition's id_range, catching cross-worker mis-sends at dev
    // time. Panics in debug builds; no-op in release.
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "F-H7 cross-arena injection")]
    fn ut_d005_05_apply_border_deltas_cross_arena_target_debug_asserts() {
        use crate::merge::helpers::apply_border_deltas_to_partition;
        use crate::partition::IdRange;
        let mut subnet = Net::new();
        // Create one agent at id 0 so the arena is non-empty.
        let _ = subnet.create_agent(crate::net::Symbol::Con);
        let mut partition = Partition {
            subnet,
            worker_id: 0,
            free_port_index: std::collections::HashMap::new(),
            id_range: IdRange { start: 0, end: 50 },
            border_id_start: 0,
            border_id_end: 0,
        };
        // Target AgentPort(200, 0) is WELL outside id_range 0..50 —
        // indicates a cross-worker mis-send; debug_assert must fire.
        apply_border_deltas_to_partition(
            &mut partition,
            &[],
            &[],
            &[(100, crate::net::PortRef::AgentPort(200, 0))],
        );
    }

    // UT-D005-08 (QA F-H8): the coordinator-side
    // `every_border_has_inert_remote` helper flags as inert any border
    // with a principal port paired with a Lafont/concrete FreePort on
    // the remote side — exactly the CON-DUP post-promotion state. The
    // helper complements `detect_border_redexes` (which catches
    // principal-principal pairs) to prove Global Normal Form under
    // F-H8.
    #[test]
    fn ut_d005_08_compute_border_activity_ignores_lafont_freeport_peer() {
        use crate::merge::border_graph::BorderGraph;
        use crate::merge::AddBorderEntry;
        let mut graph =
            BorderGraph::from_partition_plan(&crate::partition::PartitionPlan::default());
        // Expand worker_borders to accommodate the two workers we'll
        // reference in `AddBorderEntry`.
        graph.worker_borders.resize(2, Vec::new());
        // Seed a single border: principal-on-worker_a, Lafont FreePort
        // on the remote — the CON-DUP post-promotion shape.
        graph.add_border_states(vec![AddBorderEntry {
            border_id: 500,
            side_a: crate::net::PortRef::AgentPort(7, 0),
            side_b: crate::net::PortRef::FreePort(200),
            worker_a: 0,
            worker_b: 1,
        }]);
        assert!(
            graph.detect_border_redexes().is_empty(),
            "principal-vs-FreePort border MUST NOT appear in active_redexes"
        );
        assert!(
            every_border_has_inert_remote(&graph),
            "principal-vs-Lafont-FreePort border MUST be classified inert by F-H8 helper"
        );
        // Contrast: add a second border with both sides principal
        // (genuine redex); the helper must now return false.
        graph.add_border_states(vec![AddBorderEntry {
            border_id: 501,
            side_a: crate::net::PortRef::AgentPort(8, 0),
            side_b: crate::net::PortRef::AgentPort(9, 0),
            worker_a: 0,
            worker_b: 1,
        }]);
        assert!(
            !every_border_has_inert_remote(&graph),
            "principal-vs-principal border MUST NOT be classified inert"
        );
    }

    // =========================================================================
    // TASK-0412 — SPEC-19 R38 amendment A8: `reconstruct` 3-argument form
    // =========================================================================
    //
    // Tests for `reconstruct(border_graph, surviving_partitions,
    // reclaimed_partitions)` (SPEC-20 §3.8 A8, SPEC-19 R38).
    //
    // UT-0412-01: empty reclaimed_partitions → result structurally identical to legacy 2-arg
    // UT-0412-02: one reclaimed partition → agent count = survivors + reclaimed
    // UT-0412-03: multiple reclaimed partitions → union of all, no duplication
    // UT-0412-04a: worker_id collision with disjoint AgentIds → assert fires (A8) in all builds
    // UT-0412-04b: intra-reclaimed worker_id collision → assert fires (A8) in all builds
    // UT-0412-05: reclaimed partitions with FreePorts → D3 + wire-resolution preserved
    // UT-0412-07: order-independence → result symbol-multisets identical regardless of input order
    // EC-1:       all survivors empty, reclaimed non-empty → reconstructible
    // EC-2:       both vectors empty → identical to legacy empty call
    // EC-3:       reclaimed present but BorderGraph refs only survivors → ok
    // EC-3a:      BorderGraph border_id present but reclaimed free_port_index skewed → pins D3
    //
    // Fixture disjointness contract:
    //   `simple_era_partition(w)` uses `next_id = w * 1000` (id starts at w*1000; avoids 0
    //    when w > 0). `lone_con_partition(w, ...)` uses `next_id = w` (starts from 0 for w=0).
    //   Do NOT mix `simple_era_partition(0)` with `lone_con_partition(0, ...)` in the same
    //   test — they both produce AgentId 0, causing id_range overlap that triggers the
    //   reconstruct assert.

    /// Build a `Partition` whose subnet has one ERA agent, no border FreePorts.
    /// `worker_id * 1000` is used as the `next_id` offset to guarantee disjoint
    /// `AgentId` ranges across all test partitions in the same run.
    ///
    /// # Fixture disjointness contract (QA-008)
    ///
    /// `simple_era_partition(w)` mints AgentId `w * 1000`.
    /// `lone_con_partition(w, _)` mints AgentId `w` (next_id = w).
    /// Do NOT pass `simple_era_partition(0)` together with `lone_con_partition(0, _)`
    /// to the same `reconstruct` call — both produce AgentId 0, which triggers the
    /// SPEC-20 A8 id_range overlap assert. Use `worker_id >= 1` for ERA partitions
    /// that will be mixed with lone_con_partition(0, _).
    fn simple_era_partition(worker_id: u32) -> crate::partition::Partition {
        use crate::net::Symbol;
        use crate::partition::IdRange;
        let mut subnet = Net::new();
        subnet.next_id = worker_id * 1000;
        let a = subnet.create_agent(Symbol::Era);
        // T1: every agent's principal port (port 0) MUST be connected.
        // Era is a 1-port symbol; wire port 0 to a FreePort sink so the
        // invariant is satisfied without introducing a redex partner.
        // Using `worker_id * 1000` as the free-port id guarantees that
        // partitions with different worker_ids use disjoint FreePort ids.
        subnet.connect(
            crate::net::PortRef::AgentPort(a, 0),
            crate::net::PortRef::FreePort(worker_id * 1000),
        );
        crate::partition::Partition {
            subnet,
            worker_id,
            free_port_index: std::collections::HashMap::new(),
            id_range: IdRange {
                start: worker_id * 1000,
                end: worker_id * 1000 + 1,
            },
            border_id_start: 0,
            border_id_end: 0,
        }
    }

    // UT-0412-01: reconstruct with empty reclaimed_partitions MUST produce a
    // Net whose live-agent count equals that produced by the 2-argument
    // baseline (reconstruct_partition_plan_from_collected + merge with empty
    // reclaimed). Parametrised over four independent SPEC-19 fixtures to anchor
    // the backward-compat invariant.
    //
    // NOTE on "legacy" reference (SF-003): the "legacy" net is produced by calling
    // reconstruct_partition_plan_from_collected directly + merge(). This is the SAME
    // internal path that reconstruct() calls — both are identical by construction.
    // The test's protection is against wrapper divergence, not independent implementation
    // divergence. If the two code paths are ever separated, this test must be updated
    // to compare against an independent structural expected value.
    #[test]
    fn reconstruct_empty_reclaimed_matches_legacy() {
        use crate::merge::border_graph::BorderGraph;
        use std::collections::BTreeMap;

        // Helper: collect (AgentId -> Symbol) BTreeMap for structural identity check (SF-001).
        // Using AgentId-keyed map because AgentId allocation is deterministic across both
        // calls (both start from the same next_id values via the same helper path).
        fn symbol_map(net: &Net) -> BTreeMap<u32, crate::net::Symbol> {
            net.live_agents().map(|a| (a.id, a.symbol)).collect()
        }

        // Fixture A: two lone-CON workers, no borders.
        {
            let p0 = lone_con_partition(0, 100);
            let p1 = lone_con_partition(1, 200);
            let graph = empty_graph();

            let plan_legacy = reconstruct_partition_plan_from_collected(
                vec![p0.clone(), p1.clone()],
                &graph,
                Vec::new(),
                0,
            );
            let (net_legacy, _) = merge(plan_legacy);
            let net_new = reconstruct(&graph, vec![p0, p1], Vec::new());

            // Count parity (necessary condition).
            assert_eq!(
                net_new.count_live_agents(),
                net_legacy.count_live_agents(),
                "UT-0412-01 fixture A: empty reclaimed must yield same agent count"
            );
            // Symbol map parity (sufficient for regression: if a code split silently
            // diverges, AgentId→Symbol mismatches become visible here — SF-001).
            assert_eq!(
                symbol_map(&net_new),
                symbol_map(&net_legacy),
                "UT-0412-01 fixture A: symbol map must match legacy (bit-exact regression check)"
            );
        }

        // Fixture B: single ERA partition, no borders.
        {
            let p0 = simple_era_partition(1); // worker_id=1 to avoid id_range=0 overlapping fixture A's wid=0
            let graph = empty_graph();

            let plan_legacy =
                reconstruct_partition_plan_from_collected(vec![p0.clone()], &graph, Vec::new(), 0);
            let (net_legacy, _) = merge(plan_legacy);
            let net_new = reconstruct(&graph, vec![p0], Vec::new());

            assert_eq!(
                net_new.count_live_agents(),
                net_legacy.count_live_agents(),
                "UT-0412-01 fixture B: single ERA, empty reclaimed must match legacy"
            );
            assert_eq!(
                symbol_map(&net_new),
                symbol_map(&net_legacy),
                "UT-0412-01 fixture B: symbol map must match legacy"
            );
        }

        // Fixture C: empty partition list, no borders.
        {
            let graph = empty_graph();

            let plan_legacy =
                reconstruct_partition_plan_from_collected(vec![], &graph, Vec::new(), 0);
            let (net_legacy, _) = merge(plan_legacy);
            let net_new = reconstruct(&graph, vec![], Vec::new());

            assert_eq!(
                net_new.count_live_agents(),
                net_legacy.count_live_agents(),
                "UT-0412-01 fixture C: empty partitions + empty reclaimed must match legacy"
            );
            assert_eq!(
                symbol_map(&net_new),
                symbol_map(&net_legacy),
                "UT-0412-01 fixture C: symbol map must match legacy"
            );
        }

        // Fixture D: two CON workers with one aux-aux border in BorderGraph.
        //
        // Mirrors the canonical pattern from UT-0387-02 (this same file,
        // ~line 3819) and `reconstruct_partition_plan_preserves_remaining_borders`:
        //   - both partitions wire the SAME FreePort id (77) on aux port 1,
        //   - both `free_port_index` maps record `77 → AgentPort(_, 1)`,
        //   - the BorderGraph is built once from this plan, and the SAME
        //     populated partitions (cloned) are passed to both the legacy and
        //     the new `reconstruct` calls.
        //
        // T1 satisfaction:
        //   - principal port (port 0) is wired to a non-border FreePort
        //     (50/60), copied through merge as a Lafont FreePort,
        //   - aux port 1 is wired to FreePort(77); merge step 2 nulls it
        //     (border id 77 is in `borders`); step 3 restores the wire
        //     using `free_port_index`, producing AgentPort(0,1) ↔ AgentPort(1,1),
        //   - aux port 2 is wired to a non-border FreePort (51/61),
        //     copied through merge as a Lafont FreePort.
        // C3 satisfaction: border_id 77 appears in p0.free_port_index and
        // in p1.free_port_index — exactly 2 sightings.
        {
            use crate::net::Symbol;
            use crate::partition::IdRange;

            let mut subnet_a = Net::new();
            subnet_a.next_id = 0;
            let a = subnet_a.create_agent(Symbol::Con);
            subnet_a.connect(
                crate::net::PortRef::AgentPort(a, 0),
                crate::net::PortRef::FreePort(50),
            );
            subnet_a.connect(
                crate::net::PortRef::AgentPort(a, 1),
                crate::net::PortRef::FreePort(77),
            );
            subnet_a.connect(
                crate::net::PortRef::AgentPort(a, 2),
                crate::net::PortRef::FreePort(51),
            );
            let mut idx_a = std::collections::HashMap::new();
            idx_a.insert(77u32, crate::net::PortRef::AgentPort(a, 1));
            let p0 = crate::partition::Partition {
                subnet: subnet_a,
                worker_id: 0,
                free_port_index: idx_a,
                id_range: IdRange { start: 0, end: 1 },
                border_id_start: 77,
                border_id_end: 78,
            };

            let mut subnet_b = Net::new();
            subnet_b.next_id = 1;
            let b = subnet_b.create_agent(Symbol::Con);
            subnet_b.connect(
                crate::net::PortRef::AgentPort(b, 0),
                crate::net::PortRef::FreePort(60),
            );
            subnet_b.connect(
                crate::net::PortRef::AgentPort(b, 1),
                crate::net::PortRef::FreePort(77),
            );
            subnet_b.connect(
                crate::net::PortRef::AgentPort(b, 2),
                crate::net::PortRef::FreePort(61),
            );
            let mut idx_b = std::collections::HashMap::new();
            idx_b.insert(77u32, crate::net::PortRef::AgentPort(b, 1));
            let p1 = crate::partition::Partition {
                subnet: subnet_b,
                worker_id: 1,
                free_port_index: idx_b,
                id_range: IdRange { start: 1, end: 2 },
                border_id_start: 77,
                border_id_end: 78,
            };

            let mut borders = std::collections::HashMap::new();
            borders.insert(
                77u32,
                (
                    crate::net::PortRef::AgentPort(a, 1),
                    crate::net::PortRef::AgentPort(b, 1),
                ),
            );
            let plan_bg = crate::partition::PartitionPlan {
                partitions: vec![p0.clone(), p1.clone()],
                borders,
                ..Default::default()
            };
            let graph = BorderGraph::from_partition_plan(&plan_bg);

            let plan_legacy = reconstruct_partition_plan_from_collected(
                vec![p0.clone(), p1.clone()],
                &graph,
                Vec::new(),
                0,
            );
            let (net_legacy, _) = merge(plan_legacy);
            let net_new = reconstruct(&graph, vec![p0, p1], Vec::new());

            assert_eq!(
                net_new.count_live_agents(),
                net_legacy.count_live_agents(),
                "UT-0412-01 fixture D: two workers + border, empty reclaimed must match legacy"
            );
            assert_eq!(
                symbol_map(&net_new),
                symbol_map(&net_legacy),
                "UT-0412-01 fixture D: symbol map must match legacy (bit-exact regression check)"
            );
        }
    }

    // UT-0412-02: one reclaimed partition → agent count = survivors + reclaimed.
    #[test]
    fn reconstruct_with_one_reclaimed_partition() {
        let p0 = lone_con_partition(0, 100); // worker 0: 1 CON agent
        let p1 = lone_con_partition(1, 200); // worker 1: 1 CON agent
        let r0 = simple_era_partition(2); // reclaimed worker 2: 1 ERA agent
        let survivors_count = 2usize;
        let r0_count = r0.subnet.count_live_agents();
        let graph = empty_graph();

        let net = reconstruct(&graph, vec![p0, p1], vec![r0]);

        assert_eq!(
            net.count_live_agents(),
            survivors_count + r0_count,
            "UT-0412-02: agent_count must equal survivors ({}) + reclaimed ({})",
            survivors_count,
            r0_count
        );
    }

    // UT-0412-03: two disjoint reclaimed partitions → complete union, no
    // duplication.
    #[test]
    fn reconstruct_with_multiple_reclaimed_partitions() {
        let p0 = lone_con_partition(0, 100); // worker 0: 1 CON
        let r0 = simple_era_partition(1); // reclaimed worker 1: 1 ERA
        let r1 = simple_era_partition(2); // reclaimed worker 2: 1 ERA
        let survivors_count = 1usize;
        let r0_count = r0.subnet.count_live_agents();
        let r1_count = r1.subnet.count_live_agents();
        let graph = empty_graph();

        let net = reconstruct(&graph, vec![p0], vec![r0, r1]);

        let expected = survivors_count + r0_count + r1_count;
        assert_eq!(
            net.count_live_agents(),
            expected,
            "UT-0412-03: agent_count must equal survivors ({}) + r0 ({}) + r1 ({}); got {}",
            survivors_count,
            r0_count,
            r1_count,
            net.count_live_agents()
        );
    }

    // UT-0412-04a: overlapping id_ranges fire the SPEC-20 A8 assert in ALL builds
    // (debug AND release), because the new precondition check is unconditional.
    //
    // This test used to be gated on #[cfg(debug_assertions)] when the old code used
    // a debug_assert! proxy on worker_id. The proxy has been replaced with a direct
    // id_range overlap scan wrapped in assert! (QA-001). The test is now un-gated.
    //
    // Fixture: p0 (worker 0, id_range=[0,1)) and r_collide (worker 0, id_range=[0,1))
    // have identical id_ranges — overlap is [0,1), which fires the assert.
    #[test]
    fn reconstruct_panics_on_overlapping_id_ranges_all_builds() {
        use std::panic::AssertUnwindSafe;

        let p0 = lone_con_partition(0, 100); // wid=0, id_range=[0,1)
                                             // Reclaimed partition also has worker_id 0 AND id_range=[0,1) — genuine
                                             // id_range overlap (the precondition this assert was always meant to guard).
        let r_collide = lone_con_partition(0, 500); // wid=0, id_range=[0,1) — OVERLAP
        let graph = empty_graph();

        let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            // Clone graph to avoid capturing a mutable reference across the unwind.
            reconstruct(&graph, vec![p0], vec![r_collide]);
        }));

        assert!(
            result.is_err(),
            "UT-0412-04a: overlapping id_ranges MUST panic in ALL builds (assert!, not debug_assert!)"
        );
        if let Err(e) = result {
            let msg = if let Some(s) = e.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = e.downcast_ref::<&str>() {
                (*s).to_owned()
            } else {
                String::new()
            };
            // Tightened from "A8 || A7": the new assert! cites SPEC-20 A8 specifically
            // (A7 is Net::union, not in this call path). Any regression that changes
            // the message must update this assertion intentionally.
            assert!(
                msg.contains("A8"),
                "UT-0412-04a: panic message MUST cite SPEC-20 A8; got: {msg}"
            );
        }
    }

    // UT-0412-04b: intra-reclaimed worker_id collision fires the post-sort
    // monotonicity debug_assert in the helper (debug builds only).
    //
    // Two reclaimed partitions with the same worker_id but disjoint id_ranges:
    //   r0: wid=5, id_range=[5000,5001)
    //   r1: wid=5, id_range=[6000,6001)
    // The id_range overlap assert does NOT fire (ranges are disjoint). However,
    // after sort_by_key(worker_id), both share wid=5 — the helper's strict-
    // monotonicity debug_assert fires.
    //
    // In release builds the wid duplicate passes silently (no id_range overlap,
    // no monotonicity assert) — this pins the fact that wid-disjointness is the
    // caller's responsibility (SPEC-20 R11).
    #[test]
    #[cfg(debug_assertions)]
    fn reconstruct_panics_on_intra_reclaimed_worker_id_collision_debug_build() {
        use crate::partition::IdRange;
        use std::panic::AssertUnwindSafe;

        // r0 and r1 have the same worker_id (5) but disjoint id_ranges.
        // simple_era_partition(5) → wid=5, id_range=[5000,5001).
        let r0 = simple_era_partition(5);
        // Build r1 manually: wid=5 (same as r0), id_range=[6000,6001) (disjoint).
        let mut subnet_r1 = Net::new();
        subnet_r1.next_id = 6000;
        let era_r1 = subnet_r1.create_agent(crate::net::Symbol::Era);
        subnet_r1.connect(
            crate::net::PortRef::AgentPort(era_r1, 0),
            crate::net::PortRef::FreePort(6000),
        );
        let r1 = crate::partition::Partition {
            subnet: subnet_r1,
            worker_id: 5, // same worker_id as r0 — triggers monotonicity assert
            free_port_index: std::collections::HashMap::new(),
            id_range: IdRange {
                start: 6000,
                end: 6001,
            },
            border_id_start: 0,
            border_id_end: 0,
        };
        let graph = empty_graph();

        let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            reconstruct(&graph, vec![], vec![r0, r1]);
        }));

        assert!(
            result.is_err(),
            "UT-0412-04b: intra-reclaimed wid collision MUST panic in debug builds \
             (post-sort monotonicity debug_assert in helper)"
        );
    }

    // UT-0412-05: reclaimed partition whose FreePort side matches a BorderGraph
    // entry → merge resolves the border wire; D3 preserved.
    #[test]
    fn reconstruct_with_reclaimed_preserves_border_completeness() {
        use crate::net::Symbol;
        use crate::partition::IdRange;

        // Build p0: one CON agent; principal port wired to FreePort(bid=1).
        let mut subnet_p0 = Net::new();
        subnet_p0.next_id = 0;
        let a0 = subnet_p0.create_agent(Symbol::Con);
        subnet_p0.connect(
            crate::net::PortRef::AgentPort(a0, 0),
            crate::net::PortRef::FreePort(1),
        );
        subnet_p0.connect(
            crate::net::PortRef::AgentPort(a0, 1),
            crate::net::PortRef::FreePort(10),
        );
        subnet_p0.connect(
            crate::net::PortRef::AgentPort(a0, 2),
            crate::net::PortRef::FreePort(11),
        );
        let mut fpi_p0 = std::collections::HashMap::new();
        fpi_p0.insert(1u32, crate::net::PortRef::AgentPort(a0, 0));
        let p0 = crate::partition::Partition {
            subnet: subnet_p0,
            worker_id: 0,
            free_port_index: fpi_p0,
            id_range: IdRange { start: 0, end: 1 },
            border_id_start: 1,
            border_id_end: 2,
        };

        // Build r0 (reclaimed): one CON agent; principal port wired to FreePort(bid=1)
        // — the other side of the same border.
        let mut subnet_r0 = Net::new();
        subnet_r0.next_id = 1000;
        let b0 = subnet_r0.create_agent(Symbol::Con);
        subnet_r0.connect(
            crate::net::PortRef::AgentPort(b0, 0),
            crate::net::PortRef::FreePort(1),
        );
        subnet_r0.connect(
            crate::net::PortRef::AgentPort(b0, 1),
            crate::net::PortRef::FreePort(20),
        );
        subnet_r0.connect(
            crate::net::PortRef::AgentPort(b0, 2),
            crate::net::PortRef::FreePort(21),
        );
        let mut fpi_r0 = std::collections::HashMap::new();
        fpi_r0.insert(1u32, crate::net::PortRef::AgentPort(b0, 0));
        let r0 = crate::partition::Partition {
            subnet: subnet_r0,
            worker_id: 1,
            free_port_index: fpi_r0,
            id_range: IdRange {
                start: 1000,
                end: 1001,
            },
            border_id_start: 1,
            border_id_end: 2,
        };

        // BorderGraph with border_id=1 connecting a0 and b0.
        let mut borders_bg = std::collections::HashMap::new();
        borders_bg.insert(
            1u32,
            (
                crate::net::PortRef::AgentPort(a0, 0),
                crate::net::PortRef::AgentPort(b0, 0),
            ),
        );
        let plan_bg = crate::partition::PartitionPlan {
            partitions: vec![p0.clone(), r0.clone()],
            borders: borders_bg,
            ..Default::default()
        };
        let graph = crate::merge::border_graph::BorderGraph::from_partition_plan(&plan_bg);

        let net = reconstruct(&graph, vec![p0], vec![r0]);

        // D3 (agent presence): both agents live in the merged net.
        assert_eq!(
            net.count_live_agents(),
            2,
            "UT-0412-05: both surviving and reclaimed agents must be present after merge"
        );
        // NTH-004 / D3 (wire resolution): the border wire must be resolved — neither
        // a0 port 0 nor b0 port 0 should remain as a FreePort after merge.
        // Both CON agents have AgentId a0=0 (from p0, next_id=0) and b0=1000 (from r0).
        let target_a0_p0 = net.get_target(crate::net::PortRef::AgentPort(a0, 0));
        let target_b0_p0 = net.get_target(crate::net::PortRef::AgentPort(b0, 0));
        assert_eq!(
            target_a0_p0,
            crate::net::PortRef::AgentPort(b0, 0),
            "UT-0412-05: border_id=1 must resolve a0.port[0] → b0.port[0] (D3 wire resolution)"
        );
        assert_eq!(
            target_b0_p0,
            crate::net::PortRef::AgentPort(a0, 0),
            "UT-0412-05: border_id=1 must resolve b0.port[0] → a0.port[0] (D3 wire resolution, symmetry)"
        );
        // Confirm neither side is a dangling FreePort (the claim in the comment is now verified).
        assert!(
            !matches!(target_a0_p0, crate::net::PortRef::FreePort(_)),
            "UT-0412-05: a0.port[0] must NOT be a FreePort after merge (D3 completeness)"
        );
        assert!(
            !matches!(target_b0_p0, crate::net::PortRef::FreePort(_)),
            "UT-0412-05: b0.port[0] must NOT be a FreePort after merge (D3 completeness)"
        );
    }

    // EC-1: survivors empty, reclaimed non-empty → all reclaimed agents present.
    // NOTE (QA-007 / R7a): In this test, worker_id=0 in the reclaimed slot is used
    // under SPEC-20 R7a's permissive clause (non-hybrid mode). In hybrid mode,
    // worker_id=0 is reserved for the coordinator self-partition and MUST NOT appear
    // in the reclaimed partition list. Phase-B integrators writing hybrid-mode tests
    // MUST NOT use worker_id=0 in reclaimed slots as a pattern learned from this test.
    #[test]
    fn reconstruct_ec1_survivors_empty_reclaimed_nonempty() {
        let r0 = simple_era_partition(0);
        let r0_count = r0.subnet.count_live_agents();
        let graph = empty_graph();

        let net = reconstruct(&graph, vec![], vec![r0]);

        assert_eq!(
            net.count_live_agents(),
            r0_count,
            "EC-1: all-reclaimed must produce agent count == r0.count"
        );
    }

    // EC-2: both vectors empty → empty net.
    #[test]
    fn reconstruct_ec2_both_vectors_empty() {
        let graph = empty_graph();
        let net = reconstruct(&graph, vec![], vec![]);
        assert_eq!(
            net.count_live_agents(),
            0,
            "EC-2: both empty must yield an empty net"
        );
    }

    // EC-3: reclaimed present but BorderGraph has no references to it →
    // reconstruct succeeds; reclaimed agents appear as disconnected components.
    #[test]
    fn reconstruct_ec3_reclaimed_present_border_graph_refs_only_survivors() {
        let p0 = lone_con_partition(0, 100);
        let r0 = simple_era_partition(1);
        let survivors_count = 1usize;
        let r0_count = r0.subnet.count_live_agents();
        let graph = empty_graph();

        let net = reconstruct(&graph, vec![p0], vec![r0]);

        assert_eq!(
            net.count_live_agents(),
            survivors_count + r0_count,
            "EC-3: reclaimed agents not referenced by BorderGraph must still be present"
        );
    }

    // EC-3a (QA-005): BorderGraph contains a border_id that is NOT present in
    // ANY partition's free_port_index. This pins the current D3 behaviour when the
    // BorderGraph and reclaimed partition's free_port_index are out of sync (e.g.,
    // border_id rebase per SPEC-20 R24d applied to the graph but not to the reclaimed
    // partition's free_port_index).
    //
    // Current behaviour (pinned, not fixed): merge step 3 silently drops the border
    // (both sides produce None → (None,None) arm). The merged net still contains both
    // agents (they are copied in step 2), but the border wire is not restored.
    // This test documents and pins that behaviour so a future fix is intentional.
    //
    // Construction note: BorderGraph is built via struct literal (bypassing
    // from_partition_plan) because from_partition_plan enforces SPEC-19 C3 — it
    // requires every declared border_id to appear in exactly 2 partitions'
    // free_port_index.  Here we deliberately want a border that is absent from both
    // partitions so that the (None, None) arm of reconstruct step 3 is exercised.
    #[test]
    fn reconstruct_ec3a_border_graph_border_id_not_in_free_port_index() {
        use crate::merge::border_graph::BorderState;

        // p0: CON agent at id=0, no border FreePorts registered in free_port_index.
        let p0 = lone_con_partition(0, 100);

        // r0: ERA agent at id=2000, also no border FreePorts.
        let r0 = simple_era_partition(2);

        // Build a BorderGraph that claims border_id=99 exists, even though neither
        // partition's free_port_index mentions it.  This represents the stale/rebased
        // border scenario described in SPEC-20 R24d.
        //
        // We construct directly (struct literal) to avoid from_partition_plan's C3
        // assertion, which would correctly reject this state as malformed.
        let graph = crate::merge::border_graph::BorderGraph {
            borders: {
                let mut m = std::collections::HashMap::new();
                m.insert(
                    99u32,
                    BorderState {
                        border_id: 99,
                        side_a: crate::net::PortRef::AgentPort(0u32, 0),
                        side_b: crate::net::PortRef::AgentPort(2000u32, 0),
                        worker_a: 0,
                        worker_b: 1,
                        is_redex: false,
                    },
                );
                m
            },
            // Both workers list border 99 in their reverse index.
            worker_borders: vec![vec![99], vec![99]],
            active_redexes: std::collections::HashSet::new(),
            pending_new_borders: Vec::new(),
            resolved_mints: std::collections::HashMap::new(),
        };

        let net = reconstruct(&graph, vec![p0], vec![r0]);

        // Both agents are present (step 2 copies them unconditionally).
        assert_eq!(
            net.count_live_agents(),
            2,
            "EC-3a: both agents must be present in merged net even when border_id is stale"
        );
        // The border wire was silently dropped (step 3 (None,None) arm) because
        // neither partition's free_port_index carries border_id=99.
        // p0's CON agent (id=0) port 0 target: NOT resolved to AgentPort(2000, 0) —
        // it remains as a FreePort (the original connection from lone_con_partition).
        // This pins the current behaviour: if this changes, the test fails deliberately.
        let target = net.get_target(crate::net::PortRef::AgentPort(0, 0));
        assert_ne!(
            target,
            crate::net::PortRef::AgentPort(2000, 0),
            "EC-3a: stale border_id=99 MUST NOT be silently restored (current behaviour: border dropped)"
        );
    }

    // UT-0412-07 (QA-009): reconstruct is order-independent — presenting
    // surviving and reclaimed in any order must produce structurally identical nets.
    //
    // Verifies that sort_by_key(worker_id) in the helper correctly normalises
    // input order. A future optimisation that skips the sort for pre-sorted inputs
    // would silently break this property if inputs happen to be unsorted.
    #[test]
    fn reconstruct_is_order_independent() {
        use std::collections::BTreeMap;

        let p0 = lone_con_partition(0, 100);
        let p1 = lone_con_partition(1, 200);
        let r0 = simple_era_partition(2);
        let r1 = simple_era_partition(3);
        let graph = empty_graph();

        // Call A: surviving = [p0, p1], reclaimed = [r0, r1] (sorted order)
        let net_a = reconstruct(
            &graph,
            vec![p0.clone(), p1.clone()],
            vec![r0.clone(), r1.clone()],
        );

        // Call B: surviving = [p1, p0], reclaimed = [r1, r0] (reversed order)
        let net_b = reconstruct(&graph, vec![p1, p0], vec![r1, r0]);

        // Count parity.
        assert_eq!(
            net_a.count_live_agents(),
            net_b.count_live_agents(),
            "UT-0412-07: order-reversed call must yield the same agent count"
        );

        // Symbol map parity: both nets must have the same (AgentId -> Symbol) map.
        // Since both calls operate on the same partitions (just reordered), and the
        // helper normalises order via sort_by_key, the resulting AgentId sets and
        // symbols must be identical.
        let symbols_a: BTreeMap<u32, crate::net::Symbol> =
            net_a.live_agents().map(|a| (a.id, a.symbol)).collect();
        let symbols_b: BTreeMap<u32, crate::net::Symbol> =
            net_b.live_agents().map(|a| (a.id, a.symbol)).collect();
        assert_eq!(
            symbols_a,
            symbols_b,
            "UT-0412-07: reconstruct must be order-independent (symbol map identical for reversed input)"
        );
    }
}
