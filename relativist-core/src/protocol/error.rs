//! Protocol error types (SPEC-06, Section 4.4).
//!
//! Defines ProtocolError with 8 variants covering all wire protocol
//! failure modes: connection loss, payload limits, checksum, serde, etc.
//!
//! This is the canonical definition. SPEC-13 R16 provides a high-level
//! sketch; this definition is authoritative for field names and types.

use std::time::Duration;

/// Possible errors in the wire protocol.
///
/// Covers all failure modes of the framing layer and transport.
/// Coordinator-level errors (WorkerError, WorkerCountMismatch) are
/// defined separately in CoordinatorError (SPEC-13 R16).
#[derive(Debug)]
pub enum ProtocolError {
    /// Connection lost (I/O error in TCP communication).
    /// Named `ConnectionLost` (SPEC-13 convention) rather than `Io`
    /// for clarity.
    ConnectionLost(std::io::Error),

    /// Declared payload exceeds the maximum allowed size.
    /// Field types are `u32` consistent with the frame header's `length` field.
    PayloadTooLarge { size: u32, max: u32 },

    /// CRC32 checksum of the payload does not match the header declaration.
    /// Structured fields enable diagnostics.
    ChecksumMismatch { expected: u32, computed: u32 },

    /// Bincode deserialization error.
    Deserialize(bincode::Error),

    /// Bincode serialization error.
    Serialize(bincode::Error),

    /// Unexpected message for the current FSM state.
    /// E.g., worker received PartitionResult, or coordinator received AssignPartition.
    UnexpectedMessage {
        expected: &'static str,
        received: String,
    },

    /// Timeout exceeded in an operation.
    Timeout {
        phase: &'static str,
        elapsed: Duration,
    },

    /// Authentication failed (SPEC-10).
    /// Emitted when a worker's Register message contains an invalid
    /// or missing auth token.
    AuthFailed,
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConnectionLost(e) => write!(f, "connection lost: {}", e),
            Self::PayloadTooLarge { size, max } => {
                write!(f, "payload too large: {} bytes (max {})", size, max)
            }
            Self::ChecksumMismatch { expected, computed } => {
                write!(
                    f,
                    "checksum mismatch: expected 0x{:08x}, computed 0x{:08x}",
                    expected, computed
                )
            }
            Self::Deserialize(e) => write!(f, "deserialization error: {}", e),
            Self::Serialize(e) => write!(f, "serialization error: {}", e),
            Self::UnexpectedMessage { expected, received } => {
                write!(
                    f,
                    "unexpected message: expected {}, received {}",
                    expected, received
                )
            }
            Self::Timeout { phase, elapsed } => {
                write!(f, "timeout in {}: {:?}", phase, elapsed)
            }
            Self::AuthFailed => write!(f, "authentication failed"),
        }
    }
}

impl std::error::Error for ProtocolError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ConnectionLost(e) => Some(e),
            Self::Deserialize(e) => Some(e.as_ref()),
            Self::Serialize(e) => Some(e.as_ref()),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ProtocolError {
    fn from(e: std::io::Error) -> Self {
        Self::ConnectionLost(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // T1: Each variant can be created and has Debug output
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
            ProtocolError::Deserialize(bincode::deserialize::<u64>(&[0u8; 3]).unwrap_err()),
            ProtocolError::Serialize(Box::new(bincode::ErrorKind::Custom(
                "test serialize error".into(),
            ))),
            ProtocolError::UnexpectedMessage {
                expected: "PartitionResult",
                received: "Shutdown".into(),
            },
            ProtocolError::Timeout {
                phase: "collect",
                elapsed: Duration::from_secs(600),
            },
            ProtocolError::AuthFailed,
        ];
        for v in &variants {
            let debug = format!("{:?}", v);
            assert!(!debug.is_empty());
        }
    }

    // T2: From<io::Error> converts to ConnectionLost
    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::ConnectionReset, "reset");
        let proto_err: ProtocolError = io_err.into();
        assert!(matches!(proto_err, ProtocolError::ConnectionLost(_)));
    }

    // T3: Display messages are descriptive
    #[test]
    fn test_display_messages() {
        let err = ProtocolError::PayloadTooLarge {
            size: 999,
            max: 100,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("999"));
        assert!(msg.contains("100"));

        let err = ProtocolError::ChecksumMismatch {
            expected: 0xAB,
            computed: 0xCD,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("000000ab"));
        assert!(msg.contains("000000cd"));

        let err = ProtocolError::AuthFailed;
        assert_eq!(format!("{}", err), "authentication failed");
    }

    // T4: AuthFailed has no fields
    #[test]
    fn test_auth_failed_no_fields() {
        let err = ProtocolError::AuthFailed;
        assert!(matches!(err, ProtocolError::AuthFailed));
    }

    // T5: Timeout variant holds phase and elapsed
    #[test]
    fn test_timeout_fields() {
        let err = ProtocolError::Timeout {
            phase: "distribute",
            elapsed: Duration::from_secs(60),
        };
        if let ProtocolError::Timeout { phase, elapsed } = err {
            assert_eq!(phase, "distribute");
            assert_eq!(elapsed, Duration::from_secs(60));
        } else {
            panic!("wrong variant");
        }
    }
}
