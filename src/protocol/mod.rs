//! Wire protocol for coordinator-worker communication (SPEC-06).
//!
//! Defines message types, length-delimited framing with CRC32,
//! and the coordinator/worker orchestration for TCP grid mode.

pub mod config;
pub mod coordinator;
pub mod error;
pub mod frame;
pub mod types;
pub mod worker;

// Re-exports: convenience access via `crate::protocol::*`
pub use config::*;
pub use error::*;
pub use frame::*;
pub use types::*;
