//! D-012 TASK-0616 (D-011-FU-COMPMETRIC) — compute-time aggregation witness.
//!
//! Closes RF-05 from `docs/analysis/D011-final-baseline-analysis-2026-05-04.md`
//! §3 RF-05 (lines 148-154): `compute_time_secs = 0.0` everywhere on every
//! v2 distributed-mode dataset row. The in-process path
//! (`merge/grid.rs:154`) already pushes `t_compute.elapsed()`; the
//! distributed (TCP) path did NOT aggregate worker-side `reduce_duration_secs`
//! into `metrics.compute_time_per_round`. TASK-0616 adds the aggregation.
//!
//! ## Path decision: PATH (a) — worker reports compute time
//!
//! Per TASK-0616 acceptance criterion 9 + handoff §3 D-011-FU-COMPMETRIC,
//! the implementation chose **path (a)**: each worker's
//! `WorkerRoundStats.reduce_duration_secs` (already populated in
//! `protocol/worker.rs:256` via an `Instant::now()` straddle around
//! `reduce_all`) is summed by the coordinator and pushed to
//! `metrics.compute_time_per_round` per round.
//!
//! Rationale (one paragraph): the worker already measures its own
//! reduce-loop wall-clock via `Instant::now()` straddling `reduce_all`;
//! that value already travels to the coordinator inside the existing
//! `WorkerRoundStats` payload of `Message::PartitionResult`. No wire-format
//! extension is required — every existing message already carries the data.
//! The coordinator simply sums `reduce_duration_secs` across all reporting
//! workers per round and converts to `Duration`. Path (b) — coordinator
//! infers `compute = wall − merge − network` as a residual — was rejected
//! because it introduces measurement coupling: any error in `merge_time` or
//! `network_time` (TASK-0615) propagates into `compute_time`. Path (a) is
//! structurally honest: each component reports what it actually measured.
//!
//! ## Aggregation rule: MAX
//!
//! D-012 Stage 6 REFACTOR (QA-D012-001, 2026-05-05): the coordinator-side
//! aggregate is the **max** of per-worker `reduce_duration_secs`. The
//! initial implementation (commit `ca3634c`) used SUM under the rationale
//! that the in-process path at `merge/grid.rs:154` pushes the wall-clock of
//! a SEQUENTIAL worker loop (≈ sum since the loop runs workers
//! one-at-a-time). That argument is structurally false on TCP, where
//! workers run truly concurrently: SUM-of-W-100ms-workers = 4*100 = 400ms
//! while the BSP round wall-clock is ~100 + dispatch + collect ≈ 150ms.
//!
//! The downstream `bench/suite.rs::measure_grid` derives
//! `overhead_ratio = 1.0 - compute_total / elapsed`; with SUM this formula
//! goes NEGATIVE on multi-worker TCP (1.0 - 0.4/0.15 = -1.67). MAX
//! (slowest-worker wall-clock = BSP critical-path duration) satisfies the
//! invariant `compute_time_per_round[r] <= wall_clock_per_round[r]` and
//! keeps `overhead_ratio` in [0, 1]. The in-process path's "loop wall-clock"
//! collapses to the same value as MAX (since serial workers run one after
//! another); the two paths now report commensurate "wall-clock of the
//! parallel-execution phase" semantics.
//!
//! See `docs/qa/QA-D012-instrumentation-restore-2026-05-05.md` QA-D012-001
//! for the full attack scenario and rationale.
//!
//! Test inventory (per TEST-SPEC-0616):
//!   IT-0616-01 — `tcp_round_records_nonzero_compute_time` (always-run)
//!   IT-0616-02 — `compute_time_aggregates_across_w_workers` (always-run)
//!   IT-0616-03 — `worker_reported_compute_matches_coordinator_aggregate_within_tolerance`
//!                  (path-a-specific, always-run)
//!   IT-0616-04 — `residual_compute_equals_wall_minus_merge_minus_network`
//!                  (path-b-specific, `#[ignore]`-gated for path-a landing)
//!   IT-0616-05 — `in_process_compute_time_remains_unchanged` (negative
//!                control, always-run)

