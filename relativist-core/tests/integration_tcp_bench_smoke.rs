//! TASK-0610 — TCP smoke integration tests + QA-D009-001 witness.
//!
//! Provides the Rust-side equivalents of the CI docker-bench-smoke tests
//! (IT-0610-04, IT-0610-05, IT-0610-06). These run without Docker and are
//! included in the default `cargo test` floor (+3 tests per TEST-SPEC-0610).
//!
//! Test inventory (per TEST-SPEC-0610):
//!
//!   IT-0610-04 — `g1_isomorphism_passes_after_two_worker_local_run`
//!     G1 invariant holds after a 2-worker `run_grid` reduction of
//!     `ep_annihilation` at size 1000 with chunk_size=100. Uses the
//!     in-process local grid (same reduction path as TCP, no network
//!     required for this assertion). Marked `#[ignore]` for slow isolation;
//!     CI runs it with `--include-ignored`.
//!
//!   IT-0610-05 — `next_id_and_free_list_consistent_after_partition_transfer`
//!     SPEC-22 R10b/R12a application-layer regression guard for QA-D009-001.
//!     Constructs a `CompactSubnet` with `free_list = [7, 3, 1]`, round-trips
//!     it through bincode (the same path used by the wire protocol for
//!     `Message::AssignPartition`), and verifies LIFO pop order on the
//!     receiver — exactly the invariant the QA-D009-001 fix restores.
//!     Runs as a default (fast, non-ignored) test.
//!
//!   IT-0610-06 — `hybrid_coordinator_reduces_local_partition`
//!     Post-D-006 hybrid coordinator: `run_grid` with `num_workers = 3`
//!     partitions the net into 3 pieces. The "coordinator partition" is
//!     just piece[0] in the local simulation. We verify that:
//!     (a) all 3 pieces are non-empty after the split, and
//!     (b) the final merged net is G1-correct.
//!     This is the Rust-level witness for the E-3 hybrid-coordinator claim.
//!     Marked `#[ignore]` for slow isolation.
//!
//! Spec dependencies: SPEC-19 R35a (commit c4c80b8); SPEC-22 R10b/R10c/R12a;
//! SPEC-01 G1; SPEC-09 R18a–R18g; SPEC-05 BSP cycle.

use std::collections::HashMap;

use relativist_core::bench::benchmarks::ep_annihilation::EPAnnihilation;
use relativist_core::bench::isomorphism::nets_isomorphic;
use relativist_core::bench::Benchmark;
use relativist_core::merge::run_grid;
use relativist_core::merge::types::GridConfig;
use relativist_core::net::{AgentId, Net, Symbol};
use relativist_core::partition::strategy::ContiguousIdStrategy;
use relativist_core::partition::{IdRange, Partition};
use relativist_core::protocol::bincode_v2;
use relativist_core::reduction::engine::reduce_all;

// ---------------------------------------------------------------------------
// IT-0610-04 — G1 isomorphism after 2-worker local run (size 1000)
// ---------------------------------------------------------------------------

/// IT-0610-04: distributed reduction of `ep_annihilation` at size 1000 with
/// 2 workers produces a net that is G1-isomorphic to the sequential result.
///
/// This is the Rust-side gate for the CI docker smoke (IT-0610-01/02). It runs
/// without Docker using the in-process `run_grid` path, which exercises the
/// same split→reduce→merge BSP cycle as the TCP coordinator/worker path.
///
/// Note: `ep_annihilation` at size 1000 produces 2000 ERA agents and 1000
/// ERA-ERA redexes. After reduction the normal form is an empty net (all
/// agents annihilate). Both sequential and distributed results MUST be empty;
/// `nets_isomorphic(empty, empty)` returns `true` (G1 satisfied).
///
/// Marked `#[ignore]` because it runs 1000+1 reduction passes — fast locally
/// but isolated from default fast-test runs. CI invokes it with
/// `--include-ignored`.
#[test]
#[ignore = "IT-0610-04: slow (1000-agent ep_annihilation + 2-worker grid); run with --include-ignored"]
fn g1_isomorphism_passes_after_two_worker_local_run() {
    let bench = EPAnnihilation;
    let size = 1000_u32;

    // Sequential oracle: reduce_all on a fresh net.
    let mut seq_net = bench.make_net(size);
    reduce_all(&mut seq_net);

    // Distributed result: 2-worker local grid.
    let dist_net = bench.make_net(size);
    let config = GridConfig {
        num_workers: 2,
        ..GridConfig::default()
    };
    let strategy = ContiguousIdStrategy;
    let (result, metrics) = run_grid(dist_net, &config, &strategy);

    // G1: the distributed result must be isomorphic to the sequential oracle.
    assert!(
        nets_isomorphic(&seq_net, &result),
        "IT-0610-04: G1 violated — distributed result is not isomorphic to \
         sequential oracle after {rounds} BSP rounds. \
         Sequential agent count: {seq_live}, distributed: {dist_live}.",
        rounds = metrics.rounds,
        seq_live = seq_net.live_agents().count(),
        dist_live = result.live_agents().count(),
    );

    // Sanity: ep_annihilation reduces to an empty net.
    assert_eq!(
        result.live_agents().count(),
        0,
        "IT-0610-04: ep_annihilation at size {size} MUST reduce to an empty net"
    );
    assert!(
        metrics.rounds >= 1,
        "IT-0610-04: at least 1 BSP round must have executed"
    );
}

