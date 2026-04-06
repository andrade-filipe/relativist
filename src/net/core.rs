//! Net struct and operations.
//!
//! The complete interaction net data structure with agent arena,
//! port array, redex queue, and all CRUD operations.

use std::collections::VecDeque;

use super::types::{Agent, AgentId, PortRef, Symbol, DISCONNECTED, PORTS_PER_SLOT};

/// The complete interaction net.
///
/// Formally, a Net is a pair (A, W) where A is the set of agents and W
/// is the set of wires (DISC-004 v2, Section 1.1; REF-013, p.219).
///
/// Agents are stored in an arena indexed by AgentId.
/// Connections are represented implicitly by a flat port array.
/// The redex queue maintains known active pairs for incremental reduction.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Net {
    /// Agent arena. `agents[id] == Some(agent)` if the agent is live.
    /// `agents[id] == None` if the slot is free (removed or never created).
    pub agents: Vec<Option<Agent>>,

    /// Flat port array. The slot for port `(id, port_id)` is at index
    /// `id * PORTS_PER_SLOT + port_id`. Each slot stores the `PortRef` to
    /// which the port is connected.
    pub ports: Vec<PortRef>,

    /// Queue of active pairs (redexes) for incremental reduction.
    /// May contain stale entries; the reduction engine verifies validity
    /// before reducing (SPEC-02 R17, SPEC-01 I4).
    pub redex_queue: VecDeque<(AgentId, AgentId)>,

    /// Next AgentId to be assigned. Strictly greater than any AgentId
    /// in use. Incremented on each agent creation (SPEC-01 I3).
    pub next_id: AgentId,

    /// Root port: the AgentPort connected to the external observation point.
    /// `None` if the net has no root (e.g., a partition sub-net).
    /// Constrained to `None` or `Some(AgentPort(id, 0))` where `id` is a
    /// live agent (R6a). `FreePort` values are NOT valid for root.
    pub root: Option<PortRef>,
}

impl Default for Net {
    fn default() -> Self {
        Self::new()
    }
}

impl Net {
    /// Creates an empty Net with no agents, wires, or redexes.
    pub fn new() -> Self {
        Self {
            agents: Vec::new(),
            ports: Vec::new(),
            redex_queue: VecDeque::new(),
            next_id: 0,
            root: None,
        }
    }

