//! Helper functions for merge and grid cycle (SPEC-05).
//!
//! - `is_principal_pair`: checks if both ports are principal (for border redex counting)
//! - `rebuild_free_port_index`: lazy reconstruction of the FreePort index after local reduction
//! - `drain_stale_redexes`: removes stale entries from the redex queue

use std::collections::HashMap;
use std::collections::VecDeque;

use crate::net::{total_ports, Net, PortRef};

/// Returns true if both ports are principal ports (port index 0).
///
/// Used during merge to count border redexes for metrics (SPEC-05, R12-R13).
/// The actual redex detection is performed by Net::connect (SPEC-02, R13);
/// this function only identifies principal-principal pairs for counting.
pub(crate) fn is_principal_pair(a: PortRef, b: PortRef) -> bool {
    matches!((a, b), (PortRef::AgentPort(_, 0), PortRef::AgentPort(_, 0)))
}

/// Rebuilds the free_port_index for a partition by scanning the port array.
///
/// After local reduction, the connections to FreePort (Boundary) sentinels
/// may have changed (reconnection, erasure/transfer, CON-DUP inheritance).
/// This function produces a fresh index reflecting the current state.
///
/// Uses `border_id_start` and `border_id_end` (SPEC-04, R15a) to discriminate:
/// - FreePort(id) with border_id_start <= id < border_id_end: boundary (included)
/// - FreePort(id) with id < border_id_start: Lafont FreePort (excluded)
/// - FreePort(u32::MAX): DISCONNECTED sentinel (excluded)
///
/// Complexity: O(A_i * PORTS_PER_SLOT) where A_i is the number of live agents.
///
/// SPEC-05, R20-R23; SPEC-04, Section 4.6, R15a.
pub fn rebuild_free_port_index(
    subnet: &Net,
    border_id_start: u32,
    border_id_end: u32,
) -> HashMap<u32, PortRef> {
    let mut index = HashMap::new();

    for (i, slot) in subnet.agents.iter().enumerate() {
        if let Some(agent) = slot {
            let num_ports = total_ports(agent.symbol);
            for p in 0..num_ports {
                let target = subnet.get_target(PortRef::AgentPort(i as u32, p));
                if let PortRef::FreePort(bid) = target {
                    // Include only boundary FreePorts (not Lafont or DISCONNECTED)
                    if bid >= border_id_start && bid < border_id_end && bid != u32::MAX {
                        index.insert(bid, PortRef::AgentPort(agent.id, p));
                    }
                }
            }
        }
    }

    index
}