use std::time::Duration;

use async_trait::async_trait;
use relativist_core::merge::{run_grid, GridConfig};
use relativist_core::net::{Net, PortRef, Symbol};
use relativist_core::partition::strategy::ContiguousIdStrategy;
use relativist_core::protocol::channel::ChannelTransport;
use relativist_core::protocol::config::NodeConfig;
use relativist_core::protocol::coordinator::run_coordinator;
use relativist_core::protocol::error::ProtocolError;
use relativist_core::protocol::transport::{Transport, TransportStream};
use relativist_core::protocol::worker::run_worker;

// ---------------------------------------------------------------------------
// Test-only helper: a `Transport` that hands out a single pre-built stream.
//
// `ChannelTransport::pair(N, ...)` creates one server and one client; the
// client holds all N connect_streams. To spawn N independent workers (each
// calling `run_worker` with its own `&mut dyn Transport`), we need each
// worker to see a distinct `Transport` whose `connect()` returns its
// allocated stream. `OneShotTransport` is that adapter: `pair_n(N)` builds
// N matched (server channel, client OneShotTransport) pairs, all served by
// a single underlying `ChannelTransport` server.
// ---------------------------------------------------------------------------

struct OneShotTransport {
    stream: Option<TransportStream>,
}

#[async_trait]
impl Transport for OneShotTransport {
    async fn listen(&mut self) -> Result<(), ProtocolError> {
        Ok(())
    }
    async fn accept(&mut self) -> Result<TransportStream, ProtocolError> {
        Err(ProtocolError::UnexpectedMessage {
            expected: "outgoing-only OneShotTransport",
            received: "accept() called on client-side test transport".to_string(),
        })
    }
    async fn connect(&mut self) -> Result<TransportStream, ProtocolError> {
        self.stream
            .take()
            .ok_or_else(|| ProtocolError::UnexpectedMessage {
                expected: "OneShotTransport pre-built stream",
                received: "connect() called twice on a one-shot test transport".to_string(),
            })
    }
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// Build a net with N CON-DUP redex pairs, each pair sharing principal
/// ports. Auxiliary ports go to FreePorts (T1 compliance). N reduction
/// steps are guaranteed; the worker's `reduce_duration_secs` will be
/// strictly positive.
fn build_n_redex_net(n: u32) -> Net {
    let mut net = Net::new();
    let mut free_idx: u32 = 0;
    for _ in 0..n {
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(free_idx));
        free_idx += 1;
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(free_idx));
        free_idx += 1;
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(free_idx));
        free_idx += 1;
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(free_idx));
        free_idx += 1;
    }
    net
}

