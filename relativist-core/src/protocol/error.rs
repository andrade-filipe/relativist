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

    /// Bincode deserialization error (bincode v2 — SPEC-18 §3.1).
    Deserialize(bincode::error::DecodeError),

    /// Bincode serialization error (bincode v2 — SPEC-18 §3.1).
    Serialize(bincode::error::EncodeError),

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

    /// Frame header carries flag bits that this build does not recognise
    /// (SPEC-18 R19). Forward-compat hardening: any reserved bit set in
    /// the v2 frame header's `flags` byte triggers this error so older
    /// builds refuse rather than silently mis-decoding a future frame.
    UnknownFlags { flags: u8 },

    /// LZ4 decompression of a `FLAG_COMPRESSED` payload failed
    /// (SPEC-18 R13). Carries the decoder's error message so the
    /// coordinator can log the precise reason.
    DecompressionFailed(String),

    /// Wire-protocol version mismatch detected during the Register
    /// handshake (SPEC-18 R29-R30, item 2.23 §3.6). Emitted by the
    /// worker when it receives a `RegisterNack` whose reason
    /// indicates that the coordinator runs an incompatible protocol
    /// version. Distinct from `AuthFailed` so daemon-mode workers can
    /// fail fast (no point retrying against an incompatible peer).
    VersionMismatch { expected: u8, received: u8 },

    /// rkyv archive validation failed (SPEC-18 §3.5 R26, item 2.24).
    ///
    /// Emitted in two situations under the zero-copy path:
    ///   1. **Send-side serialization failure** — `rkyv::to_bytes` returned
    ///      an error. The wrapped string is prefixed with `"serialize: "`
    ///      (mandated by SPEC-18 §3.5 — DC-4) so log scrapers can
    ///      distinguish send from receive failures.
    ///   2. **Receive-side validation failure** — `rkyv::access` (the
    ///      validating API mandated by R24 step 3) rejected the archive
    ///      because of layout drift, alignment violation, or out-of-bounds
    ///      pointer. The wrapped string is the rancor error rendering.
    ///
    /// Also used by the recv path to reject FLAG_ARCHIVED frames that
    /// carry non-hot-path messages (R22), with reason
    /// `"non-hot-path message: <variant>"`.
    ///
    /// The variant is unconditional (NOT `#[cfg(feature = "zero-copy")]`)
    /// so default-features builds can still pattern-match on it when
    /// describing rejected frames produced by zero-copy peers.
    ArchiveValidationFailed(String),

    /// SPEC-19 §3.4 R33c — the worker received a `PendingCommutation`
    /// whose `local_wiring` is internally inconsistent. Triggered by
    /// R23a clauses 3/4/6 violations and by R48a stray slot-marker
    /// guards. The coordinator NACKs the worker consistent with R48's
    /// stray-token treatment.
    ///
    /// **NR3-002 absorption (2026-04-23):** this variant MUST be read
    /// as distinct from [`ProtocolError::Deserialize`] / the rkyv
    /// [`ProtocolError::ArchiveValidationFailed`] path.
    /// `Deserialize`/`ArchiveValidationFailed` fire when bincode/rkyv
    /// cannot parse the wire bytes at all (pre-validation);
    /// `MalformedLocalWiring` fires when the bytes decode successfully
    /// but the semantic content violates one of R33c cases 1-3/5-7
    /// (case 4 is a `tracing::warn!` path, not a reject). This split
    /// sharpens R37 wording.
    MalformedLocalWiring {
        request_id: u32,
        reason: MalformedLocalWiringReason,
    },
}

/// SPEC-19 §3.4 R33c reason enumeration for
/// [`ProtocolError::MalformedLocalWiring`]. Every variant corresponds
/// to one R33c case plus (optionally) NR3-003 case 8 (DEFERRED —
/// 2026-04-23 default).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MalformedLocalWiringReason {
    /// Case 1: `src_slot >= pc.target_symbols.len()`.
    SrcSlotOutOfRange { src_slot: u8, symbol_count: u8 },
    /// Case 2: `src_port` exceeds the port count of
    /// `target_symbols[src_slot]`.
    SrcPortOutOfRange { src_slot: u8, src_port: u8 },
    /// Case 3: sibling-slot placeholder target with
    /// `sibling_slot >= pc.target_symbols.len()` — mirrors R48a.
    TargetSiblingOutOfRange { sibling_slot: u8, symbol_count: u8 },
    /// Case 4 (SHOULD warn, not reject): concrete-AgentId target where
    /// the id is absent from the worker's partition. Symmetric to the
    /// resolver's lenient dangling-FreePort path at
    /// `border_resolver.rs:1061-1077`. See R33c case 4.
    DanglingConcreteAgent { agent_id: u32, port: u8 },
    /// Case 5: duplicate `(src_slot, src_port)` keys — two entries
    /// trying to rewrite the same minted port. Detected via R23a
    /// clause 6 HashSet pre-pass.
    DuplicateSourcePort { src_slot: u8, src_port: u8 },
    /// Case 6: `target = PortRef::FreePort(_)`. Reserved for future
    /// wire break (R48a, SC-008).
    ReservedForFuture { border_id: u32 },
    /// Case 7: `pc.target_symbols` is empty (NF-004). Rejected before
    /// R24.1.6a allocation so `minted_ids_per_pc[0]` (consumed by
    /// R24.1.6c echo) is always well-defined.
    ZeroArity,
    // TODO: NR3-003 defer — optional 8th case `TargetSymbolsTooLong
    // { len: u8 }` mirroring the coordinator-side 16-symbol cap at
    // `border_resolver.rs:318-322`. The resolver-side assert already
    // bounds the field; worker-side re-validation is defensive only,
    // not semantically required under G1. Flag as SF for Stage 5 QA.
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
            Self::UnknownFlags { flags } => {
                write!(f, "unknown frame flags: 0b{:08b}", flags)
            }
            Self::DecompressionFailed(reason) => {
                write!(f, "LZ4 decompression failed: {}", reason)
            }
            Self::VersionMismatch { expected, received } => {
                write!(
                    f,
                    "protocol version mismatch: expected {}, received {}",
                    expected, received
                )
            }
            Self::ArchiveValidationFailed(reason) => {
                write!(f, "rkyv archive validation failed: {}", reason)
            }
            Self::MalformedLocalWiring { request_id, reason } => {
                write!(
                    f,
                    "malformed local_wiring in request_id={}: {:?}",
                    request_id, reason
                )
            }
        }
    }
}

