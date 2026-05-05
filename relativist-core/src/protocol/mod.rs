//! Wire protocol for coordinator-worker communication (SPEC-06).
//!
//! Defines message types, length-delimited framing with CRC32,
//! and the coordinator/worker orchestration for TCP grid mode.

pub mod bincode_v2;
pub mod channel;
pub mod compression;
pub mod config;
pub mod coordinator;
pub mod error;
pub mod frame;
pub mod retained;
pub mod self_worker;
pub mod tcp;
pub mod timers;
pub mod transport;
pub mod types;
#[cfg(unix)]
pub mod unix;
pub mod worker;

// SPEC-18 §3.5 (item 2.24) — zero-copy integration test suite (T11-T14).
// Lives under the `zero-copy` cargo feature only.
#[cfg(all(test, feature = "zero-copy"))]
mod zero_copy_tests;

// SPEC-19 §3.4 (item 2.26-A) — delta-protocol wire-layer integration
// tests. Feature-agnostic (rides plain `send_frame_with_threshold`;
// SPEC-18 R22 whitelists `AssignPartition` / `PartitionResult` only for
// the rkyv fast path, so delta variants ride bincode).
#[cfg(test)]
mod delta_wire_tests;

// Re-exports: convenience access via `crate::protocol::*`
pub use config::*;
pub use error::*;
pub use frame::*;
pub use retained::*;
pub use timers::*;
pub use transport::{create_transport, Transport, TransportStream};
pub use types::*;

// SPEC-19 §3.4 DC-A1 — re-export the merge-owned border-level wire
// structs under `crate::protocol::*` so downstream delta-protocol
// callers can name them via a single path. The structs stay in
// `merge/` (pure-core, R19) per SPEC-13 R6-R8 layering.
pub use crate::merge::{BorderDelta, LocalReconnection, MintedAgent, PendingCommutation};
