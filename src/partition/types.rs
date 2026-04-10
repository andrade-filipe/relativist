//! Core types for net partitioning (SPEC-04 Section 4.1).

use std::collections::HashMap;

use crate::net::{AgentId, Net, PortRef};

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

/// A partition: sub-net assigned to a worker (SPEC-04 Section 4.1).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Partition {
    /// The sub-net containing the agents of this partition.
    /// Border wires appear as connections to FreePort(borderId).
    ///
    /// Wire format: the dense `Net` layout (with `max_id + 1` arena slots)
    /// is replaced on the wire by `CompactSubnet`, which only carries live
    /// agents and non-`DISCONNECTED` ports. In memory the field is still
    /// a `Net`. This mitigates L6 (PHASE2-FINDINGS.md): under
    /// `ContiguousIdStrategy` the last worker's subnet would otherwise
    /// span the full arena and push past the 256 MiB frame cap.
    #[serde(
        serialize_with = "crate::partition::compact::serialize_subnet_compact",
        deserialize_with = "crate::partition::compact::deserialize_subnet_compact"
    )]
    pub subnet: Net,

    /// Identifier of the worker responsible for this partition.
    pub worker_id: WorkerId,

    /// Reverse index of boundary FreePorts: borderId -> AgentPort local.
    /// Enables O(1) lookup during merge, instead of linear scan.
    pub free_port_index: HashMap<u32, PortRef>,

    /// ID range reserved for this worker to generate new agents.
    pub id_range: IdRange,

    /// Start of border ID range assigned during split (inclusive).
    /// Used for lazy FreePort index reconstruction: a FreePort(id) in
    /// the port array is a boundary FreePort iff
    /// `border_id_start <= id && id < border_id_end && id != u32::MAX`.
    pub border_id_start: u32,

    /// End of border ID range assigned during split (exclusive).
    pub border_id_end: u32,
}

/// The complete partitioning plan (SPEC-04 Section 4.1).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PartitionPlan {
    /// List of partitions, one per worker. `partitions[i].worker_id == i`.
    pub partitions: Vec<Partition>,

    /// Border map: borderId -> (original_endpoint_A, original_endpoint_B).
    /// Records the two endpoints of each wire that was cut.
    /// Used by the merge protocol (SPEC-05) to restore connections.
    pub borders: HashMap<u32, (PortRef, PortRef)>,
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
    #[allow(clippy::clone_on_copy)]
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

    // -----------------------------------------------------------------------
    // Partition tests
    // -----------------------------------------------------------------------

    fn make_empty_partition() -> Partition {
        Partition {
            subnet: Net::new(),
            worker_id: 0,
            free_port_index: HashMap::new(),
            id_range: IdRange {
                start: 0,
                end: u32::MAX,
            },
            border_id_start: 0,
            border_id_end: 0,
        }
    }

    // T1: Partition has all 6 fields
    #[test]
    fn test_partition_fields() {
        let p = make_empty_partition();
        let _ = &p.subnet;
        let _ = p.worker_id;
        let _ = &p.free_port_index;
        let _ = p.id_range;
        let _ = p.border_id_start;
        let _ = p.border_id_end;
    }

    // T2: Partition derives Debug, Clone, Serialize, Deserialize
    #[test]
    fn test_partition_derives() {
        let p = make_empty_partition();
        let cloned = p.clone();
        assert_eq!(cloned.worker_id, 0);
        assert!(format!("{:?}", p).contains("Partition"));
    }

    // T3: free_port_index is HashMap<u32, PortRef>
    #[test]
    fn test_partition_free_port_index() {
        let mut p = make_empty_partition();
        p.free_port_index.insert(42, PortRef::AgentPort(0, 1));
        assert_eq!(p.free_port_index.get(&42), Some(&PortRef::AgentPort(0, 1)));
    }

    // T4: border_id_start and border_id_end are u32
    #[test]
    fn test_partition_border_id_fields() {
        let mut p = make_empty_partition();
        p.border_id_start = 100;
        p.border_id_end = 200;
        assert_eq!(p.border_id_start, 100);
        assert_eq!(p.border_id_end, 200);
    }

    // T5: Partition round-trips through bincode
    #[test]
    fn test_partition_serde() {
        let p = make_empty_partition();
        let bytes = bincode::serialize(&p).unwrap();
        let restored: Partition = bincode::deserialize(&bytes).unwrap();
        assert_eq!(restored.worker_id, p.worker_id);
        assert_eq!(restored.id_range, p.id_range);
        assert_eq!(restored.border_id_start, p.border_id_start);
        assert_eq!(restored.border_id_end, p.border_id_end);
    }

    // E1: Empty partition
    #[test]
    fn test_partition_empty() {
        let p = make_empty_partition();
        assert_eq!(p.subnet.count_live_agents(), 0);
        assert!(p.free_port_index.is_empty());
    }

    // E2: Partition with empty free_port_index
    #[test]
    fn test_partition_no_borders() {
        let p = make_empty_partition();
        assert!(p.free_port_index.is_empty());
        assert_eq!(p.border_id_start, p.border_id_end);
    }

    // -----------------------------------------------------------------------
    // PartitionPlan tests
    // -----------------------------------------------------------------------

    // T1: PartitionPlan has partitions and borders fields
    #[test]
    fn test_partition_plan_fields() {
        let plan = PartitionPlan {
            partitions: vec![make_empty_partition()],
            borders: HashMap::new(),
        };
        assert_eq!(plan.partitions.len(), 1);
        assert!(plan.borders.is_empty());
    }

    // T2: PartitionPlan derives Debug, Clone, Serialize, Deserialize
    #[test]
    fn test_partition_plan_derives() {
        let plan = PartitionPlan {
            partitions: vec![],
            borders: HashMap::new(),
        };
        let cloned = plan.clone();
        assert!(cloned.partitions.is_empty());
        assert!(format!("{:?}", plan).contains("PartitionPlan"));
    }

    // T3: borders is HashMap<u32, (PortRef, PortRef)>
    #[test]
    fn test_partition_plan_borders() {
        let mut borders = HashMap::new();
        borders.insert(1, (PortRef::AgentPort(0, 1), PortRef::AgentPort(2, 0)));
        let plan = PartitionPlan {
            partitions: vec![],
            borders,
        };
        let (a, b) = plan.borders[&1];
        assert_eq!(a, PortRef::AgentPort(0, 1));
        assert_eq!(b, PortRef::AgentPort(2, 0));
    }

    // T4: PartitionPlan round-trips through bincode
    #[test]
    fn test_partition_plan_serde() {
        let plan = PartitionPlan {
            partitions: vec![make_empty_partition()],
            borders: HashMap::new(),
        };
        let bytes = bincode::serialize(&plan).unwrap();
        let restored: PartitionPlan = bincode::deserialize(&bytes).unwrap();
        assert_eq!(restored.partitions.len(), 1);
        assert!(restored.borders.is_empty());
    }

    // E1: Empty plan (no partitions, no borders)
    #[test]
    fn test_partition_plan_empty() {
        let plan = PartitionPlan {
            partitions: vec![],
            borders: HashMap::new(),
        };
        assert!(plan.partitions.is_empty());
        assert!(plan.borders.is_empty());
    }
}
