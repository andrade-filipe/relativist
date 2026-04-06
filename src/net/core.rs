//! Net struct and operations.
//!
//! The complete interaction net data structure with agent arena,
//! port array, redex queue, and all CRUD operations.

use std::collections::VecDeque;

use super::types::{total_ports, Agent, AgentId, PortRef, Symbol, DISCONNECTED, PORTS_PER_SLOT};

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

    /// Returns the `PortRef` to which the given port is connected.
    ///
    /// For `AgentPort(id, p)`: looks up `ports[port_index(id, p)]`.
    /// Returns `DISCONNECTED` if the index is out of bounds.
    /// For `FreePort(_)`: returns `DISCONNECTED` (FreePort targets are
    /// resolved during merge, SPEC-05).
    ///
    /// Complexity: O(1).
    pub fn get_target(&self, port: PortRef) -> PortRef {
        match port {
            PortRef::AgentPort(id, p) => {
                let idx = super::types::port_index(id, p);
                if idx < self.ports.len() {
                    self.ports[idx]
                } else {
                    DISCONNECTED
                }
            }
            PortRef::FreePort(_) => DISCONNECTED,
        }
    }

    /// Writes the target of a port in the port array.
    ///
    /// Only operates on `AgentPort`; `FreePort` is a no-op (FreePort has
    /// no slot in the port array). Silently ignores out-of-bounds indices.
    ///
    /// This is intentionally private — external code should use `connect`
    /// and `disconnect` to maintain bidirectionality (SPEC-01 T1/I1).
    fn set_port(&mut self, port: PortRef, target: PortRef) {
        if let PortRef::AgentPort(id, p) = port {
            let idx = super::types::port_index(id, p);
            if idx < self.ports.len() {
                self.ports[idx] = target;
            }
        }
    }

    /// Establishes a bidirectional connection between two ports.
    ///
    /// Writes both directions in the port array: `a -> b` and `b -> a`.
    /// If both are principal ports (`AgentPort(_, 0)`), inserts the pair
    /// into the redex queue for incremental reduction (SPEC-02 R9, R13).
    ///
    /// Self-loop policy (R18b): intra-agent connections (different ports
    /// of the same agent) are valid. Same-port self-connections are
    /// rejected by a debug assertion.
    ///
    /// Complexity: O(1).
    /// Postcondition: `get_target(a) == b && get_target(b) == a`.
    pub fn connect(&mut self, a: PortRef, b: PortRef) {
        debug_assert_ne!(a, b, "Same-port self-connection is invalid: {:?}", a);

        self.set_port(a, b);
        self.set_port(b, a);

        // Incremental redex detection: if both are principal ports,
        // an active pair is formed.
        if let (PortRef::AgentPort(id_a, 0), PortRef::AgentPort(id_b, 0)) = (a, b) {
            self.redex_queue.push_back((id_a, id_b));
        }
    }

    /// Removes the bidirectional connection of a port.
    ///
    /// Both the port itself and its former target are set to `DISCONNECTED`.
    /// If the port is already disconnected, this is a no-op.
    /// Does NOT remove stale entries from the redex queue — those are
    /// discarded at dequeue time (SPEC-01 I4, SPEC-02 R17).
    ///
    /// Complexity: O(1).
    pub fn disconnect(&mut self, port: PortRef) {
        let target = self.get_target(port);
        if target != DISCONNECTED {
            self.set_port(target, DISCONNECTED);
        }
        self.set_port(port, DISCONNECTED);
    }

    /// Removes an agent from the net.
    ///
    /// Disconnects all of the agent's ports (based on its symbol's
    /// `total_ports`), then marks the slot as `None`. The `AgentId` is
    /// NOT reused — the slot stays `None` for the rest of the execution.
    /// No-op if the slot is already `None` or out of bounds.
    ///
    /// Does NOT clean up the redex queue — stale entries are detected
    /// at dequeue time (SPEC-02 R17).
    ///
    /// Complexity: O(1) (at most 3 ports to disconnect).
    pub fn remove_agent(&mut self, id: AgentId) {
        let idx = id as usize;
        if idx < self.agents.len() {
            if let Some(agent) = self.agents[idx] {
                let num_ports = total_ports(agent.symbol);
                for p in 0..num_ports {
                    self.disconnect(PortRef::AgentPort(id, p));
                }
                self.agents[idx] = None;
            }
        }
    }

    /// Returns a reference to the agent with the given ID.
    ///
    /// Returns `None` if the ID is out of range or the slot is empty.
    /// This is the canonical accessor for agent lookup (SPEC-02 R15a).
    /// Callers MUST NOT index into `agents` directly for read access.
    ///
    /// Complexity: O(1).
    pub fn get_agent(&self, id: AgentId) -> Option<&Agent> {
        self.agents.get(id as usize).and_then(|slot| slot.as_ref())
    }

    /// Returns a mutable reference to the agent with the given ID.
    ///
    /// Returns `None` if the ID is out of range or the slot is empty.
    ///
    /// Complexity: O(1).
    pub fn get_agent_mut(&mut self, id: AgentId) -> Option<&mut Agent> {
        self.agents
            .get_mut(id as usize)
            .and_then(|slot| slot.as_mut())
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

    // --- get_target / set_port tests (TASK-0010) ---

    // T1: set_port then get_target reads back the value
    #[test]
    fn test_set_port_get_target_roundtrip() {
        let mut net = Net::new();
        let id = net.create_agent(Symbol::Con);
        let target = PortRef::AgentPort(99, 1);
        net.set_port(PortRef::AgentPort(id, 0), target);
        assert_eq!(net.get_target(PortRef::AgentPort(id, 0)), target);
    }

    // T2: get_target out of bounds returns DISCONNECTED
    #[test]
    fn test_get_target_out_of_bounds() {
        let net = Net::new();
        assert_eq!(net.get_target(PortRef::AgentPort(999, 0)), DISCONNECTED);
    }

    // T3: get_target(FreePort) returns DISCONNECTED
    #[test]
    fn test_get_target_freeport() {
        let net = Net::new();
        assert_eq!(net.get_target(PortRef::FreePort(42)), DISCONNECTED);
    }

    // T4: set_port(FreePort) is a no-op
    #[test]
    fn test_set_port_freeport_noop() {
        let mut net = Net::new();
        let id = net.create_agent(Symbol::Con);
        let before = net.clone();
        // This should not panic or change anything meaningful
        net.set_port(PortRef::FreePort(42), PortRef::AgentPort(id, 0));
        // The only thing that could differ is the FreePort slot (which doesn't exist),
        // so the net should still equal before
        assert_eq!(net, before);
    }

    // E6: set_port on out-of-bounds is silent no-op
    #[test]
    fn test_set_port_out_of_bounds_noop() {
        let mut net = Net::new();
        // No agents, so port array is empty
        net.set_port(PortRef::AgentPort(999, 0), PortRef::FreePort(1));
        // Should not panic, net unchanged
        assert!(net.ports.is_empty());
    }

    // E7: get_target on freshly created agent returns DISCONNECTED
    #[test]
    fn test_get_target_fresh_agent() {
        let mut net = Net::new();
        let id = net.create_agent(Symbol::Dup);
        for p in 0..3u8 {
            assert_eq!(net.get_target(PortRef::AgentPort(id, p)), DISCONNECTED);
        }
    }

    // --- connect tests (TASK-0011) ---

    // T1: Bidirectional linkage
    #[test]
    fn test_connect_bidirectional() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 2));
        assert_eq!(
            net.get_target(PortRef::AgentPort(a, 1)),
            PortRef::AgentPort(b, 2)
        );
        assert_eq!(
            net.get_target(PortRef::AgentPort(b, 2)),
            PortRef::AgentPort(a, 1)
        );
    }

    // T2: Principal-principal connection enqueues redex
    #[test]
    fn test_connect_principal_enqueues_redex() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        assert_eq!(net.redex_queue.len(), 1);
        assert_eq!(net.redex_queue[0], (a, b));
    }

    // T3: Principal-auxiliary does NOT enqueue redex
    #[test]
    fn test_connect_principal_auxiliary_no_redex() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 1));
        assert!(net.redex_queue.is_empty());
    }

    // T4: Auxiliary-auxiliary does NOT enqueue redex
    #[test]
    fn test_connect_auxiliary_auxiliary_no_redex() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 2));
        assert!(net.redex_queue.is_empty());
    }

    // T5: Connect AgentPort to FreePort
    #[test]
    fn test_connect_agent_to_freeport() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(42));
        assert_eq!(
            net.get_target(PortRef::AgentPort(a, 0)),
            PortRef::FreePort(42)
        );
        // FreePort side: no slot, get_target returns DISCONNECTED
        assert_eq!(net.get_target(PortRef::FreePort(42)), DISCONNECTED);
        // No redex: FreePort is not AgentPort(_, 0)
        assert!(net.redex_queue.is_empty());
    }

    // T6: Intra-agent connection is valid (R18b)
    #[test]
    fn test_connect_intra_agent() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(a, 2));
        assert_eq!(
            net.get_target(PortRef::AgentPort(a, 1)),
            PortRef::AgentPort(a, 2)
        );
        assert_eq!(
            net.get_target(PortRef::AgentPort(a, 2)),
            PortRef::AgentPort(a, 1)
        );
    }

    // T7: Same-port self-connection panics in debug mode (R18b)
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "Same-port self-connection is invalid")]
    fn test_connect_self_loop_panics() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(a, 1));
    }

    // --- disconnect tests (TASK-0012) ---

    // T1: Disconnect breaks both sides
    #[test]
    fn test_disconnect_both_sides() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 2));
        net.disconnect(PortRef::AgentPort(a, 1));
        assert_eq!(net.get_target(PortRef::AgentPort(a, 1)), DISCONNECTED);
        assert_eq!(net.get_target(PortRef::AgentPort(b, 2)), DISCONNECTED);
    }

    // T2: Disconnect already-disconnected port is no-op
    #[test]
    fn test_disconnect_already_disconnected() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        // Port is already DISCONNECTED after creation
        net.disconnect(PortRef::AgentPort(a, 0)); // no panic
        assert_eq!(net.get_target(PortRef::AgentPort(a, 0)), DISCONNECTED);
    }

    // T3: Disconnect FreePort is no-op
    #[test]
    fn test_disconnect_freeport_noop() {
        let mut net = Net::new();
        net.disconnect(PortRef::FreePort(99)); // no panic
    }

    // E8: Disconnect one side of a connection, other side becomes DISCONNECTED
    #[test]
    fn test_disconnect_from_target_side() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        // Disconnect from b's side
        net.disconnect(PortRef::AgentPort(b, 0));
        assert_eq!(net.get_target(PortRef::AgentPort(a, 0)), DISCONNECTED);
        assert_eq!(net.get_target(PortRef::AgentPort(b, 0)), DISCONNECTED);
    }

    // --- remove_agent tests (TASK-0013) ---

    // T1: Remove CON agent disconnects all 3 ports
    #[test]
    fn test_remove_agent_con() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);
        let c = net.create_agent(Symbol::Con);
        // Wire a's ports to b and c
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(c, 1));
        net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(c, 2));
        net.remove_agent(a);
        assert_eq!(net.agents[a as usize], None);
        // All targets disconnected
        assert_eq!(net.get_target(PortRef::AgentPort(b, 0)), DISCONNECTED);
        assert_eq!(net.get_target(PortRef::AgentPort(c, 1)), DISCONNECTED);
        assert_eq!(net.get_target(PortRef::AgentPort(c, 2)), DISCONNECTED);
    }

    // T2: Remove ERA agent (only 1 port)
    #[test]
    fn test_remove_agent_era() {
        let mut net = Net::new();
        let e = net.create_agent(Symbol::Era);
        let a = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(e, 0), PortRef::AgentPort(a, 0));
        net.remove_agent(e);
        assert_eq!(net.agents[e as usize], None);
        assert_eq!(net.get_target(PortRef::AgentPort(a, 0)), DISCONNECTED);
    }

    // T3: Remove already-removed agent is no-op
    #[test]
    fn test_remove_agent_already_removed() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        net.remove_agent(a);
        net.remove_agent(a); // no panic
        assert_eq!(net.agents[a as usize], None);
    }

    // T4: next_id unchanged after removal
    #[test]
    fn test_remove_agent_next_id_unchanged() {
        let mut net = Net::new();
        net.create_agent(Symbol::Con);
        let next_before = net.next_id;
        net.remove_agent(0);
        assert_eq!(net.next_id, next_before);
    }

    // E9: Remove out-of-bounds id is no-op
    #[test]
    fn test_remove_agent_out_of_bounds() {
        let mut net = Net::new();
        net.remove_agent(999); // no panic
    }

    // --- get_agent / get_agent_mut tests (TASK-0019) ---

    // T1: get_agent on live agent returns Some
    #[test]
    fn test_get_agent_live() {
        let mut net = Net::new();
        let id = net.create_agent(Symbol::Con);
        let agent = net.get_agent(id).unwrap();
        assert_eq!(agent.symbol, Symbol::Con);
        assert_eq!(agent.id, id);
    }

    // T2: get_agent out-of-range returns None
    #[test]
    fn test_get_agent_out_of_range() {
        let net = Net::new();
        assert!(net.get_agent(999).is_none());
    }

    // T3: get_agent on removed agent returns None
    #[test]
    fn test_get_agent_removed() {
        let mut net = Net::new();
        let id = net.create_agent(Symbol::Era);
        net.remove_agent(id);
        assert!(net.get_agent(id).is_none());
    }

    // T4: get_agent on empty net returns None
    #[test]
    fn test_get_agent_empty_net() {
        let net = Net::new();
        assert!(net.get_agent(0).is_none());
    }

    // T5: get_agent_mut allows mutation
    #[test]
    fn test_get_agent_mut_mutation() {
        let mut net = Net::new();
        let id = net.create_agent(Symbol::Con);
        // Mutate the symbol (unusual but tests the API)
        net.get_agent_mut(id).unwrap().symbol = Symbol::Dup;
        assert_eq!(net.get_agent(id).unwrap().symbol, Symbol::Dup);
    }

    // T6: get_agent_mut out-of-range returns None
    #[test]
    fn test_get_agent_mut_out_of_range() {
        let mut net = Net::new();
        assert!(net.get_agent_mut(999).is_none());
    }
}
