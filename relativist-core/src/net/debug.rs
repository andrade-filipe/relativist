//! Debug assertions for Net invariants (SPEC-01).
//!
//! Runtime checks for T1 (linearity), I1 (bidirectionality),
//! I2 (reference validity), I3 (ID monotonicity), I6 (ERA slots),
//! I7 (root consistency). Active only in debug mode.

#[cfg(debug_assertions)]
use super::core::Net;
#[cfg(debug_assertions)]
use super::types::{arity, port_index, total_ports, AgentId, PortRef, Symbol, DISCONNECTED};

#[cfg(debug_assertions)]
impl Net {
    /// Verifies I1/T1: bidirectionality of the port array.
    ///
    /// R18a: The root agent's principal port is permanently DISCONNECTED
    /// and is exempt from T1. FreePort targets skip the bidirectional check
    /// (no slot in port array; BorderMap handles reverse lookup).
    pub fn assert_adjacency_consistent(&self) {
        let root_agent_id: Option<AgentId> = match self.root {
            Some(PortRef::AgentPort(id, 0)) => Some(id),
            _ => None,
        };

        for agent in self.agents.iter().flatten() {
            let num_ports = total_ports(agent.symbol);
            for p in 0..num_ports {
                let port = PortRef::AgentPort(agent.id, p);
                let target = self.get_target(port);

                // R18a: root agent's principal port is exempt
                if p == 0 && root_agent_id == Some(agent.id) && target == DISCONNECTED {
                    continue;
                }

                assert_ne!(
                    target, DISCONNECTED,
                    "T1 violated: port {:?} is DISCONNECTED (not root principal)",
                    port
                );

                // FreePort targets: no slot in port array, skip bidirectional check
                if matches!(target, PortRef::FreePort(_)) {
                    continue;
                }

                // Bidirectional check for AgentPort targets
                let reverse = self.get_target(target);
                assert_eq!(
                    reverse, port,
                    "I1 violated: {:?} -> {:?}, but {:?} -> {:?}",
                    port, target, target, reverse
                );
            }
        }
    }

    /// Verifies I2: all PortRef values in the port array reference
    /// existing agents with valid port indices.
    pub fn assert_refs_valid(&self) {
        for agent in self.agents.iter().flatten() {
            let num_ports = total_ports(agent.symbol);
            for p in 0..num_ports {
                let target = self.get_target(PortRef::AgentPort(agent.id, p));
                if let PortRef::AgentPort(tid, tp) = target {
                    let t_agent = self.get_agent(tid);
                    assert!(
                        t_agent.is_some(),
                        "I2 violated: port {:?} references nonexistent agent {}",
                        PortRef::AgentPort(agent.id, p),
                        tid
                    );
                    let t_arity = arity(t_agent.unwrap().symbol);
                    assert!(
                        tp <= t_arity,
                        "I2 violated: port {:?} references invalid port {} on {:?} (arity {})",
                        PortRef::AgentPort(agent.id, p),
                        tp,
                        t_agent.unwrap().symbol,
                        t_arity
                    );
                }
            }
        }
    }

    /// Verifies I3: `next_id` > max AgentId in use.
    pub fn assert_next_id_valid(&self) {
        for (idx, slot) in self.agents.iter().enumerate() {
            if slot.is_some() {
                assert!(
                    self.next_id > idx as u32,
                    "I3 violated: next_id ({}) <= live agent id ({})",
                    self.next_id,
                    idx
                );
            }
        }
    }

    /// Verifies I6: ERA agents' unused auxiliary slots (ports 1, 2)
    /// contain DISCONNECTED.
    pub fn assert_era_unused_ports_clean(&self) {
        for agent in self.agents.iter().flatten() {
            if agent.symbol == Symbol::Era {
                for p in 1..3u8 {
                    let idx = port_index(agent.id, p);
                    if idx < self.ports.len() {
                        assert_eq!(
                            self.ports[idx], DISCONNECTED,
                            "I6 violated: ERA agent {} has non-DISCONNECTED at auxiliary slot {}",
                            agent.id, p
                        );
                    }
                }
            }
        }
    }

    /// Verifies I7/R6a: root port consistency.
    ///
    /// Root MUST be `None` or `Some(AgentPort(id, 0))` where `id` is live.
    /// FreePort roots are rejected (R6a).
    /// Root port slot MUST contain DISCONNECTED (R18a).
    pub fn assert_root_consistent(&self) {
        if let Some(root_ref) = self.root {
            match root_ref {
                PortRef::AgentPort(id, p) => {
                    assert_eq!(
                        p, 0,
                        "R6a violated: root must be AgentPort(id, 0), got port {}",
                        p
                    );
                    assert!(
                        self.get_agent(id).is_some(),
                        "R6a violated: root references nonexistent agent {}",
                        id
                    );
                    let idx = port_index(id, 0);
                    assert_eq!(
                        self.ports[idx], DISCONNECTED,
                        "R18a violated: root port {:?} is not DISCONNECTED",
                        root_ref
                    );
                }
                PortRef::FreePort(_) => {
                    panic!(
                        "R6a violated: root is FreePort, must be None or Some(AgentPort(id, 0))"
                    );
                }
            }
        }
    }

