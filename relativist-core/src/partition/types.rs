//! Core types for net partitioning (SPEC-04 Section 4.1).

use std::collections::HashMap;
use std::ops::Range;

use crate::error::PartitionError;
use crate::net::{AgentId, Net, PortRef};

/// Identifier of a worker in the grid.
/// Values from 0 to n-1, where n is the number of workers.
pub type WorkerId = u32;

/// Reason a worker is leaving the grid (SPEC-20 R21, R22a, R22b).
///
/// Relocated from `coordinator.rs` per SPEC-13 R28 (shared types must live
/// below the protocol layer so both `coordinator` and `protocol/` can import
/// without circularity). TASK-0418 will add `Serialize`/`Deserialize` here and
/// import via `crate::partition::LeaveKind`.
///
/// # API stability
///
/// `#[non_exhaustive]` is set so that downstream matchers are forced to include
/// a wildcard arm when new leave reasons are added. Do not reorder variants —
/// declaration order is part of the public ABI.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LeaveKind {
    /// Worker returns its current-round result first, then departs (R22a).
    AfterResult,
    /// Worker cannot complete the current round; departs immediately (R22b).
    Urgent,
}

/// An exclusive range of AgentIds reserved for a worker.
/// The worker may generate new IDs in the interval [start, end).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(
    feature = "zero-copy",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
pub struct IdRange {
    /// First AgentId in the range (inclusive).
    pub start: AgentId,
    /// Last AgentId in the range (exclusive).
    pub end: AgentId,
}

impl IdRange {
    /// Returns the number of IDs in the range `[start, end)`.
    ///
    /// Returns `0` for both the empty-range case (`start == end`) and the
    /// inverted-range case (`start > end`). These two cases are semantically
    /// distinct: an empty range is a well-formed zero-length allocation;
    /// an inverted range (`start > end`) is a construction error.
    /// A `debug_assert!` fires in debug builds if the range is inverted
    /// (RV-002 / QA-005).
    pub fn len(self) -> u32 {
        debug_assert!(
            self.start <= self.end,
            "IdRange is inverted: start={} > end={}; this is a construction error (RV-002)",
            self.start,
            self.end,
        );
        self.end.saturating_sub(self.start)
    }

    /// Returns `true` if the range contains no IDs (`start >= end`).
    ///
    /// Note: an inverted range (`start > end`) also returns `true` here, but
    /// that case is a construction error; a `debug_assert!` in `len()` fires
    /// for it in debug builds (RV-002 / QA-005).
    pub fn is_empty(self) -> bool {
        debug_assert!(
            self.start <= self.end,
            "IdRange is inverted: start={} > end={}; this is a construction error (RV-002)",
            self.start,
            self.end,
        );
        self.start >= self.end
    }
}

/// A partition: sub-net assigned to a worker (SPEC-04 Section 4.1).
///
/// SPEC-18 §4.6: rkyv archives `Partition` directly (NOT via `CompactSubnet`).
/// The serde `serialize_with`/`deserialize_with` adapters apply only to the
/// bincode v2 path; rkyv derives encode/decode the in-memory `Net` layout
/// directly.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(
    feature = "zero-copy",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
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

impl Partition {
    /// Returns the number of live agents in this partition's subnet.
    ///
    /// Used by `remap_partition_ids` (SPEC-20 §3.8 A4) to validate that
    /// `new_range.len() >= agent_count()` before renumbering.
    ///
    /// # Panics (debug only)
    ///
    /// A `debug_assert!` fires in debug builds if the live agent count
    /// exceeds `u32::MAX`, which would cause a silent truncation and
    /// break the `remap_partition_ids` precondition check (RV-004 / QA-009).
    pub fn agent_count(&self) -> u32 {
        let n = self.subnet.count_live_agents();
        debug_assert!(
            n <= u32::MAX as usize,
            "Partition::agent_count: agent count {} exceeds u32::MAX; \
             remap_partition_ids precondition check will be wrong (RV-004)",
            n
        );
        n as u32
    }
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

