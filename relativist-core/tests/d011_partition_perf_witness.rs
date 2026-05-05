//! D-011 BLOCKER 2026-05-04 — partition performance regression witness.
//!
//! For a healthy workload (live agents densely packed inside their id_range),
//! `partition::split_with_config` MUST route every partition through the DENSE
//! `build_subnet` path. Using the SPARSE path here is a 5–7× wall-clock
//! regression (proven empirically by the bisect in `docs/next-steps.md`
//! BLOCKER 2026-05-04).
//!
//! Regression witness for SPEC-22 v2.4 R22 amendment (effective_arena_size
//! metric, replacing the broken id_range_size metric).
//!
//! ## Discriminator: `subnet.next_id >= id_range.start`
//!
//! After QA-D009-005, `SparseNet::to_dense` sizes the dense arena by
//! `max_id + 1` regardless of the requested `id_range`. Both branches
//! therefore produce the same `subnet.agents.len()`, so arena size cannot
//! discriminate.
//!
//! After QA-D011-BUG2 (2026-05-04), the DENSE path also initializes
//! `subnet.next_id = id_range.start` (it previously initialized to 0,
//! which made worker 0 allocate fresh IDs OUTSIDE its assigned range,
//! colliding with another worker's pre-existing IDs). With both branches
//! now satisfying that invariant, `next_id` is no longer a unit-level
//! discriminator between dense and sparse construction.
//!
//! What remains observable — and is the actual contract the witness pins —
//! is the post-fix invariant: every partition's `subnet.next_id` MUST be
//! at least `id_range.start`, so that subsequent `create_agent` calls
//! allocate inside the partition's assigned range (SPEC-22 R10 / D3).
//!
//! Pre-Bug2-fix DENSE behavior: `build_subnet` returned `next_id = 0`,
//! `split.rs:96-98` widened to `max(0, max_agent_id + 1) = max_agent_id + 1`.
//! For worker 0 (max_agent_id = 499, id_range.start = 1000) this yielded
//! `next_id = 500 < id_range.start = 1000` → **invariant violated**, and
//! worker 0 would proceed to allocate fresh IDs at 500..., colliding with
//! worker 1's range. The `assert!(subnet.next_id >= id_range.start)`
//! below failed RED at the witness commit (4e76341) and FLIPS GREEN with
//! the QA-D011-BUG2 fix.

use relativist_core::net::{Net, PortRef, Symbol, PORTS_PER_SLOT};
use relativist_core::partition::{self, ContiguousIdStrategy, PartitionConfig};

/// Build a small healthy net: N live CON agents at IDs 0..N (densely packed),
/// principal ports wired to FreePorts (T1 compliance).
fn build_dense_packed_net(n: u32) -> Net {
    let mut net = Net::new();
    for _ in 0..n {
        net.create_agent(Symbol::Con);
    }
    for i in 0..n {
        let port_idx = i as usize * PORTS_PER_SLOT;
        net.ports[port_idx] = PortRef::FreePort(1_000_000 + i);
    }
    net
}

#[test]
fn d011_witness_partition_dense_branch_for_healthy_workload() {
    // 1000 live CON agents densely packed at IDs 0..999. With `ContiguousIdStrategy`
    // (the only `PartitionStrategy` impl wired into the offline `split_with_config`
    // path; FennelStrategy lives in the streaming module and isn't reachable here),
    // workers receive contiguous chunks: worker 0 gets IDs 0..499, worker 1 gets
    // IDs 500..999. `compute_id_ranges(2, 1000)` yields chunk_size = max(100_000,
    // 1000 × 10) = 100_000, so:
    //   worker 0: id_range = [1000, 101_000),   live = 500, max_agent_id = 499.
    //   worker 1: id_range = [101_000, u32::MAX), live = 500, max_agent_id = 999.
    //
    // Post-override next_id (split.rs:96-98) per branch (POST QA-D011-BUG2 fix):
    //   worker 0 — DENSE: build_subnet returns next_id = id_range.start = 1_000;
    //                     split widens to max(1_000, 500) = 1_000.
    //   worker 0 — SPARSE: build_subnet_sparse returns next_id = id_range.start = 1_000;
    //                     split widens to max(1_000, 500) = 1_000.
    //   worker 1 — DENSE: returns 101_000; split widens to max(101_000, 1_000) = 101_000.
    //   worker 1 — SPARSE: same.
    //
    // Pre-Bug2-fix: DENSE incorrectly returned next_id = 0; split widened to
    // max(0, 500) = 500 for worker 0 — VIOLATES `next_id >= id_range.start = 1_000`,
    // and worker 0 would then allocate fresh IDs starting at 500, colliding with
    // worker 1's pre-existing IDs (500..999). The assertion below pins the post-fix
    // invariant: subnet.next_id >= id_range.start MUST hold for every partition.
    let net = build_dense_packed_net(1000);
    let strategy = ContiguousIdStrategy;
    let cfg = PartitionConfig::default();

    let plan = partition::split_with_config(net, 2, &strategy, &cfg);

    for (i, partition) in plan.partitions.iter().enumerate() {
        let live_count = partition.subnet.count_live_agents();
        if live_count == 0 {
            continue; // empty partition: both branches return Net::new()
        }
        let max_live_id = partition
            .subnet
            .agents
            .iter()
            .enumerate()
            .filter_map(|(idx, slot)| slot.as_ref().map(|_| idx as u32))
            .max()
            .expect("non-empty partition has at least one live agent");
        let id_range_start = partition.id_range.start;

        // QA-D011-BUG2 invariant: post-fix, every partition's next_id MUST be
        // at least id_range.start so fresh allocations stay inside the assigned
        // range. Pre-fix, dense build_subnet returned next_id = 0 → split's
        // max(0, max_agent_id + 1) yielded next_id = max_agent_id + 1, which
        // for worker 0 was 500 (< id_range.start = 1000) — outside the
        // partition's range and colliding with worker 1's pre-existing IDs.
        assert!(
            partition.subnet.next_id >= id_range_start,
            "QA-D011-BUG2 / SPEC-22 R10: partition {i} subnet.next_id = {} \
             violates the invariant next_id >= id_range.start = {}. Pre-fix \
             this would allow worker {i} to allocate fresh IDs (starting at \
             {}) OUTSIDE its assigned range, colliding with another worker. \
             Live count = {}, max_live_id = {}. \
             See docs/qa/QA-D011-BUG2-i1-violation-2026-05-04.md.",
            partition.subnet.next_id,
            id_range_start,
            partition.subnet.next_id,
            live_count,
            max_live_id,
        );
        // I3' upper bound: next_id MUST also exceed every live agent's ID.
        assert!(
            partition.subnet.next_id > max_live_id,
            "I3': partition {i} subnet.next_id = {} must be > max_live_id = {}",
            partition.subnet.next_id,
            max_live_id,
        );
    }
}
