//! Helper functions for net partitioning (SPEC-04).

use crate::net::{Net, PortRef, DISCONNECTED};

use super::types::IdRange;

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
}