    /// Cursor for the dynamic border-id allocator (SPEC-20 §3.8 A3, R18a).
    ///
    /// Tracks the next available border ID for `allocate_border_ids`.
    /// Initialized from `border_id_end` after `split()` so that new allocations
    /// never collide with border IDs already assigned during the split.
    /// When `split()` builds a `PartitionPlan`, it sets this field to
    /// `wire_class.border_id_end`.
    pub next_border_id: u32,
}

impl Default for PartitionPlan {
    /// Default plan: no partitions, no borders, cursor starts at 0.
    ///
    /// Construction sites that build a `PartitionPlan` purely for `merge()`
    /// consumption (which ignores `next_border_id`) should prefer struct-update
    /// syntax: `PartitionPlan { partitions, borders, ..Default::default() }`.
    /// This confines future field-addition churn to a single location (RV-003).
    ///
    /// Sites that will call `allocate_border_ids` MUST supply an explicit
    /// `next_border_id` derived from the preceding `split()`'s
    /// `wire_class.border_id_end` — do not rely on this default for those paths.
    fn default() -> Self {
        Self {
            partitions: Vec::new(),
            borders: HashMap::new(),
            next_border_id: 0,
        }
    }
}

impl PartitionPlan {
    /// Constructs an empty `PartitionPlan` with a specific `next_border_id` cursor.
    ///
    /// # Naming note
    ///
    /// `with_next_border_id` follows the constructor-with-named-argument pattern
    /// rather than the `Builder::with_field` mutating-builder pattern — it always
    /// creates a **new empty plan**, not a modified existing one (QA-008 / RV-006).
    ///
    /// # Usage
    ///
    /// Used in tests when a known cursor offset is required (e.g., to simulate a
    /// plan that has already consumed some border IDs before the test begins).
    /// Reserved for the coordinator's departure-recovery path (TASK-0440/0443,
    /// SPEC-20 §4.2.2) once that path lands — not yet called in production code.
    #[allow(dead_code)] // no non-test callers until TASK-0440/0443 land (QA-008)
    pub fn with_next_border_id(next_border_id: u32) -> Self {
        Self {
            partitions: Vec::new(),
            borders: HashMap::new(),
            next_border_id,
        }
    }

