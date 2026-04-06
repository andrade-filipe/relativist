//! Core types for net partitioning (SPEC-04 Section 4.1).

use crate::net::AgentId;

/// Identifier of a worker in the grid.
/// Values from 0 to n-1, where n is the number of workers.
pub type WorkerId = u32;

/// An exclusive range of AgentIds reserved for a worker.
/// The worker may generate new IDs in the interval [start, end).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IdRange {
    /// First AgentId in the range (inclusive).
    pub start: AgentId,
    /// Last AgentId in the range (exclusive).
    pub end: AgentId,
}

#[cfg(test)]
mod tests {
    use super::*;

    // T1: WorkerId is a u32 type alias
    #[test]
    fn test_worker_id_is_u32() {
        let w: WorkerId = 7u32;
        assert_eq!(w, 7);
    }

    // T2: IdRange has start and end fields
    #[test]
    fn test_id_range_fields() {
        let r = IdRange { start: 0, end: 100 };
        assert_eq!(r.start, 0);
        assert_eq!(r.end, 100);
    }

    // T3: IdRange derives Debug, Clone, Copy, PartialEq, Eq
    #[test]
    fn test_id_range_derives() {
        let a = IdRange { start: 0, end: 10 };
        let b = a; // Copy
        let c = a.clone(); // Clone
        assert_eq!(a, b); // PartialEq
        assert_eq!(a, c);
        assert_ne!(a, IdRange { start: 0, end: 20 });
        assert!(format!("{:?}", a).contains("IdRange")); // Debug
    }

    // T4: IdRange round-trips through bincode (Serialize + Deserialize)
    #[test]
    fn test_id_range_serde() {
        let original = IdRange {
            start: 42,
            end: 1000,
        };
        let bytes = bincode::serialize(&original).unwrap();
        let restored: IdRange = bincode::deserialize(&bytes).unwrap();
        assert_eq!(original, restored);
    }

    // T5: IdRange represents [start, end) exclusive
    #[test]
    fn test_id_range_exclusive_semantics() {
        let r = IdRange { start: 0, end: 100 };
        assert_eq!(r.start, 0);
        assert_eq!(r.end, 100);
        // end is exclusive: the range contains IDs 0..99
    }

    // T6: Empty range (start == end) is valid
    #[test]
    fn test_id_range_empty() {
        let r = IdRange { start: 5, end: 5 };
        assert_eq!(r.start, r.end);
    }

    // E1: Full u32 range
    #[test]
    fn test_id_range_full_u32() {
        let r = IdRange {
            start: 0,
            end: u32::MAX,
        };
        assert_eq!(r.start, 0);
        assert_eq!(r.end, u32::MAX);
    }

    // E2: Single-element range
    #[test]
    fn test_id_range_single_element() {
        let r = IdRange { start: 5, end: 6 };
        assert_eq!(r.end - r.start, 1);
    }
}
