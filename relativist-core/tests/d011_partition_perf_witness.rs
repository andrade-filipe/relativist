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
//! ## Discriminator: `subnet.next_id`
//!
//! After QA-D009-005, `SparseNet::to_dense` sizes the dense arena by
//! `max_id + 1` regardless of the requested `id_range`. Both branches
//! therefore produce the same `subnet.agents.len()`, so arena size cannot
//! discriminate. We use `subnet.next_id` instead. From `partition/split.rs:93-98`
//! the post-build override is `subnet.next_id = max(subnet.next_id, max_agent_id + 1)`:
//!
//! - DENSE: `build_subnet` initializes `next_id = 0` → final = `max_agent_id + 1`.
//! - SPARSE: `build_subnet_sparse` initializes `next_id = id_range.start` →
//!   final = `max(id_range.start, max_agent_id + 1)` = `id_range.start`
//!   (because `compute_id_ranges` always assigns `id_range.start ≥ base_next_id ≥ live_count`).

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
    // Post-override next_id (split.rs:93-98) per branch:
    //   worker 0 — DENSE: max(0, 500) = 500   ; SPARSE: max(1_000, 500) = 1_000.
    //   worker 1 — DENSE: max(0, 1_000) = 1_000; SPARSE: max(101_000, 1_000) = 101_000.
    //
    // OLD metric (pre-v2.4): id_range_size (100_000) > 4 × 500 = 2_000 → SPARSE for both
    //                        → next_id = {1_000, 101_000} → BUG.
    // NEW metric (v2.4): effective_arena_size = max_live_id + 1 ({500, 1_000}) ≤ 4 × 500
    //                    → DENSE for both → next_id = {500, 1_000} → CORRECT.
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
        let expected_dense_next_id = max_live_id + 1;
        let id_range_start = partition.id_range.start;

        assert_eq!(
            partition.subnet.next_id, expected_dense_next_id,
            "partition {i} took SPARSE branch: subnet.next_id = {} (= id_range.start = {}); \
             expected DENSE branch: subnet.next_id = max_agent_id + 1 = {}. \
             See docs/next-steps.md BLOCKER 2026-05-04.",
            partition.subnet.next_id, id_range_start, expected_dense_next_id,
        );
        // Sanity: confirm the discriminator is real (id_range.start would be a different value).
        assert_ne!(
            partition.subnet.next_id, id_range_start,
            "discriminator collapse: max_agent_id + 1 == id_range.start for partition {i}; \
             test setup must scatter live IDs differently to maintain the SPARSE/DENSE distinction",
        );
    }
}
