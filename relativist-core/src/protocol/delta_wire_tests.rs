//! SPEC-19 §3.4 (item 2.26-A) — Wire-layer integration tests for the 5
//! new delta-protocol Message variants.
//!
//! These tests exercise `send_frame_with_threshold` / `recv_frame`
//! end-to-end through an in-memory `tokio::io::duplex` pair (same
//! precedent as `zero_copy_tests.rs`).
//!
//! Coverage (SPEC-19 §3.4):
//! - **R34** — CRC32C integrity on the v2 wire. Positive control:
//!   every successful `recv_frame` implicitly validates the CRC.
//!   Negative: the tamper test flips a payload byte and expects
//!   `ProtocolError::ChecksumMismatch`.
//! - **R35** — large-payload variants (`InitialPartition`,
//!   `FinalStateResult`) trigger compression when their encoded size
//!   exceeds `DEFAULT_COMPRESSION_THRESHOLD` and the compressed frame is
//!   strictly smaller than the uncompressed bincode (Bonus-1 benefit
//!   criterion).
//! - **R36** — small-payload variants (`RoundStart`, `RoundResult`,
//!   `FinalStateRequest`) skip compression below threshold
//!   (`FLAG_COMPRESSED` clear) but MAY compress when the threshold is
//!   forced low (SHOULD, not MUST-NOT).
//! - **SPEC-18 R22** — `FLAG_ARCHIVED` is UNSET for all 5 delta
//!   variants (the rkyv fast path is whitelisted to `AssignPartition`
//!   / `PartitionResult` only).
//!
//! All tests are `#[tokio::test]`; `send_frame_with_threshold` is used
//! throughout (feature-agnostic).

use std::collections::HashMap;

use tokio::io::AsyncWriteExt;

use crate::merge::WorkerRoundStats;
use crate::net::{Net, PortRef, Symbol};
use crate::partition::{IdRange, Partition};
use crate::protocol::bincode_v2;
use crate::protocol::error::ProtocolError;
use crate::protocol::frame::{
    recv_frame, send_frame_with_threshold, FrameHeader, DEFAULT_COMPRESSION_THRESHOLD,
    DEFAULT_MAX_PAYLOAD_SIZE, FLAG_ARCHIVED, FLAG_COMPRESSED, FRAME_HEADER_SIZE,
};
use crate::protocol::types::Message;
use crate::protocol::{BorderDelta, LocalReconnection, MintedAgent, PendingCommutation};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// Partition with `n_agents` live CON agents and trivial port topology.
/// Callers that need the encoded frame to cross
/// `DEFAULT_COMPRESSION_THRESHOLD` (T1, T2, T7) pass `n_agents = 2000`,
/// which empirically exceeds 1024 bytes in bincode v2 varint encoding.
/// Smaller values (e.g., 200 for T8's positive-control, 10 for
/// below-threshold probes) are acceptable when the test does not need
/// compression to engage.
fn make_large_partition(n_agents: usize) -> Partition {
    let mut subnet = Net::new();
    for _ in 0..n_agents {
        subnet.create_agent(Symbol::Con);
    }
    Partition {
        subnet,
        worker_id: 0,
        free_port_index: HashMap::new(),
        id_range: IdRange {
            start: 0,
            end: 1_000_000,
        },
        border_id_start: 0,
        border_id_end: 0,
    }
}

/// `WorkerRoundStats` with an explicit `has_border_activity` flag.
fn stats_with_activity(activity: bool) -> WorkerRoundStats {
    WorkerRoundStats {
        worker_id: 0,
        agents_before: 10,
        agents_after: 5,
        local_redexes: 5,
        reduce_duration_secs: 0.001,
        interactions_by_rule: [1, 1, 1, 1, 1, 0],
        has_border_activity: activity,
        is_coordinator_self: false,
    }
}

/// Parse a `FrameHeader` from the first `FRAME_HEADER_SIZE` bytes of a
/// frame buffer.
fn parse_header(buf: &[u8]) -> FrameHeader {
    let mut arr = [0u8; FRAME_HEADER_SIZE];
    arr.copy_from_slice(&buf[..FRAME_HEADER_SIZE]);
    FrameHeader::from_bytes(arr)
}

