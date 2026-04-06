//! Interaction rules and the link helper.
//!
//! Contains the 4 interaction functions (interact_void, interact_anni,
//! interact_eras, interact_comm) and the safe link procedure.

use crate::net::{AgentId, Net, PortRef};

/// Safe link: wraps `Net::connect` with a guard for removed agents (R25).
///
/// If either endpoint is an `AgentPort` whose agent has been removed
/// (`agents[id]` is `None`), the link is a no-op. This handles the
/// self-referencing auxiliary port edge case in annihilation rules.
///
/// For non-annihilation rules (commutation, erasure), the guard is
/// never triggered because auxiliary ports of the active pair always
/// point to agents outside the pair (or to `FreePort` sentinels).
///
/// **FreePort behavior (R26):** `FreePort` is not considered "removed".
/// When one endpoint is `FreePort(bid)`, `connect` writes `FreePort` to
/// the `AgentPort` side's port array. The `FreePort` side is a no-op
/// in `set_port` (no port array slot). This one-sided write is
/// acceptable: `free_port_index` is reconstructed post-reduction
/// (SPEC-05, Section 4.3).
#[allow(dead_code)] // Called by interact_anni, interact_comm, interact_eras (TASK-0024+)
fn link(net: &mut Net, a: PortRef, b: PortRef) {
    let is_removed = |net: &Net, p: &PortRef| -> bool {
        if let PortRef::AgentPort(id, _) = p {
            net.agents
                .get(*id as usize)
                .is_none_or(|slot| slot.is_none())
        } else {
            false // FreePort is not "removed"; connect handles it
        }
    };
    if is_removed(net, &a) || is_removed(net, &b) {
        return;
    }
    net.connect(a, b);
}

