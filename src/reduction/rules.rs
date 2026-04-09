//! Interaction rules and the link helper.
//!
//! Contains the 4 interaction functions (interact_void, interact_anni,
//! interact_eras, interact_comm) and the safe link procedure.

use crate::net::{AgentId, Net, PortRef, Symbol};

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

/// Annihilation: two agents of the SAME symbol annihilate (SPEC-03 Sections 4.1.1, 4.1.2).
///
/// - CON-CON: reconnection in CROSS pattern (a.1<->b.2, a.2<->b.1).
///   The cross pattern is what distinguishes CON from DUP and is essential
///   for the universality of the IC system (REF-002 p.90).
/// - DUP-DUP: reconnection in PARALLEL pattern (a.1<->b.1, a.2<->b.2).
///
/// Precondition: both agent IDs MUST refer to live agents of the same
///   symbol (Con or Dup). Guaranteed by `reduce_step`'s validity check (R12).
/// Postcondition: both removed; auxiliary ports reconnected (or no-op'd
///   if self-referencing, per R25).
///
/// Agent balance: -2. Link calls: 2.
/// Complexity: O(1).
pub fn interact_anni(net: &mut Net, a_id: AgentId, b_id: AgentId) {
    let sym = net.agents[a_id as usize].unwrap().symbol;

    // Read all auxiliary port targets BEFORE removing agents
    let a1 = net.get_target(PortRef::AgentPort(a_id, 1));
    let a2 = net.get_target(PortRef::AgentPort(a_id, 2));
    let b1 = net.get_target(PortRef::AgentPort(b_id, 1));
    let b2 = net.get_target(PortRef::AgentPort(b_id, 2));

    // Detect self-loops BEFORE removing agents.
    // A self-loop means p1 <-> p2 on the same agent (e.g., Church(0)'s
    // `lambda x. x` where lam_x.p1 connects to lam_x.p2).
    // When one agent has a self-loop, it acts as identity: the other
    // agent's external ports should be connected directly to each other.
    let a_self_loop =
        a1 == PortRef::AgentPort(a_id, 2) && a2 == PortRef::AgentPort(a_id, 1);
    let b_self_loop =
        b1 == PortRef::AgentPort(b_id, 2) && b2 == PortRef::AgentPort(b_id, 1);

    net.remove_agent(a_id);
    net.remove_agent(b_id);

    if a_self_loop && b_self_loop {
        // Both agents have self-loops — nothing external to connect
        return;
    } else if b_self_loop {
        // b is identity — connect a's external ports together
        link(net, a1, a2);
        return;
    } else if a_self_loop {
        // a is identity — connect b's external ports together
        link(net, b1, b2);
        return;
    }

    match sym {
        Symbol::Con => {
            // CROSS: a.1 <-> b.2, a.2 <-> b.1
            link(net, a1, b2);
            link(net, a2, b1);
        }
        Symbol::Dup => {
            // PARALLEL: a.1 <-> b.1, a.2 <-> b.2
            link(net, a1, b1);
            link(net, a2, b2);
        }
        _ => unreachable!("interact_anni called with non-arity-2 symbol"),
    }
}

/// Erasure: ERA encounters an arity-2 agent (CON or DUP), propagating
/// erasure through auxiliary ports (SPEC-03 Sections 4.1.5, 4.1.6).
///
/// Creates 2 new ERA agents connected to the auxiliary neighbors of the
/// arity-2 agent. This initiates an erasure cascade: each new ERA may form
/// a redex with its neighbor (detected by `link`), propagating erasure
/// until meeting other ERA agents (terminating with ERA-ERA void) or free ports.
///
/// Self-loop handling: if the arity-2 agent has a self-loop (p1 <-> p2,
/// e.g., Church(0)'s `lambda x. x`), there are no external neighbors to
/// propagate erasure to. Both agents are simply removed without creating
/// new ERAs. Without this check, the new ERAs would be linked to ports
/// of the already-removed self-looping agent, resulting in no-ops that
/// leave DISCONNECTED principal ports (T1 violation).
///
/// Precondition: `node_id` MUST be Con or Dup, `era_id` MUST be Era.
///   Guaranteed by `normalize_pair` (R9) and `reduce_step` (R12).
/// Postcondition: both removed; 2 new ERA connected to old aux neighbors
///   (or 0 new ERA if self-loop detected).
///
/// Agent balance: 0 (removes 2, creates 0 or 2). Link calls: 0 or 2.
/// Complexity: O(1).
pub fn interact_eras(net: &mut Net, node_id: AgentId, era_id: AgentId) {
    // Read auxiliary port targets of the arity-2 agent
    let a1 = net.get_target(PortRef::AgentPort(node_id, 1));
    let a2 = net.get_target(PortRef::AgentPort(node_id, 2));

    // Detect self-loop: p1 <-> p2 on the arity-2 agent (e.g., Church(0) identity).
    // If self-looping, there are no external ports — just remove both agents.
    let self_loop = a1 == PortRef::AgentPort(node_id, 2)
        && a2 == PortRef::AgentPort(node_id, 1);

    net.remove_agent(node_id);
    net.remove_agent(era_id);

    if self_loop {
        return;
    }

    // Create 2 new ERA, one for each auxiliary port
    let e1 = net.create_agent(Symbol::Era);
    let e2 = net.create_agent(Symbol::Era);

    // Connect new ERA principal ports to old neighbors
    link(net, PortRef::AgentPort(e1, 0), a1);
    link(net, PortRef::AgentPort(e2, 0), a2);
}