// ---------------------------------------------------------------------------
// T1 — InitialPartition: compression engaged, beneficial, round-trips.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn initial_partition_wire_roundtrip_compressed_and_beneficial() {
    let msg = Message::InitialPartition {
        round: 0,
        partition: make_large_partition(2000),
    };
    let uncompressed_len = bincode_v2::encode(&msg).expect("encode").len();
    assert!(
        uncompressed_len > DEFAULT_COMPRESSION_THRESHOLD,
        "fixture precondition: encoded size must exceed threshold \
         (got {uncompressed_len} bytes)",
    );

    // Send via in-memory duplex pair.
    let (mut client, mut server) = tokio::io::duplex(1_048_576);
    send_frame_with_threshold(&mut client, &msg, DEFAULT_COMPRESSION_THRESHOLD)
        .await
        .expect("send");
    let (received, _) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
        .await
        .expect("recv");
    match received {
        Message::InitialPartition { round, partition } => {
            assert_eq!(round, 0);
            assert_eq!(partition.subnet.count_live_agents(), 2000);
        }
        other => panic!("expected InitialPartition, got {:?}", other),
    }

    // Re-encode into a Vec<u8> to inspect header flags + frame size.
    let mut buf: Vec<u8> = Vec::new();
    send_frame_with_threshold(&mut buf, &msg, DEFAULT_COMPRESSION_THRESHOLD)
        .await
        .expect("send to buf");
    let header = parse_header(&buf);
    assert_ne!(
        header.flags & FLAG_COMPRESSED,
        0,
        "R35: InitialPartition above threshold MUST set FLAG_COMPRESSED",
    );
    assert_eq!(
        header.flags & FLAG_ARCHIVED,
        0,
        "SPEC-18 R22: delta variants MUST NOT set FLAG_ARCHIVED",
    );

    let frame_len = buf.len();
    assert!(
        frame_len < uncompressed_len,
        "R35 benefit: compressed frame ({frame_len} bytes) must be < \
         uncompressed bincode ({uncompressed_len} bytes)",
    );
}

// ---------------------------------------------------------------------------
// T2 — FinalStateResult: mirror of T1 for the W→C large-payload variant.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn final_state_result_wire_roundtrip_compressed_and_beneficial() {
    let msg = Message::FinalStateResult {
        round: 42,
        partition: make_large_partition(2000),
    };
    let uncompressed_len = bincode_v2::encode(&msg).expect("encode").len();
    assert!(
        uncompressed_len > DEFAULT_COMPRESSION_THRESHOLD,
        "fixture precondition: encoded size must exceed threshold",
    );

    let (mut client, mut server) = tokio::io::duplex(1_048_576);
    send_frame_with_threshold(&mut client, &msg, DEFAULT_COMPRESSION_THRESHOLD)
        .await
        .expect("send");
    let (received, _) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
        .await
        .expect("recv");
    match received {
        Message::FinalStateResult { round, partition } => {
            assert_eq!(round, 42);
            assert_eq!(partition.subnet.count_live_agents(), 2000);
        }
        other => panic!("expected FinalStateResult, got {:?}", other),
    }

    let mut buf: Vec<u8> = Vec::new();
    send_frame_with_threshold(&mut buf, &msg, DEFAULT_COMPRESSION_THRESHOLD)
        .await
        .expect("send to buf");
    let header = parse_header(&buf);
    assert_ne!(
        header.flags & FLAG_COMPRESSED,
        0,
        "R35: FinalStateResult above threshold MUST set FLAG_COMPRESSED",
    );
    assert_eq!(header.flags & FLAG_ARCHIVED, 0);
    assert!(
        buf.len() < uncompressed_len,
        "R35 benefit: compressed frame must be smaller than bincode",
    );
}