    /// Counts stale redex entries in the queue (diagnostic).
    pub fn count_stale_redexes(&self) -> usize {
        self.redex_queue
            .iter()
            .filter(|(a, b)| !self.is_valid_redex(*a, *b))
            .count()
    }

    /// Runs all invariant checks: I1, I2, I3, I6, I7.
    pub fn assert_all_invariants(&self) {
        self.assert_adjacency_consistent();
        self.assert_refs_valid();
        self.assert_next_id_valid();
        self.assert_era_unused_ports_clean();
        self.assert_root_consistent();
    }
}

#[cfg(all(test, debug_assertions))]
mod tests {
    use super::super::core::Net;
    use super::super::types::*;

    // T1: Valid net passes all invariants
    #[test]
    fn test_valid_net_passes_invariants() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 1));
        net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(b, 2));
        net.assert_all_invariants();
    }

    // T2: Valid net with root passes invariants
    #[test]
    fn test_valid_net_with_root() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.root = Some(PortRef::AgentPort(a, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 1));
        net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(b, 2));
        net.connect(PortRef::AgentPort(b, 0), PortRef::FreePort(0));
        net.assert_all_invariants();
    }

    // T3: Corrupted bidirectionality panics
    #[test]
    #[should_panic(expected = "I1 violated")]
    fn test_adjacency_inconsistent_panics() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 1));
        net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(b, 2));
        let idx = port_index(b, 0);
        net.ports[idx] = PortRef::AgentPort(a, 1);
        net.assert_adjacency_consistent();
    }

    // T4: next_id too low panics
    #[test]
    #[should_panic(expected = "I3 violated")]
    fn test_next_id_invalid_panics() {
        let mut net = Net::new();
        net.create_agent(Symbol::Con);
        net.next_id = 0;
        net.assert_next_id_valid();
    }

    // T5: ERA with non-DISCONNECTED auxiliary panics
    #[test]
    #[should_panic(expected = "I6 violated")]
    fn test_era_dirty_auxiliary_panics() {
        let mut net = Net::new();
        let e = net.create_agent(Symbol::Era);
        let idx = port_index(e, 1);
        net.ports[idx] = PortRef::AgentPort(0, 0);
        net.assert_era_unused_ports_clean();
    }

    // T6: FreePort root panics (R6a)
    #[test]
    #[should_panic(expected = "R6a violated")]
    fn test_root_freeport_panics() {
        let mut net = Net::new();
        net.root = Some(PortRef::FreePort(0));
        net.assert_root_consistent();
    }

    // T7: Root on non-principal port panics (R6a)
    #[test]
    #[should_panic(expected = "R6a violated")]
    fn test_root_non_principal_panics() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        net.root = Some(PortRef::AgentPort(a, 1));
        net.assert_root_consistent();
    }

    // T8: Root on dead agent panics (R6a)
    #[test]
    #[should_panic(expected = "R6a violated")]
    fn test_root_dead_agent_panics() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        net.remove_agent(a);
        net.root = Some(PortRef::AgentPort(a, 0));
        net.assert_root_consistent();
    }

    // T9: Root port not DISCONNECTED panics (R18a)
    #[test]
    #[should_panic(expected = "R18a violated")]
    fn test_root_port_not_disconnected_panics() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.root = Some(PortRef::AgentPort(a, 0));
        net.assert_root_consistent();
    }

    // T10: DISCONNECTED at non-root port panics (T1)
    #[test]
    #[should_panic(expected = "T1 violated")]
    fn test_disconnected_non_root_panics() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 1));
        // a:2 and b:2 still DISCONNECTED — triggers T1
        net.assert_adjacency_consistent();
    }

    // T11: Cross-port self-loop passes (Church numeral pattern)
    #[test]
    fn test_cross_port_self_loop_passes() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(b, 1), PortRef::AgentPort(b, 2));
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
        net.assert_adjacency_consistent();
    }

    // T12: count_stale_redexes
    #[test]
    fn test_count_stale_redexes() {
        let mut net = Net::new();
        assert_eq!(net.count_stale_redexes(), 0);
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        assert_eq!(net.count_stale_redexes(), 0);
        net.remove_agent(a);
        assert_eq!(net.count_stale_redexes(), 1);
    }
}
