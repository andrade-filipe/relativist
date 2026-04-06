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

/// Partitioning strategy by contiguous ID ranges (SPEC-04 Section 4.3).
///
/// Live agents are sorted by AgentId in ascending order and divided
/// into chunks of approximately equal size. Worker 0 receives the
/// first ceil(|A|/n) agents, Worker 1 the next, and so on.
///
/// Properties:
/// - O(A log A) time (sort + traverse).
/// - Deterministic: same net + same n = same result.
/// - Ignores graph topology (DISC-004 v2, Perspective 1).
/// - Same strategy as the Haskell prototype (AC-002, partitionNet).
pub struct ContiguousIdStrategy;

impl PartitionStrategy for ContiguousIdStrategy {
    fn allocate(&self, net: &Net, num_workers: u32) -> HashMap<AgentId, WorkerId> {
        // 1. Collect IDs of all live agents, sorted ascending.
        let mut live_ids: Vec<AgentId> = net
            .agents
            .iter()
            .enumerate()
            .filter_map(|(i, slot)| slot.as_ref().map(|_| i as AgentId))
            .collect();
        live_ids.sort_unstable();

        let total = live_ids.len();
        if total == 0 {
            return HashMap::new();
        }

        // 2. Divide into chunks of size ceil(total / num_workers).
        let n = num_workers as usize;
        let chunk_size = total.div_ceil(n);

        // 3. Assign each chunk to a worker sequentially.
        let mut map = HashMap::with_capacity(total);
        for (i, &agent_id) in live_ids.iter().enumerate() {
            let worker = (i / chunk_size) as WorkerId;
            map.insert(agent_id, worker);
        }

        map
    }
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

    // -----------------------------------------------------------------------
    // ContiguousIdStrategy tests
    // -----------------------------------------------------------------------

    // C1: Empty net
    #[test]
    fn test_contiguous_empty_net() {
        let strategy = ContiguousIdStrategy;
        let net = Net::new();
        let map = strategy.allocate(&net, 4);
        assert!(map.is_empty());
    }

    // C2: Single agent, 1 worker
    #[test]
    fn test_contiguous_single_agent_one_worker() {
        let strategy = ContiguousIdStrategy;
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let map = strategy.allocate(&net, 1);
        assert_eq!(map.len(), 1);
        assert_eq!(map[&a], 0);
    }

    // C3: 4 agents, 2 workers -> 2 per worker
    #[test]
    fn test_contiguous_even_split() {
        let strategy = ContiguousIdStrategy;
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con); // id=0
        let b = net.create_agent(Symbol::Dup); // id=1
        let c = net.create_agent(Symbol::Era); // id=2
        let d = net.create_agent(Symbol::Con); // id=3

        let map = strategy.allocate(&net, 2);
        assert_eq!(map.len(), 4);
        assert_eq!(map[&a], 0);
        assert_eq!(map[&b], 0);
        assert_eq!(map[&c], 1);
        assert_eq!(map[&d], 1);
    }

    // C4: 3 agents, 2 workers -> ceil split (2+1)
    #[test]
    fn test_contiguous_uneven_split() {
        let strategy = ContiguousIdStrategy;
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);
        let c = net.create_agent(Symbol::Era);

        let map = strategy.allocate(&net, 2);
        assert_eq!(map.len(), 3);
        // chunk_size = ceil(3/2) = 2
        assert_eq!(map[&a], 0);
        assert_eq!(map[&b], 0);
        assert_eq!(map[&c], 1);
    }

    // C5: More workers than agents -> some workers get 0 agents
    #[test]
    fn test_contiguous_more_workers_than_agents() {
        let strategy = ContiguousIdStrategy;
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);

        let map = strategy.allocate(&net, 8);
        assert_eq!(map.len(), 2);
        // All WorkerIds in range [0, 8)
        for &wid in map.values() {
            assert!(wid < 8);
        }
    }

    // C6: Deterministic -- same result on repeated calls
    #[test]
    fn test_contiguous_deterministic() {
        let strategy = ContiguousIdStrategy;
        let mut net = Net::new();
        for _ in 0..10 {
            net.create_agent(Symbol::Con);
        }
        let map1 = strategy.allocate(&net, 3);
        let map2 = strategy.allocate(&net, 3);
        assert_eq!(map1, map2);
    }

    // C7: All WorkerIds in [0, num_workers) -- post-condition
    #[test]
    fn test_contiguous_worker_ids_in_range() {
        let strategy = ContiguousIdStrategy;
        let mut net = Net::new();
        for _ in 0..20 {
            net.create_agent(Symbol::Dup);
        }
        let map = strategy.allocate(&net, 4);
        for &wid in map.values() {
            assert!(wid < 4);
        }
    }

    // C8: Every live agent covered -- post-condition
    #[test]
    fn test_contiguous_covers_all_live() {
        let strategy = ContiguousIdStrategy;
        let mut net = Net::new();
        let ids: Vec<AgentId> = (0..5).map(|_| net.create_agent(Symbol::Era)).collect();
        net.remove_agent(ids[2]); // remove one

        let map = strategy.allocate(&net, 2);
        assert_eq!(map.len(), 4); // 5 - 1 removed
        for &id in &ids {
            if id != ids[2] {
                assert!(map.contains_key(&id));
            }
        }
        assert!(!map.contains_key(&ids[2]));
    }

    // C9: Object-safe with ContiguousIdStrategy
    #[test]
    fn test_contiguous_as_dyn() {
        let strategy: Box<dyn PartitionStrategy> = Box::new(ContiguousIdStrategy);
        let mut net = Net::new();
        net.create_agent(Symbol::Con);
        let map = strategy.allocate(&net, 1);
        assert_eq!(map.len(), 1);
    }
}