/// Void: two ERA agents annihilate without creating anything (SPEC-03 Section 4.1.3).
///
/// Precondition: both agent IDs MUST refer to live agents
///   (`agents[id].is_some()`) and both MUST be Era. This precondition
///   is guaranteed by `reduce_step`'s validity check (R12).
/// Postcondition: both removed. No agents created, no reconnections.
///
/// Agent balance: -2. Link calls: 0.
/// Complexity: O(1).
///
/// Invariants preserved: T1 (ERA has no auxiliary ports, so removing them
/// leaves no dangling ports), I1/I2 (`remove_agent` cleans up port array slots).
pub fn interact_void(net: &mut Net, a: AgentId, b: AgentId) {
    net.remove_agent(a);
    net.remove_agent(b);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::types::{Symbol, DISCONNECTED};

    // --- Helper ---

    /// Creates a minimal net with two CON agents connected at their principal ports.
    /// Returns (net, a_id, b_id).
    fn two_con_pair() -> (Net, u32, u32) {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        (net, a, b)
    }

    // --- T1: Link two live AgentPorts establishes bidirectional connection ---

    #[test]
    fn test_link_two_live_agent_ports() {
        let (mut net, a, b) = two_con_pair();

        // Link auxiliary ports: a.1 <-> b.1
        link(&mut net, PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 1));

        assert_eq!(
            net.get_target(PortRef::AgentPort(a, 1)),
            PortRef::AgentPort(b, 1)
        );
        assert_eq!(
            net.get_target(PortRef::AgentPort(b, 1)),
            PortRef::AgentPort(a, 1)
        );
    }

    // --- T2: Link where first endpoint is removed agent is a no-op ---

    #[test]
    fn test_link_first_endpoint_removed() {
        let (mut net, a, b) = two_con_pair();
        net.remove_agent(a);

        // a is removed, b is live. link should be a no-op.
        link(&mut net, PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 1));

        // b.1 should remain DISCONNECTED (no connection was made)
        assert_eq!(net.get_target(PortRef::AgentPort(b, 1)), DISCONNECTED);
    }

    // --- T3: Link where second endpoint is removed agent is a no-op ---

    #[test]
    fn test_link_second_endpoint_removed() {
        let (mut net, a, b) = two_con_pair();
        net.remove_agent(b);

        // a is live, b is removed. link should be a no-op.
        link(&mut net, PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 1));

        // a.1 should remain DISCONNECTED
        assert_eq!(net.get_target(PortRef::AgentPort(a, 1)), DISCONNECTED);
    }

    // --- T4: Link where both endpoints are removed agents is a no-op ---

    #[test]
    fn test_link_both_endpoints_removed() {
        let (mut net, a, b) = two_con_pair();
        net.remove_agent(a);
        net.remove_agent(b);

        // Both removed. link should be a no-op (no panic, no mutation).
        link(&mut net, PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 1));

        // Port array slots for removed agents should still be DISCONNECTED
        assert_eq!(net.get_target(PortRef::AgentPort(a, 1)), DISCONNECTED);
        assert_eq!(net.get_target(PortRef::AgentPort(b, 1)), DISCONNECTED);
    }

    // --- T5: Link with FreePort endpoint always proceeds ---

    #[test]
    fn test_link_with_freeport() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);

        // Link a.1 <-> FreePort(0). FreePort is NOT "removed".
        link(&mut net, PortRef::AgentPort(a, 1), PortRef::FreePort(0));

        // AgentPort side should have FreePort(0) written (one-sided write, R26)
        assert_eq!(
            net.get_target(PortRef::AgentPort(a, 1)),
            PortRef::FreePort(0)
        );
    }

    // --- T6: Link between two principal ports detects new redex ---

    #[test]
    fn test_link_principal_ports_detects_redex() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);

        // Clear any existing redex queue entries
        net.redex_queue.clear();

        // Link principal ports: should trigger redex detection
        link(&mut net, PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        assert_eq!(net.redex_queue.len(), 1);
        assert_eq!(net.redex_queue[0], (a, b));
    }

    // --- E1: Link with FreePort on both sides (no panic, connect called) ---

    #[test]
    fn test_link_two_freeports() {
        let mut net = Net::new();

        // Neither FreePort is "removed", so connect is called.
        // set_port is a no-op for both sides. No panic expected.
        link(&mut net, PortRef::FreePort(0), PortRef::FreePort(1));

        // No observable state change (FreePort has no port array slot),
        // but the function should not panic.
    }

    // --- E2: Self-referencing annihilation pattern (integration-level, link) ---

    #[test]
    fn test_self_referencing_annihilation_pattern() {
        // Build: CON(a) <-p0-p0-> CON(b), with a.1<->b.2 and a.2<->b.1
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 2));
        net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(b, 1));

        // Save neighbor PortRefs before removal (as interact_anni would)
        let a1_target = net.get_target(PortRef::AgentPort(a, 1)); // AgentPort(b, 2)
        let a2_target = net.get_target(PortRef::AgentPort(a, 2)); // AgentPort(b, 1)
        let b1_target = net.get_target(PortRef::AgentPort(b, 1)); // AgentPort(a, 2)
        let b2_target = net.get_target(PortRef::AgentPort(b, 2)); // AgentPort(a, 1)

        assert_eq!(a1_target, PortRef::AgentPort(b, 2));
        assert_eq!(a2_target, PortRef::AgentPort(b, 1));
        assert_eq!(b1_target, PortRef::AgentPort(a, 2));
        assert_eq!(b2_target, PortRef::AgentPort(a, 1));

        // Remove both agents (as interact_anni does)
        net.remove_agent(a);
        net.remove_agent(b);

        // Now all saved PortRef values point to removed agents.
        // CON-CON cross pattern: link(a1_target, b2_target), link(a2_target, b1_target)
        // Both calls should be no-ops because all endpoints are removed.
        link(&mut net, a1_target, b2_target);
        link(&mut net, a2_target, b1_target);

        // Verify: no live agents remain
        assert_eq!(net.count_live_agents(), 0);

        // Verify: port array slots for removed agents are DISCONNECTED
        for id in [a, b] {
            for port in 0..3u8 {
                assert_eq!(
                    net.get_target(PortRef::AgentPort(id, port)),
                    DISCONNECTED,
                    "Port ({}, {}) should be DISCONNECTED after self-referencing annihilation",
                    id,
                    port
                );
            }
        }
    }

    // ===================================================================
    // interact_void tests (TASK-0023)
    // ===================================================================

    /// Helper: create two ERA agents connected at their principal ports.
    /// Returns (net, era_a_id, era_b_id).
    fn setup_era_pair() -> (Net, AgentId, AgentId) {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        (net, a, b)
    }

    // T1: Two ERA agents connected at principal ports -- both removed after interact_void
    #[test]
    fn test_interact_void_removes_both_agents() {
        let (mut net, a, b) = setup_era_pair();

        assert!(net.get_agent(a).is_some());
        assert!(net.get_agent(b).is_some());

        interact_void(&mut net, a, b);

        assert!(net.get_agent(a).is_none());
        assert!(net.get_agent(b).is_none());
    }

    // T2: Net agent count decreases by exactly 2
    #[test]
    fn test_interact_void_decreases_agent_count_by_two() {
        let (mut net, a, b) = setup_era_pair();

        let count_before = net.count_live_agents();
        assert_eq!(count_before, 2);

        interact_void(&mut net, a, b);

        let count_after = net.count_live_agents();
        assert_eq!(count_after, 0);
        assert_eq!(count_before - count_after, 2);
    }

    // T3: Ports of removed agents are DISCONNECTED
    #[test]
    fn test_interact_void_ports_are_disconnected() {
        let (mut net, a, b) = setup_era_pair();

        interact_void(&mut net, a, b);

        // Principal ports (port 0) must be DISCONNECTED
        assert_eq!(net.get_target(PortRef::AgentPort(a, 0)), DISCONNECTED);
        assert_eq!(net.get_target(PortRef::AgentPort(b, 0)), DISCONNECTED);

        // Auxiliary slots (ports 1, 2) were never connected and remain DISCONNECTED.
        // ERA has arity 0, but the 3-slot layout means slots 1 and 2 exist.
        assert_eq!(net.get_target(PortRef::AgentPort(a, 1)), DISCONNECTED);
        assert_eq!(net.get_target(PortRef::AgentPort(a, 2)), DISCONNECTED);
        assert_eq!(net.get_target(PortRef::AgentPort(b, 1)), DISCONNECTED);
        assert_eq!(net.get_target(PortRef::AgentPort(b, 2)), DISCONNECTED);
    }

    // T4: Stale redex left in queue after removal
    #[test]
    fn test_interact_void_leaves_stale_redex_in_queue() {
        let (mut net, a, b) = setup_era_pair();

        // connect() pushed a redex (a, b) to the queue
        assert!(!net.redex_queue.is_empty());

        interact_void(&mut net, a, b);

        // interact_void does NOT drain the queue -- stale entry persists
        assert!(!net.redex_queue.is_empty());
        // The stale redex is no longer valid
        assert!(!net.is_valid_redex(a, b));
    }

    // E1: Other agents in the net are unaffected
    #[test]
    fn test_interact_void_does_not_affect_other_agents() {
        let mut net = Net::new();
        let con = net.create_agent(Symbol::Con);
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);

        // Connect CON's principal port to a free port (so it has a defined target)
        net.connect(PortRef::AgentPort(con, 0), PortRef::FreePort(0));
        // Connect the two ERAs at their principal ports
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        assert_eq!(net.count_live_agents(), 3);

        interact_void(&mut net, a, b);

        // CON is still live and its ports are intact
        assert_eq!(net.count_live_agents(), 1);
        assert!(net.get_agent(con).is_some());
        assert_eq!(
            net.get_target(PortRef::AgentPort(con, 0)),
            PortRef::FreePort(0)
        );
    }
}