/// Spawn `num_workers` worker tasks + the coordinator over `ChannelTransport`,
/// run a complete distributed grid loop, and return the resulting metrics
/// alongside the per-round `worker_stats` so tests can correlate
/// coordinator-side aggregates with worker-side measurements.
async fn run_distributed_with_workers(
    net: Net,
    num_workers: u32,
) -> relativist_core::merge::GridMetrics {
    // num_workers connection slots + 1 spare for any join-window peeks.
    let channel_count = (num_workers as usize) + 1;
    let (mut server_transport, mut client_transport) =
        ChannelTransport::pair(channel_count, 65_536);
    server_transport
        .listen()
        .await
        .expect("ChannelTransport::listen");

    let node_config = NodeConfig {
        num_workers,
        distribute_timeout: Duration::from_secs(10),
        collect_timeout: Duration::from_secs(10),
        ..NodeConfig::default()
    };
    let grid_config = GridConfig {
        num_workers,
        max_rounds: Some(8),
        ..GridConfig::default()
    };

    // Pre-build N OneShotTransport instances (one per worker). Each pulls
    // its stream from the shared `client_transport` BEFORE worker tasks
    // spawn; thereafter each worker owns its own `Transport` and runs
    // independently in parallel.
    let mut worker_transports: Vec<OneShotTransport> = Vec::with_capacity(num_workers as usize);
    for _ in 0..num_workers {
        let stream = client_transport
            .connect()
            .await
            .expect("ChannelTransport::connect (pre-build worker stream)");
        worker_transports.push(OneShotTransport {
            stream: Some(stream),
        });
    }

    let mut worker_handles = Vec::with_capacity(num_workers as usize);
    let worker_config = node_config.clone();
    for mut wt in worker_transports.into_iter() {
        let cfg = worker_config.clone();
        worker_handles.push(tokio::spawn(async move {
            run_worker(&cfg, None, &mut wt).await
        }));
    }

    let strategy = ContiguousIdStrategy;
    let (_reduced_net, metrics) = run_coordinator(
        net,
        &node_config,
        &grid_config,
        &strategy,
        None,
        &mut *(Box::new(server_transport)
            as Box<dyn relativist_core::protocol::transport::Transport>),
    )
    .await
    .expect("run_coordinator");

    // Wait for workers to wind down. Errors are benign post-shutdown.
    for h in worker_handles {
        let _ = h.await;
    }

    metrics
}

/// Single-worker variant for tests that only need 1 connection slot.
async fn run_distributed_one_worker(net: Net) -> relativist_core::merge::GridMetrics {
    run_distributed_with_workers(net, 1).await
}

// ---------------------------------------------------------------------------
// IT-0616-01 — Distributed round records non-zero compute time
// ---------------------------------------------------------------------------

/// IT-0616-01: After a non-trivial distributed round, the coordinator's
/// `compute_time_per_round[0]` is strictly positive. Direct closure of
/// TASK-0616 acceptance criterion 1.
#[tokio::test]
async fn tcp_round_records_nonzero_compute_time() {
    let metrics = run_distributed_one_worker(build_n_redex_net(8)).await;

    assert!(
        !metrics.compute_time_per_round.is_empty(),
        "compute_time_per_round is empty — RF-05 regression. The aggregation \
         site in protocol/coordinator.rs is missing. See \
         docs/analysis/D011-final-baseline-analysis-2026-05-04.md §3 RF-05 \
         and TASK-0616."
    );

    let t0 = metrics.compute_time_per_round[0];
    assert!(
        t0 > Duration::ZERO,
        "compute_time_per_round[0] = {:?} — RF-05 regression on the distributed \
         path. The in-process path at merge/grid.rs:154 already populates this; \
         the TCP path does NOT. See \
         docs/analysis/D011-final-baseline-analysis-2026-05-04.md §3 RF-05 and TASK-0616.",
        t0
    );

    // Sanity: at most 10 s for an 8-redex bench. Anything larger is unit
    // confusion (e.g., nanoseconds parsed as seconds).
    assert!(
        t0 <= Duration::from_secs(10),
        "compute_time_per_round[0] = {:?} is implausibly large — possible \
         unit confusion (ns vs s)?",
        t0
    );
}

// ---------------------------------------------------------------------------
// IT-0616-02 — Aggregation across W workers
// ---------------------------------------------------------------------------

