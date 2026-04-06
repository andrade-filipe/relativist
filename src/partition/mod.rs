//! Net partitioning for distributed reduction (SPEC-04).
//!
//! Splits a Net into K partitions with disjoint agent sets,
//! boundary FreePort markers, and ID space partitioning.

pub mod helpers;
pub mod split;
pub mod strategy;
pub mod types;

// Re-exports: convenience access via `crate::partition::*`
pub use helpers::{classify_wires, compute_id_ranges, max_freeport_id, WireClassification};
pub use split::split;
pub use strategy::{ContiguousIdStrategy, PartitionStrategy};
pub use types::{IdRange, Partition, PartitionPlan, WorkerId};
