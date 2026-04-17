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
pub mod tcp;
pub mod transport;
pub mod types;
#[cfg(unix)]
pub mod unix;
pub mod worker;

// SPEC-18 §3.5 (item 2.24) — zero-copy integration test suite (T11-T14).
// Lives under the `zero-copy` cargo feature only.
#[cfg(all(test, feature = "zero-copy"))]
mod zero_copy_tests;

// Re-exports: convenience access via `crate::protocol::*`
pub use config::*;
pub use error::*;
pub use frame::*;
pub use transport::{create_transport, Transport, TransportStream};
pub use types::*;