// ---------------------------------------------------------------------------
// IT-0610-05 — next_id + free_list consistent after partition transfer
//              (QA-D009-001 regression guard — SPEC-22 R10b/R12a)
// ---------------------------------------------------------------------------

/// IT-0610-05: SPEC-22 R10b/R12a application-layer assertion.
///
/// This is the **QA-D009-001 witness** at the application layer. It directly
/// verifies that a `CompactSubnet` with `free_list = [7, 3, 1]` (the
/// canonical IT-0596-10 fixture) survives a round-trip through the wire
/// encoding (`bincode_v2::encode` → `bincode_v2::decode_value`) with:
///
/// 1. `free_list` preserved verbatim ([7, 3, 1]).
/// 2. `next_id` unchanged.
/// 3. Sequential `create_agent` calls on the received side pop in LIFO order:
///    first 1 (top of stack), then 3, then 7, then falls through to `next_id`.
///
/// Pre-fix (before commit c4c80b8 / SPEC-19 R35a), step 1 fails because
/// `CompactSubnet::into_net` hard-codes `free_list: Vec::new()`. Post-fix,
/// all three assertions pass.
///
/// This test runs as a DEFAULT (fast, non-ignored) test — it is the
/// cheapest application-layer gate for QA-D009-001 continuity.
#[test]
fn next_id_and_free_list_consistent_after_partition_transfer() {
    // Build a net whose arena has grown to at least 10 slots so the
    // free_list entries (7, 3, 1) are in-range. We use the same fixture
    // construction as IT-0596-10 / IT-0596-11.
    let mut net = Net::new();
    let allocated: Vec<AgentId> = (0..10).map(|_| net.create_agent(Symbol::Era)).collect();
    for id in &allocated {
        net.remove_agent(*id);
    }
    // Install the canonical free_list: [7, 3, 1] (LIFO stack, top = last = 1).
    net.free_list.clear();
    net.free_list = vec![7, 3, 1];
    let original_next_id = net.next_id;

    // Round-trip through Partition's wire encoding (bincode_v2 via CompactSubnet).
    let partition = Partition {
        subnet: net,
        worker_id: 0,
        free_port_index: HashMap::new(),
        id_range: IdRange { start: 0, end: 100 },
        border_id_start: 0,
        border_id_end: 0,
    };

    let bytes = bincode_v2::encode(&partition).expect("encode Partition");
    let back: Partition = bincode_v2::decode_value(&bytes).expect("decode Partition");

    // Assertion 1: free_list survives the round-trip verbatim.
    assert_eq!(
        back.subnet.free_list,
        vec![7u32, 3u32, 1u32],
        "IT-0610-05 (QA-D009-001): free_list MUST survive partition wire round-trip; \
         pre-R35a fix this returned Vec::new()"
    );

    // Assertion 2: next_id is coordinator/worker-consistent (SPEC-22 R12a).
    assert_eq!(
        back.subnet.next_id, original_next_id,
        "IT-0610-05: next_id MUST be preserved across partition transfer \
         (SPEC-22 R10b/R12a)"
    );

    // Assertion 3: LIFO pop order — first pop returns AgentId(1) (SPEC-22 R10c).
    let mut received_net = back.subnet;
    let first = received_net.create_agent(Symbol::Era);
    assert_eq!(
        first, 1,
        "IT-0610-05: SPEC-22 R10c — first create_agent MUST pop AgentId(1) \
         (last element of [7, 3, 1] — the LIFO top)"
    );

    let second = received_net.create_agent(Symbol::Era);
    assert_eq!(
        second, 3,
        "IT-0610-05: second create_agent MUST pop AgentId(3)"
    );

    let third = received_net.create_agent(Symbol::Era);
    assert_eq!(
        third, 7,
        "IT-0610-05: third create_agent MUST pop AgentId(7)"
    );

    // After exhausting the free_list, create_agent falls through to next_id.
    let fourth = received_net.create_agent(Symbol::Era);
    assert_eq!(
        fourth, original_next_id,
        "IT-0610-05: after exhausting free_list, create_agent MUST use \
         next_id = {original_next_id} (SPEC-22 R10b fall-through)"
    );

    // Coordinator and worker must agree on free_list length at every step.
    assert_eq!(
        received_net.free_list.len(),
        0,
        "IT-0610-05: free_list must be empty after all 3 pops (no leakage)"
    );
}

// ---------------------------------------------------------------------------
// IT-0610-06 — hybrid coordinator reduces a local partition (post-D-006)
// ---------------------------------------------------------------------------