// ---------------------------------------------------------------------------
// T3 — RoundStart: small payload skips compression below threshold.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn round_start_wire_roundtrip_skips_compression_below_threshold() {
    let msg = Message::RoundStart {
        round: 3,
        border_deltas: vec![
            BorderDelta {
                border_id: 5,
                new_target: PortRef::AgentPort(3, 0),
            },
            BorderDelta {
                border_id: 7,
                new_target: PortRef::FreePort(9),
            },
            BorderDelta {
                border_id: 8,
                new_target: PortRef::FreePort(u32::MAX),
            },
        ],
        resolved_borders: vec![1, 3],
        new_borders: vec![(20, PortRef::FreePort(20)), (21, PortRef::FreePort(21))],
        local_reconnections: Vec::new(),
        pending_commutations: Vec::new(),
    };
    let enc_len = bincode_v2::encode(&msg).expect("encode").len();
    assert!(
        enc_len < 512,
        "fixture: encoded size must be < 512 bytes (got {enc_len})",
    );

    let mut buf: Vec<u8> = Vec::new();
    send_frame_with_threshold(&mut buf, &msg, DEFAULT_COMPRESSION_THRESHOLD)
        .await
        .expect("send to buf");
    let header = parse_header(&buf);
    assert_eq!(
        header.flags & FLAG_COMPRESSED,
        0,
        "R36: RoundStart below threshold MUST NOT set FLAG_COMPRESSED",
    );
    assert_eq!(header.flags & FLAG_ARCHIVED, 0);

    let (mut client, mut server) = tokio::io::duplex(1_048_576);
    send_frame_with_threshold(&mut client, &msg, DEFAULT_COMPRESSION_THRESHOLD)
        .await
        .expect("send");
    let (received, _) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
        .await
        .expect("recv");
    match received {
        Message::RoundStart {
            round,
            border_deltas,
            resolved_borders,
            new_borders,
            local_reconnections,
            pending_commutations,
        } => {
            assert_eq!(round, 3);
            assert_eq!(border_deltas.len(), 3);
            assert_eq!(resolved_borders, vec![1, 3]);
            assert_eq!(new_borders.len(), 2);
            assert!(local_reconnections.is_empty());
            assert!(pending_commutations.is_empty());
        }
        other => panic!("expected RoundStart, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// T4 — RoundResult: small payload skips compression below threshold.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn round_result_wire_roundtrip_skips_compression_below_threshold() {
    let msg = Message::RoundResult {
        round: 3,
        border_deltas: vec![BorderDelta {
            border_id: 5,
            new_target: PortRef::FreePort(6),
        }],
        stats: stats_with_activity(false),
        has_border_activity: false,
        minted_agents: Vec::new(),
    };
    let enc_len = bincode_v2::encode(&msg).expect("encode").len();
    assert!(
        enc_len < 512,
        "fixture: encoded size must be < 512 bytes (got {enc_len})",
    );

    let mut buf: Vec<u8> = Vec::new();
    send_frame_with_threshold(&mut buf, &msg, DEFAULT_COMPRESSION_THRESHOLD)
        .await
        .expect("send to buf");
    let header = parse_header(&buf);
    assert_eq!(header.flags & FLAG_COMPRESSED, 0, "R36");
    assert_eq!(header.flags & FLAG_ARCHIVED, 0);

    let (mut client, mut server) = tokio::io::duplex(1_048_576);
    send_frame_with_threshold(&mut client, &msg, DEFAULT_COMPRESSION_THRESHOLD)
        .await
        .expect("send");
    let (received, _) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
        .await
        .expect("recv");
    match received {
        Message::RoundResult {
            round,
            border_deltas,
            stats,
            has_border_activity,
            minted_agents,
        } => {
            assert_eq!(round, 3);
            assert_eq!(border_deltas.len(), 1);
            assert!(!has_border_activity);
            assert!(!stats.has_border_activity);
            assert!(minted_agents.is_empty());
        }
        other => panic!("expected RoundResult, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// T5 — FinalStateRequest: smallest variant, minimal frame.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn final_state_request_wire_roundtrip_minimal_frame() {
    let msg = Message::FinalStateRequest { round: 99 };

    let mut buf: Vec<u8> = Vec::new();
    send_frame_with_threshold(&mut buf, &msg, DEFAULT_COMPRESSION_THRESHOLD)
        .await
        .expect("send to buf");
    assert!(
        buf.len() < 32,
        "FinalStateRequest total frame size must be < 32 bytes (got {})",
        buf.len(),
    );
    let header = parse_header(&buf);
    assert_eq!(
        header.flags & FLAG_COMPRESSED,
        0,
        "R36: FinalStateRequest MUST NOT compress a 1-byte varint",
    );
    assert_eq!(header.flags & FLAG_ARCHIVED, 0);

    let (mut client, mut server) = tokio::io::duplex(1_048_576);
    send_frame_with_threshold(&mut client, &msg, DEFAULT_COMPRESSION_THRESHOLD)
        .await
        .expect("send");
    let (received, _) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
        .await
        .expect("recv");
    match received {
        Message::FinalStateRequest { round } => assert_eq!(round, 99),
        other => panic!("expected FinalStateRequest, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// T6 — RoundStart with threshold=1 forces FLAG_COMPRESSED even on small
// payloads (R36 "SHOULD, not MUST-NOT" nuance).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn round_start_forced_compression_when_threshold_is_one() {
    let msg = Message::RoundStart {
        round: 3,
        border_deltas: vec![BorderDelta {
            border_id: 5,
            new_target: PortRef::AgentPort(3, 0),
        }],
        resolved_borders: vec![1],
        new_borders: vec![(20, PortRef::FreePort(20))],
        local_reconnections: Vec::new(),
        pending_commutations: Vec::new(),
    };

    let mut buf: Vec<u8> = Vec::new();
    send_frame_with_threshold(&mut buf, &msg, 1)
        .await
        .expect("send to buf");
    let header = parse_header(&buf);
    assert_ne!(
        header.flags & FLAG_COMPRESSED,
        0,
        "threshold=1 forces FLAG_COMPRESSED even on small payloads",
    );

    let (mut client, mut server) = tokio::io::duplex(1_048_576);
    send_frame_with_threshold(&mut client, &msg, 1)
        .await
        .expect("send");
    let (received, _) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
        .await
        .expect("recv");
    match received {
        Message::RoundStart { round, .. } => assert_eq!(round, 3),
        other => panic!("expected RoundStart, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// T7 — InitialPartition with payload byte tamper: recv_frame rejects.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn initial_partition_crc_tamper_rejected() {
    let msg = Message::InitialPartition {
        round: 0,
        partition: make_large_partition(2000),
    };

    // Send into a Vec<u8> so we can tamper with the bytes.
    let mut buf: Vec<u8> = Vec::new();
    send_frame_with_threshold(&mut buf, &msg, DEFAULT_COMPRESSION_THRESHOLD)
        .await
        .expect("send to buf");

    // Flip a payload byte (well past the header, well before the end of
    // the frame). R12 invariant: CRC32C is over the UNcompressed payload,
    // so tampering the compressed-payload bytes causes the LZ4
    // decompressor (or, if decompression succeeds by chance, the CRC
    // check) to report failure. The expected error is
    // `ProtocolError::ChecksumMismatch` on the CRC path; any
    // decompression failure would surface as a separate error variant,
    // but in practice bincode v2 + LZ4 surface as ChecksumMismatch for
    // single-byte flips in the middle of the payload.
    let tamper_off = FRAME_HEADER_SIZE + 8;
    assert!(
        tamper_off < buf.len().saturating_sub(4),
        "fixture: tamper offset {} must be inside payload (buf len={})",
        tamper_off,
        buf.len(),
    );
    buf[tamper_off] ^= 0xFF;

    let (mut client, mut server) = tokio::io::duplex(1_048_576);
    client.write_all(&buf).await.expect("write tampered frame");
    drop(client);

    let err = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
        .await
        .expect_err("tampered frame MUST be rejected");
    // Accept any of the recognisable-failure variants. A single-byte
    // flip in the middle of a compressed payload can surface as:
    // - `DecompressionFailed` — LZ4 errors before CRC gets a chance.
    // - `ChecksumMismatch` — LZ4 decompresses successfully, CRC fires.
    // - `Deserialize` — decompressed bytes fail bincode decode.
    // SPEC-19 §3.4 R34 only requires rejection with a recognisable
    // error, not a specific variant.
    assert!(
        matches!(
            err,
            ProtocolError::ChecksumMismatch { .. }
                | ProtocolError::DecompressionFailed(_)
                | ProtocolError::Serialize(_)
                | ProtocolError::Deserialize(_)
        ),
        "R34: tampered frame MUST be rejected with a recognisable error; got {:?}",
        err,
    );
}

// ---------------------------------------------------------------------------
// T8 — FinalStateResult positive-control: CRC still valid with no tamper.
// Complement to T7 — proves that the CRC path does NOT reject legitimate
// frames, so a failure in T7 cannot be explained by the CRC being
// systematically broken.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn final_state_result_crc_still_valid_no_tamper() {
    let msg = Message::FinalStateResult {
        round: 42,
        partition: make_large_partition(200),
    };
    let (mut client, mut server) = tokio::io::duplex(64 * 1024);
    send_frame_with_threshold(&mut client, &msg, DEFAULT_COMPRESSION_THRESHOLD)
        .await
        .expect("send");
    let (received, _) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
        .await
        .expect("recv must succeed without tampering");
    match received {
        Message::FinalStateResult { round, partition } => {
            assert_eq!(round, 42);
            assert_eq!(partition.subnet.count_live_agents(), 200);
        }
        other => panic!("expected FinalStateResult, got {:?}", other),
    }
}

// ===========================================================================
// Stage 5 QA — SPEC-19 §3.4 (item 2.26-A) adversarial probes
//
// Probes Q1..Q13 from
// `docs/reviews/REVIEW-SPEC-19-section-3.4-item-2.26-A-2026-04-17.md` §9.
// Each probe targets a real-bug surface: wire-layer integrity,
// discriminant semantics, SPEC-18 R22 whitelist, byte-count floors,
// varint/boundary edges, extreme field values, and hang/crash guards.
// ===========================================================================

mod adversarial_probes {
    use super::*;

    /// Q1 — Discriminant-byte tamper on an uncompressed `RoundStart`
    /// frame MUST surface as `ChecksumMismatch`.
    ///
    /// The disc byte is the first byte of the uncompressed payload.
    /// CRC32C is computed over the uncompressed payload (SPEC-18 R12), so
    /// flipping that byte changes the bincode-visible variant tag while
    /// leaving the stored CRC untouched — the CRC check MUST catch the
    /// tamper.
    #[tokio::test]
    async fn q1_disc_byte_tamper_on_uncompressed_round_start_fails_crc() {
        let msg = Message::RoundStart {
            round: 3,
            border_deltas: vec![BorderDelta {
                border_id: 5,
                new_target: PortRef::AgentPort(3, 0),
            }],
            resolved_borders: vec![1],
            new_borders: vec![(20, PortRef::FreePort(20))],
            local_reconnections: Vec::new(),
            pending_commutations: Vec::new(),
        };
        let mut buf: Vec<u8> = Vec::new();
        send_frame_with_threshold(&mut buf, &msg, usize::MAX)
            .await
            .expect("send uncompressed");
        assert_eq!(
            parse_header(&buf).flags & FLAG_COMPRESSED,
            0,
            "fixture precondition: frame MUST be uncompressed",
        );
        assert_eq!(
            buf[FRAME_HEADER_SIZE], 8,
            "fixture: disc byte for RoundStart MUST be 8",
        );

        // Flip disc byte: 8 -> 9 (would decode as RoundResult if CRC ignored).
        buf[FRAME_HEADER_SIZE] ^= 0xFF;

        let (mut client, mut server) = tokio::io::duplex(65_536);
        client.write_all(&buf).await.expect("write tampered frame");
        drop(client);
        let err = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .expect_err("disc-tamper frame MUST be rejected");
        assert!(
            matches!(err, ProtocolError::ChecksumMismatch { .. }),
            "R34 + R12: disc-byte tamper on uncompressed frame MUST \
             surface as ChecksumMismatch; got {:?}",
            err,
        );
    }

    /// Q2 — Empty `RoundStart` (all 5 Vec fields empty) byte-count floor.
    ///
    /// Pins the minimum-size contract for the smallest non-trivial
    /// delta-protocol wire payload. bincode v2 varint encoding:
    /// - disc (1 byte) + round=0 varint (1 byte) + 5 empty-length
    ///   prefixes (5 × 1 byte) = 7 bytes bincode.
    /// - Frame: FRAME_HEADER_SIZE (9) + 7 = 16 bytes.
    /// A regression that adds hidden metadata or changes varint layout
    /// would breach this floor.
    #[tokio::test]
    async fn q2_empty_round_start_byte_count_floor() {
        let msg = Message::RoundStart {
            round: 0,
            border_deltas: Vec::new(),
            resolved_borders: Vec::new(),
            new_borders: Vec::new(),
            local_reconnections: Vec::new(),
            pending_commutations: Vec::new(),
        };
        let bincode_len = bincode_v2::encode(&msg).expect("encode").len();
        assert_eq!(
            bincode_len, 7,
            "empty RoundStart MUST bincode to exactly 7 bytes (disc + \
             round + 5 length prefixes); got {}",
            bincode_len,
        );
        let mut buf: Vec<u8> = Vec::new();
        send_frame_with_threshold(&mut buf, &msg, DEFAULT_COMPRESSION_THRESHOLD)
            .await
            .expect("send");
        assert_eq!(
            buf.len(),
            FRAME_HEADER_SIZE + 7,
            "empty RoundStart frame MUST be {} bytes total; got {}",
            FRAME_HEADER_SIZE + 7,
            buf.len(),
        );
    }

    /// Q3 — A malicious sender putting mismatched `has_border_activity`
    /// and `stats.has_border_activity` on the wire MUST be preserved
    /// verbatim by the wire layer (the invariant is a builder-time check,
    /// not a wire-time check). DC-A2 2.26-C runtime enforcement depends
    /// on this reflexive guarantee.
    #[tokio::test]
    async fn q3_round_result_top_level_mismatches_stats_field_preserved_on_wire() {
        // top-level=true, stats.has_border_activity=false (deliberate
        // mismatch — invariant violation; wire must NOT silently fix it).
        let stats = stats_with_activity(false);
        let msg = Message::RoundResult {
            round: 7,
            border_deltas: Vec::new(),
            stats,
            has_border_activity: true,
            minted_agents: Vec::new(),
        };

        let (mut client, mut server) = tokio::io::duplex(65_536);
        send_frame_with_threshold(&mut client, &msg, DEFAULT_COMPRESSION_THRESHOLD)
            .await
            .expect("send");
        let (received, _) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .expect("recv must succeed even on invariant-violating input");
        match received {
            Message::RoundResult {
                has_border_activity,
                stats,
                ..
            } => {
                assert!(has_border_activity, "top-level=true MUST round-trip");
                assert!(
                    !stats.has_border_activity,
                    "stats.has_border_activity=false MUST round-trip; \
                     the wire MUST NOT coerce agreement (DC-A2 is a \
                     builder-time invariant, not a wire-time coercion)",
                );
            }
            other => panic!("expected RoundResult, got {:?}", other),
        }
    }

    /// Q4 — `PendingCommutation` (NF-001 Shape A, TASK-0400) round-trips
    /// `request_id = u32::MAX` and a multi-slot `target_symbols` vector
    /// without truncation. Replaces the pre-Shape-A `arity = u8::MAX`
    /// probe (`arity` field no longer exists; slot count is derived from
    /// `target_symbols.len()`).
    #[tokio::test]
    async fn q4_pending_commutation_shape_a_roundtrip_boundary() {
        let original = Message::RoundStart {
            round: 0,
            border_deltas: Vec::new(),
            resolved_borders: Vec::new(),
            new_borders: Vec::new(),
            local_reconnections: Vec::new(),
            pending_commutations: vec![PendingCommutation {
                request_id: u32::MAX,
                target_symbols: vec![Symbol::Dup, Symbol::Con],
                local_wiring: Vec::new(),
            }],
        };
        let bytes = bincode_v2::encode(&original).expect("encode");
        let decoded: Message = bincode_v2::decode_value(&bytes).expect("decode");
        match decoded {
            Message::RoundStart {
                pending_commutations,
                ..
            } => {
                assert_eq!(pending_commutations.len(), 1);
                assert_eq!(pending_commutations[0].request_id, u32::MAX);
                assert_eq!(
                    pending_commutations[0].target_symbols,
                    vec![Symbol::Dup, Symbol::Con]
                );
                assert!(pending_commutations[0].local_wiring.is_empty());
            }
            other => panic!("expected RoundStart, got {:?}", other),
        }
    }

    /// Q5 — `LocalReconnection { agent_id: u32::MAX, port: u8::MAX, ... }`
    /// round-trips without sentinel collision.
    ///
    /// `agent_id = u32::MAX` is NOT the DISCONNECTED sentinel (which is
    /// `PortRef::FreePort(u32::MAX)` — a *PortRef* construction, not an
    /// *AgentId*). Probe pins that `LocalReconnection` is immune to
    /// sentinel confusion.
    #[tokio::test]
    async fn q5_local_reconnection_max_agent_id_and_port_roundtrip() {
        let original = Message::RoundStart {
            round: 0,
            border_deltas: Vec::new(),
            resolved_borders: Vec::new(),
            new_borders: Vec::new(),
            local_reconnections: vec![LocalReconnection {
                agent_id: u32::MAX,
                port: u8::MAX,
                new_target: PortRef::AgentPort(u32::MAX - 1, u8::MAX - 1),
            }],
            pending_commutations: Vec::new(),
        };
        let bytes = bincode_v2::encode(&original).expect("encode");
        let decoded: Message = bincode_v2::decode_value(&bytes).expect("decode");
        match decoded {
            Message::RoundStart {
                local_reconnections,
                ..
            } => {
                assert_eq!(local_reconnections.len(), 1);
                assert_eq!(local_reconnections[0].agent_id, u32::MAX);
                assert_eq!(local_reconnections[0].port, u8::MAX);
                assert_eq!(
                    local_reconnections[0].new_target,
                    PortRef::AgentPort(u32::MAX - 1, u8::MAX - 1),
                );
            }
            other => panic!("expected RoundStart, got {:?}", other),
        }
    }

    /// Q6 — `MintedAgent.minted_agent_id` inside the coordinator-reserved
    /// AgentId range (R48: `u32::MAX - 10_000 .. u32::MAX`) MUST round-trip
    /// on the wire. R48 enforcement is a coordinator-runtime responsibility
    /// (2.26-C), not a wire-layer rejection. This probe pins the
    /// separation of concerns.
    #[tokio::test]
    async fn q6_minted_agent_in_coordinator_reserved_range_roundtrips_on_wire() {
        let reserved_id = u32::MAX - 5_000;
        let stats = stats_with_activity(false);
        let original = Message::RoundResult {
            round: 0,
            border_deltas: Vec::new(),
            stats,
            has_border_activity: false,
            minted_agents: vec![MintedAgent {
                request_id: 42,
                minted_agent_id: reserved_id,
            }],
        };
        let bytes = bincode_v2::encode(&original).expect("encode");
        let decoded: Message =
            bincode_v2::decode_value(&bytes).expect("wire MUST decode (R48 is runtime-only)");
        match decoded {
            Message::RoundResult { minted_agents, .. } => {
                assert_eq!(minted_agents.len(), 1);
                assert_eq!(
                    minted_agents[0].minted_agent_id, reserved_id,
                    "wire MUST preserve R48-reserved AgentIds verbatim; \
                     enforcement is a coordinator runtime concern",
                );
            }
            other => panic!("expected RoundResult, got {:?}", other),
        }
    }

    /// Q7 — `BorderDelta.new_target = AgentPort(u32::MAX, 255)` is NOT
    /// the DISCONNECTED sentinel.
    ///
    /// DISCONNECTED is `PortRef::FreePort(u32::MAX)`. `AgentPort` with
    /// the same inner `u32::MAX` MUST NOT be confused by any layer —
    /// bincode discriminates via the PortRef enum variant tag.
    #[tokio::test]
    async fn q7_border_delta_agent_port_u32_max_is_not_disconnected_sentinel() {
        let original = Message::RoundStart {
            round: 0,
            border_deltas: vec![BorderDelta {
                border_id: u32::MAX,
                new_target: PortRef::AgentPort(u32::MAX, u8::MAX),
            }],
            resolved_borders: Vec::new(),
            new_borders: Vec::new(),
            local_reconnections: Vec::new(),
            pending_commutations: Vec::new(),
        };
        let bytes = bincode_v2::encode(&original).expect("encode");
        let decoded: Message = bincode_v2::decode_value(&bytes).expect("decode");
        match decoded {
            Message::RoundStart { border_deltas, .. } => {
                assert_eq!(border_deltas.len(), 1);
                assert_eq!(border_deltas[0].border_id, u32::MAX);
                assert_eq!(
                    border_deltas[0].new_target,
                    PortRef::AgentPort(u32::MAX, u8::MAX),
                );
                // Adversarial assertion: the PortRef discriminator MUST
                // distinguish AgentPort from FreePort(u32::MAX).
                assert_ne!(
                    border_deltas[0].new_target,
                    PortRef::FreePort(u32::MAX),
                    "AgentPort(u32::MAX, _) MUST NOT be confused with \
                     DISCONNECTED sentinel FreePort(u32::MAX)",
                );
            }
            other => panic!("expected RoundStart, got {:?}", other),
        }
    }

    /// Q8 — `InitialPartition` below the threshold MUST NOT compress.
    /// Boundary direction 1 of the `>=` comparison at
    /// `send_frame_with_threshold` (frame.rs:147).
    #[tokio::test]
    async fn q8_initial_partition_below_threshold_skips_compression() {
        let msg = Message::InitialPartition {
            round: 0,
            partition: make_large_partition(10),
        };
        let encoded_len = bincode_v2::encode(&msg).expect("encode").len();
        // Fixture precondition: 10 agents bincodes well below 1024.
        assert!(encoded_len < DEFAULT_COMPRESSION_THRESHOLD);

        let mut buf: Vec<u8> = Vec::new();
        send_frame_with_threshold(&mut buf, &msg, DEFAULT_COMPRESSION_THRESHOLD)
            .await
            .expect("send");
        assert_eq!(
            parse_header(&buf).flags & FLAG_COMPRESSED,
            0,
            "R36 off-by-one guard: payload len={} MUST NOT trigger \
             compression at threshold={}",
            encoded_len,
            DEFAULT_COMPRESSION_THRESHOLD,
        );
    }

    /// Q9 — `InitialPartition` at EXACTLY the threshold MUST compress.
    /// Boundary direction 2 of the `>=` comparison at
    /// `send_frame_with_threshold` (frame.rs:147 uses `>=`, so
    /// equality is the compress side).
    #[tokio::test]
    async fn q9_initial_partition_at_exact_threshold_triggers_compression() {
        let msg = Message::InitialPartition {
            round: 0,
            partition: make_large_partition(2000),
        };
        let encoded_len = bincode_v2::encode(&msg).expect("encode").len();
        assert!(encoded_len >= 1024, "fixture: encoded_len >= 1024");

        // Threshold = exact size MUST trigger compression (>= semantics).
        let mut buf_eq: Vec<u8> = Vec::new();
        send_frame_with_threshold(&mut buf_eq, &msg, encoded_len)
            .await
            .expect("send eq");
        assert_ne!(
            parse_header(&buf_eq).flags & FLAG_COMPRESSED,
            0,
            "threshold=encoded_len={} MUST trigger compression \
             (frame.rs:147 uses `>=`)",
            encoded_len,
        );

        // Threshold = encoded_len + 1 MUST skip compression.
        let mut buf_gt: Vec<u8> = Vec::new();
        send_frame_with_threshold(&mut buf_gt, &msg, encoded_len + 1)
            .await
            .expect("send gt");
        assert_eq!(
            parse_header(&buf_gt).flags & FLAG_COMPRESSED,
            0,
            "threshold=encoded_len+1={} MUST NOT trigger compression",
            encoded_len + 1,
        );
    }

    /// Q10 — 10 000-entry `border_deltas` Vec inside `RoundStart`
    /// encodes+decodes without int-overflow, panic, or O(n²) path.
    #[tokio::test]
    async fn q10_round_start_with_10k_border_deltas_roundtrips() {
        let deltas: Vec<BorderDelta> = (0..10_000u32)
            .map(|i| BorderDelta {
                border_id: i,
                new_target: PortRef::FreePort(i.wrapping_add(1)),
            })
            .collect();
        let original = Message::RoundStart {
            round: 0,
            border_deltas: deltas,
            resolved_borders: Vec::new(),
            new_borders: Vec::new(),
            local_reconnections: Vec::new(),
            pending_commutations: Vec::new(),
        };
        let bytes = bincode_v2::encode(&original).expect("encode 10k deltas");
        let decoded: Message = bincode_v2::decode_value(&bytes).expect("decode 10k deltas");
        match decoded {
            Message::RoundStart { border_deltas, .. } => {
                assert_eq!(border_deltas.len(), 10_000);
                // Spot-check contract: first, middle, last entries
                // preserve (border_id, new_target).
                assert_eq!(border_deltas[0].border_id, 0);
                assert_eq!(border_deltas[0].new_target, PortRef::FreePort(1));
                assert_eq!(border_deltas[5_000].border_id, 5_000);
                assert_eq!(border_deltas[5_000].new_target, PortRef::FreePort(5_001));
                assert_eq!(border_deltas[9_999].border_id, 9_999);
                assert_eq!(border_deltas[9_999].new_target, PortRef::FreePort(10_000));
            }
            other => panic!("expected RoundStart, got {:?}", other),
        }
    }

    /// Q11 — `FinalStateRequest { round: u32::MAX }` exercises the
    /// max-value varint path (5-byte varint for u32::MAX).
    #[tokio::test]
    async fn q11_final_state_request_round_u32_max_roundtrips() {
        let original = Message::FinalStateRequest { round: u32::MAX };
        let bytes = bincode_v2::encode(&original).expect("encode");
        // bincode v2 varint for u32::MAX: 1 byte marker + 4 bytes LE = 5.
        // Plus 1 byte disc (10). Total = 6 bytes.
        assert!(
            bytes.len() <= 8,
            "FinalStateRequest with round=u32::MAX MUST encode in <= 8 \
             bytes; got {}",
            bytes.len(),
        );
        let decoded: Message = bincode_v2::decode_value(&bytes).expect("decode");
        match decoded {
            Message::FinalStateRequest { round } => assert_eq!(round, u32::MAX),
            other => panic!("expected FinalStateRequest, got {:?}", other),
        }
    }

    /// Q12 — Truncated frame (fewer bytes than a full header) MUST fail
    /// with a recognisable I/O error, NOT panic or hang.
    #[tokio::test]
    async fn q12_truncated_frame_below_header_size_rejected() {
        // Header is 9 bytes; send 8 only.
        let truncated = vec![0u8; FRAME_HEADER_SIZE - 1];
        let (mut client, mut server) = tokio::io::duplex(64);
        client
            .write_all(&truncated)
            .await
            .expect("write truncated bytes");
        drop(client);

        let err = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .expect_err("truncated frame MUST be rejected");
        // read_exact on 8 bytes of a 9-byte buffer returns UnexpectedEof;
        // that propagates as ConnectionLost per frame.rs:420.
        assert!(
            matches!(err, ProtocolError::ConnectionLost(_)),
            "truncated frame MUST surface as ConnectionLost; got {:?}",
            err,
        );
    }

    /// Q13a — Adversarial `FLAG_ARCHIVED` on a delta variant (default
    /// features, i.e. `zero-copy` OFF).
    ///
    /// Current behaviour: frame.rs L501 gates the rkyv decode path on
    /// `#[cfg(feature = "zero-copy")]`, so on default features a frame
    /// with `FLAG_ARCHIVED` set falls through to bincode and succeeds.
    /// This probe PINS that forward-compat pass-through: a default-build
    /// that receives a (spec-violating) FLAG_ARCHIVED frame containing
    /// bincode bytes does NOT panic and does NOT corrupt state.
    ///
    /// SPEC-18 R22 (whitelist) is a **sender-side** obligation; this
    /// probe confirms the receiver is defensive-by-default (preserves
    /// the invariant on the happy path and does not crash on
    /// non-whitelisted flags).
    #[cfg(not(feature = "zero-copy"))]
    #[tokio::test]
    async fn q13a_flag_archived_tamper_on_round_start_default_features_falls_through_to_bincode() {
        // Build an uncompressed RoundStart, then flip FLAG_ARCHIVED ON
        // in the flags byte. CRC stays valid because CRC is computed
        // over the bincode payload (unchanged); the flags byte is
        // OUTSIDE the CRC-covered bytes (it's in the header).
        let msg = Message::RoundStart {
            round: 0,
            border_deltas: Vec::new(),
            resolved_borders: Vec::new(),
            new_borders: Vec::new(),
            local_reconnections: Vec::new(),
            pending_commutations: Vec::new(),
        };
        let mut buf: Vec<u8> = Vec::new();
        send_frame_with_threshold(&mut buf, &msg, usize::MAX)
            .await
            .expect("send uncompressed");
        // Flags byte is at offset 8 (header layout: length[0..4],
        // checksum[4..8], flags[8]).
        assert_eq!(
            buf[8] & FLAG_ARCHIVED,
            0,
            "fixture precondition: RoundStart MUST NOT set FLAG_ARCHIVED",
        );
        buf[8] |= FLAG_ARCHIVED;

        let (mut client, mut server) = tokio::io::duplex(65_536);
        client
            .write_all(&buf)
            .await
            .expect("write flag-tampered frame");
        drop(client);
        let (received, _) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .expect(
                "default features: FLAG_ARCHIVED + bincode payload \
                 MUST fall through to bincode (no rkyv decode path \
                 compiled)",
            );
        assert!(
            matches!(received, Message::RoundStart { .. }),
            "received message MUST decode as RoundStart (bincode \
             pass-through preserves the wire tag); got {:?}",
            received,
        );
    }

    /// Q13b — Adversarial `FLAG_ARCHIVED` on a delta variant
    /// (`--features zero-copy`).
    ///
    /// Under the zero-copy feature the rkyv decode path activates for
    /// FLAG_ARCHIVED frames (frame.rs:500-508). A RoundStart bincode
    /// payload cannot satisfy either `ArchiveAssignPayload` or
    /// `ArchivePartitionResultPayload` schemas, so rkyv access MUST
    /// fail the frame with a deserialize-family error.
    #[cfg(feature = "zero-copy")]
    #[tokio::test]
    async fn q13b_flag_archived_tamper_on_round_start_zero_copy_feature_rejected() {
        let msg = Message::RoundStart {
            round: 0,
            border_deltas: Vec::new(),
            resolved_borders: Vec::new(),
            new_borders: Vec::new(),
            local_reconnections: Vec::new(),
            pending_commutations: Vec::new(),
        };
        let mut buf: Vec<u8> = Vec::new();
        send_frame_with_threshold(&mut buf, &msg, usize::MAX)
            .await
            .expect("send uncompressed");
        buf[8] |= FLAG_ARCHIVED;

        let (mut client, mut server) = tokio::io::duplex(65_536);
        client
            .write_all(&buf)
            .await
            .expect("write flag-tampered frame");
        drop(client);
        let err = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .expect_err(
                "zero-copy features: FLAG_ARCHIVED on bincode bytes \
                 MUST fail rkyv archive validation",
            );
        // Accept any deserialize-family or archive-validation error.
        // The specific variant depends on SPEC-18 §3.5's error mapping.
        assert!(
            matches!(
                err,
                ProtocolError::Deserialize(_) | ProtocolError::ArchiveValidationFailed(_)
            ),
            "zero-copy: FLAG_ARCHIVED on bincode bytes MUST surface as \
             Deserialize or ArchiveValidationFailed; got {:?}",
            err,
        );
    }
}