/// Commutation: CON and DUP commute, creating 4 new agents (SPEC-03 Section 4.1.4).
///
/// This is the ONLY rule that INCREASES the number of agents in the net.
/// It creates 2 new DUP agents (inheriting the CON's auxiliary positions)
/// and 2 new CON agents (inheriting the DUP's auxiliary positions),
/// connected in a crossed internal pattern.
///
/// The expansion is what drives the parallelism potential: more agents
/// means more potential redexes for distributed reduction.
///
/// Precondition: `con_id` MUST be Con, `dup_id` MUST be Dup.
///   Guaranteed by `normalize_pair` (R9) and `reduce_step` (R12).
/// Postcondition: both removed; 4 new agents created and fully wired.
///
/// Agent balance: +2 (removes 2, creates 4). Link calls: 8 (4 external + 4 internal).
/// Complexity: O(1).
pub fn interact_comm(net: &mut Net, con_id: AgentId, dup_id: AgentId) {
    // Read all auxiliary port targets BEFORE removing agents
    let a1 = net.get_target(PortRef::AgentPort(con_id, 1));
    let a2 = net.get_target(PortRef::AgentPort(con_id, 2));
    let b1 = net.get_target(PortRef::AgentPort(dup_id, 1));
    let b2 = net.get_target(PortRef::AgentPort(dup_id, 2));

    net.remove_agent(con_id);
    net.remove_agent(dup_id);

    // Create 4 new agents: 2 DUP + 2 CON
    let p = net.create_agent(Symbol::Dup); // DUP: inherits side of con.1
    let q = net.create_agent(Symbol::Dup); // DUP: inherits side of con.2
    let r = net.create_agent(Symbol::Con); // CON: inherits side of dup.1
    let s = net.create_agent(Symbol::Con); // CON: inherits side of dup.2

    // External wires: principal ports of new agents <-> old neighbors
    // Note: old neighbors (a1, a2, b1, b2) may be FreePort(bid) in
    // partitioned sub-nets. The link helper handles this correctly (R26).
    link(net, PortRef::AgentPort(p, 0), a1);
    link(net, PortRef::AgentPort(q, 0), a2);
    link(net, PortRef::AgentPort(r, 0), b1);
    link(net, PortRef::AgentPort(s, 0), b2);

    // Internal wires: auxiliary ports of new agents to each other (crossed)
    // These are always AgentPort-to-AgentPort (never FreePort), so we can
    // call net.connect directly -- no removed-agent guard needed.
    net.connect(PortRef::AgentPort(p, 1), PortRef::AgentPort(r, 1));
    net.connect(PortRef::AgentPort(p, 2), PortRef::AgentPort(s, 1));
    net.connect(PortRef::AgentPort(q, 1), PortRef::AgentPort(r, 2));
    net.connect(PortRef::AgentPort(q, 2), PortRef::AgentPort(s, 2));
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

    // ===================================================================
    // interact_anni tests (TASK-0024)
    // ===================================================================

    /// Helper: create CON(a)<->CON(b) active pair with 4 external agents on aux ports.
    /// Layout: X<-a.1- CON(a) -p0><p0- CON(b) -b.1->Z
    ///         Y<-a.2-                 -b.2->W
    /// Returns (net, a, b, x, y, z, w)
    fn setup_con_con_with_context() -> (Net, AgentId, AgentId, AgentId, AgentId, AgentId, AgentId) {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        let x = net.create_agent(Symbol::Dup); // context agent for a.1
        let y = net.create_agent(Symbol::Dup); // context agent for a.2
        let z = net.create_agent(Symbol::Dup); // context agent for b.1
        let w = net.create_agent(Symbol::Dup); // context agent for b.2

        // Principal ports: active pair
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        // Auxiliary ports to context agents (using their aux port 1)
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(x, 1));
        net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(y, 1));
        net.connect(PortRef::AgentPort(b, 1), PortRef::AgentPort(z, 1));
        net.connect(PortRef::AgentPort(b, 2), PortRef::AgentPort(w, 1));

        (net, a, b, x, y, z, w)
    }

    // T1: CON-CON cross reconnection topology
    #[test]
    fn test_interact_anni_con_con_cross() {
        let (mut net, a, b, x, y, z, w) = setup_con_con_with_context();

        net.redex_queue.clear();
        interact_anni(&mut net, a, b);

        // CROSS: a.1_target(x.1) <-> b.2_target(w.1), a.2_target(y.1) <-> b.1_target(z.1)
        assert_eq!(
            net.get_target(PortRef::AgentPort(x, 1)),
            PortRef::AgentPort(w, 1)
        );
        assert_eq!(
            net.get_target(PortRef::AgentPort(w, 1)),
            PortRef::AgentPort(x, 1)
        );
        assert_eq!(
            net.get_target(PortRef::AgentPort(y, 1)),
            PortRef::AgentPort(z, 1)
        );
        assert_eq!(
            net.get_target(PortRef::AgentPort(z, 1)),
            PortRef::AgentPort(y, 1)
        );
    }

    // T2: CON-CON removes both agents
    #[test]
    fn test_interact_anni_con_con_removes_both() {
        let (mut net, a, b, _, _, _, _) = setup_con_con_with_context();

        interact_anni(&mut net, a, b);

        assert!(net.get_agent(a).is_none());
        assert!(net.get_agent(b).is_none());
    }

    // T3: CON-CON agent count decreases by 2
    #[test]
    fn test_interact_anni_con_con_count() {
        let (mut net, a, b, _, _, _, _) = setup_con_con_with_context();

        let before = net.count_live_agents();
        interact_anni(&mut net, a, b);
        let after = net.count_live_agents();

        assert_eq!(before - after, 2);
    }

    /// Helper: create DUP(a)<->DUP(b) active pair with 4 external agents on aux ports.
    fn setup_dup_dup_with_context() -> (Net, AgentId, AgentId, AgentId, AgentId, AgentId, AgentId) {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Dup);
        let b = net.create_agent(Symbol::Dup);
        let x = net.create_agent(Symbol::Con);
        let y = net.create_agent(Symbol::Con);
        let z = net.create_agent(Symbol::Con);
        let w = net.create_agent(Symbol::Con);

        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(x, 1));
        net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(y, 1));
        net.connect(PortRef::AgentPort(b, 1), PortRef::AgentPort(z, 1));
        net.connect(PortRef::AgentPort(b, 2), PortRef::AgentPort(w, 1));

        (net, a, b, x, y, z, w)
    }

    // T4: DUP-DUP parallel reconnection topology
    #[test]
    fn test_interact_anni_dup_dup_parallel() {
        let (mut net, a, b, x, y, z, w) = setup_dup_dup_with_context();

        net.redex_queue.clear();
        interact_anni(&mut net, a, b);

        // PARALLEL: a.1_target(x.1) <-> b.1_target(z.1), a.2_target(y.1) <-> b.2_target(w.1)
        assert_eq!(
            net.get_target(PortRef::AgentPort(x, 1)),
            PortRef::AgentPort(z, 1)
        );
        assert_eq!(
            net.get_target(PortRef::AgentPort(z, 1)),
            PortRef::AgentPort(x, 1)
        );
        assert_eq!(
            net.get_target(PortRef::AgentPort(y, 1)),
            PortRef::AgentPort(w, 1)
        );
        assert_eq!(
            net.get_target(PortRef::AgentPort(w, 1)),
            PortRef::AgentPort(y, 1)
        );
    }

    // T5: DUP-DUP removes both agents
    #[test]
    fn test_interact_anni_dup_dup_removes_both() {
        let (mut net, a, b, _, _, _, _) = setup_dup_dup_with_context();

        interact_anni(&mut net, a, b);

        assert!(net.get_agent(a).is_none());
        assert!(net.get_agent(b).is_none());
    }

    // T6: New redex detected when reconnection links two principal ports
    #[test]
    fn test_interact_anni_new_redex_detection() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        let c = net.create_agent(Symbol::Con); // will form new redex
        let d = net.create_agent(Symbol::Con); // will form new redex

        // a<->b active pair
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        // a.1 -> c.p0 (principal port!)
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(c, 0));
        // b.2 -> d.p0 (principal port!)
        net.connect(PortRef::AgentPort(b, 2), PortRef::AgentPort(d, 0));
        // a.2, b.1 connected to non-principal ports
        net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(c, 1));
        net.connect(PortRef::AgentPort(b, 1), PortRef::AgentPort(d, 1));

        net.redex_queue.clear();
        interact_anni(&mut net, a, b);

        // CROSS: a.1_target(c.p0) <-> b.2_target(d.p0) -- both principal, new redex!
        assert!(net
            .redex_queue
            .iter()
            .any(|&(x, y)| (x == c && y == d) || (x == d && y == c)));
    }

    // T7: CON-CON fully self-referencing (a.1<->b.2, a.2<->b.1)
    #[test]
    fn test_interact_anni_con_con_self_referencing() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);

        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        // Cross self-reference: a.1<->b.2, a.2<->b.1
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 2));
        net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(b, 1));

        interact_anni(&mut net, a, b);

        // Both agents vanish cleanly
        assert!(net.get_agent(a).is_none());
        assert!(net.get_agent(b).is_none());
        assert_eq!(net.count_live_agents(), 0);

        // All port slots are DISCONNECTED (no ghost entries)
        for id in [a, b] {
            for port in 0..3u8 {
                assert_eq!(net.get_target(PortRef::AgentPort(id, port)), DISCONNECTED);
            }
        }
    }

    // T8: DUP-DUP fully self-referencing (a.1<->b.1, a.2<->b.2)
    #[test]
    fn test_interact_anni_dup_dup_self_referencing() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Dup);
        let b = net.create_agent(Symbol::Dup);

        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        // Parallel self-reference: a.1<->b.1, a.2<->b.2
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 1));
        net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(b, 2));

        interact_anni(&mut net, a, b);

        assert!(net.get_agent(a).is_none());
        assert!(net.get_agent(b).is_none());
        assert_eq!(net.count_live_agents(), 0);

        for id in [a, b] {
            for port in 0..3u8 {
                assert_eq!(net.get_target(PortRef::AgentPort(id, port)), DISCONNECTED);
            }
        }
    }

    // T9: Partial self-reference (one link is no-op, other proceeds normally)
    #[test]
    fn test_interact_anni_partial_self_reference() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        let x = net.create_agent(Symbol::Dup); // external context

        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        // a.1 <-> b.2 (self-ref: cross link will try to connect these back)
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 2));
        // a.2 <-> x.1 (external)
        net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(x, 1));
        // b.1 <-> x.2 (external)
        net.connect(PortRef::AgentPort(b, 1), PortRef::AgentPort(x, 2));

        interact_anni(&mut net, a, b);

        // CROSS: link(a1_target=b.2, b2_target=a.1) -- both removed, no-op
        // CROSS: link(a2_target=x.1, b1_target=x.2) -- both live, proceeds
        assert_eq!(
            net.get_target(PortRef::AgentPort(x, 1)),
            PortRef::AgentPort(x, 2)
        );
        assert_eq!(
            net.get_target(PortRef::AgentPort(x, 2)),
            PortRef::AgentPort(x, 1)
        );
        assert_eq!(net.count_live_agents(), 1);
    }

    // E1: Aux ports connected to FreePort (boundary sentinel, R26)
    #[test]
    fn test_interact_anni_with_freeport() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        let z = net.create_agent(Symbol::Dup);

        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        // a.1 -> FreePort(0) (boundary sentinel)
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        // b.2 -> z.1
        net.connect(PortRef::AgentPort(b, 2), PortRef::AgentPort(z, 1));
        // a.2 and b.1 to other free ports
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(2));

        net.redex_queue.clear();
        interact_anni(&mut net, a, b);

        // CROSS: a.1_target=FreePort(0) <-> b.2_target=z.1
        // z.1 should now point to FreePort(0)
        assert_eq!(
            net.get_target(PortRef::AgentPort(z, 1)),
            PortRef::FreePort(0)
        );
    }

    // E2: Other agents in the net are unaffected
    #[test]
    fn test_interact_anni_does_not_affect_other_agents() {
        let (mut net, a, b, x, y, z, w) = setup_con_con_with_context();
        let extra = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(extra, 0), PortRef::FreePort(99));

        interact_anni(&mut net, a, b);

        // Extra agent still live
        assert!(net.get_agent(extra).is_some());
        assert_eq!(
            net.get_target(PortRef::AgentPort(extra, 0)),
            PortRef::FreePort(99)
        );
        // Context agents still live
        for id in [x, y, z, w] {
            assert!(net.get_agent(id).is_some());
        }
    }

    // ===================================================================
    // interact_eras tests (TASK-0025)
    // ===================================================================

    // T1: CON-ERA removes both agents and creates 2 new ERA
    #[test]
    fn test_interact_eras_con_era() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con); // node (arity 2)
        let b = net.create_agent(Symbol::Era); // era
        let x = net.create_agent(Symbol::Dup); // context for a.1
        let y = net.create_agent(Symbol::Dup); // context for a.2

        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(x, 1));
        net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(y, 1));

        net.redex_queue.clear();
        interact_eras(&mut net, a, b);

        // Original agents removed
        assert!(net.get_agent(a).is_none());
        assert!(net.get_agent(b).is_none());

        // x.1 now points to a new ERA's principal port
        let x1_target = net.get_target(PortRef::AgentPort(x, 1));
        if let PortRef::AgentPort(e1, 0) = x1_target {
            assert_eq!(net.get_agent(e1).unwrap().symbol, Symbol::Era);
        } else {
            panic!(
                "Expected x.1 to point to new ERA's port 0, got {:?}",
                x1_target
            );
        }

        // y.1 now points to a different new ERA's principal port
        let y1_target = net.get_target(PortRef::AgentPort(y, 1));
        if let PortRef::AgentPort(e2, 0) = y1_target {
            assert_eq!(net.get_agent(e2).unwrap().symbol, Symbol::Era);
            // e1 and e2 are different agents
            if let PortRef::AgentPort(e1, _) = x1_target {
                assert_ne!(e1, e2);
            }
        } else {
            panic!(
                "Expected y.1 to point to new ERA's port 0, got {:?}",
                y1_target
            );
        }
    }

    // T2: DUP-ERA removes both and creates 2 new ERA (identical topology)
    #[test]
    fn test_interact_eras_dup_era() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Dup); // node (arity 2)
        let b = net.create_agent(Symbol::Era); // era
        let x = net.create_agent(Symbol::Con);
        let y = net.create_agent(Symbol::Con);

        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(x, 1));
        net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(y, 1));

        interact_eras(&mut net, a, b);

        assert!(net.get_agent(a).is_none());
        assert!(net.get_agent(b).is_none());

        // x.1 -> new ERA.p0
        let x1_target = net.get_target(PortRef::AgentPort(x, 1));
        if let PortRef::AgentPort(e1, 0) = x1_target {
            assert_eq!(net.get_agent(e1).unwrap().symbol, Symbol::Era);
        } else {
            panic!("Expected ERA, got {:?}", x1_target);
        }

        // y.1 -> different new ERA.p0
        let y1_target = net.get_target(PortRef::AgentPort(y, 1));
        if let PortRef::AgentPort(e2, 0) = y1_target {
            assert_eq!(net.get_agent(e2).unwrap().symbol, Symbol::Era);
        } else {
            panic!("Expected ERA, got {:?}", y1_target);
        }
    }

    // T3: Agent balance is 0 (removes 2, creates 2)
    #[test]
    fn test_interact_eras_agent_balance() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Era);
        let x = net.create_agent(Symbol::Dup);
        let y = net.create_agent(Symbol::Dup);

        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(x, 1));
        net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(y, 1));

        let before = net.count_live_agents(); // 4
        interact_eras(&mut net, a, b);
        let after = net.count_live_agents(); // 4 (removed 2, created 2)

        assert_eq!(before, after);
    }

    // T4: New ERA agents have Symbol::Era
    #[test]
    fn test_interact_eras_new_agents_are_era() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Era);

        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));

        interact_eras(&mut net, a, b);

        // 2 new ERA agents should exist (IDs after a and b)
        let live: Vec<_> = net.live_agents().collect();
        assert_eq!(live.len(), 2);
        assert!(live.iter().all(|agent| agent.symbol == Symbol::Era));
    }

    // T5: Erasure cascade -- new redex when a1_target is a principal port
    #[test]
    fn test_interact_eras_cascade_redex() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Era);
        let c = net.create_agent(Symbol::Dup); // cascade target

        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        // a.1 -> c.p0 (principal port! will form new redex with new ERA)
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(c, 0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(0));

        net.redex_queue.clear();
        interact_eras(&mut net, a, b);

        // New ERA(e1).p0 <-> c.p0 should form a redex
        assert!(!net.redex_queue.is_empty());
        // The redex should involve c
        assert!(net.redex_queue.iter().any(|&(x, y)| x == c || y == c));
    }

    // T6: FreePort aux target (boundary sentinel, R26)
    #[test]
    fn test_interact_eras_with_freeport() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Era);

        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));

        interact_eras(&mut net, a, b);

        // New ERA agents should have FreePort in their port 0
        let live: Vec<_> = net.live_agents().collect();
        assert_eq!(live.len(), 2);
        // One ERA.p0 -> FreePort(0), other ERA.p0 -> FreePort(1)
        let e1_id = live[0].id;
        let e2_id = live[1].id;
        let targets: Vec<_> = [e1_id, e2_id]
            .iter()
            .map(|&id| net.get_target(PortRef::AgentPort(id, 0)))
            .collect();
        assert!(targets.contains(&PortRef::FreePort(0)));
        assert!(targets.contains(&PortRef::FreePort(1)));
    }

    // E1: Other agents in the net are unaffected
    #[test]
    fn test_interact_eras_does_not_affect_other_agents() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Era);
        let x = net.create_agent(Symbol::Dup);
        let extra = net.create_agent(Symbol::Con);

        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(x, 1));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(extra, 0), PortRef::FreePort(99));

        interact_eras(&mut net, a, b);

        assert!(net.get_agent(extra).is_some());
        assert_eq!(
            net.get_target(PortRef::AgentPort(extra, 0)),
            PortRef::FreePort(99)
        );
    }

    // E2: ERA's unused auxiliary slots remain clean after removal
    #[test]
    fn test_interact_eras_era_slots_clean() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Era);

        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));

        interact_eras(&mut net, a, b);

        // ERA(b) was removed, its slots should all be DISCONNECTED
        assert_eq!(net.get_target(PortRef::AgentPort(b, 0)), DISCONNECTED);
        assert_eq!(net.get_target(PortRef::AgentPort(b, 1)), DISCONNECTED);
        assert_eq!(net.get_target(PortRef::AgentPort(b, 2)), DISCONNECTED);
    }

    // ===================================================================
    // interact_comm tests (TASK-0026)
    // ===================================================================

    /// Helper: create CON(a)<->DUP(b) active pair with 4 context agents on aux ports.
    /// Returns (net, con_id, dup_id, x, y, z, w) where:
    ///   x is on con.1, y on con.2, z on dup.1, w on dup.2
    fn setup_con_dup_with_context() -> (Net, AgentId, AgentId, AgentId, AgentId, AgentId, AgentId) {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con); // con
        let b = net.create_agent(Symbol::Dup); // dup
        let x = net.create_agent(Symbol::Con); // context for con.1
        let y = net.create_agent(Symbol::Con); // context for con.2
        let z = net.create_agent(Symbol::Dup); // context for dup.1
        let w = net.create_agent(Symbol::Dup); // context for dup.2

        // Active pair
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        // Aux ports to context (using their aux port 1)
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(x, 1));
        net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(y, 1));
        net.connect(PortRef::AgentPort(b, 1), PortRef::AgentPort(z, 1));
        net.connect(PortRef::AgentPort(b, 2), PortRef::AgentPort(w, 1));

        (net, a, b, x, y, z, w)
    }

    // T1: CON-DUP creates 4 new agents (2 DUP + 2 CON)
    #[test]
    fn test_interact_comm_creates_4_agents() {
        let (mut net, a, b, _, _, _, _) = setup_con_dup_with_context();

        interact_comm(&mut net, a, b);

        // Original pair removed
        assert!(net.get_agent(a).is_none());
        assert!(net.get_agent(b).is_none());

        // 4 context agents + 4 new agents = 8 live agents
        // (6 original - 2 removed + 4 created = 8)
        assert_eq!(net.count_live_agents(), 8);
    }

    // T2: Agent balance is +2
    #[test]
    fn test_interact_comm_agent_balance() {
        let (mut net, a, b, _, _, _, _) = setup_con_dup_with_context();

        let before = net.count_live_agents(); // 6
        interact_comm(&mut net, a, b);
        let after = net.count_live_agents(); // 8

        assert_eq!(after as i32 - before as i32, 2);
    }

    // T3: External wires correct
    #[test]
    fn test_interact_comm_external_wires() {
        let (mut net, a, b, x, y, z, w) = setup_con_dup_with_context();

        net.redex_queue.clear();
        interact_comm(&mut net, a, b);

        // x.1 was connected to con.1, should now point to new DUP(p).p0
        let x1_target = net.get_target(PortRef::AgentPort(x, 1));
        if let PortRef::AgentPort(p, 0) = x1_target {
            assert_eq!(net.get_agent(p).unwrap().symbol, Symbol::Dup);
        } else {
            panic!("Expected x.1 -> DUP.p0, got {:?}", x1_target);
        }

        // y.1 was connected to con.2, should now point to new DUP(q).p0
        let y1_target = net.get_target(PortRef::AgentPort(y, 1));
        if let PortRef::AgentPort(q, 0) = y1_target {
            assert_eq!(net.get_agent(q).unwrap().symbol, Symbol::Dup);
        } else {
            panic!("Expected y.1 -> DUP.p0, got {:?}", y1_target);
        }

        // z.1 was connected to dup.1, should now point to new CON(r).p0
        let z1_target = net.get_target(PortRef::AgentPort(z, 1));
        if let PortRef::AgentPort(r, 0) = z1_target {
            assert_eq!(net.get_agent(r).unwrap().symbol, Symbol::Con);
        } else {
            panic!("Expected z.1 -> CON.p0, got {:?}", z1_target);
        }

        // w.1 was connected to dup.2, should now point to new CON(s).p0
        let w1_target = net.get_target(PortRef::AgentPort(w, 1));
        if let PortRef::AgentPort(s, 0) = w1_target {
            assert_eq!(net.get_agent(s).unwrap().symbol, Symbol::Con);
        } else {
            panic!("Expected w.1 -> CON.p0, got {:?}", w1_target);
        }
    }

    // T4: Internal wires correct -- crossed pattern
    #[test]
    fn test_interact_comm_internal_wires() {
        let (mut net, a, b, x, y, z, w) = setup_con_dup_with_context();

        interact_comm(&mut net, a, b);

        // Extract new agent IDs from external wire endpoints
        let p = match net.get_target(PortRef::AgentPort(x, 1)) {
            PortRef::AgentPort(id, 0) => id,
            other => panic!("Expected AgentPort, got {:?}", other),
        };
        let q = match net.get_target(PortRef::AgentPort(y, 1)) {
            PortRef::AgentPort(id, 0) => id,
            other => panic!("Expected AgentPort, got {:?}", other),
        };
        let r = match net.get_target(PortRef::AgentPort(z, 1)) {
            PortRef::AgentPort(id, 0) => id,
            other => panic!("Expected AgentPort, got {:?}", other),
        };
        let s = match net.get_target(PortRef::AgentPort(w, 1)) {
            PortRef::AgentPort(id, 0) => id,
            other => panic!("Expected AgentPort, got {:?}", other),
        };

        // Internal crossed wires:
        // p.1 <-> r.1
        assert_eq!(
            net.get_target(PortRef::AgentPort(p, 1)),
            PortRef::AgentPort(r, 1)
        );
        assert_eq!(
            net.get_target(PortRef::AgentPort(r, 1)),
            PortRef::AgentPort(p, 1)
        );

        // p.2 <-> s.1
        assert_eq!(
            net.get_target(PortRef::AgentPort(p, 2)),
            PortRef::AgentPort(s, 1)
        );
        assert_eq!(
            net.get_target(PortRef::AgentPort(s, 1)),
            PortRef::AgentPort(p, 2)
        );

        // q.1 <-> r.2
        assert_eq!(
            net.get_target(PortRef::AgentPort(q, 1)),
            PortRef::AgentPort(r, 2)
        );
        assert_eq!(
            net.get_target(PortRef::AgentPort(r, 2)),
            PortRef::AgentPort(q, 1)
        );

        // q.2 <-> s.2
        assert_eq!(
            net.get_target(PortRef::AgentPort(q, 2)),
            PortRef::AgentPort(s, 2)
        );
        assert_eq!(
            net.get_target(PortRef::AgentPort(s, 2)),
            PortRef::AgentPort(q, 2)
        );
    }

    // T5: New agents have correct symbols
    #[test]
    fn test_interact_comm_new_agent_symbols() {
        let (mut net, a, b, x, y, z, w) = setup_con_dup_with_context();

        interact_comm(&mut net, a, b);

        // p, q = DUP (inherit CON side); r, s = CON (inherit DUP side)
        let p = match net.get_target(PortRef::AgentPort(x, 1)) {
            PortRef::AgentPort(id, 0) => id,
            _ => panic!(),
        };
        let q = match net.get_target(PortRef::AgentPort(y, 1)) {
            PortRef::AgentPort(id, 0) => id,
            _ => panic!(),
        };
        let r = match net.get_target(PortRef::AgentPort(z, 1)) {
            PortRef::AgentPort(id, 0) => id,
            _ => panic!(),
        };
        let s = match net.get_target(PortRef::AgentPort(w, 1)) {
            PortRef::AgentPort(id, 0) => id,
            _ => panic!(),
        };

        assert_eq!(net.get_agent(p).unwrap().symbol, Symbol::Dup);
        assert_eq!(net.get_agent(q).unwrap().symbol, Symbol::Dup);
        assert_eq!(net.get_agent(r).unwrap().symbol, Symbol::Con);
        assert_eq!(net.get_agent(s).unwrap().symbol, Symbol::Con);
    }

    // T6: New redex detection from external wires
    #[test]
    fn test_interact_comm_new_redex_detection() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);
        let c = net.create_agent(Symbol::Con); // will form redex with new DUP(p)

        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        // con.1 -> c.p0 (principal port! new DUP(p).p0 <-> c.p0 = redex)
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(c, 0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(2));

        net.redex_queue.clear();
        interact_comm(&mut net, a, b);

        // c should be involved in a new redex (with new DUP)
        assert!(net.redex_queue.iter().any(|&(x, y)| x == c || y == c));
    }

    // T7: Internal wires do NOT generate redexes
    #[test]
    fn test_interact_comm_internal_wires_no_redex() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);

        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        // All aux ports to FreePort (no external principal ports)
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(3));

        net.redex_queue.clear();
        interact_comm(&mut net, a, b);

        // No redexes: all external wires go to FreePort (not principal),
        // and all internal wires are aux-to-aux.
        assert!(net.redex_queue.is_empty());
    }

    // E1: FreePort aux targets (boundary sentinel, R26)
    #[test]
    fn test_interact_comm_with_freeport() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);

        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(10));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(20));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(30));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(40));

        interact_comm(&mut net, a, b);

        // Find new agents by scanning live agents (excluding removed a, b)
        let new_agents: Vec<_> = net
            .live_agents()
            .filter(|agent| agent.id != a && agent.id != b)
            .collect();
        assert_eq!(new_agents.len(), 4);

        // New DUPs' principal ports should point to FreePort(10) and FreePort(20)
        let dup_agents: Vec<_> = new_agents
            .iter()
            .filter(|a| a.symbol == Symbol::Dup)
            .collect();
        assert_eq!(dup_agents.len(), 2);
        let dup_targets: Vec<_> = dup_agents
            .iter()
            .map(|a| net.get_target(PortRef::AgentPort(a.id, 0)))
            .collect();
        assert!(dup_targets.contains(&PortRef::FreePort(10)));
        assert!(dup_targets.contains(&PortRef::FreePort(20)));

        // New CONs' principal ports should point to FreePort(30) and FreePort(40)
        let con_agents: Vec<_> = new_agents
            .iter()
            .filter(|a| a.symbol == Symbol::Con)
            .collect();
        assert_eq!(con_agents.len(), 2);
        let con_targets: Vec<_> = con_agents
            .iter()
            .map(|a| net.get_target(PortRef::AgentPort(a.id, 0)))
            .collect();
        assert!(con_targets.contains(&PortRef::FreePort(30)));
        assert!(con_targets.contains(&PortRef::FreePort(40)));
    }

    // E2: Other agents in the net are unaffected
    #[test]
    fn test_interact_comm_does_not_affect_other_agents() {
        let (mut net, a, b, x, _, _, _) = setup_con_dup_with_context();
        let extra = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(extra, 0), PortRef::FreePort(99));

        interact_comm(&mut net, a, b);

        assert!(net.get_agent(extra).is_some());
        assert_eq!(
            net.get_target(PortRef::AgentPort(extra, 0)),
            PortRef::FreePort(99)
        );
        assert!(net.get_agent(x).is_some());
    }

    // E3: PortRef values survive Vec reallocation
    #[test]
    fn test_interact_comm_portref_survives_realloc() {
        // Create a minimal net to force potential reallocation
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);
        let c = net.create_agent(Symbol::Con);

        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(c, 1));
        net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(c, 2));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(1));

        // This will create 4 new agents, potentially reallocating the Vec
        interact_comm(&mut net, a, b);

        // If PortRef was pointer-based, c's connections would be corrupted.
        // Since PortRef is index-based (AgentPort(id, port)), reallocation is safe.
        // c should be connected to two new DUP agents' principal ports.
        let c1_target = net.get_target(PortRef::AgentPort(c, 1));
        let c2_target = net.get_target(PortRef::AgentPort(c, 2));
        // Both should be valid AgentPort references to new DUP agents
        if let PortRef::AgentPort(p, 0) = c1_target {
            assert!(net.get_agent(p).is_some());
            assert_eq!(net.get_agent(p).unwrap().symbol, Symbol::Dup);
        } else {
            panic!("Expected AgentPort, got {:?}", c1_target);
        }
        if let PortRef::AgentPort(q, 0) = c2_target {
            assert!(net.get_agent(q).is_some());
            assert_eq!(net.get_agent(q).unwrap().symbol, Symbol::Dup);
        } else {
            panic!("Expected AgentPort, got {:?}", c2_target);
        }
    }

    // ================================================================
    // TASK-0232: Self-loop annihilation tests (intra-agent self-loops)
    // ================================================================

    /// Helper: creates a CON agent with p1 <-> p2 self-loop (like Church(0) lambda_x).
    fn con_with_self_loop(net: &mut Net) -> AgentId {
        let a = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(a, 2));
        a
    }

    /// Helper: creates a DUP agent with p1 <-> p2 self-loop.
    fn dup_with_self_loop(net: &mut Net) -> AgentId {
        let a = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(a, 2));
        a
    }

    // T1: CON-CON with b self-loop, a has external ports → externals linked
    #[test]
    fn test_self_loop_con_con_b_has_self_loop() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = con_with_self_loop(&mut net);
        let x = net.create_agent(Symbol::Era); // external for a.p1
        let y = net.create_agent(Symbol::Era); // external for a.p2
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(x, 0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(y, 0));
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        interact_anni(&mut net, a, b);

        // a and b removed
        assert!(net.get_agent(a).is_none());
        assert!(net.get_agent(b).is_none());
        // x and y connected to each other (a's externals linked)
        assert_eq!(
            net.get_target(PortRef::AgentPort(x, 0)),
            PortRef::AgentPort(y, 0)
        );
        assert_eq!(
            net.get_target(PortRef::AgentPort(y, 0)),
            PortRef::AgentPort(x, 0)
        );
    }

    // T2: CON-CON with a self-loop, b has external ports → externals linked
    #[test]
    fn test_self_loop_con_con_a_has_self_loop() {
        let mut net = Net::new();
        let a = con_with_self_loop(&mut net);
        let b = net.create_agent(Symbol::Con);
        let x = net.create_agent(Symbol::Era); // external for b.p1
        let y = net.create_agent(Symbol::Era); // external for b.p2
        net.connect(PortRef::AgentPort(b, 1), PortRef::AgentPort(x, 0));
        net.connect(PortRef::AgentPort(b, 2), PortRef::AgentPort(y, 0));
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        interact_anni(&mut net, a, b);

        assert!(net.get_agent(a).is_none());
        assert!(net.get_agent(b).is_none());
        // b's externals linked together
        assert_eq!(
            net.get_target(PortRef::AgentPort(x, 0)),
            PortRef::AgentPort(y, 0)
        );
        assert_eq!(
            net.get_target(PortRef::AgentPort(y, 0)),
            PortRef::AgentPort(x, 0)
        );
    }

    // T3: DUP-DUP with b self-loop → externals linked
    #[test]
    fn test_self_loop_dup_dup_b_has_self_loop() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Dup);
        let b = dup_with_self_loop(&mut net);
        let x = net.create_agent(Symbol::Era);
        let y = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(x, 0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(y, 0));
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        interact_anni(&mut net, a, b);

        assert!(net.get_agent(a).is_none());
        assert!(net.get_agent(b).is_none());
        assert_eq!(
            net.get_target(PortRef::AgentPort(x, 0)),
            PortRef::AgentPort(y, 0)
        );
    }

    // T5: CON-CON both self-loops → both removed, nothing to connect
    #[test]
    fn test_self_loop_con_con_both_self_loops() {
        let mut net = Net::new();
        let a = con_with_self_loop(&mut net);
        let b = con_with_self_loop(&mut net);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        interact_anni(&mut net, a, b);

        assert!(net.get_agent(a).is_none());
        assert!(net.get_agent(b).is_none());
        // No other live agents affected
        assert_eq!(net.count_live_agents(), 0);
    }

    // T11: New redex after self-loop annihilation
    #[test]
    fn test_self_loop_creates_new_redex() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = con_with_self_loop(&mut net);
        // a's externals are principal ports of other agents
        let c = net.create_agent(Symbol::Con);
        let d = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(c, 0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(d, 0));
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        // Drain the existing redexes from queue (a<->b)
        let initial_redexes = net.redex_queue.len();

        interact_anni(&mut net, a, b);

        // c.p0 <-> d.p0 should form a new redex
        assert_eq!(
            net.get_target(PortRef::AgentPort(c, 0)),
            PortRef::AgentPort(d, 0)
        );
        // New redex should be in the queue
        assert!(
            net.redex_queue.len() > 0,
            "Expected new redex from self-loop annihilation linking two principal ports"
        );
    }

    // E1: Self-loop agent with FreePort external
    #[test]
    fn test_self_loop_with_freeport_external() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = con_with_self_loop(&mut net);
        let y = net.create_agent(Symbol::Era);
        // a.p1 connects to FreePort, a.p2 connects to y
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(y, 0));
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        interact_anni(&mut net, a, b);

        // FreePort(0) linked to y.p0
        assert_eq!(
            net.get_target(PortRef::AgentPort(y, 0)),
            PortRef::FreePort(0)
        );
    }
}