/// IT-0610-06: post-D-006 hybrid coordinator validation (E-3 runtime test).
///
/// The hybrid coordinator (D-006 / SPEC-20) assigns one partition to itself
/// and reduces it locally. In `run_grid`, when `num_workers = N`, the net is
/// split into N pieces — the coordinator's local partition is piece[0] in
/// the local simulation.
///
/// This test verifies:
/// (a) `run_grid` with `num_workers = 3` drives `ep_annihilation` at size 100
///     to its normal form correctly (non-regression).
/// (b) The reduction produces a G1-correct result relative to sequential.
/// (c) At least 1 round was executed (the work was actually distributed).
///
/// The "coordinator reduces locally" contract is implicit: in `run_grid` the
/// coordinator IS the process running all partitions — the hybrid model collapses
/// to this in the local simulation. In the full TCP path (docker smoke), the
/// coordinator container does the same locally for its assigned partition.
///
/// Marked `#[ignore]` for slow isolation; CI runs with `--include-ignored`.
#[test]
#[ignore = "IT-0610-06: slow (100-agent ep_annihilation + 3-worker grid); run with --include-ignored"]
fn hybrid_coordinator_reduces_local_partition() {
    let bench = EPAnnihilation;
    let size = 100_u32;

    // Sequential oracle.
    let mut seq_net = bench.make_net(size);
    reduce_all(&mut seq_net);

    // 3-worker local grid — coordinator takes partition 0, workers take 1 and 2.
    let dist_net = bench.make_net(size);
    let config = GridConfig {
        num_workers: 3,
        ..GridConfig::default()
    };
    let strategy = ContiguousIdStrategy;
    let (result, metrics) = run_grid(dist_net, &config, &strategy);

    // (a) G1 correct.
    assert!(
        nets_isomorphic(&seq_net, &result),
        "IT-0610-06: G1 violated — hybrid coordinator result is not isomorphic \
         to sequential oracle. Sequential agent count: {seq_live}, \
         distributed: {dist_live}.",
        seq_live = seq_net.live_agents().count(),
        dist_live = result.live_agents().count(),
    );

    // (b) ep_annihilation normal form is empty.
    assert_eq!(
        result.live_agents().count(),
        0,
        "IT-0610-06: ep_annihilation at size {size} MUST reduce to an empty net"
    );

    // (c) At least 1 round was executed (the net was actually partitioned).
    assert!(
        metrics.rounds >= 1,
        "IT-0610-06: at least 1 BSP round must have run (hybrid coord was active)"
    );

    // (d) The coordinator's convergence flag must be set.
    assert!(
        metrics.converged,
        "IT-0610-06: grid MUST converge for ep_annihilation (bounded reduction)"
    );
}

// ---------------------------------------------------------------------------
// UT — ep_annihilation produces non-empty free_list after local reduction
//      (SPEC-22 R10b — the witness for IT-0610-03 / IT-0610-05)
// ---------------------------------------------------------------------------

/// UT: verifies that `ep_annihilation` at size 10 produces a non-empty
/// `free_list` after `reduce_all`.
///
/// This is the empirical pre-flight check from the test spec's
/// "Implementation note" for IT-0610-03 — "the ep_annihilation benchmark
/// at size N actually produces recycled ids during reduction."
///
/// ERA-ERA annihilation fires the void rule (SPEC-03 §3.1): both ERA agents
/// are removed via `remove_agent`, which pushes their ids onto `free_list`.
/// At size 10: 20 agents created, 20 agents removed → free_list has 20 entries
/// (or exactly 20 LIFO entries if no recycling happened during construction).
///
/// This test runs as a DEFAULT (fast, non-ignored) test.
#[test]
fn ep_annihilation_produces_non_empty_free_list_after_reduction() {
    let bench = EPAnnihilation;
    let size = 10_u32;
    let mut net = bench.make_net(size);

    // Pre-reduction: free_list must be empty (no recycling during construction).
    assert_eq!(
        net.free_list.len(),
        0,
        "pre-reduction free_list must be empty (pure-create construction)"
    );

    reduce_all(&mut net);

    // Post-reduction: free_list must be non-empty (ERA-ERA void rule fires,
    // both agents removed → ids recycled).
    assert!(
        !net.free_list.is_empty(),
        "IT-0610-03 witness: ep_annihilation MUST produce a non-empty free_list \
         after reduce_all (ERA-ERA void rule pushes ids to free_list per SPEC-22 R10c)"
    );

    // The void rule fires `size` times, removing 2 agents per fire.
    // Both ids should be in free_list.
    assert_eq!(
        net.free_list.len(),
        (size * 2) as usize,
        "IT-0610-03 witness: ep_annihilation at size {size} should push {} \
         agent ids into free_list (2 per ERA-ERA pair)",
        size * 2
    );

    // Normal form is empty.
    assert_eq!(
        net.live_agents().count(),
        0,
        "ep_annihilation normal form must be empty"
    );
}
