//! Protocol error types (SPEC-06, Section 4.4).
//!
//! Defines ProtocolError covering all wire protocol failure modes:
//! connection loss, payload limits, checksum, serde, etc.

use std::time::Duration;
use thiserror::Error;

/// Possible errors in the wire protocol.
///
/// Covers all failure modes of the framing layer and transport.
/// Coordinator-level errors (WorkerError, WorkerCountMismatch) are
/// defined separately in CoordinatorError (SPEC-13 R16).
#[derive(Debug, Error)]
pub enum ProtocolError {
    /// Connection lost (I/O error in TCP communication).
    #[error("connection lost: {0}")]
    ConnectionLost(#[from] std::io::Error),

    /// Declared payload exceeds the maximum allowed size.
    #[error("payload too large (size {size}, max {max})")]
    PayloadTooLarge { size: u32, max: u32 },

    /// CRC32 checksum of the payload does not match the header declaration.
    #[error("checksum mismatch (expected {expected:08x}, computed {computed:08x})")]
    ChecksumMismatch { expected: u32, computed: u32 },

    /// Bincode deserialization error (bincode v2 — SPEC-18 §3.1).
    #[error("bincode decode error: {0}")]
    Deserialize(#[from] bincode::error::DecodeError),

    /// Bincode serialization error (bincode v2 — SPEC-18 §3.1).
    #[error("bincode encode error: {0}")]
    Serialize(#[from] bincode::error::EncodeError),

    /// Unexpected message for the current FSM state.
    #[error("unexpected message: expected {expected}, received {received}")]
    UnexpectedMessage {
        expected: &'static str,
        received: String,
    },

    /// Timeout exceeded in an operation.
    #[error("timeout in phase {phase} after {elapsed:?}")]
    Timeout {
        phase: &'static str,
        elapsed: Duration,
    },

    /// Authentication failed (SPEC-10).
    #[error("authentication failed: {reason}")]
    AuthFailed { reason: String },

    /// SPEC-20: coordinator-level logic error surfaced via protocol.
    #[error(transparent)]
    Coordinator(#[from] Box<crate::error::CoordinatorError>),

    /// SPEC-20: fatal unrecoverable grid error.
    #[error("fatal error: {0}")]
    Fatal(String),

    /// Frame header carries flag bits that this build does not recognise (SPEC-18 R19).
    #[error("unknown frame flags: 0b{flags:08b}")]
    UnknownFlags { flags: u8 },

    /// LZ4 decompression of a `FLAG_COMPRESSED` payload failed (SPEC-18 R13).
    #[error("LZ4 decompression failed: {0}")]
    DecompressionFailed(String),

    /// Wire-protocol version mismatch detected during the Register handshake (SPEC-18 R29-R30).
    #[error("protocol version mismatch: expected {expected}, received {received}")]
    VersionMismatch { expected: u8, received: u8 },

    /// rkyv archive validation failed (SPEC-18 §3.5 R26).
    #[error("rkyv archive validation failed: {0}")]
    ArchiveValidationFailed(String),

    /// SPEC-19 §3.4 R33c — malformed local_wiring in PendingCommutation.
    #[error("malformed local_wiring in request_id={request_id}: {reason:?}")]
    MalformedLocalWiring {
        request_id: u32,
        reason: MalformedLocalWiringReason,
    },
}

/// Reasons for MalformedLocalWiring (SPEC-19 §3.4 R33c).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MalformedLocalWiringReason {
    SrcSlotOutOfRange { src_slot: u8, symbol_count: u8 },
    SrcPortOutOfRange { src_slot: u8, src_port: u8 },
    TargetSiblingOutOfRange { sibling_slot: u8, symbol_count: u8 },
    DanglingConcreteAgent { agent_id: u32, port: u8 },
    DuplicateSourcePort { src_slot: u8, src_port: u8 },
    ReservedForFuture { border_id: u32 },
    ZeroArity,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_variants_debug() {
        let variants: Vec<ProtocolError> = vec![
            ProtocolError::ConnectionLost(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "test",
            )),
            ProtocolError::PayloadTooLarge {
                size: 1000,
                max: 500,
            },
            ProtocolError::ChecksumMismatch {
                expected: 0xDEAD,
                computed: 0xBEEF,
            },
            ProtocolError::Deserialize(bincode::error::DecodeError::UnexpectedEnd {
                additional: 5,
            }),
            ProtocolError::Serialize(bincode::error::EncodeError::OtherString(
                "test error".into(),
            )),
            ProtocolError::UnexpectedMessage {
                expected: "A",
                received: "B".into(),
            },
            ProtocolError::Timeout {
                phase: "P",
                elapsed: Duration::from_secs(1),
            },
            ProtocolError::AuthFailed { reason: "R".into() },
            ProtocolError::Fatal("F".into()),
            ProtocolError::UnknownFlags { flags: 4 },
            ProtocolError::DecompressionFailed("D".into()),
            ProtocolError::VersionMismatch {
                expected: 2,
                received: 1,
            },
            ProtocolError::ArchiveValidationFailed("V".into()),
        ];
        for v in &variants {
            let _ = format!("{:?}", v);
        }
    }
}
