//! The split function: decomposes a Net into partitions (SPEC-04 Section 4.5).

use std::collections::HashMap;

use crate::net::{AgentId, Net, PortRef};

#[cfg(debug_assertions)]
use std::collections::HashSet;

use super::helpers::{build_subnet, classify_wires, compute_id_ranges};
use super::strategy::PartitionStrategy;
use super::types::{IdRange, Partition, PartitionPlan, WorkerId};

/// Splits a net into `num_workers` partitions using the given strategy.
///
/// If `num_workers <= 1`, returns the entire net as a single partition
/// with no borders (trivial case, SPEC-04 R2).
///
/// For `num_workers > 1`, executes the 7-step split algorithm (SPEC-04 Section 4.5).
///
/// Panics if `num_workers == 0`.
pub fn split(net: Net, num_workers: u32, strategy: &dyn PartitionStrategy) -> PartitionPlan {
    assert!(num_workers >= 1, "num_workers must be >= 1");

    if num_workers <= 1 {
        return trivial_plan(net);
    }

    // Step 2: Compute allocation function sigma
    let sigma = strategy.allocate(&net, num_workers);

    // Step 3: Group agents by worker
    let mut worker_agents: Vec<Vec<AgentId>> = vec![vec![]; num_workers as usize];
    for (&agent_id, &worker_id) in &sigma {
        worker_agents[worker_id as usize].push(agent_id);
    }

    // Step 4: Classify wires and generate borders
    let wire_class = classify_wires(&net, &sigma, num_workers);

    // Step 5 + 6: Build sub-nets and compute ID ranges
    let id_ranges = compute_id_ranges(num_workers, net.next_id);
    let mut partitions = Vec::with_capacity(num_workers as usize);

    for i in 0..num_workers as usize {
        let mut subnet = build_subnet(
            &net,
            &worker_agents[i],
            &sigma,
            &wire_class.border_entries[i],
            i as WorkerId,
        );

        // Set next_id: max(id_range.start, max_agent_id + 1)
        let max_agent_id = worker_agents[i].iter().copied().max();
        subnet.next_id =
            std::cmp::max(id_ranges[i].start, max_agent_id.map(|m| m + 1).unwrap_or(0));

        // R28: Root port propagation
        subnet.root = propagate_root(&net, &sigma, i as WorkerId);

        // Build FreePort index from border entries (TASK-0052)
        let free_port_index: HashMap<u32, PortRef> = wire_class.border_entries[i]
            .iter()
            .map(|&(agent_id, port_id, bid)| (bid, PortRef::AgentPort(agent_id, port_id)))
            .collect();

        partitions.push(Partition {
            subnet,
            worker_id: i as WorkerId,
            free_port_index,
            id_range: id_ranges[i],
            border_id_start: wire_class.border_id_start,
            border_id_end: wire_class.border_id_end,
        });
    }

    let plan = PartitionPlan {
        partitions,
        borders: wire_class.borders,
    };

    // Step 7: Debug assertions for C1, C2, C3 (SPEC-04 R10)
    #[cfg(debug_assertions)]
    {
        assert_c1_coverage(&net, &plan.partitions);
        assert_c3_border_consistency(&plan.partitions, &plan.borders);
    }

    plan
}

/// Trivial case: single partition with the entire net (SPEC-04 R2).
///
/// O(1) — moves the net into the partition without copying.
fn trivial_plan(net: Net) -> PartitionPlan {
    let partition = Partition {
        subnet: net,
        worker_id: 0,
        free_port_index: HashMap::new(),
        id_range: IdRange {
            start: 0,
            end: u32::MAX,
        },
        border_id_start: 0,
        border_id_end: 0,
    };

    PartitionPlan {
        partitions: vec![partition],
        borders: HashMap::new(),
    }
}

