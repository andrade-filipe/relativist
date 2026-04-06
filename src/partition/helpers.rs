//! Helper functions for net partitioning (SPEC-04).

use std::collections::HashMap;

use crate::net::{total_ports, AgentId, Net, PortId, PortRef, DISCONNECTED};

use super::types::{IdRange, WorkerId};

/// Returns the maximum FreePort ID in the net's port array, or `None` if
/// there are no FreePort entries (excluding DISCONNECTED sentinels).
///
/// Used by the split function (SPEC-04 R12) to compute the starting border
/// ID: `border_id_start = max_freeport_id(net).unwrap_or(0) + 1`, ensuring
/// that new border IDs never collide with pre-existing (Lafont) FreePort IDs.
///
/// Complexity: O(P) where P is the size of the port array.
pub fn max_freeport_id(net: &Net) -> Option<u32> {
    let mut max_id: Option<u32> = None;
    for &port_ref in &net.ports {
        if let PortRef::FreePort(id) = port_ref {
            if port_ref != DISCONNECTED {
                max_id = Some(match max_id {
                    Some(current) => current.max(id),
                    None => id,
                });
            }
        }
    }
    max_id
}

/// Computes the static ID space ranges for `num_workers` workers (SPEC-04 Section 4.7).
///
/// Divides the `u32` space (~4.29 billion IDs) into `num_workers` contiguous
/// ranges. The last worker receives any remainder from integer division.
///
/// Panics if `num_workers == 0`.
pub fn compute_id_ranges(num_workers: u32) -> Vec<IdRange> {
    assert!(num_workers > 0, "num_workers must be >= 1");

    let chunk_size = u32::MAX / num_workers;
    (0..num_workers)
        .map(|i| {
            let start = i * chunk_size;
            let end = if i == num_workers - 1 {
                u32::MAX
            } else {
                (i + 1) * chunk_size
            };
            IdRange { start, end }
        })
        .collect()
}

/// Result of wire classification (SPEC-04 Section 4.4, Step 4).
pub struct WireClassification {
    /// Border map: borderId -> (original endpoint A, original endpoint B).
    pub borders: HashMap<u32, (PortRef, PortRef)>,

    /// Per-worker border entries: `border_entries[worker_id]` contains
    /// `(agent_id, port_id, border_id)` for each border wire touching that worker.
    pub border_entries: Vec<Vec<(AgentId, PortId, u32)>>,

    /// The next border ID after all assignments (exclusive end).
    pub border_id_end: u32,

    /// The first border ID assigned (inclusive start).
    pub border_id_start: u32,
}