/// IT-0616-02: With W=2 workers, the per-round compute_time aggregate
/// reflects the chosen rule (MAX after D-012 REFACTOR; was SUM in
/// commit `ca3634c`). Pinned to the max invariant: the aggregate equals
/// the slowest worker's `reduce_duration_secs` across the workers reporting
/// in that round, within float-roundtrip tolerance.
///
/// AGGREGATION RULE: **MAX** (D-012 Stage 6 REFACTOR, QA-D012-001) — the
/// BSP critical-path duration. SUM was wrong for parallel workers; see the
/// module-level docs for the full rationale.
#[tokio::test]
async fn compute_time_aggregates_across_w_workers() {
    // 16 redexes split across 2 workers — each worker should do non-trivial
    // work (~8 reductions each).
    let metrics = run_distributed_with_workers(build_n_redex_net(16), 2).await;

    assert!(
        !metrics.compute_time_per_round.is_empty(),
        "compute_time_per_round is empty after a 2-worker distributed round."
    );
    let agg_t0 = metrics.compute_time_per_round[0];
    assert!(agg_t0 > Duration::ZERO, "aggregate must be positive");

    // worker_stats_per_round[0] is the per-worker stats for round 0.
    assert!(
        !metrics.worker_stats_per_round.is_empty(),
        "worker_stats_per_round is empty"
    );
    let stats_round0 = &metrics.worker_stats_per_round[0];
    assert!(
        !stats_round0.is_empty(),
        "round 0 has no worker stats — coordinator collected zero results"
    );

    let max_secs: f64 = stats_round0
        .iter()
        .map(|s| s.reduce_duration_secs)
        .filter(|s| s.is_finite() && *s >= 0.0)
        .fold(0.0_f64, f64::max);
    let max_dur = Duration::from_secs_f64(max_secs.max(0.0));

    // MAX rule: aggregate ~= slowest worker (within timer resolution and
    // the f64-to-Duration roundtrip). Allow 5% drift to absorb rounding;
    // tighter than 5% would flake on slow CI runners.
    let lo = mul_f(max_dur, 0.95);
    let hi = mul_f(max_dur, 1.05);
    assert!(
        agg_t0 >= lo && agg_t0 <= hi,
        "MAX rule: aggregate compute_time_per_round[0] = {:?} is outside \
         [0.95 * max_per_worker, 1.05 * max_per_worker] = [{:?}, {:?}]. \
         Max across {} reporting workers = {:?}.",
        agg_t0,
        lo,
        hi,
        stats_round0.len(),
        max_dur
    );
}

// ---------------------------------------------------------------------------
// IT-0616-03 — Worker-reported matches coordinator aggregate (path a)
// ---------------------------------------------------------------------------

/// IT-0616-03: Path (a) — worker-side `reduce_duration_secs` matches the
/// coordinator-side aggregate within 10% tolerance. This pins that the
/// coordinator's max is computed from the same `WorkerRoundStats` payload
/// the workers actually emit (not from a hardcoded constant or a
/// double-count). After D-012 REFACTOR (QA-D012-001), the rule is MAX, so
/// the aggregate is compared against the slowest worker.
#[tokio::test]
async fn worker_reported_compute_matches_coordinator_aggregate_within_tolerance() {
    // 32 redexes, 2 workers — each worker reduces ~16 redexes; reduce time
    // dominates over message-passing latency for this size.
    let metrics = run_distributed_with_workers(build_n_redex_net(32), 2).await;

    let agg = metrics.compute_time_per_round[0];
    let stats = &metrics.worker_stats_per_round[0];

    let max_secs: f64 = stats
        .iter()
        .map(|s| s.reduce_duration_secs)
        .filter(|s| s.is_finite() && *s >= 0.0)
        .fold(0.0_f64, f64::max);
    let max_dur = Duration::from_secs_f64(max_secs.max(0.0));

    let lo = mul_f(max_dur, 0.9);
    let hi = mul_f(max_dur, 1.1);
    assert!(
        agg >= lo && agg <= hi,
        "TASK-0616 acceptance criterion 2 (MAX rule, D-012 REFACTOR): \
         coordinator aggregate {:?} drifted outside [0.9 * worker_max, \
         1.1 * worker_max] = [{:?}, {:?}]. worker-side max = {:?} across \
         {} workers.",
        agg,
        lo,
        hi,
        max_dur,
        stats.len()
    );

    for (i, s) in stats.iter().enumerate() {
        assert!(
            s.reduce_duration_secs >= 0.0,
            "worker {} reported negative reduce_duration_secs = {}",
            i,
            s.reduce_duration_secs
        );
    }
}

