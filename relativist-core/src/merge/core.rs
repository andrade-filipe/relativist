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
    let PartitionPlan {
        partitions,
        borders,
        ..
    } = plan;

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
                // Restore the boundary wire (R5)
                // connect() auto-detects redexes (SPEC-02, R13)
                result.connect(port_a, port_b);
                if is_principal_pair(port_a, port_b) {
                    border_redex_count += 1;
                }
            }
            // One side removed by erasure (R6)
            (Some(_), None) | (None, Some(_)) => {}
            // Both sides removed by erasure (R7)
            (None, None) => {}
        }
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
    use crate::partition::{split, ContiguousIdStrategy};

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
}
