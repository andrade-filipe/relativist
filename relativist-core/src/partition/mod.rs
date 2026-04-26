//! Net partitioning for distributed reduction (SPEC-04).
//!
//! Splits a Net into K partitions with disjoint agent sets,
//! boundary FreePort markers, and ID space partitioning.
//!
//! SPEC-20 §3.8 amendments:
//! - A3 (R18a): `PartitionPlan::allocate_border_ids` — dynamic border-id allocator.
//! - A4 (R19a): `remap_partition_ids` — renumbers reclaimed partitions for re-split.

pub mod compact;
pub mod departure_recovery;
pub mod helpers;
pub mod remap;
pub mod split;
pub mod strategy;
pub mod types;

// Re-exports: convenience access via `crate::partition::*`
pub use compact::CompactSubnet;
pub use departure_recovery::materialize_reclaimed_partitions;
pub use helpers::{classify_wires, compute_id_ranges, max_freeport_id, WireClassification};
pub use remap::remap_partition_ids;
pub use split::split;
pub use strategy::{ContiguousIdStrategy, PartitionStrategy};
pub use types::{IdRange, LeaveKind, Partition, PartitionPlan, WorkerId};