    /// Creates a Net with pre-allocated capacity for `capacity` agents.
    ///
    /// Pre-allocates the agent arena for `capacity` slots and the port
    /// array for `capacity * PORTS_PER_SLOT` slots.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            agents: Vec::with_capacity(capacity),
            ports: Vec::with_capacity(capacity * PORTS_PER_SLOT),
            redex_queue: VecDeque::new(),
            next_id: 0,
            root: None,
        }
    }

    /// Creates a new agent with the given symbol and returns its assigned ID.
    ///
    /// The agent gets `next_id` as its ID, and `next_id` is incremented.
    /// The agent arena and port array are expanded as needed. All new port
    /// slots are initialized to `DISCONNECTED`.
    ///
    /// Complexity: O(1) amortized (may trigger Vec reallocation).
    /// Postcondition: `agents[id] == Some(Agent { symbol, id })`, `next_id == id + 1`.
    pub fn create_agent(&mut self, symbol: Symbol) -> AgentId {
        let id = self.next_id;
        self.next_id += 1;

        let agent = Agent { symbol, id };

        // Expand arena to contain index `id`
        if self.agents.len() <= id as usize {
            self.agents.resize((id as usize) + 1, None);
        }
        self.agents[id as usize] = Some(agent);

        // Expand port array for the new agent's 3 slots
        let required_len = (id as usize + 1) * PORTS_PER_SLOT;
        if self.ports.len() < required_len {
            self.ports.resize(required_len, DISCONNECTED);
        }

        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // T1: Net::new() returns empty net
    #[test]
    fn test_net_new_empty() {
        let net = Net::new();
        assert!(net.agents.is_empty());
        assert!(net.ports.is_empty());
        assert!(net.redex_queue.is_empty());
        assert_eq!(net.next_id, 0);
        assert_eq!(net.root, None);
    }

    // T2: Net::with_capacity pre-allocates
    #[test]
    fn test_net_with_capacity() {
        let net = Net::with_capacity(100);
        assert!(net.agents.capacity() >= 100);
        assert!(net.ports.capacity() >= 300); // 100 * PORTS_PER_SLOT
        assert!(net.agents.is_empty());
        assert!(net.ports.is_empty());
        assert!(net.redex_queue.is_empty());
        assert_eq!(net.next_id, 0);
        assert_eq!(net.root, None);
    }

    // T3: Net implements Clone
    #[test]
    fn test_net_clone() {
        let net = Net::new();
        let net2 = net.clone();
        assert_eq!(net, net2);
    }

    // T4: Net implements PartialEq and Eq (R26a)
    #[test]
    fn test_net_equality() {
        let a = Net::new();
        let b = Net::new();
        assert_eq!(a, b);
    }

    // T5: Net implements Debug
    #[test]
    fn test_net_debug() {
        let net = Net::new();
        let debug_str = format!("{:?}", net);
        assert!(debug_str.contains("Net"));
    }

    // T6: Net serde round-trip
    #[test]
    fn test_net_serde_roundtrip() {
        let net = Net::new();
        let bytes = bincode::serialize(&net).unwrap();
        let des: Net = bincode::deserialize(&bytes).unwrap();
        assert_eq!(net, des);
    }

    // T7: Net::with_capacity(0) works like new()
    #[test]
    fn test_net_with_capacity_zero() {
        let net = Net::with_capacity(0);
        assert!(net.agents.is_empty());
        assert!(net.ports.is_empty());
        assert_eq!(net.next_id, 0);
        assert_eq!(net.root, None);
    }

    // E1: Net::with_capacity large value
    #[test]
    fn test_net_with_capacity_large() {
        let net = Net::with_capacity(10_000);
        assert!(net.agents.capacity() >= 10_000);
        assert!(net.ports.capacity() >= 30_000);
    }

    // E3: Cloned net is independent
    #[test]
    fn test_net_clone_independence() {
        let mut net = Net::new();
        let clone = net.clone();
        net.next_id = 42;
        assert_ne!(net, clone);
    }

    // --- create_agent tests (TASK-0009) ---

    // T1: Create one agent, verify id and state
    #[test]
    fn test_create_agent_first() {
        let mut net = Net::new();
        let id = net.create_agent(Symbol::Con);
        assert_eq!(id, 0);
        assert_eq!(net.next_id, 1);
        assert_eq!(
            net.agents[0],
            Some(Agent {
                symbol: Symbol::Con,
                id: 0
            })
        );
    }

    // T2: Create 3 agents sequentially
    #[test]
    fn test_create_agent_sequential() {
        let mut net = Net::new();
        let id0 = net.create_agent(Symbol::Con);
        let id1 = net.create_agent(Symbol::Dup);
        let id2 = net.create_agent(Symbol::Era);
        assert_eq!(id0, 0);
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(net.next_id, 3);
    }

    // T3: Port array expands correctly
    #[test]
    fn test_create_agent_port_array_size() {
        let mut net = Net::new();
        net.create_agent(Symbol::Con);
        net.create_agent(Symbol::Dup);
        net.create_agent(Symbol::Era);
        // 3 agents * 3 slots = 9
        assert!(net.ports.len() >= 9);
    }

    // T4: New port slots are DISCONNECTED
    #[test]
    fn test_create_agent_ports_disconnected() {
        use crate::net::types::{port_index, DISCONNECTED};
        let mut net = Net::new();
        let id = net.create_agent(Symbol::Con);
        for p in 0..3u8 {
            assert_eq!(net.ports[port_index(id, p)], DISCONNECTED);
        }
    }

    // E4: ERA also gets 3 port slots (uniform layout)
    #[test]
    fn test_create_agent_era_uniform_slots() {
        use crate::net::types::{port_index, DISCONNECTED};
        let mut net = Net::new();
        let id = net.create_agent(Symbol::Era);
        // ERA has arity 0 but still gets 3 slots
        for p in 0..3u8 {
            assert_eq!(net.ports[port_index(id, p)], DISCONNECTED);
        }
    }

    // E5: Agent symbols are stored correctly
    #[test]
    fn test_create_agent_symbols() {
        let mut net = Net::new();
        net.create_agent(Symbol::Con);
        net.create_agent(Symbol::Dup);
        net.create_agent(Symbol::Era);
        assert_eq!(net.agents[0].unwrap().symbol, Symbol::Con);
        assert_eq!(net.agents[1].unwrap().symbol, Symbol::Dup);
        assert_eq!(net.agents[2].unwrap().symbol, Symbol::Era);
    }
}
