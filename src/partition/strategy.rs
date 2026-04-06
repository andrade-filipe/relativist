//! Partition strategies: allocation functions sigma (SPEC-04 Section 4.2).

use std::collections::HashMap;

use crate::net::{AgentId, Net};

use super::types::WorkerId;

/// Trait that abstracts the allocation function sigma: AgentId -> WorkerId.
///
/// Implementations of this trait determine how agents are distributed
/// among workers. The correctness of the system does NOT depend on the
/// strategy chosen (DISC-004 v2, Section 1.6: "the distinction between
/// partitionings is one of quality, not correctness"), but performance
/// depends significantly.
pub trait PartitionStrategy {
    /// Assigns each agent to a worker.
    ///
    /// Input: reference to the net and number of workers.
    /// Output: map of AgentId -> WorkerId for every live agent.
    ///
    /// Post-conditions:
    /// - Every live agent in the net has an entry in the returned map (C1).
    /// - Every WorkerId in the map is in range [0, num_workers).
    fn allocate(&self, net: &Net, num_workers: u32) -> HashMap<AgentId, WorkerId>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::Symbol;

    // A trivial strategy for testing: all agents go to worker 0.
    struct AllToZero;

    impl PartitionStrategy for AllToZero {
        fn allocate(&self, net: &Net, _num_workers: u32) -> HashMap<AgentId, WorkerId> {
            let mut map = HashMap::new();
            for (id, slot) in net.agents.iter().enumerate() {
                if slot.is_some() {
                    map.insert(id as AgentId, 0);
                }
            }
            map
        }
    }

    // T1: PartitionStrategy is object-safe (can be used as dyn trait)
    #[test]
    fn test_partition_strategy_object_safe() {
        let strategy: Box<dyn PartitionStrategy> = Box::new(AllToZero);
        let net = Net::new();
        let result = strategy.allocate(&net, 1);
        assert!(result.is_empty());
    }

    // T2: allocate returns entry for every live agent
    #[test]
    fn test_allocate_covers_all_agents() {
        let strategy = AllToZero;
        let mut net = Net::new();
        let _a = net.create_agent(Symbol::Con);
        let _b = net.create_agent(Symbol::Dup);
        let _c = net.create_agent(Symbol::Era);

        let map = strategy.allocate(&net, 2);
        assert_eq!(map.len(), 3);
    }

    // T3: allocate assigns WorkerIds in range [0, num_workers)
    #[test]
    fn test_allocate_worker_ids_in_range() {
        let strategy = AllToZero;
        let mut net = Net::new();
        net.create_agent(Symbol::Con);

        let map = strategy.allocate(&net, 4);
        for &wid in map.values() {
            assert!(wid < 4);
        }
    }

    // T4: Empty net returns empty map
    #[test]
    fn test_allocate_empty_net() {
        let strategy = AllToZero;
        let net = Net::new();
        let map = strategy.allocate(&net, 8);
        assert!(map.is_empty());
    }

    // T5: Removed agents are not in the map
    #[test]
    fn test_allocate_skips_removed_agents() {
        let strategy = AllToZero;
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.remove_agent(a);

        let map = strategy.allocate(&net, 1);
        assert_eq!(map.len(), 1);
        assert!(map.contains_key(&b));
        assert!(!map.contains_key(&a));
    }
}