impl std::error::Error for ProtocolError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ConnectionLost(e) => Some(e),
            Self::Deserialize(e) => Some(e),
            Self::Serialize(e) => Some(e),
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
            ProtocolError::Deserialize(bincode::error::DecodeError::UnexpectedEnd {
                additional: 5,
            }),
            ProtocolError::Serialize(bincode::error::EncodeError::OtherString(
                "test serialize error".into(),
            )),
            ProtocolError::UnexpectedMessage {
                expected: "PartitionResult",
                received: "Shutdown".into(),
            },
            ProtocolError::Timeout {
                phase: "collect",
                elapsed: Duration::from_secs(600),
            },
            ProtocolError::AuthFailed,
            ProtocolError::UnknownFlags { flags: 0b0000_0100 },
            ProtocolError::DecompressionFailed("invalid block".into()),
            ProtocolError::VersionMismatch {
                expected: 2,
                received: 1,
            },
            ProtocolError::ArchiveValidationFailed("test archive failure".into()),
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

    /// TASK-0346 R7: `DecompressionFailed` Display preserves the inner reason.
    #[test]
    fn decompression_failed_error_renders() {
        let e = ProtocolError::DecompressionFailed("invalid block".into());
        let s = e.to_string();
        assert!(s.contains("LZ4 decompression failed"), "got: {}", s);
        assert!(s.contains("invalid block"), "inner reason missing: {}", s);
    }

    /// TASK-0347 R4: `VersionMismatch` Display surfaces both versions
    /// and the canonical "expected N, received M" phrasing that downstream
    /// log scrapers and the worker NACK parser key on (SPEC-18 R30).
    #[test]
    fn version_mismatch_error_renders() {
        let e = ProtocolError::VersionMismatch {
            expected: 2,
            received: 1,
        };
        let s = e.to_string();
        assert!(s.contains("expected 2"), "got: {}", s);
        assert!(s.contains("received 1"), "got: {}", s);
        assert!(
            s.contains("protocol version mismatch"),
            "missing canonical phrase: {}",
            s
        );
    }

    // -----------------------------------------------------------------------
    // TASK-0354 — ArchiveValidationFailed (SPEC-18 §3.5 R26).
    //
    // The variant is a tuple of one `String`. The Display rendering MUST
    // start with the canonical "rkyv archive validation failed:" phrase
    // (log-scraper / SPEC-18 §3.5 contract) and include the wrapped reason.
    //
    // The variant is unconditional (lives in the default-features build
    // too) so the same pattern-match code can run regardless of feature.
    // -----------------------------------------------------------------------

    /// UT-0354-01: Display contains the canonical phrase and the inner reason.
    #[test]
    fn archive_validation_failed_error_renders() {
        let e = ProtocolError::ArchiveValidationFailed("layout drift".into());
        let s = e.to_string();
        assert!(
            s.contains("rkyv archive validation failed"),
            "missing canonical phrase: {}",
            s
        );
        assert!(s.contains("layout drift"), "missing inner reason: {}", s);
    }

    /// UT-0354-02: Variant is constructible in the default-features build
    /// and pattern-matches as expected (no `#[cfg]` gate on the variant).
    #[test]
    fn archive_validation_failed_unconditional_variant() {
        let e = ProtocolError::ArchiveValidationFailed("ok".into());
        match e {
            ProtocolError::ArchiveValidationFailed(s) => assert_eq!(s, "ok"),
            other => panic!("expected ArchiveValidationFailed, got {:?}", other),
        }
    }

    /// UT-0354-03: Mandated `"serialize: "` prefix (SPEC-18 §3.5 DC-4)
    /// is preserved through the Display wrapper. This is the conventional
    /// shape used by `send_frame_v2` for rkyv `to_bytes` failures.
    #[test]
    fn archive_validation_failed_serialize_prefix() {
        let e = ProtocolError::ArchiveValidationFailed(format!("serialize: {}", "AlignedVec OOM"));
        let s = e.to_string();
        assert!(s.contains("serialize: "), "missing DC-4 prefix: {}", s);
        assert!(s.contains("AlignedVec OOM"), "missing inner cause: {}", s);
    }

    /// UT-0354-04: Debug rendering exists and is non-empty (matches the
    /// other variants' baseline). Backstop for the Debug derive.
    #[test]
    fn archive_validation_failed_debug_non_empty() {
        let e = ProtocolError::ArchiveValidationFailed("x".into());
        assert!(!format!("{:?}", e).is_empty());
    }

    /// UT-0354-05: `source()` returns None for this variant — the wrapped
    /// reason is a flat String, not an `&dyn Error` chain link.
    #[test]
    fn archive_validation_failed_no_source() {
        use std::error::Error as _;
        let e = ProtocolError::ArchiveValidationFailed("y".into());
        assert!(e.source().is_none());
    }
}