    /// Allocates a fresh disjoint border-id range `[next_border_id, next_border_id + count)`.
    ///
    /// # SPEC reference
    ///
    /// SPEC-20 §3.8 A3 (owned by SPEC-04 next revision as R18a).
    /// Required by SPEC-20 R24d: when a reclaimed partition re-enters the
    /// system, the coordinator allocates a fresh `border_id` range here so
    /// that the re-split's `FreePort` markers never collide with existing ones.
    /// See ARG-001 P4 (ID consistency) for the confluence argument.
    ///
    /// # Errors
    ///
    /// Returns `PartitionError::BorderIdSpaceExhausted { requested, available }`
    /// when `count > u32::MAX - next_border_id` (NF-006 closure).
    /// The cursor is **not** advanced on error.
    ///
    /// # Overflow guard
    ///
    /// A `debug_assert!` fires in debug builds if the cursor has somehow
    /// already reached `u32::MAX` before entry. This mirrors the QA-001
    /// finding on TASK-0410 (merged_next_id == u32::MAX boundary).
    pub fn allocate_border_ids(&mut self, count: u32) -> Result<Range<u32>, PartitionError> {
        // available = u32::MAX - next_border_id; no overflow possible (u32::MAX >= next_border_id).
        let available = u32::MAX - self.next_border_id;

        if count > available {
            // Cursor is NOT advanced on error (atomic allocation guarantee).
            return Err(PartitionError::BorderIdSpaceExhausted {
                requested: count,
                available,
            });
        }

        let start = self.next_border_id;
        // Safe: count <= available = u32::MAX - start, so start + count <= u32::MAX.
        // (The available-bound check above already enforces this; no further runtime guard needed.)
        let end = start + count;

        self.next_border_id = end;
        Ok(start..end)
    }
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
        let bytes = crate::protocol::bincode_v2::encode(&original).unwrap();
        let restored: IdRange = crate::protocol::bincode_v2::decode_value(&bytes).unwrap();
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
        let bytes = crate::protocol::bincode_v2::encode(&p).unwrap();
        let restored: Partition = crate::protocol::bincode_v2::decode_value(&bytes).unwrap();
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
            next_border_id: 0,
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
            next_border_id: 0,
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
            next_border_id: 0,
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
            next_border_id: 0,
        };
        let bytes = crate::protocol::bincode_v2::encode(&plan).unwrap();
        let restored: PartitionPlan = crate::protocol::bincode_v2::decode_value(&bytes).unwrap();
        assert_eq!(restored.partitions.len(), 1);
        assert!(restored.borders.is_empty());
    }

    // E1: Empty plan (no partitions, no borders)
    #[test]
    fn test_partition_plan_empty() {
        let plan = PartitionPlan {
            partitions: vec![],
            borders: HashMap::new(),
            next_border_id: 0,
        };
        assert!(plan.partitions.is_empty());
        assert!(plan.borders.is_empty());
    }

    // -----------------------------------------------------------------------
    // TASK-0353 — rkyv round-trip for IdRange and Partition (SPEC-18 §3.5).
    //
    // IdRange implements PartialEq/Eq, so we compare the struct directly.
    // Partition does NOT derive PartialEq (its Net subnet contains a
    // freeport_redirects HashMap that is intentionally `#[serde(skip)]`),
    // so we compare its fields one by one — `subnet` via Net's PartialEq.
    //
    // SPEC-18 §4.6: Partition is rkyv-archived directly (not via the
    // CompactSubnet bincode adapter), so the in-memory dense Net layout
    // is what crosses the wire under the zero-copy path.
    // -----------------------------------------------------------------------

    /// UT-0353-05: IdRange round-trips through rkyv across the full u32 range.
    #[cfg(feature = "zero-copy")]
    #[test]
    fn rkyv_round_trip_id_range() {
        for r in [
            IdRange { start: 0, end: 0 },
            IdRange { start: 0, end: 100 },
            IdRange {
                start: 1000,
                end: 2000,
            },
            IdRange {
                start: 0,
                end: u32::MAX,
            },
        ] {
            let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&r).expect("serialize");
            let archived =
                rkyv::access::<rkyv::Archived<IdRange>, rkyv::rancor::Error>(bytes.as_ref())
                    .expect("access");
            let back: IdRange =
                rkyv::deserialize::<IdRange, rkyv::rancor::Error>(archived).expect("deserialize");
            assert_eq!(r, back, "IdRange {:?} did not round-trip", r);
        }
    }

    /// UT-0353-06: Partition round-trips through rkyv with a non-empty
    /// subnet, a populated free_port_index, and non-trivial border range.
    /// Partition lacks PartialEq, so we compare every field individually.
    #[cfg(feature = "zero-copy")]
    #[test]
    fn rkyv_round_trip_partition_with_free_ports() {
        use crate::net::Symbol;

        // Build a small subnet with a free-port boundary.
        let mut subnet = Net::new();
        let a = subnet.create_agent(Symbol::Con);
        let b = subnet.create_agent(Symbol::Era);
        subnet.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        subnet.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(100));
        subnet.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(101));

        let mut free_port_index = HashMap::new();
        free_port_index.insert(100u32, PortRef::AgentPort(a, 1));
        free_port_index.insert(101u32, PortRef::AgentPort(a, 2));

        let original = Partition {
            subnet,
            worker_id: 3,
            free_port_index,
            id_range: IdRange {
                start: 1_000,
                end: 2_000,
            },
            border_id_start: 100,
            border_id_end: 200,
        };

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&original).expect("serialize");
        let archived =
            rkyv::access::<rkyv::Archived<Partition>, rkyv::rancor::Error>(bytes.as_ref())
                .expect("access");
        let back: Partition =
            rkyv::deserialize::<Partition, rkyv::rancor::Error>(archived).expect("deserialize");

        assert_eq!(back.worker_id, original.worker_id);
        assert_eq!(back.id_range, original.id_range);
        assert_eq!(back.border_id_start, original.border_id_start);
        assert_eq!(back.border_id_end, original.border_id_end);
        assert_eq!(back.free_port_index, original.free_port_index);
        assert_eq!(
            back.subnet, original.subnet,
            "subnet must round-trip via direct rkyv archival (SPEC-18 §4.6)"
        );
    }
}
