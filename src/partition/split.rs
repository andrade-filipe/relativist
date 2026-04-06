//! The split function: decomposes a Net into partitions (SPEC-04 Section 4.5).

use std::collections::HashMap;

use crate::net::Net;

use super::strategy::PartitionStrategy;
use super::types::{IdRange, Partition, PartitionPlan};

/// Splits a net into `num_workers` partitions using the given strategy.
///
/// If `num_workers <= 1`, returns the entire net as a single partition
/// with no borders (trivial case, SPEC-04 R2).
///
/// Panics if `num_workers == 0`.
pub fn split(net: Net, num_workers: u32, _strategy: &dyn PartitionStrategy) -> PartitionPlan {
    assert!(num_workers >= 1, "num_workers must be >= 1");

    if num_workers <= 1 {
        return trivial_plan(net);
    }

    // General case (TASK-0049): will use _strategy.allocate()
    todo!("General split case for num_workers > 1")
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
}