/// Classifies all wires in the net as internal, interface, or border
/// (SPEC-04 Section 4.4, Step 4 of the split algorithm).
///
/// A border wire is detected only from the side with the smaller AgentId
/// (`agent_id < other_id`), but FreePort entries are generated for BOTH
/// partitions in a single pass.
///
/// Complexity: O(A * PORTS_PER_SLOT) where A is the number of live agents.
pub fn classify_wires(
    net: &Net,
    sigma: &HashMap<AgentId, WorkerId>,
    num_workers: u32,
) -> WireClassification {
    let border_id_start = max_freeport_id(net).map_or(0, |id| id + 1);
    let mut border_id_counter = border_id_start;
    let mut borders = HashMap::new();
    let mut border_entries: Vec<Vec<(AgentId, PortId, u32)>> = vec![vec![]; num_workers as usize];

    for (i, slot) in net.agents.iter().enumerate() {
        let agent = match slot {
            Some(a) => a,
            None => continue,
        };
        let agent_id = i as AgentId;

        for port_id in 0..total_ports(agent.symbol) as PortId {
            let target = net.get_target(PortRef::AgentPort(agent_id, port_id));

            if let PortRef::AgentPort(other_id, other_port) = target {
                // Only process from the smaller-ID side to avoid duplicates
                if agent_id < other_id {
                    let w_a = sigma[&agent_id];
                    let w_b = sigma[&other_id];

                    if w_a != w_b {
                        // Border wire: assign new border ID
                        let bid = border_id_counter;
                        border_id_counter += 1;

                        borders.insert(
                            bid,
                            (
                                PortRef::AgentPort(agent_id, port_id),
                                PortRef::AgentPort(other_id, other_port),
                            ),
                        );

                        border_entries[w_a as usize].push((agent_id, port_id, bid));
                        border_entries[w_b as usize].push((other_id, other_port, bid));
                    }
                }
            }
            // FreePort -> interface wire (no action)
            // DISCONNECTED -> skip (no action)
            // AgentPort with same worker -> internal wire (no action)
        }
    }

    WireClassification {
        borders,
        border_entries,
        border_id_end: border_id_counter,
        border_id_start,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::Symbol;

    // T1: Empty net returns None
    #[test]
    fn test_max_freeport_id_empty_net() {
        let net = Net::new();
        assert_eq!(max_freeport_id(&net), None);
    }

    // T2: Net with agents but no FreePort connections returns None
    #[test]
    fn test_max_freeport_id_no_freeports() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        // Principal ports connected to each other (not FreePort)
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        assert_eq!(max_freeport_id(&net), None);
    }

    // T3: Net with one FreePort connection returns that ID
    #[test]
    fn test_max_freeport_id_single() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(5));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(3));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
        assert_eq!(max_freeport_id(&net), Some(5));
    }

    // T4: Returns maximum among multiple FreePort IDs
    #[test]
    fn test_max_freeport_id_multiple() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(10));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(42));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(7));
        net.connect(PortRef::AgentPort(b, 0), PortRef::FreePort(99));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(50));
        assert_eq!(max_freeport_id(&net), Some(99));
    }

    // T5: DISCONNECTED (FreePort(u32::MAX)) is excluded
    #[test]
    fn test_max_freeport_id_excludes_disconnected() {
        let mut net = Net::new();
        // Creating an agent leaves aux ports as DISCONNECTED = FreePort(u32::MAX)
        let a = net.create_agent(Symbol::Con);
        // Only connect principal port to a real FreePort
        net.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(5));
        // Ports 1 and 2 are DISCONNECTED
        let result = max_freeport_id(&net);
        assert_eq!(result, Some(5)); // u32::MAX excluded
    }

    // E1: FreePort(0) is valid and returned
    #[test]
    fn test_max_freeport_id_zero() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(0));
        assert_eq!(max_freeport_id(&net), Some(0));
    }

    // -----------------------------------------------------------------------
    // compute_id_ranges tests
    // -----------------------------------------------------------------------

    // R1: Single worker gets entire range
    #[test]
    fn test_id_ranges_single_worker() {
        let ranges = compute_id_ranges(1);
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].start, 0);
        assert_eq!(ranges[0].end, u32::MAX);
    }

    // R2: Two workers split the range
    #[test]
    fn test_id_ranges_two_workers() {
        let ranges = compute_id_ranges(2);
        assert_eq!(ranges.len(), 2);
        assert_eq!(ranges[0].start, 0);
        assert_eq!(ranges[1].end, u32::MAX);
        // Contiguous: first ends where second starts
        assert_eq!(ranges[0].end, ranges[1].start);
    }

    // R3: 8 workers (TCC scope)
    #[test]
    fn test_id_ranges_eight_workers() {
        let ranges = compute_id_ranges(8);
        assert_eq!(ranges.len(), 8);
        assert_eq!(ranges[0].start, 0);
        assert_eq!(ranges[7].end, u32::MAX);
        // All contiguous
        for i in 0..7 {
            assert_eq!(ranges[i].end, ranges[i + 1].start);
        }
    }

    // R4: Ranges are disjoint and cover full u32 space
    #[test]
    fn test_id_ranges_cover_full_space() {
        let ranges = compute_id_ranges(4);
        assert_eq!(ranges[0].start, 0);
        assert_eq!(ranges[3].end, u32::MAX);
        for i in 0..3 {
            assert_eq!(ranges[i].end, ranges[i + 1].start);
        }
    }

    // R5: Last worker gets remainder
    #[test]
    fn test_id_ranges_last_worker_remainder() {
        let ranges = compute_id_ranges(3);
        let chunk = u32::MAX / 3;
        // First two workers get exactly chunk_size IDs
        assert_eq!(ranges[0].end - ranges[0].start, chunk);
        assert_eq!(ranges[1].end - ranges[1].start, chunk);
        // Last worker extends to u32::MAX
        assert_eq!(ranges[2].end, u32::MAX);
        assert!(ranges[2].end - ranges[2].start >= chunk);
    }

    // E2: Panics on 0 workers
    #[test]
    #[should_panic(expected = "num_workers must be >= 1")]
    fn test_id_ranges_zero_workers_panics() {
        compute_id_ranges(0);
    }

    // -----------------------------------------------------------------------
    // classify_wires tests
    // -----------------------------------------------------------------------

    // Helper: create a sigma map from a list of (agent_id, worker_id) pairs
    fn make_sigma(pairs: &[(AgentId, WorkerId)]) -> HashMap<AgentId, WorkerId> {
        pairs.iter().copied().collect()
    }

    // W1: Empty net — no borders
    #[test]
    fn test_classify_wires_empty_net() {
        let net = Net::new();
        let sigma = HashMap::new();
        let result = classify_wires(&net, &sigma, 2);
        assert!(result.borders.is_empty());
        assert_eq!(result.border_entries.len(), 2);
        assert!(result.border_entries[0].is_empty());
        assert!(result.border_entries[1].is_empty());
    }

    // W2: Two agents in same partition (internal wire) — no borders
    #[test]
    fn test_classify_wires_internal() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(3));

        let sigma = make_sigma(&[(a, 0), (b, 0)]); // same worker
        let result = classify_wires(&net, &sigma, 2);
        assert!(result.borders.is_empty());
    }

    // W3: Two agents in different partitions — one border wire
    #[test]
    fn test_classify_wires_single_border() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        let sigma = make_sigma(&[(a, 0), (b, 1)]); // different workers
        let result = classify_wires(&net, &sigma, 2);

        assert_eq!(result.borders.len(), 1);
        assert_eq!(result.border_entries[0].len(), 1);
        assert_eq!(result.border_entries[1].len(), 1);
        // Same border ID in both entries
        assert_eq!(result.border_entries[0][0].2, result.border_entries[1][0].2);
    }

    // W4: Border ID starts after max existing FreePort
    #[test]
    fn test_classify_wires_border_id_after_freeport() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        // Principal ports cross partitions
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        // Aux ports use FreePort IDs
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(10));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(20));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(5));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(15));

        let sigma = make_sigma(&[(a, 0), (b, 1)]);
        let result = classify_wires(&net, &sigma, 2);

        // max_freeport_id = 20, so border IDs start at 21
        assert_eq!(result.border_id_start, 21);
        let bid = result.border_entries[0][0].2;
        assert!(bid >= 21);
    }

    // W5: Multiple border wires
    #[test]
    fn test_classify_wires_multiple_borders() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        // All ports cross partitions
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 2));
        net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(b, 1));

        let sigma = make_sigma(&[(a, 0), (b, 1)]);
        let result = classify_wires(&net, &sigma, 2);

        // 3 border wires (principal + 2 aux cross)
        assert_eq!(result.borders.len(), 3);
        assert_eq!(result.border_entries[0].len(), 3);
        assert_eq!(result.border_entries[1].len(), 3);
    }

    // W6: Interface wire (FreePort) — not classified as border
    #[test]
    fn test_classify_wires_interface_ignored() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(5));

        let sigma = make_sigma(&[(a, 0)]);
        let result = classify_wires(&net, &sigma, 2);
        assert!(result.borders.is_empty());
    }

    // W7: border_id_start and border_id_end bracket the assigned IDs
    #[test]
    fn test_classify_wires_border_id_range() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        let sigma = make_sigma(&[(a, 0), (b, 1)]);
        let result = classify_wires(&net, &sigma, 2);

        assert_eq!(result.border_id_end, result.border_id_start + 1);
    }

    // W8: No borders when net has no FreePort -> start at 0
    #[test]
    fn test_classify_wires_no_preexisting_freeport() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        let sigma = make_sigma(&[(a, 0), (b, 1)]);
        let result = classify_wires(&net, &sigma, 2);

        // No pre-existing FreePorts (only DISCONNECTED), start at 0
        assert_eq!(result.border_id_start, 0);
    }

    // W9: Each border appears exactly once in borders map
    #[test]
    fn test_classify_wires_no_duplicate_borders() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(3));

        let sigma = make_sigma(&[(a, 0), (b, 1)]);
        let result = classify_wires(&net, &sigma, 2);

        // Only principal-principal is a border (1 border wire)
        assert_eq!(result.borders.len(), 1);
    }
}
