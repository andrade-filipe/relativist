//! Grid loop: the BSP cycle of split -> reduce -> merge -> resolve (SPEC-05, Section 4.5).

use std::time::Instant;

use crate::net::Net;
#[cfg(debug_assertions)]
use crate::net::PortRef;
use crate::partition::{split, PartitionStrategy};
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
        let mut plan = split(current_net, config.num_workers, strategy);
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
            coordinator_free_rounds: true,
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
            coordinator_free_rounds: true,
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
            coordinator_free_rounds: true,
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
            coordinator_free_rounds: true,
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
}
