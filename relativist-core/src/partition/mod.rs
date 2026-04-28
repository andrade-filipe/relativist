//! Net partitioning for distributed reduction (SPEC-04).
//!
//! Splits a Net into K partitions with disjoint agent sets,
//! boundary FreePort markers, and ID space partitioning.
//!
//! SPEC-20 §3.8 amendments:
//! - A3 (R18a): `PartitionPlan::allocate_border_ids` — dynamic border-id allocator.
//! - A4 (R19a): `remap_partition_ids` — renumbers reclaimed partitions for re-split.
//!
//! **Phase D Option A (2026-04-27):** `materialize_reclaimed_partitions`
//! and the `departure_recovery` module were removed. Their callers in
//! `protocol/coordinator.rs` were corrupting inputs (R24c violation per
//! MF-009 D and a placeholder `IdRange { 0, 100_000 }` per MF-003/QA-003).
//! With `elastic_departure = false` enforced as v2.0 default, no caller
//! remains; the helper was correct in isolation but its only call site is
//! gone. The full reclaim + reconstruct path is deferred to v2.1.

pub mod compact;
pub mod helpers;
pub mod remap;
pub mod split;
pub mod strategy;
pub mod streaming;
pub mod types;

// Re-exports: convenience access via `crate::partition::*`
pub use compact::CompactSubnet;
pub use helpers::{classify_wires, compute_id_ranges, max_freeport_id, WireClassification};
pub use remap::remap_partition_ids;
pub use split::split;
pub use strategy::{ContiguousIdStrategy, PartitionStrategy};
pub use streaming::{
    AgentBatch, ChunkedPartitionResult, ConnectionDirective, StreamingPartitionStats,
    StreamingPartitionStrategy,
};
pub use types::{IdRange, LeaveKind, Partition, PartitionConfig, PartitionPlan, WorkerId};
