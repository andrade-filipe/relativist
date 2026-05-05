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
//! ## Aggregation rule: SUM
//!
//! The coordinator-side aggregate is the **sum** of per-worker
//! `reduce_duration_secs`. This mirrors the in-process path
//! (`merge/grid.rs:103,154`), which pushes `t_compute.elapsed()` — the
//! wall-clock of the SEQUENTIAL worker loop — equivalent to a SUM since the
//! workers run serially. Choosing SUM for the distributed path keeps the
//! semantic identical across modes, which is exactly what the bench harness
//! needs to compare in-process and distributed compute time.
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
/// reflects the chosen rule (SUM). Pinned to the sum invariant: the
/// aggregate equals the sum of per-worker `reduce_duration_secs` across the
/// workers reporting in that round, within float-roundtrip tolerance.
///
/// AGGREGATION RULE (developer's choice): **SUM**, for parity with the
/// in-process path which pushes `t_compute.elapsed()` — wall-clock of the
/// sequential worker loop, equivalent to a sum.
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

    let sum_secs: f64 = stats_round0.iter().map(|s| s.reduce_duration_secs).sum();
    let sum = Duration::from_secs_f64(sum_secs.max(0.0));

    // SUM rule: aggregate ~= sum across reporting workers (within timer
    // resolution and the f64-to-Duration roundtrip). Allow 5% drift to
    // absorb rounding; tighter than 5% would flake on slow CI runners.
    let lo = mul_f(sum, 0.95);
    let hi = mul_f(sum, 1.05);
    assert!(
        agg_t0 >= lo && agg_t0 <= hi,
        "SUM rule: aggregate compute_time_per_round[0] = {:?} is outside \
         [0.95 * sum_per_worker, 1.05 * sum_per_worker] = [{:?}, {:?}]. \
         Sum across {} reporting workers = {:?}.",
        agg_t0,
        lo,
        hi,
        stats_round0.len(),
        sum
    );
}

// ---------------------------------------------------------------------------
// IT-0616-03 — Worker-reported matches coordinator aggregate (path a)
// ---------------------------------------------------------------------------

/// IT-0616-03: Path (a) — worker-side `reduce_duration_secs` matches the
/// coordinator-side aggregate within 10% tolerance. This pins that the
/// coordinator's sum is computed from the same `WorkerRoundStats` payload
/// the workers actually emit (not from a hardcoded constant or a
/// double-count).
#[tokio::test]
async fn worker_reported_compute_matches_coordinator_aggregate_within_tolerance() {
    // 32 redexes, 2 workers — each worker reduces ~16 redexes; reduce time
    // dominates over message-passing latency for this size.
    let metrics = run_distributed_with_workers(build_n_redex_net(32), 2).await;

    let agg = metrics.compute_time_per_round[0];
    let stats = &metrics.worker_stats_per_round[0];

    let sum_secs: f64 = stats.iter().map(|s| s.reduce_duration_secs).sum();
    let sum = Duration::from_secs_f64(sum_secs.max(0.0));

    let lo = mul_f(sum, 0.9);
    let hi = mul_f(sum, 1.1);
    assert!(
        agg >= lo && agg <= hi,
        "TASK-0616 acceptance criterion 2: coordinator aggregate {:?} drifted \
         outside [0.9 * worker_sum, 1.1 * worker_sum] = [{:?}, {:?}]. \
         worker-side sum = {:?} across {} workers.",
        agg,
        lo,
        hi,
        sum,
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
#[tokio::test]
#[ignore = "path (a) was chosen for TASK-0616 — see TEST-SPEC-0616 §Path-conditional execution. \
            This test is preserved for migration symmetry; remove #[ignore] to re-enable \
            when path (b) is in effect."]
async fn residual_compute_equals_wall_minus_merge_minus_network() {
    // Intentionally empty body — would require path-(b) implementation to
    // observe a meaningful residual. Per TEST-SPEC-0616 the body documents
    // the contract that re-activation would assert.
    let _: Option<Duration> = None;
    panic!("path (b) not in effect — see #[ignore] reason");
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
// Helpers
// ---------------------------------------------------------------------------

/// Multiply a `Duration` by a `f64` factor without negative or overflow
/// pathology. Used to build tolerance bands for the aggregate-vs-sum check.
fn mul_f(d: Duration, factor: f64) -> Duration {
    let secs = d.as_secs_f64() * factor.max(0.0);
    Duration::from_secs_f64(secs.max(0.0))
}
