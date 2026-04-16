//! Wire protocol for coordinator-worker communication (SPEC-06).
//!
//! Defines message types, length-delimited framing with CRC32,
//! and the coordinator/worker orchestration for TCP grid mode.

pub mod channel;
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

// Re-exports: convenience access via `crate::protocol::*`
pub use config::*;
pub use error::*;
pub use frame::*;
pub use transport::{create_transport, Transport, TransportStream};
pub use types::*;
