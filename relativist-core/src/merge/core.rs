//! The merge function: recombines partitions into a single Net (SPEC-05, Section 4.2).

use crate::net::{total_ports, Net, PortRef, DISCONNECTED, PORTS_PER_SLOT};
use crate::partition::PartitionPlan;

use super::helpers::is_principal_pair;

/// Recombines locally-reduced partitions into a single Net (SPEC-05, R1-R11).
///
/// Accepts PartitionPlan by value, consuming partitions and borders.
///
/// Steps:
/// 1. Compute capacity and set next_id to max of all partitions (R8).
/// 2. Unite agents and internal connections from all partitions (R2, R3).
///    - Boundary FreePorts (in border map) are temporarily DISCONNECTED.
///    - Lafont FreePorts (NOT in border map) are copied directly.
///    - Debug assertion verifies partition queues contain only stale entries (R9).
/// 3. Restore boundary connections via the border map (R4-R7, R12-R14).
///
/// Returns: (merged_net, border_redex_count)
///
/// Pre-conditions:
/// - All partitions have been locally reduced.
/// - AgentIds across partitions are mutually disjoint (SPEC-04, R16-R19).
/// - free_port_index has been rebuilt for each partition (TASK-0063).
///
/// Post-conditions:
/// - Result net contains all live agents from all partitions.
/// - All FreePort (Boundary) sentinels have been resolved or discarded.
/// - Pre-existing Lafont FreePorts are preserved.
/// - Redex queue contains only border redexes (partition queues discarded).
/// - In debug mode, assert_all_invariants() passes (R11).
pub fn merge(plan: PartitionPlan) -> (Net, u32) {
    // `mut` is only needed by the debug-only protected_tombstones drain below
    // (`for partition in &mut partitions` under cfg(debug_assertions)).
    // In release builds that loop is elided, so the binding looks unused-mut.
    #[allow(unused_mut)]
    let PartitionPlan {
        mut partitions,
        borders,
        ..
    } = plan;

    // --- Invariant Defense (TASK-0452, MF-006) ---
    // Verifies that AgentIds across partitions are mutually disjoint
    // (SPEC-04, R16-R19). D3-elastic: catches logic errors where
    // overlapping ID ranges are merged. Uses `debug_assert!` consistently
    // (TASK-0452 contract); the cfg gate elides the supporting Vec
    // allocation in release builds where the assertion is a no-op.
    #[cfg(debug_assertions)]
    {
        let mut sorted_ranges: Vec<_> = partitions.iter().map(|p| p.id_range).collect();
        sorted_ranges.sort_by_key(|r| r.start);
        for pair in sorted_ranges.windows(2) {
            debug_assert!(
                pair[0].end <= pair[1].start,
                "D3 violated: overlapping ID ranges in merge: {:?} vs {:?}",
                pair[0],
                pair[1]
            );
        }
    }

    // --- Step 1: Compute capacity (R8) ---
    //
    // next_id comes from static ID space partitioning and can be very large
    // (e.g., u32::MAX/2). We use the max agents array length to size the
    // result arrays, and set next_id separately for correctness.
    let max_next_id = partitions
        .iter()
        .map(|p| p.subnet.next_id)
        .max()
        .unwrap_or(0);

    let max_agents_len: usize = partitions
        .iter()
        .map(|p| p.subnet.agents.len())
        .max()
        .unwrap_or(0);

    let mut result = Net::with_capacity(max_agents_len);
    result.next_id = max_next_id;

    // Size arrays based on actual agent arena size, not next_id
    if result.agents.len() < max_agents_len {
        result.agents.resize(max_agents_len, None);
    }
    let required_ports = max_agents_len * PORTS_PER_SLOT;
    if result.ports.len() < required_ports {
        result.ports.resize(required_ports, DISCONNECTED);
    }

    // --- Step 2: Unite agents and internal connections (R2, R3, R9) ---
    for partition in &partitions {
        // Debug assertion: partition queues should contain only stale entries (R9)
        #[cfg(debug_assertions)]
        {
            for &(a, b) in &partition.subnet.redex_queue {
                debug_assert!(
                    !partition.subnet.is_valid_redex(a, b),
                    "Non-stale redex ({}, {}) found in partition {} queue after reduce_all",
                    a,
                    b,
                    partition.worker_id
                );
            }
        }

        for (i, slot) in partition.subnet.agents.iter().enumerate() {
            if let Some(agent) = slot {
                // Copy agent to result arena
                let id = i as u32;
                if result.agents.len() <= i {
                    result.agents.resize(i + 1, None);
                }
                // QA-D011-BUG2 AF-3 (2026-05-04): catch cross-partition agent
                // collisions at the merge boundary instead of waiting for I1
                // panic at end-of-merge. If multiple partitions wrote live
                // agents to the same arena slot, this fires with a clear
                // diagnostic. Cheap, debug-only.
                //
                // QA-D011-POST-FIX-AUDIT F-006 (2026-05-04): this guard is
                // debug-only. Release builds will silently overwrite colliding
                // slots (the Bug 2 failure mode), then surface as I1 violations
                // later in `assert_all_invariants` OR as silent wrong results.
                debug_assert!(
                    result.agents[i].is_none(),
                    "merge: agent ID {} appears in multiple partitions (D3 live-set violation)",
                    i
                );
                result.agents[i] = Some(*agent);

                // Expand port array if needed
                let port_end = (i + 1) * PORTS_PER_SLOT;
                if result.ports.len() < port_end {
                    result.ports.resize(port_end, DISCONNECTED);
                }

                // Copy connections for all ports of this agent
                let num_ports = total_ports(agent.symbol);
                for p in 0..num_ports {
                    let target = partition.subnet.get_target(PortRef::AgentPort(id, p));
                    if target == DISCONNECTED {
                        continue;
                    }

                    match target {
                        PortRef::AgentPort(_, _) => {
                            // Internal connection: copy directly via set_port
                            let idx = i * PORTS_PER_SLOT + p as usize;
                            result.ports[idx] = target;
                        }
                        PortRef::FreePort(fid) => {
                            let idx = i * PORTS_PER_SLOT + p as usize;
                            if borders.contains_key(&fid) {
                                // Boundary FreePort: will be restored in Step 3
                                result.ports[idx] = DISCONNECTED;
                            } else {
                                // Lafont FreePort: copy directly (SPEC-04, R15)
                                result.ports[idx] = target;
                            }
                        }
                    }
                }
            }
        }

        // Propagate root from the partition that has it (R28 from SPEC-04)
        if partition.subnet.root.is_some() {
            result.root = partition.subnet.root;
        }
    }

    // --- Step 3: Restore boundary connections (R4-R7, R12-R14) ---
    //
    // FreePort-chain resolution (SC-015 closure):
    //
    // When a local reduction inside a partition links two border FreePorts
    // together (e.g. CON-CON rule connects FreePort(A) ↔ FreePort(B)),
    // `rebuild_free_port_index` records this as a FreePort redirect:
    // `free_port_index[A] = FreePort(B)` and `free_port_index[B] = FreePort(A)`.
    //
    // Naively connecting port_X to FreePort(B) leaves the two AgentPorts that
    // "own" A and B (in other partitions) connected only to dangling FreePorts
    // instead of to each other. This helper resolves the chain: if one endpoint
    // is FreePort(bid_Y) and bid_Y is also a tracked border, the AgentPort on
    // the OTHER side of bid_Y is the true intended connection partner.
    //
    // The `max_chain_depth` guard prevents infinite loops in case of degenerate
    // FreePort cycles (which should not occur in well-formed nets, but we guard
    // defensively). Depth 8 is sufficient for all realistic partition depths.
    let resolve_freeport_chain = |initial: PortRef| -> PortRef {
        let mut current = initial;
        const MAX_CHAIN_DEPTH: usize = 8;
        for _ in 0..MAX_CHAIN_DEPTH {
            if let PortRef::FreePort(bid_y) = current {
                if borders.contains_key(&bid_y) {
                    // Find the AgentPort endpoint of bid_y (prefer AgentPort over FreePort).
                    let mut resolved: Option<PortRef> = None;
                    for partition in &partitions {
                        if let Some(&port_ref) = partition.free_port_index.get(&bid_y) {
                            match port_ref {
                                PortRef::AgentPort(_, _) => {
                                    resolved = Some(port_ref);
                                    break; // AgentPort is the terminal; stop searching
                                }
                                PortRef::FreePort(_) => {
                                    // Another FreePort redirect — continue the chain.
                                    if resolved.is_none() {
                                        resolved = Some(port_ref);
                                    }
                                }
                            }
                        }
                    }
                    match resolved {
                        Some(r) if r != current => {
                            current = r;
                            // If we found an AgentPort, stop immediately.
                            if matches!(current, PortRef::AgentPort(_, _)) {
                                break;
                            }
                        }
                        _ => break, // No further resolution possible.
                    }
                } else {
                    break; // Not a border FreePort — it's a Lafont interface port.
                }
            } else {
                break; // AgentPort — already resolved.
            }
        }
        current
    };

    let mut border_redex_count: u32 = 0;

    for (border_id, (_orig_a, _orig_b)) in &borders {
        let mut current_a: Option<PortRef> = None;
        let mut current_b: Option<PortRef> = None;

        for partition in &partitions {
            if let Some(port_ref) = partition.free_port_index.get(border_id) {
                if current_a.is_none() {
                    current_a = Some(*port_ref);
                } else {
                    current_b = Some(*port_ref);
                }
            }
        }

        match (current_a, current_b) {
            (Some(port_a), Some(port_b)) => {
                // Resolve any FreePort-chain redirects produced by local reductions
                // that linked two border FreePorts (SC-015 closure).
                let resolved_a = resolve_freeport_chain(port_a);
                let resolved_b = resolve_freeport_chain(port_b);
                // Restore the boundary wire (R5)
                // connect() auto-detects redexes (SPEC-02, R13)
                result.connect(resolved_a, resolved_b);
                if is_principal_pair(resolved_a, resolved_b) {
                    border_redex_count += 1;
                }
            }
            // One side removed by erasure (R6)
            (Some(_), None) | (None, Some(_)) => {}
            // Both sides removed by erasure (R7)
            (None, None) => {}
        }
    }

    // --- SPEC-22 R12 / §3.8 A8: free-list reconciliation ---
    //
    // Walk every input partition's free_list. For each ID:
    //   1. Check whether the merged arena slot is still None.
    //   2. Check whether the ID was already pushed by a prior partition (QA-D009-004 dedup).
    // Only push if both conditions hold. Complexity: O(sum of |partition.free_list|).
    //
    // The D4 disjointness ASSUMPTION was previously a comment claiming no duplicates
    // were possible. QA-D009-004 showed that a bugged coordinator or release-build
    // state corruption can still produce cross-partition duplicates. The HashSet guard
    // makes the invariant explicit and always-enforced.
    let mut seen_free: std::collections::HashSet<crate::net::AgentId> =
        std::collections::HashSet::new();
    for partition in &partitions {
        for &id in &partition.subnet.free_list {
            if result.agents.get(id as usize).is_some_and(|s| s.is_none()) && seen_free.insert(id) {
                result.free_list.push(id);
            }
            // else: slot is occupied OR already pushed — discard (A8 discard branch)
        }
    }

    // --- SPEC-22 R12: drain protected_tombstones (debug-only) ---
    //
    // Any tombstone still alive at merge time is reclaimable if its slot is None.
    // This handles the case where a worker held a protected tombstone (border-
    // referenced ID under delta mode) that was never recycled before the merge.
    #[cfg(debug_assertions)]
    for partition in &mut partitions {
        if let Some(tombstones) = partition.subnet.protected_tombstones.take() {
            for id in tombstones {
                if result.agents.get(id as usize).is_some_and(|s| s.is_none())
                    && !result.free_list.contains(&id)
                {
                    result.free_list.push(id);
                }
            }
        }
    }

    // SPEC-22 R12: post-merge invariant check (debug only)
    #[cfg(debug_assertions)]
    debug_assert!(
        result.validate_free_list().is_ok(),
        "SPEC-22 R12: merged free_list invariant violated after reconciliation"
    );

    // SPEC-22 R12: dissolve partition-local state
    result.id_range = None;
    result.border_entries_shadow = None;
    #[cfg(debug_assertions)]
    {
        result.protected_tombstones = None;
    }

    // Debug assertion: verify all invariants on the merged net (R11)
    #[cfg(debug_assertions)]
    result.assert_all_invariants();

    (result, border_redex_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::Symbol;
    use crate::partition::{split, ContiguousIdStrategy, IdRange, Partition};
    use std::collections::{HashMap, HashSet};

    // === TASK-0065: Unite agents tests ===

    // T1: Merge single partition with no borders -> identical net
    //
    // Uses a net without redexes (principal ports to FreePort) to satisfy
    // the merge precondition (all partitions locally reduced).
    #[test]
    fn test_merge_single_partition_no_borders() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        // No principal-principal connections (no redexes)
        net.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(b, 0), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 1));
        net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(b, 2));

        let plan = split(net, 1, &ContiguousIdStrategy);
        let (merged, border_count) = merge(plan);

        assert_eq!(border_count, 0);
        assert_eq!(merged.count_live_agents(), 2);
        assert_eq!(
            merged.get_target(PortRef::AgentPort(a, 1)),
            PortRef::AgentPort(b, 1)
        );
        assert_eq!(
            merged.get_target(PortRef::AgentPort(a, 2)),
            PortRef::AgentPort(b, 2)
        );
    }

    // T2: Merge two partitions with disjoint agents and no borders
    #[test]
    fn test_merge_two_disjoint_no_borders() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        let c = net.create_agent(Symbol::Era);
        let d = net.create_agent(Symbol::Era);
        // a-b internal to partition 0, c-d internal to partition 1
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(c, 0), PortRef::AgentPort(d, 0));

        // Reduce first to make partition queues stale
        crate::reduction::reduce_all(&mut net);

        // After reduce_all, ERA-ERA pairs are consumed (0 agents)
        // Let's use a non-reducible example instead
        let mut net2 = Net::new();
        let a2 = net2.create_agent(Symbol::Con);
        let b2 = net2.create_agent(Symbol::Con);
        let c2 = net2.create_agent(Symbol::Con);
        let d2 = net2.create_agent(Symbol::Con);
        // Wire a2-b2 so they stay in partition 0 (no principal-port connection)
        net2.connect(PortRef::AgentPort(a2, 1), PortRef::AgentPort(b2, 1));
        net2.connect(PortRef::AgentPort(a2, 2), PortRef::AgentPort(b2, 2));
        net2.connect(PortRef::AgentPort(c2, 1), PortRef::AgentPort(d2, 1));
        net2.connect(PortRef::AgentPort(c2, 2), PortRef::AgentPort(d2, 2));
        // Principal ports as FreePort (Lafont)
        net2.connect(PortRef::AgentPort(a2, 0), PortRef::FreePort(0));
        net2.connect(PortRef::AgentPort(b2, 0), PortRef::FreePort(1));
        net2.connect(PortRef::AgentPort(c2, 0), PortRef::FreePort(2));
        net2.connect(PortRef::AgentPort(d2, 0), PortRef::FreePort(3));

        let plan = split(net2, 2, &ContiguousIdStrategy);
        let (merged, border_count) = merge(plan);

        assert_eq!(merged.count_live_agents(), 4);
        // No borders between these partitions (auxiliary ports are internal)
        assert_eq!(border_count, 0);
    }

    // T3: next_id is the max across partitions
    #[test]
    fn test_merge_next_id_is_max() {
        let mut net = Net::new();
        for _ in 0..4 {
            net.create_agent(Symbol::Era);
        }
        // Connect with FreePort to avoid redexes
        net.connect(PortRef::AgentPort(0, 0), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(1, 0), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(2, 0), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(3, 0), PortRef::FreePort(3));

        let original_next_id = net.next_id;
        let plan = split(net, 2, &ContiguousIdStrategy);
        let (merged, _) = merge(plan);

        assert!(merged.next_id >= original_next_id);
    }

    // === TASK-0066: Boundary restoration tests ===

    // T4: Border wire restored after merge
    #[test]
    fn test_merge_border_wire_restored() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        // Principal ports connected -> will be a border wire when split
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(3));

        let plan = split(net, 2, &ContiguousIdStrategy);
        // Rebuild free_port_index for each partition (simulating post-reduce)
        let (merged, border_count) = merge(plan);

        // The principal port wire should be restored
        assert_eq!(
            merged.get_target(PortRef::AgentPort(a, 0)),
            PortRef::AgentPort(b, 0)
        );
        assert_eq!(
            merged.get_target(PortRef::AgentPort(b, 0)),
            PortRef::AgentPort(a, 0)
        );
        // It's a principal-principal wire -> border redex
        assert_eq!(border_count, 1);
    }

    // T5: Border with principal + auxiliary -> border_redex_count == 0
    #[test]
    fn test_merge_border_auxiliary_no_redex() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        // Auxiliary port connected across partitions
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 1));
        // Other ports as FreePort
        net.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(b, 0), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(3));

        let plan = split(net, 2, &ContiguousIdStrategy);
        let (merged, border_count) = merge(plan);

        // Auxiliary-auxiliary wire -> not a border redex
        assert_eq!(border_count, 0);
        assert_eq!(
            merged.get_target(PortRef::AgentPort(a, 1)),
            PortRef::AgentPort(b, 1)
        );
    }

    // T6: Lafont FreePorts preserved after merge
    #[test]
    fn test_merge_preserves_lafont_freeports() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        // Lafont FreePorts
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(3));

        let plan = split(net, 2, &ContiguousIdStrategy);
        let (merged, _) = merge(plan);

        // Lafont FreePorts should be preserved
        assert_eq!(
            merged.get_target(PortRef::AgentPort(a, 1)),
            PortRef::FreePort(0)
        );
        assert_eq!(
            merged.get_target(PortRef::AgentPort(a, 2)),
            PortRef::FreePort(1)
        );
        assert_eq!(
            merged.get_target(PortRef::AgentPort(b, 1)),
            PortRef::FreePort(2)
        );
        assert_eq!(
            merged.get_target(PortRef::AgentPort(b, 2)),
            PortRef::FreePort(3)
        );
    }

    // T7: Root port propagation through split/merge
    #[test]
    fn test_merge_root_propagation() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.root = Some(PortRef::AgentPort(a, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 1));
        net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(b, 2));
        net.connect(PortRef::AgentPort(b, 0), PortRef::FreePort(0));

        let plan = split(net, 2, &ContiguousIdStrategy);
        let (merged, _) = merge(plan);

        assert_eq!(merged.root, Some(PortRef::AgentPort(a, 0)));
    }

    // T8: Empty net merge
    #[test]
    fn test_merge_empty_net() {
        let net = Net::new();
        let plan = split(net, 2, &ContiguousIdStrategy);
        let (merged, border_count) = merge(plan);

        assert_eq!(merged.count_live_agents(), 0);
        assert_eq!(border_count, 0);
    }

    // === TASK-0074: Split/merge identity (D1) ===

    // D1: merge(split(net)) ~ net (structural identity)
    //
    // 2 agents, 2 workers -> each agent in its own partition.
    // Principal-principal wire becomes a border wire, so no valid redex
    // in either partition queue (merge precondition satisfied).
    #[test]
    fn test_split_merge_identity() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con); // id=0 -> w0
        let b = net.create_agent(Symbol::Con); // id=1 -> w1
                                               // a:0-b:0 becomes a border wire
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(3));

        let original_agents = net.count_live_agents();
        let original_a0 = net.get_target(PortRef::AgentPort(a, 0));
        let original_a1 = net.get_target(PortRef::AgentPort(a, 1));
        let original_b1 = net.get_target(PortRef::AgentPort(b, 1));

        let plan = split(net, 2, &ContiguousIdStrategy);
        let (merged, _) = merge(plan);

        assert_eq!(merged.count_live_agents(), original_agents);
        assert_eq!(merged.get_target(PortRef::AgentPort(a, 0)), original_a0);
        assert_eq!(merged.get_target(PortRef::AgentPort(a, 1)), original_a1);
        assert_eq!(merged.get_target(PortRef::AgentPort(b, 1)), original_b1);
        assert_eq!(merged.get_agent(a).unwrap().symbol, Symbol::Con);
        assert_eq!(merged.get_agent(b).unwrap().symbol, Symbol::Con);
    }

    // D1b: Identity with 3 workers
    #[test]
    fn test_split_merge_identity_three_workers() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);
        let c = net.create_agent(Symbol::Con);
        // a:0-b:0, b:1-c:1, others as FreePort
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(b, 1), PortRef::AgentPort(c, 1));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(c, 0), PortRef::FreePort(3));
        net.connect(PortRef::AgentPort(c, 2), PortRef::FreePort(4));

        let plan = split(net, 3, &ContiguousIdStrategy);
        let (merged, _) = merge(plan);

        assert_eq!(merged.count_live_agents(), 3);
        // All cross-partition wires restored
        assert_eq!(
            merged.get_target(PortRef::AgentPort(a, 0)),
            PortRef::AgentPort(b, 0)
        );
        assert_eq!(
            merged.get_target(PortRef::AgentPort(b, 1)),
            PortRef::AgentPort(c, 1)
        );
        // Lafont FreePorts preserved
        assert_eq!(
            merged.get_target(PortRef::AgentPort(a, 1)),
            PortRef::FreePort(0)
        );
    }

    // === TASK-0483: merge free-list reconciliation (SPEC-22 R12 / §3.8 A8) ===

    /// Builds a test Partition with a subnet that has Era agents at `live_ids`
    /// (each with port 0 connected to a unique Lafont FreePort) and `None` slots
    /// at `free_ids`. `id_range` is the range assigned to this worker.
    ///
    /// All principal ports are wired to FreePort(u32::MAX - freeport_counter) to
    /// satisfy T1 (no DISCONNECTED ports on live agents). The FreePort sentinel
    /// starts far from 0 to avoid colliding with border IDs in other tests.
    fn make_partition_with_free_list(
        worker_id: u32,
        live_ids: &[u32],
        free_ids: &[u32],
        id_range: std::ops::Range<u32>,
    ) -> Partition {
        let max_id = live_ids
            .iter()
            .chain(free_ids.iter())
            .copied()
            .max()
            .unwrap_or(0) as usize;
        let arena_size = max_id + 1;

        let mut net = Net::new();
        net.agents.resize(arena_size, None);
        net.ports
            .resize(arena_size * crate::net::PORTS_PER_SLOT, DISCONNECTED);

        // Connect each live Era agent's principal port (p=0) to a unique Lafont
        // FreePort. FreePort(u32::MAX) is the DISCONNECTED sentinel — must avoid it.
        // We use FreePort(1_000_000 + id) as a unique, non-sentinel, non-border ID.
        // Era has arity 0 (only principal port), so this is sufficient for T1.
        for &id in live_ids {
            net.agents[id as usize] = Some(crate::net::Agent {
                symbol: Symbol::Era,
                id,
            });
            let fp_id = 1_000_000u32 + id; // unique, non-MAX, far from border IDs
            let port_idx = id as usize * crate::net::PORTS_PER_SLOT; // port 0
            net.ports[port_idx] = PortRef::FreePort(fp_id);
        }
        // free_ids are left as None — they are the recycled slots
        net.free_list = free_ids.to_vec();
        net.id_range = Some(id_range.clone());
        net.next_id = id_range.end;

        Partition {
            subnet: net,
            worker_id,
            free_port_index: HashMap::new(),
            id_range: IdRange {
                start: id_range.start,
                end: id_range.end,
            },
            border_id_start: 0,
            border_id_end: 0,
        }
    }

    // UT-0483-01: merge combines free-lists from 2 partitions correctly.
    // Partition 0: free_list [50, 75], live agents at other IDs.
    // Partition 1: free_list [125, 175], live agents at other IDs.
    // Expected: merged free_list == {50, 75, 125, 175}.
    #[test]
    fn merge_combines_partition_free_lists_correctly() {
        let live0: Vec<u32> = (0..100).filter(|&id| id != 50 && id != 75).collect();
        let live1: Vec<u32> = (100..200).filter(|&id| id != 125 && id != 175).collect();

        let p0 = make_partition_with_free_list(0, &live0, &[50, 75], 0..100);
        let p1 = make_partition_with_free_list(1, &live1, &[125, 175], 100..200);

        let plan = PartitionPlan {
            partitions: vec![p0, p1],
            borders: HashMap::new(),
            next_border_id: 0,
        };
        let (merged, _) = merge(plan);

        let free_set: HashSet<u32> = merged.free_list.iter().copied().collect();
        assert_eq!(
            free_set,
            HashSet::from([50, 75, 125, 175]),
            "merged free_list must contain all free IDs from both partitions"
        );
    }

    // UT-0483-02: validate_free_list passes on the merged net from UT-0483-01.
    #[test]
    fn merge_post_condition_validate_free_list_passes() {
        let live0: Vec<u32> = (0..100).filter(|&id| id != 50 && id != 75).collect();
        let live1: Vec<u32> = (100..200).filter(|&id| id != 125 && id != 175).collect();

        let p0 = make_partition_with_free_list(0, &live0, &[50, 75], 0..100);
        let p1 = make_partition_with_free_list(1, &live1, &[125, 175], 100..200);

        let plan = PartitionPlan {
            partitions: vec![p0, p1],
            borders: HashMap::new(),
            next_border_id: 0,
        };
        let (merged, _) = merge(plan);

        assert!(
            merged.validate_free_list().is_ok(),
            "validate_free_list must pass on the merged net"
        );
    }

    // UT-0483-03: merge discards filled slots.
    // Partition 0 free_list has [50] but slot 50 is filled by a live agent.
    // (Simulates the case where a partition's free_list had an ID that is
    //  actually occupied — the discard branch of §3.8 A8 must fire.)
    #[test]
    fn merge_discards_filled_slots() {
        // Partition 0: all IDs 0..100 are live (including slot 50).
        // The free_list says 50 is free — but it's not (data inconsistency).
        // The A8 discard branch should catch this and NOT add 50 to merged.
        let live0: Vec<u32> = (0..100).collect(); // ALL slots live, including 50
        let live1: Vec<u32> = (100..200).collect();

        let p0 = make_partition_with_free_list(0, &live0, &[50], 0..100);
        // Force slot 50 to be occupied (it's already set by live0, but
        // make_partition_with_free_list might have left it None via free_ids;
        // here free_ids=[50] but live0 also contains 50 — live0 wins in
        // the loop, so slot 50 is Some(Agent). Confirm that.
        assert!(
            p0.subnet.agents[50].is_some(),
            "test setup: slot 50 must be occupied"
        );
        let p1 = make_partition_with_free_list(1, &live1, &[], 100..200);

        let plan = PartitionPlan {
            partitions: vec![p0, p1],
            borders: HashMap::new(),
            next_border_id: 0,
        };
        let (merged, _) = merge(plan);

        assert!(
            !merged.free_list.contains(&50),
            "slot 50 is occupied — must NOT appear in merged free_list (A8 discard branch)"
        );
    }

    // UT-0483-04: merged free_list has no duplicates (D4 disjointness smoke check).
    #[test]
    fn merge_preserves_no_duplicates() {
        let live0: Vec<u32> = (0..100).filter(|&id| id != 50).collect();
        let live1: Vec<u32> = (100..200).filter(|&id| id != 150).collect();

        let p0 = make_partition_with_free_list(0, &live0, &[50], 0..100);
        let p1 = make_partition_with_free_list(1, &live1, &[150], 100..200);

        let plan = PartitionPlan {
            partitions: vec![p0, p1],
            borders: HashMap::new(),
            next_border_id: 0,
        };
        let (merged, _) = merge(plan);

        let as_vec = &merged.free_list;
        let as_set: HashSet<u32> = as_vec.iter().copied().collect();
        assert_eq!(
            as_vec.len(),
            as_set.len(),
            "merged free_list must contain no duplicates"
        );
    }

    // UT-0483-05: merge resets id_range to None on the merged net.
    #[test]
    fn merge_resets_id_range_to_none() {
        let live0: Vec<u32> = (0..100).filter(|&id| id != 50).collect();
        let p0 = make_partition_with_free_list(0, &live0, &[50], 0..100);
        // Set id_range explicitly on subnet
        assert!(
            p0.subnet.id_range.is_some(),
            "test setup: id_range must be Some"
        );

        let plan = PartitionPlan {
            partitions: vec![p0],
            borders: HashMap::new(),
            next_border_id: 0,
        };
        let (merged, _) = merge(plan);

        assert_eq!(
            merged.id_range, None,
            "merged net must have id_range == None (whole-net context dissolves partition locality)"
        );
    }

    // UT-0483-06: merge resets border_entries_shadow to None.
    #[test]
    fn merge_resets_border_entries_shadow_to_none() {
        let live0: Vec<u32> = (0..100).filter(|&id| id != 50).collect();
        let mut p0 = make_partition_with_free_list(0, &live0, &[50], 0..100);
        // Set border_entries_shadow to Some on subnet to simulate delta-mode state
        p0.subnet.border_entries_shadow = Some(HashSet::from([10, 20]));

        let plan = PartitionPlan {
            partitions: vec![p0],
            borders: HashMap::new(),
            next_border_id: 0,
        };
        let (merged, _) = merge(plan);

        assert_eq!(
            merged.border_entries_shadow, None,
            "merged net must have border_entries_shadow == None (delta state cleared)"
        );
    }

    // UT-0483-07 (debug only): merge drains protected_tombstones into free_list.
    // When a partition has protected_tombstones = Some({47}) and agents[47] == None,
    // the merged free_list must contain 47.
    #[cfg(debug_assertions)]
    #[test]
    fn merge_drains_protected_tombstones() {
        let live0: Vec<u32> = (0..100).filter(|&id| id != 47).collect();
        let mut p0 = make_partition_with_free_list(0, &live0, &[], 0..100);
        // Slot 47 is None in the subnet (not live, not in free_list yet).
        // Mark it as a protected tombstone.
        p0.subnet.protected_tombstones = Some(HashSet::from([47u32]));

        let plan = PartitionPlan {
            partitions: vec![p0],
            borders: HashMap::new(),
            next_border_id: 0,
        };
        let (merged, _) = merge(plan);

        assert!(
            merged.free_list.contains(&47),
            "protected tombstone 47 (slot None) must be drained into merged free_list"
        );
        assert_eq!(
            merged.protected_tombstones, None,
            "merged net must have protected_tombstones == None after drain"
        );
    }

    /// QA-D009-004: merge must deduplicate free-list entries across partitions.
    ///
    /// When two partitions both have the same ID in their free_list (a coordinator
    /// bug or release-build state corruption), the merged result MUST NOT contain
    /// duplicate entries. Without the HashSet dedup, a duplicate causes the next
    /// `create_agent` to issue the same ID twice (D4/I3' violation).
    ///
    /// To satisfy D3 (disjoint ID ranges), we give p0 range [0..60) and p1 range
    /// [60..120), but both have a shared free-list entry at id=50 (which is in p0's
    /// range and a None slot in p1's arena as well — simulating a buggy coordinator).
    #[test]
    fn qa_d009_004_merge_deduplicates_free_list_across_partitions() {
        // p0: range [0..60), has id 50 as a free slot.
        let mut p0_net = Net::with_capacity(60);
        p0_net.agents.resize(60, None);
        p0_net.ports.resize(60 * 3, crate::net::DISCONNECTED);
        p0_net.next_id = 60;
        p0_net.free_list.push(50); // slot 50 is None → valid free-list entry for p0

        // p1: range [60..120), but ALSO has id 50 in its free_list (the bug).
        // The merged arena will have slot 50 = None from p0, so the D3 slot check
        // passes and the dedup must prevent a second push.
        let mut p1_net = Net::with_capacity(60);
        p1_net.agents.resize(120, None); // arena covers up to id 119
        p1_net.ports.resize(120 * 3, crate::net::DISCONNECTED);
        p1_net.next_id = 120;
        p1_net.free_list.push(50); // same id — the duplication under test

        let p0 = Partition {
            subnet: p0_net,
            worker_id: 0,
            free_port_index: HashMap::new(),
            id_range: IdRange { start: 0, end: 60 },
            border_id_start: 0,
            border_id_end: 0,
        };
        let p1 = Partition {
            subnet: p1_net,
            worker_id: 1,
            free_port_index: HashMap::new(),
            id_range: IdRange {
                start: 60,
                end: 120,
            }, // disjoint from p0
            border_id_start: 0,
            border_id_end: 0,
        };

        let plan = crate::partition::PartitionPlan {
            partitions: vec![p0, p1],
            borders: HashMap::new(),
            next_border_id: 0,
        };

        let (merged, _) = merge(plan);

        let count_50 = merged.free_list.iter().filter(|&&x| x == 50).count();
        assert_eq!(
            count_50, 1,
            "QA-D009-004: id 50 must appear exactly once in merged free_list, got {} times",
            count_50
        );

        // validate_free_list must also pass (no duplicates via HashSet).
        assert!(
            merged.validate_free_list().is_ok(),
            "QA-D009-004: merged free_list must pass validate_free_list (no duplicates)"
        );
    }
}