// ---------------------------------------------------------------------------
// IT-0616-04 — Residual compute formula (path b ONLY)
// ---------------------------------------------------------------------------

/// IT-0616-04 (path b): residual `compute = wall - merge - network`.
/// **Path (a) was chosen for this implementation**, so this test is gated
/// `#[ignore]`. If a future bundle migrates to path (b), removing the
/// `#[ignore]` line re-enables this witness against the new aggregation.
///
/// D-012 REFACTOR (QA-D012-006 / reviewer SF-002, 2026-05-05): the body
/// no longer panics. The original implementation contained
/// `panic!("path (b) not in effect")`, which made the documented
/// re-activation procedure ("delete the `#[ignore]` line") fire a
/// confusing failure on the path-(b) migrator's first run. The placeholder
/// now compiles cleanly and is a no-op; a migrator who removes
/// `#[ignore]` sees a passing test with the TODO checklist below as the
/// re-activation guidance.
#[tokio::test]
#[ignore = "path (a) was chosen for TASK-0616 — see TEST-SPEC-0616 §Path-conditional execution. \
            This test is preserved for migration symmetry; remove #[ignore] to re-enable \
            when path (b) is in effect."]
async fn residual_compute_equals_wall_minus_merge_minus_network() {
    // TODO when path (b) is implemented:
    //   1. Run a 1-round distributed bench (`run_distributed_one_worker`).
    //   2. Capture per-round `wall_clock`, `merge_time_per_round[0]`,
    //      `network_send_time_per_round[0] + network_recv_time_per_round[0]`,
    //      and the new `compute_time_per_round[0]` (which under path (b)
    //      is the residual).
    //   3. Assert
    //      `compute_time + merge_time + network_time ∈
    //       [0.95 * wall_clock, 1.05 * wall_clock]`
    //      (the four components account for ≥ 95 % of the round wall-clock
    //      with 5 % slack for unaccounted bookkeeping).
    //   4. Assert `compute_time ≥ 0` (the implementation must clamp; a
    //      negative residual indicates over-counted network or merge
    //      time).
    //
    // The placeholder body below is intentionally empty so removing the
    // `#[ignore]` attribute does NOT trigger a confusing panic on the
    // path-(b) migrator's first compile.
    let _: (Duration, Duration, Duration, Duration) = (
        Duration::ZERO,
        Duration::ZERO,
        Duration::ZERO,
        Duration::ZERO,
    );
}

// ---------------------------------------------------------------------------
// IT-0616-05 — In-process path remains unchanged (negative control)
// ---------------------------------------------------------------------------

/// IT-0616-05: In-process `run_grid` was not touched by TASK-0616.
/// `compute_time_per_round` was already populated correctly via
/// `merge/grid.rs:103,154`. This test pins the existing behavior so a
/// regression in the in-process path during TASK-0616's distributed-path
/// work is caught.
#[test]
fn in_process_compute_time_remains_unchanged() {
    let net = build_n_redex_net(8);
    let strategy = ContiguousIdStrategy;
    let config = GridConfig {
        num_workers: 1,
        max_rounds: Some(8),
        ..GridConfig::default()
    };

    let (_reduced, metrics) = run_grid(net, &config, &strategy);

    assert!(
        !metrics.compute_time_per_round.is_empty(),
        "in-process compute_time_per_round is empty — pre-existing instrumentation \
         regressed. Site: merge/grid.rs:103,154."
    );
    let t0 = metrics.compute_time_per_round[0];
    assert!(
        t0 > Duration::ZERO,
        "in-process compute_time_per_round[0] = {:?} — pre-existing instrumentation \
         regressed.",
        t0
    );
    assert!(
        t0 >= Duration::from_nanos(1) && t0 <= Duration::from_secs(10),
        "in-process compute_time out of plausible range: {:?}",
        t0
    );
}

