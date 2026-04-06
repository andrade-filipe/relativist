//! Net struct and operations.
//!
//! The complete interaction net data structure with agent arena,
//! port array, redex queue, and all CRUD operations.

use std::collections::VecDeque;

use super::types::{Agent, AgentId, PortRef, PORTS_PER_SLOT};

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
}