/// Determines the root port for a partition (SPEC-04 R28).
///
/// If the original net's root refers to an agent in this partition,
/// the root is propagated. Otherwise, returns None.
fn propagate_root(
    net: &Net,
    sigma: &HashMap<AgentId, WorkerId>,
    worker_id: WorkerId,
) -> Option<PortRef> {
    match net.root {
        Some(PortRef::AgentPort(id, port)) => {
            if sigma.get(&id) == Some(&worker_id) {
                Some(PortRef::AgentPort(id, port))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Verifies C1 (Complete agent coverage, SPEC-04 R6): every agent of the
/// original net is in exactly one partition, and no agent appears in more
/// than one partition (SPEC-04 Section 4.8).
#[cfg(debug_assertions)]
fn assert_c1_coverage(original: &Net, partitions: &[Partition]) {
    let mut seen: HashSet<AgentId> = HashSet::new();
    let mut total = 0usize;
    for partition in partitions {
        for (i, slot) in partition.subnet.agents.iter().enumerate() {
            if slot.is_some() {
                let id = i as AgentId;
                assert!(
                    seen.insert(id),
                    "C1 violated: agent {} appears in more than one partition",
                    id
                );
                total += 1;
            }
        }
    }
    let original_count = original.agents.iter().filter(|s| s.is_some()).count();
    assert_eq!(
        total, original_count,
        "C1 violated: {} agents in partitions, {} in original net",
        total, original_count
    );
}

/// Verifies C3 (FreePort bijectivity, SPEC-04 R8): each borderId appears
/// in exactly two distinct partitions (SPEC-04 Section 4.8).
#[cfg(debug_assertions)]
fn assert_c3_border_consistency(
    partitions: &[Partition],
    borders: &HashMap<u32, (PortRef, PortRef)>,
) {
    for &border_id in borders.keys() {
        let mut found_in: Vec<WorkerId> = Vec::new();
        for partition in partitions {
            if partition.free_port_index.contains_key(&border_id) {
                found_in.push(partition.worker_id);
            }
        }
        assert_eq!(
            found_in.len(),
            2,
            "C3 violated: borderId {} found in {} partitions (expected: 2)",
            border_id,
            found_in.len()
        );
        assert_ne!(
            found_in[0], found_in[1],
            "C3 violated: borderId {} found twice in the same partition {}",
            border_id, found_in[0]
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::{PortRef, Symbol};
    use crate::partition::strategy::ContiguousIdStrategy;

    // T1: split with n=1 returns single partition
    #[test]
    fn test_split_trivial_single_partition() {
        let net = Net::new();
        let plan = split(net, 1, &ContiguousIdStrategy);
        assert_eq!(plan.partitions.len(), 1);
        assert!(plan.borders.is_empty());
    }

    // T2: Trivial split preserves all agents
    #[test]
    fn test_split_trivial_preserves_agents() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(3));

        let plan = split(net, 1, &ContiguousIdStrategy);
        assert_eq!(plan.partitions[0].subnet.count_live_agents(), 2);
    }

    // T3: Trivial split has worker_id = 0
    #[test]
    fn test_split_trivial_worker_id() {
        let net = Net::new();
        let plan = split(net, 1, &ContiguousIdStrategy);
        assert_eq!(plan.partitions[0].worker_id, 0);
    }

    // T4: Trivial split has full ID range
    #[test]
    fn test_split_trivial_full_id_range() {
        let net = Net::new();
        let plan = split(net, 1, &ContiguousIdStrategy);
        assert_eq!(plan.partitions[0].id_range.start, 0);
        assert_eq!(plan.partitions[0].id_range.end, u32::MAX);
    }

    // T5: Trivial split has no borders
    #[test]
    fn test_split_trivial_no_borders() {
        let net = Net::new();
        let plan = split(net, 1, &ContiguousIdStrategy);
        assert!(plan.partitions[0].free_port_index.is_empty());
        assert_eq!(plan.partitions[0].border_id_start, 0);
        assert_eq!(plan.partitions[0].border_id_end, 0);
    }

    // T6: Trivial split preserves redex queue
    #[test]
    fn test_split_trivial_preserves_redexes() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        let plan = split(net, 1, &ContiguousIdStrategy);
        assert!(!plan.partitions[0].subnet.redex_queue.is_empty());
    }

    // E1: Empty net trivial split
    #[test]
    fn test_split_trivial_empty_net() {
        let net = Net::new();
        let plan = split(net, 1, &ContiguousIdStrategy);
        assert_eq!(plan.partitions[0].subnet.count_live_agents(), 0);
    }

    // -----------------------------------------------------------------------
    // General split tests (n > 1)
    // -----------------------------------------------------------------------

    // G1: 2 agents, 2 workers -> 1 agent per partition
    #[test]
    fn test_split_two_agents_two_workers() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        let plan = split(net, 2, &ContiguousIdStrategy);
        assert_eq!(plan.partitions.len(), 2);

        // Total agents across partitions = 2
        let total: usize = plan
            .partitions
            .iter()
            .map(|p| p.subnet.count_live_agents())
            .sum();
        assert_eq!(total, 2);
    }

    // G2: Border wire creates border map entry
    #[test]
    fn test_split_border_map() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        let plan = split(net, 2, &ContiguousIdStrategy);
        // a-b cross partitions, so 1 border
        assert_eq!(plan.borders.len(), 1);
    }

    // G3: FreePort index populated for border wires
    #[test]
    fn test_split_free_port_index() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        let plan = split(net, 2, &ContiguousIdStrategy);
        // Each partition should have 1 entry in free_port_index
        let total_fpi: usize = plan
            .partitions
            .iter()
            .map(|p| p.free_port_index.len())
            .sum();
        assert_eq!(total_fpi, 2); // one per side of the border
    }

    // G4: ID ranges are disjoint
    #[test]
    fn test_split_id_ranges_disjoint() {
        let mut net = Net::new();
        for _ in 0..4 {
            net.create_agent(Symbol::Era);
        }
        // Connect pairs to keep invariants simple
        let plan = split(net, 2, &ContiguousIdStrategy);

        assert_eq!(
            plan.partitions[0].id_range.end,
            plan.partitions[1].id_range.start
        );
    }

    // G5: Internal redex stays in partition, border redex does not
    #[test]
    fn test_split_redex_filtering() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        let c = net.create_agent(Symbol::Era);
        let d = net.create_agent(Symbol::Era);
        // a-b: will be internal to worker 0 (IDs 0,1 with 4 agents / 2 workers = 2 per)
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        // c-d: will be internal to worker 1
        net.connect(PortRef::AgentPort(c, 0), PortRef::AgentPort(d, 0));

        let plan = split(net, 2, &ContiguousIdStrategy);

        // Each partition should have 1 redex (internal pair)
        assert_eq!(plan.partitions[0].subnet.redex_queue.len(), 1);
        assert_eq!(plan.partitions[1].subnet.redex_queue.len(), 1);
    }

    // G6: C1 — every agent in exactly one partition
    #[test]
    fn test_split_c1_coverage() {
        let mut net = Net::new();
        let ids: Vec<AgentId> = (0..6).map(|_| net.create_agent(Symbol::Era)).collect();
        // Connect pairs
        for chunk in ids.chunks(2) {
            net.connect(
                PortRef::AgentPort(chunk[0], 0),
                PortRef::AgentPort(chunk[1], 0),
            );
        }

        let plan = split(net, 3, &ContiguousIdStrategy);
        let total: usize = plan
            .partitions
            .iter()
            .map(|p| p.subnet.count_live_agents())
            .sum();
        assert_eq!(total, 6);
    }

    // G7: Worker IDs are sequential 0..n
    #[test]
    fn test_split_worker_ids_sequential() {
        let mut net = Net::new();
        for _ in 0..4 {
            net.create_agent(Symbol::Era);
        }
        let plan = split(net, 4, &ContiguousIdStrategy);
        for (i, p) in plan.partitions.iter().enumerate() {
            assert_eq!(p.worker_id, i as u32);
        }
    }

    // G8: Root port propagation — root goes to correct partition
    #[test]
    fn test_split_root_propagation() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(3));
        net.root = Some(PortRef::AgentPort(a, 0));

        let plan = split(net, 2, &ContiguousIdStrategy);

        // Exactly one partition has the root
        let roots: Vec<_> = plan
            .partitions
            .iter()
            .filter(|p| p.subnet.root.is_some())
            .collect();
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].subnet.root, Some(PortRef::AgentPort(a, 0)));
    }

    // G9: Empty net with n > 1
    #[test]
    fn test_split_empty_net_multi_workers() {
        let net = Net::new();
        let plan = split(net, 4, &ContiguousIdStrategy);
        assert_eq!(plan.partitions.len(), 4);
        assert!(plan.borders.is_empty());
        for p in &plan.partitions {
            assert_eq!(p.subnet.count_live_agents(), 0);
        }
    }

    // G10: More workers than agents
    #[test]
    fn test_split_more_workers_than_agents() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(0));

        let plan = split(net, 4, &ContiguousIdStrategy);
        assert_eq!(plan.partitions.len(), 4);
        // Only 1 agent total
        let total: usize = plan
            .partitions
            .iter()
            .map(|p| p.subnet.count_live_agents())
            .sum();
        assert_eq!(total, 1);
    }
}