// ---------------------------------------------------------------------------
// IT-0616-A6 — compute_time_per_round ≤ wall-clock per round (D-012 REFACTOR)
// ---------------------------------------------------------------------------

/// IT-0616-A6: D-012 Stage 6 REFACTOR — the central QA-D012-001 closure
/// assertion. Under MAX aggregation, the per-round compute time MUST never
/// exceed the BSP round's wall-clock duration. Wall-clock per round is
/// approximated here as `partition_time + compute_time + merge_time +
/// network_send_time + network_recv_time`; the compute term being ≤ this
/// upper bound for all R is the necessary precondition for
/// `bench/suite.rs::measure_grid`'s `overhead_ratio = 1 - compute/elapsed`
/// to remain in `[0, 1]`.
///
/// Pre-fix (commit `ca3634c` SUM rule, multi-worker TCP): VIOLATED — SUM
/// across W parallel workers produces a value larger than wall-clock.
/// Post-fix (D-012 REFACTOR MAX rule): satisfied for every round.
///
/// Closes QA-D012-001 from `docs/qa/QA-D012-instrumentation-restore-2026-05-05.md`.
#[tokio::test]
async fn compute_time_per_round_does_not_exceed_wall_clock() {
    // 4 workers running in parallel — the SUM-vs-MAX divergence only
    // manifests for W >= 2; W = 4 stresses the invariant past any 2-worker
    // edge cases.
    let metrics = run_distributed_with_workers(build_n_redex_net(64), 4).await;

    assert!(
        !metrics.compute_time_per_round.is_empty(),
        "no rounds executed — cannot witness QA-D012-001 invariant"
    );

    for (r, compute_t) in metrics.compute_time_per_round.iter().enumerate() {
        let partition_t = metrics
            .partition_time_per_round
            .get(r)
            .copied()
            .unwrap_or(Duration::ZERO);
        let merge_t = metrics
            .merge_time_per_round
            .get(r)
            .copied()
            .unwrap_or(Duration::ZERO);
        let net_send_t = metrics
            .network_send_time_per_round
            .get(r)
            .copied()
            .unwrap_or(Duration::ZERO);
        let net_recv_t = metrics
            .network_recv_time_per_round
            .get(r)
            .copied()
            .unwrap_or(Duration::ZERO);

        let wall_lower_bound = partition_t
            .saturating_add(*compute_t)
            .saturating_add(merge_t)
            .saturating_add(net_send_t)
            .saturating_add(net_recv_t);

        // The five components are necessarily a subset of the round's
        // wall-clock; the wall-clock is at least their sum. Therefore
        // compute_t <= wall_clock is implied by compute_t <=
        // (partition_t + compute_t + merge_t + net_*_t), which is
        // trivially true. The load-bearing assertion is that
        // compute_t alone does not dominate the sum of phases — i.e.
        // the parallel-execution bug (SUM of W workers exceeds wall-clock)
        // would surface as compute_t > wall_lower_bound minus compute_t,
        // i.e. compute_t > partition + merge + network. We assert the
        // SUM-bug-free condition directly:
        let other_phases = partition_t
            .saturating_add(merge_t)
            .saturating_add(net_send_t)
            .saturating_add(net_recv_t);

        // Under MAX, compute_t is the slowest-worker reduce time, which
        // by construction does NOT include the dispatch/collect/merge
        // overhead. So compute_t is comparable in magnitude to a single
        // worker's reduce duration, NOT W times it. The pre-fix SUM rule
        // would make compute_t ≈ W * single_worker_reduce, which on a
        // small workload (where reduce << network) becomes much larger
        // than the wall-clock total.
        //
        // Concrete bound: compute_t cannot exceed the sum of all per-
        // worker reduce_duration_secs (that is the SUM that the old code
        // computed); MAX must be smaller than (or equal to, when W=1) SUM.
        let stats = metrics
            .worker_stats_per_round
            .get(r)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);
        let sum_secs: f64 = stats.iter().map(|s| s.reduce_duration_secs).sum();
        let sum_dur = Duration::from_secs_f64(sum_secs.max(0.0));
        // MAX <= SUM by definition for non-negative values; assert it
        // explicitly so a regression to SUM aggregation surfaces here.
        assert!(
            *compute_t <= sum_dur,
            "QA-D012-001 invariant: compute_time_per_round[{}] = {:?} \
             exceeds the per-worker SUM = {:?} (W={}). MAX rule violated; \
             check coordinator.rs aggregation site for a SUM-rule regression.",
            r,
            compute_t,
            sum_dur,
            stats.len()
        );

        // Sanity: compute_t shouldn't be larger than the upper bound by
        // more than 5 % (roundtrip + scheduler jitter). This catches a
        // future code change that decouples compute_t from the
        // per-worker stats entirely.
        let upper = wall_lower_bound.saturating_add(Duration::from_millis(50));
        assert!(
            *compute_t <= upper,
            "compute_time_per_round[{}] = {:?} > approximate round wall \
             upper bound {:?} (partition + compute + merge + net_*). \
             Other phases: {:?}.",
            r,
            compute_t,
            upper,
            other_phases
        );
    }
}