/// Traverses the redex queue and discards all stale entries (SPEC-05, Section 4.4).
///
/// A stale redex is one where either agent has been consumed, or the
/// principal port connection has changed since the redex was inserted.
/// After this function, the queue contains only valid redexes.
///
/// Complexity: O(Q) where Q is the size of the redex queue.
pub fn drain_stale_redexes(net: &mut Net) {
    let mut valid = VecDeque::new();
    while let Some((a, b)) = net.redex_queue.pop_front() {
        if net.is_valid_redex(a, b) {
            valid.push_back((a, b));
        }
    }
    net.redex_queue = valid;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::Symbol;

    // === is_principal_pair tests (TASK-0064) ===

    // T1: Both principal ports -> true
    #[test]
    fn test_principal_pair_both_principal() {
        assert!(is_principal_pair(
            PortRef::AgentPort(1, 0),
            PortRef::AgentPort(2, 0)
        ));
    }

    // T2: Principal + auxiliary -> false
    #[test]
    fn test_principal_pair_one_auxiliary() {
        assert!(!is_principal_pair(
            PortRef::AgentPort(1, 0),
            PortRef::AgentPort(2, 1)
        ));
    }

    // T3: Auxiliary + principal -> false
    #[test]
    fn test_principal_pair_first_auxiliary() {
        assert!(!is_principal_pair(
            PortRef::AgentPort(1, 1),
            PortRef::AgentPort(2, 0)
        ));
    }

    // T4: FreePort + AgentPort -> false
    #[test]
    fn test_principal_pair_freeport_agent() {
        assert!(!is_principal_pair(
            PortRef::FreePort(5),
            PortRef::AgentPort(2, 0)
        ));
    }

    // T5: FreePort + FreePort -> false
    #[test]
    fn test_principal_pair_both_freeport() {
        assert!(!is_principal_pair(
            PortRef::FreePort(5),
            PortRef::FreePort(6)
        ));
    }

    // === rebuild_free_port_index tests (TASK-0063) ===

    // T1: Subnet with no FreePort connections -> empty index
    #[test]
    fn test_rebuild_no_freeports() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 1));
        net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(b, 2));

        let index = rebuild_free_port_index(&net, 100, 200);
        assert!(index.is_empty());
    }

    // T2: Subnet with one boundary FreePort -> single entry
    #[test]
    fn test_rebuild_one_boundary_freeport() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        // Port 0 -> FreePort(100) (boundary)
        net.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(100));
        // Ports 1, 2 -> Lafont FreePorts (below border_id_start)
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));

        let index = rebuild_free_port_index(&net, 100, 200);
        assert_eq!(index.len(), 1);
        assert_eq!(index[&100], PortRef::AgentPort(a, 0));
    }

    // T3: Lafont FreePorts are excluded
    #[test]
    fn test_rebuild_excludes_lafont_freeports() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        // All ports -> Lafont FreePorts (below border_id_start=50)
        net.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(2));

        let index = rebuild_free_port_index(&net, 50, 100);
        assert!(index.is_empty());
    }

    // T4: DISCONNECTED is excluded
    #[test]
    fn test_rebuild_excludes_disconnected() {
        // DISCONNECTED = FreePort(u32::MAX), should never be in the index
        let net = Net::new();
        let index = rebuild_free_port_index(&net, 0, u32::MAX);
        assert!(index.is_empty());
    }

    // T5: Multiple boundary FreePorts produce correct multi-entry index
    #[test]
    fn test_rebuild_multiple_boundary_freeports() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        // a: port 0 -> FreePort(100), port 1 -> FreePort(101)
        net.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(100));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(101));
        net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(b, 2));
        // b: port 0 -> FreePort(102)
        net.connect(PortRef::AgentPort(b, 0), PortRef::FreePort(102));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(0)); // Lafont

        let index = rebuild_free_port_index(&net, 100, 200);
        assert_eq!(index.len(), 3);
        assert_eq!(index[&100], PortRef::AgentPort(a, 0));
        assert_eq!(index[&101], PortRef::AgentPort(a, 1));
        assert_eq!(index[&102], PortRef::AgentPort(b, 0));
    }

    // T6: ERA agent with boundary FreePort on principal port
    #[test]
    fn test_rebuild_era_with_boundary() {
        let mut net = Net::new();
        let e = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(e, 0), PortRef::FreePort(50));

        let index = rebuild_free_port_index(&net, 50, 100);
        assert_eq!(index.len(), 1);
        assert_eq!(index[&50], PortRef::AgentPort(e, 0));
    }

    // T7: Removed agent (None slot) is skipped
    #[test]
    fn test_rebuild_skips_removed_agents() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(100));
        // Remove the agent
        net.remove_agent(a);

        let index = rebuild_free_port_index(&net, 100, 200);
        assert!(index.is_empty());
    }

    // === drain_stale_redexes tests (TASK-0068) ===

    // T1: Empty queue stays empty
    #[test]
    fn test_drain_empty_queue() {
        let mut net = Net::new();
        drain_stale_redexes(&mut net);
        assert!(net.redex_queue.is_empty());
    }

    // T2: Queue with only valid redexes -> all retained
    #[test]
    fn test_drain_all_valid() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        // connect() already pushed (a, b) into redex_queue
        assert_eq!(net.redex_queue.len(), 1);

        drain_stale_redexes(&mut net);
        assert_eq!(net.redex_queue.len(), 1);
        assert_eq!(net.redex_queue[0], (a, b));
    }

    // T3: Queue with only stale redexes -> empty after drain
    #[test]
    fn test_drain_all_stale() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        // Remove agents, making the redex stale
        net.remove_agent(a);
        net.remove_agent(b);

        assert_eq!(net.redex_queue.len(), 1);
        drain_stale_redexes(&mut net);
        assert!(net.redex_queue.is_empty());
    }

    // T4: Mixed valid and stale -> only valid retained, in order
    #[test]
    fn test_drain_mixed() {
        let mut net = Net::new();
        // Create two valid redexes
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        let c = net.create_agent(Symbol::Era);
        let d = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(c, 0), PortRef::AgentPort(d, 0));

        // Make first redex stale by removing agent a
        net.remove_agent(a);
        net.remove_agent(b);

        assert_eq!(net.redex_queue.len(), 2);
        drain_stale_redexes(&mut net);
        assert_eq!(net.redex_queue.len(), 1);
        assert_eq!(net.redex_queue[0], (c, d));
    }
}