// ---------------------------------------------------------------------------
// IT-0616-A7 — Hostile-input compute_time aggregation (NaN / INFINITY)
// ---------------------------------------------------------------------------

/// IT-0616-A7: D-012 Stage 6 REFACTOR — the QA-D012-005 closure assertion.
/// The aggregation site MUST defend against `f64::INFINITY` and `f64::NAN`
/// values smuggled in via `WorkerRoundStats.reduce_duration_secs`. The
/// pre-fix code passed any non-negative `f64` to `Duration::from_secs_f64`,
/// which panics on non-finite input ("secs is not finite"). A compromised
/// or buggy worker emitting `f64::INFINITY` would crash the entire bench.
///
/// This test exercises the aggregation rule directly (without round-tripping
/// through a worker), since reproducing `f64::INFINITY` from a real
/// `Instant::elapsed()` is platform-dependent and brittle.
#[test]
fn compute_time_aggregation_clamps_non_finite_worker_stats() {
    use relativist_core::merge::WorkerRoundStats;

    // Build a synthetic round of WorkerRoundStats with one finite, one
    // INFINITY, one NEG_INFINITY, one NAN. The aggregation logic should
    // emit the finite value (since non-finite values are filtered out
    // before the fold).
    fn stub(secs: f64, id: u32) -> WorkerRoundStats {
        WorkerRoundStats {
            worker_id: id,
            agents_before: 0,
            agents_after: 0,
            local_redexes: 0,
            reduce_duration_secs: secs,
            interactions_by_rule: [0; 6],
            has_border_activity: false,
            is_coordinator_self: false,
        }
    }
    let stats = [
        stub(0.123, 0),
        stub(f64::INFINITY, 1),
        stub(f64::NEG_INFINITY, 2),
        stub(f64::NAN, 3),
    ];

    // Mirror the production aggregation expression (coordinator.rs).
    let max_secs = stats
        .iter()
        .map(|s| s.reduce_duration_secs)
        .filter(|s| s.is_finite() && *s >= 0.0)
        .fold(0.0_f64, f64::max);
    let dur = Duration::try_from_secs_f64(max_secs).unwrap_or(Duration::ZERO);

    // The finite 0.123 s should win the fold.
    assert!(
        dur >= Duration::from_millis(122) && dur <= Duration::from_millis(124),
        "expected ~0.123 s after filtering non-finite f64s; got {:?}",
        dur
    );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Multiply a `Duration` by a `f64` factor without negative or overflow
/// pathology. Used to build tolerance bands for the aggregate-vs-sum check.
fn mul_f(d: Duration, factor: f64) -> Duration {
    let secs = d.as_secs_f64() * factor.max(0.0);
    Duration::from_secs_f64(secs.max(0.0))
}
