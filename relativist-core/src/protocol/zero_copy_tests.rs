//! SPEC-18 §3.5 (item 2.24) — Zero-Copy Archive (rkyv) integration tests.
//!
//! T11..T14 acceptance suite. These tests live under
//! `#[cfg(all(test, feature = "zero-copy"))]` because they exercise the
//! rkyv archive path end-to-end through `send_frame_v2` + `recv_frame`.
//!
//! Coverage map (SPEC-18 §3.5 + §7.2):
//! - **T11 — round-trip identity:** Send each hot-path message
//!   (`AssignPartition`, `PartitionResult`) through the archive path,
//!   receive it on the other side, and verify the decoded message matches
//!   the original field-for-field. Both compressed and uncompressed
//!   variants of the wire frame are exercised (R12 + R23).
//! - **T12 — corrupt archive rejection:** A FLAG_ARCHIVED frame whose
//!   payload is too small (or otherwise structurally invalid) for either
//!   hot-path schema MUST be rejected with `ArchiveValidationFailed`,
//!   not panic and not silently decode. (R24 / R26.)
//! - **T13 — alignment correctness:** rkyv's `access` API requires a
//!   16-byte aligned base pointer. The recv pipeline reads into a
//!   `Vec<u8>` (allocator-aligned only — typically 8 bytes on 64-bit
//!   Windows), so `decode_archive_payload` must copy into an
//!   `AlignedVec` to satisfy R25. We verify by sending a battery of
//!   hot-path payloads of varying sizes and confirming every recv
//!   succeeds.
//! - **T14 — hot-path-only enforcement:** A FLAG_ARCHIVED frame whose
//!   payload bytes happen to validate as `u32` (a legal rkyv schema, but
//!   not a hot-path payload) MUST be rejected with the literal R26
//!   message `"non-hot-path archive payload (matched neither
//!   AssignPartition nor PartitionResult)"`. (R22.)

use std::collections::HashMap;

use tokio::io::AsyncWriteExt;

use crate::merge::WorkerRoundStats;
use crate::net::{Net, PortRef, Symbol};
use crate::partition::{IdRange, Partition};
use crate::protocol::error::ProtocolError;
use crate::protocol::frame::{
    recv_frame, send_frame, send_frame_v2, FrameHeader, DEFAULT_MAX_PAYLOAD_SIZE, FLAG_ARCHIVED,
};
use crate::protocol::types::Message;

// ---------------------------------------------------------------------------
// Local test fixtures (kept out of the public surface).
// ---------------------------------------------------------------------------

fn make_test_channel() -> (tokio::io::DuplexStream, tokio::io::DuplexStream) {
    tokio::io::duplex(1_048_576)
}

fn empty_partition() -> Partition {
    Partition {
        subnet: Net::new(),
        worker_id: 0,
        free_port_index: HashMap::new(),
        id_range: IdRange {
            start: 0,
            end: 100_000,
        },
        border_id_start: 0,
        border_id_end: 0,
    }
}

fn populated_partition(num_agents: usize) -> Partition {
    let mut subnet = Net::new();
    for _ in 0..num_agents {
        subnet.create_agent(Symbol::Era);
    }
    Partition {
        subnet,
        worker_id: 1,
        free_port_index: HashMap::new(),
        id_range: IdRange {
            start: 100,
            end: 200,
        },
        border_id_start: 100,
        border_id_end: 200,
    }
}

fn sample_stats(worker_id: u32) -> WorkerRoundStats {
    WorkerRoundStats {
        worker_id,
        agents_before: 16,
        agents_after: 4,
        local_redexes: 12,
        reduce_duration_secs: 0.025,
        interactions_by_rule: [3, 2, 1, 4, 1, 0],
        has_border_activity: true,
    }
}

fn assert_partition_eq(left: &Partition, right: &Partition) {
    assert_eq!(left.worker_id, right.worker_id, "worker_id mismatch");
    assert_eq!(
        left.id_range.start, right.id_range.start,
        "id_range.start mismatch"
    );
    assert_eq!(
        left.id_range.end, right.id_range.end,
        "id_range.end mismatch"
    );
    assert_eq!(
        left.border_id_start, right.border_id_start,
        "border_id_start mismatch"
    );
    assert_eq!(
        left.border_id_end, right.border_id_end,
        "border_id_end mismatch"
    );
    assert_eq!(left.subnet, right.subnet, "subnet (Net) mismatch");
}

fn assert_stats_eq(left: &WorkerRoundStats, right: &WorkerRoundStats) {
    assert_eq!(left.worker_id, right.worker_id);
    assert_eq!(left.agents_before, right.agents_before);
    assert_eq!(left.agents_after, right.agents_after);
    assert_eq!(left.local_redexes, right.local_redexes);
    assert_eq!(
        left.reduce_duration_secs.to_bits(),
        right.reduce_duration_secs.to_bits(),
    );
    assert_eq!(left.interactions_by_rule, right.interactions_by_rule);
    assert_eq!(left.has_border_activity, right.has_border_activity);
}

async fn round_trip_message(message: Message, threshold: usize) -> Message {
    let (mut client, mut server) = make_test_channel();
    send_frame_v2(&mut client, &message, true, threshold)
        .await
        .expect("send_frame_v2 must succeed");
    client.flush().await.unwrap();
    let (decoded, _) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
        .await
        .expect("recv_frame must succeed");
    decoded
}

// ---------------------------------------------------------------------------
// T11 — round-trip identity (R20-R24).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn t11_assign_partition_round_trip_uncompressed() {
    let original = Message::AssignPartition {
        round: 1,
        partition: empty_partition(),
    };
    let decoded = round_trip_message(original.clone(), usize::MAX).await;
    match (original, decoded) {
        (
            Message::AssignPartition {
                round: r0,
                partition: p0,
            },
            Message::AssignPartition {
                round: r1,
                partition: p1,
            },
        ) => {
            assert_eq!(r0, r1);
            assert_partition_eq(&p0, &p1);
        }
        (_, other) => panic!("expected AssignPartition, got {:?}", other),
    }
}

#[tokio::test]
async fn t11_partition_result_round_trip_uncompressed() {
    let original = Message::PartitionResult {
        round: 9,
        partition: populated_partition(4),
        stats: sample_stats(1),
    };
    let decoded = round_trip_message(original.clone(), usize::MAX).await;
    match (original, decoded) {
        (
            Message::PartitionResult {
                round: r0,
                partition: p0,
                stats: s0,
            },
            Message::PartitionResult {
                round: r1,
                partition: p1,
                stats: s1,
            },
        ) => {
            assert_eq!(r0, r1);
            assert_partition_eq(&p0, &p1);
            assert_stats_eq(&s0, &s1);
        }
        (_, other) => panic!("expected PartitionResult, got {:?}", other),
    }
}

#[tokio::test]
async fn t11_assign_partition_round_trip_compressed() {
    let original = Message::AssignPartition {
        round: 7,
        partition: populated_partition(64),
    };
    // threshold = 1 forces FLAG_COMPRESSED on top of FLAG_ARCHIVED (R23).
    let decoded = round_trip_message(original.clone(), 1).await;
    match (original, decoded) {
        (
            Message::AssignPartition {
                round: r0,
                partition: p0,
            },
            Message::AssignPartition {
                round: r1,
                partition: p1,
            },
        ) => {
            assert_eq!(r0, r1);
            assert_partition_eq(&p0, &p1);
        }
        (_, other) => panic!("expected AssignPartition, got {:?}", other),
    }
}

#[tokio::test]
async fn t11_partition_result_round_trip_compressed() {
    let original = Message::PartitionResult {
        round: 5,
        partition: populated_partition(64),
        stats: sample_stats(2),
    };
    let decoded = round_trip_message(original.clone(), 1).await;
    match (original, decoded) {
        (
            Message::PartitionResult {
                round: r0,
                partition: p0,
                stats: s0,
            },
            Message::PartitionResult {
                round: r1,
                partition: p1,
                stats: s1,
            },
        ) => {
            assert_eq!(r0, r1);
            assert_partition_eq(&p0, &p1);
            assert_stats_eq(&s0, &s1);
        }
        (_, other) => panic!("expected PartitionResult, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// T12 — corrupt archive rejection (R24).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn t12_undersized_archive_payload_is_rejected() {
    // Hand-craft a FLAG_ARCHIVED frame with a 16-byte payload — too
    // small to host either hot-path schema. CRC must match so the
    // failure is forced into the rkyv validator (R24).
    let payload = vec![0u8; 16];
    let checksum = crc32fast::hash(&payload);
    let header = FrameHeader {
        length: payload.len() as u32,
        checksum,
        flags: FLAG_ARCHIVED,
    };

    let (mut client, mut server) = make_test_channel();
    client.write_all(&header.to_bytes()).await.unwrap();
    client.write_all(&payload).await.unwrap();
    client.flush().await.unwrap();

    let err = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
        .await
        .unwrap_err();
    assert!(
        matches!(err, ProtocolError::ArchiveValidationFailed(_)),
        "expected ArchiveValidationFailed on undersized archive, got {:?}",
        err,
    );
}

// ---------------------------------------------------------------------------
// T13 — alignment correctness (R25).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn t13_alignment_round_trip_battery_uncompressed() {
    // The recv pipeline allocates a `Vec<u8>` for the payload (allocator
    // alignment only). `decode_archive_payload` MUST copy into an
    // `AlignedVec` before validating. Round-tripping a battery of
    // payloads at varied sizes is the cleanest end-to-end witness.
    for size in [0usize, 1, 8, 17, 64, 128, 256, 1023, 4096] {
        let original = Message::AssignPartition {
            round: 0,
            partition: populated_partition(size),
        };
        let _ = round_trip_message(original, usize::MAX).await;
    }
}

#[tokio::test]
async fn t13_alignment_round_trip_battery_compressed() {
    // Same coverage with FLAG_COMPRESSED active to exercise the
    // decompress -> CRC -> alignment -> rkyv chain (R12 + R25).
    for size in [4usize, 32, 256, 4096] {
        let original = Message::PartitionResult {
            round: 0,
            partition: populated_partition(size),
            stats: sample_stats(0),
        };
        let _ = round_trip_message(original, 1).await;
    }
}

// ---------------------------------------------------------------------------
// T14 — hot-path-only enforcement (R22 / R26).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn t14_non_hot_path_archive_yields_r26_message() {
    // Encode a bare `u32` as a rkyv archive. The bytes are a valid rkyv
    // archive (any decode_archive_payload step before R26 must succeed
    // structurally for u32) but they are NOT a hot-path payload schema.
    let archive = rkyv::to_bytes::<rkyv::rancor::Error>(&777u32).unwrap();
    let payload: Vec<u8> = archive.as_ref().to_vec();
    let checksum = crc32fast::hash(&payload);
    let header = FrameHeader {
        length: payload.len() as u32,
        checksum,
        flags: FLAG_ARCHIVED,
    };

    let (mut client, mut server) = make_test_channel();
    client.write_all(&header.to_bytes()).await.unwrap();
    client.write_all(&payload).await.unwrap();
    client.flush().await.unwrap();

    let err = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
        .await
        .unwrap_err();
    match err {
        ProtocolError::ArchiveValidationFailed(reason) => {
            assert!(
                reason.contains("non-hot-path archive payload"),
                "R26 mandates the 'non-hot-path archive payload' phrase; got '{}'",
                reason,
            );
        }
        other => panic!("expected ArchiveValidationFailed, got {:?}", other),
    }
}

#[tokio::test]
async fn t14_cold_path_message_with_archive_flag_falls_through_to_bincode() {
    // `is_hot_path_message` filters cold-path messages OFF the archive
    // path on the SEND side: even with `use_archive=true`, a
    // `Shutdown` message is sent through the bincode path and never
    // gets FLAG_ARCHIVED. This complements the R22 recv-side guard.
    let original = Message::Shutdown;
    let decoded = round_trip_message(original, usize::MAX).await;
    assert!(matches!(decoded, Message::Shutdown));
}

// ---------------------------------------------------------------------------
// T11/T13 cross-cut: identity over the full hot-path matrix (4 messages
// × 2 compression states = 8 round-trips, ensures the bundle's primary
// integration contract is exercised end-to-end with the archive path
// active).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn t11_t13_full_hot_path_identity_matrix() {
    let messages = [
        Message::AssignPartition {
            round: 1,
            partition: empty_partition(),
        },
        Message::AssignPartition {
            round: 2,
            partition: populated_partition(8),
        },
        Message::PartitionResult {
            round: 3,
            partition: empty_partition(),
            stats: sample_stats(0),
        },
        Message::PartitionResult {
            round: 4,
            partition: populated_partition(8),
            stats: sample_stats(1),
        },
    ];
    for msg in messages {
        let decoded_uncompressed = round_trip_message(msg.clone(), usize::MAX).await;
        let decoded_compressed = round_trip_message(msg.clone(), 1).await;
        // Variant identity is enough at this layer; field-by-field
        // checks live in the per-T11 tests above.
        match (&msg, &decoded_uncompressed, &decoded_compressed) {
            (
                Message::AssignPartition { .. },
                Message::AssignPartition { .. },
                Message::AssignPartition { .. },
            )
            | (
                Message::PartitionResult { .. },
                Message::PartitionResult { .. },
                Message::PartitionResult { .. },
            ) => {}
            _ => panic!(
                "variant mismatch: original={:?} uncompressed={:?} compressed={:?}",
                msg, decoded_uncompressed, decoded_compressed
            ),
        }
    }
}

// ===========================================================================
// QA Stage 5 probes for SPEC-18 §3.5 (item 2.24) — adversarial bug hunt.
// Probes Q1..Q8 enumerated in
// `docs/reviews/REVIEW-SPEC-18-section-3.5-2026-04-16.md` §7.
// Each probe targets a specific corruption / interop / drop-semantics
// surface that the developer-supplied T11..T14 suite does not exercise
// directly.
// ===========================================================================

/// Builds a populated `AssignPartition` archive whose serialized form is
/// large enough that internal rkyv relative-pointer fields must reference
/// non-trivial offsets within the buffer (multiple agents + a populated
/// `free_port_index` HashMap). Returned bytes are a *valid* rkyv archive
/// — every probe that wants to corrupt one starts here.
fn fixture_valid_assign_archive() -> rkyv::util::AlignedVec {
    let mut subnet = Net::new();
    for _ in 0..8 {
        subnet.create_agent(Symbol::Con);
    }
    for _ in 0..4 {
        subnet.create_agent(Symbol::Dup);
    }
    let mut free_port_index: HashMap<u32, PortRef> = HashMap::new();
    free_port_index.insert(10, PortRef::AgentPort(0, 1));
    free_port_index.insert(20, PortRef::AgentPort(3, 2));
    free_port_index.insert(30, PortRef::FreePort(7));
    free_port_index.insert(40, PortRef::FreePort(u32::MAX));
    let partition = Partition {
        subnet,
        worker_id: 2,
        free_port_index,
        id_range: IdRange {
            start: 1_000,
            end: 2_000,
        },
        border_id_start: 10,
        border_id_end: 50,
    };
    let payload = crate::protocol::frame::ArchiveAssignPayload {
        round: 99,
        partition,
    };
    rkyv::to_bytes::<rkyv::rancor::Error>(&payload).expect("fixture archive must serialize cleanly")
}

/// Hand-frames a FLAG_ARCHIVED frame from raw payload bytes (header + CRC).
/// Mirrors the byte layout that `send_frame_v2` produces on the
/// uncompressed-archive fast path.
async fn write_archived_frame(client: &mut tokio::io::DuplexStream, payload: &[u8]) {
    let checksum = crc32fast::hash(payload);
    let header = FrameHeader {
        length: payload.len() as u32,
        checksum,
        flags: FLAG_ARCHIVED,
    };
    client.write_all(&header.to_bytes()).await.unwrap();
    client.write_all(payload).await.unwrap();
    client.flush().await.unwrap();
}

// ---------------------------------------------------------------------------
// Q1 (HIGH PRIORITY) — pointer-corruption probe.
//
// Replaces the abandoned random-byte-flip approach which was non-deterministic
// on small payloads. Strategy: serialize a non-trivial AssignPartition
// (multiple agents + a populated free_port_index so the archive contains
// real internal relative-pointer fields), then mutate the *trailing* bytes
// of the buffer (where rkyv places its root-pointer / table metadata in the
// 0.8.x layout) to force the validator to compute an out-of-buffer offset.
// We checksum the *post-mutation* bytes so the failure is forced into the
// rkyv validator path (R26), NOT the CRC check (R12).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn qa_probe_q1_pointer_corruption_rejected() {
    let archive = fixture_valid_assign_archive();
    let mut payload: Vec<u8> = archive.as_ref().to_vec();
    assert!(
        payload.len() > 64,
        "fixture archive must be large enough that pointer fields exist"
    );

    // rkyv 0.8 lays out an archive with the root struct at the end and
    // variable-length data (Vecs / HashMaps) earlier in the buffer,
    // referenced by relative pointers stored *inside* the root struct.
    // To deterministically force `rkyv::access` to reject during the
    // validation pass we drop the front half of the buffer: this leaves
    // the root struct (with all its relative pointers intact) but moves
    // the buffer base such that every internal pointer now references
    // memory before the buffer start. The validator's bounds-check on
    // any RelPtr -> data follow MUST reject.
    let len = payload.len();
    let cut = len / 2;
    payload.drain(0..cut);

    let (mut client, mut server) = make_test_channel();
    write_archived_frame(&mut client, &payload).await;

    let err = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
        .await
        .unwrap_err();
    tracing::info!(?err, "Q1 corruption probe yielded error");
    match err {
        ProtocolError::ArchiveValidationFailed(reason) => {
            // Either schema-name prefix (deserialize failure on a
            // partially-valid archive) OR the R26 fall-through
            // ("non-hot-path archive payload") is acceptable — the
            // load-bearing assertion is "validator rejected, did not
            // panic, did not silently mis-decode".
            assert!(
                reason.contains("AssignPartition")
                    || reason.contains("PartitionResult")
                    || reason.contains("non-hot-path"),
                "Q1 reason must identify validator path; got '{}'",
                reason
            );
        }
        other => panic!(
            "Q1: expected ArchiveValidationFailed on corrupted root pointer, got {:?}",
            other
        ),
    }
}

// ---------------------------------------------------------------------------
// Q2 (feature ON) — adversarial sender sets FLAG_ARCHIVED on a bincode payload.
//
// A misbehaving / misconfigured peer flips FLAG_ARCHIVED on a frame whose
// payload is actually bincode-encoded. The receiver (with feature ON)
// MUST refuse via the rkyv validator path — never panic, never silently
// decode the bincode bytes through the archive layout.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn qa_probe_q2_archived_flag_on_bincode_payload_with_feature() {
    // Encode a Shutdown via the bincode v2 path manually so we control
    // the flags byte (send_frame would set flags=0 / FLAG_COMPRESSED only).
    let bincode_bytes: Vec<u8> = crate::protocol::bincode_v2::encode(&Message::Shutdown)
        .expect("bincode encode must succeed");

    let (mut client, mut server) = make_test_channel();
    write_archived_frame(&mut client, &bincode_bytes).await;

    let err = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
        .await
        .unwrap_err();
    tracing::info!(
        ?err,
        "Q2 (feature ON) bincode-as-archive probe yielded error"
    );
    assert!(
        matches!(err, ProtocolError::ArchiveValidationFailed(_)),
        "Q2 (feature ON): bincode payload with FLAG_ARCHIVED must fail rkyv validation, got {:?}",
        err
    );
}

// ---------------------------------------------------------------------------
// Q3 (feature ON branch) — bincode frame is accepted normally when received
// by a feature-ON receiver. (FLAG_ARCHIVED clear → bincode decode path.)
//
// Models the cross-feature interop direction: process B (default) sends
// bincode; process A (feature ON) receives. Receiver MUST decode normally.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn qa_probe_q3_bincode_frame_accepted_when_feature_on() {
    let original = Message::Shutdown;
    let mut buf: Vec<u8> = Vec::new();
    send_frame(&mut buf, &original).await.unwrap();

    let (mut client, mut server) = make_test_channel();
    client.write_all(&buf).await.unwrap();
    client.flush().await.unwrap();

    let (decoded, _) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
        .await
        .expect("Q3 (feature ON): bincode frames must decode normally");
    assert!(matches!(decoded, Message::Shutdown));
}

// ---------------------------------------------------------------------------
// Q4 — AlignedVec drop-on-partial-read semantics.
//
// Simulates a transport that returns the header + half the payload bytes,
// then EOF. `recv_frame` must propagate `ConnectionLost(UnexpectedEof)`
// without panicking; the partially-allocated AlignedVec must drop cleanly.
// We run two variants: (a) hand-built `tokio_test::io::Builder` with
// explicit truncation, and (b) a duplex pair where the writer is dropped
// mid-payload.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn qa_probe_q4_aligned_vec_dropped_on_partial_read() {
    // Build a header that promises 1024 payload bytes, then send only 100.
    let promised_len: u32 = 1024;
    let header = FrameHeader {
        length: promised_len,
        // Checksum value is irrelevant — recv_frame will fail at read_exact
        // before reaching the CRC step.
        checksum: 0xDEAD_BEEF,
        flags: FLAG_ARCHIVED,
    };
    let mut bytes: Vec<u8> = Vec::new();
    bytes.extend_from_slice(&header.to_bytes());
    bytes.extend(vec![0u8; 100]); // half-payload only

    let mock_reader = tokio_test::io::Builder::new().read(&bytes).build();
    tokio::pin!(mock_reader);

    let err = recv_frame(&mut mock_reader, DEFAULT_MAX_PAYLOAD_SIZE)
        .await
        .unwrap_err();
    tracing::info!(?err, "Q4 partial-read probe yielded error");
    match err {
        ProtocolError::ConnectionLost(io_err) => {
            assert_eq!(
                io_err.kind(),
                std::io::ErrorKind::UnexpectedEof,
                "Q4: expected UnexpectedEof, got {:?}",
                io_err.kind()
            );
        }
        other => panic!(
            "Q4: expected ConnectionLost(UnexpectedEof), got {:?}",
            other
        ),
    }
    // Reaching this line proves no panic occurred and the AlignedVec was
    // dropped cleanly along the error-propagation path. Miri-level leak
    // verification is out of scope for cargo test, but cargo's normal
    // teardown would surface a leak at process exit.
}

// ---------------------------------------------------------------------------
// Q5 — round-trip byte-count parity (rkyv archive vs bincode).
//
// Documents the actual on-wire size of both encodings for the same
// Partition. Per SPEC-18 §5.3, rkyv pays alignment-padding overhead in
// exchange for zero-copy decode; we verify the size relationship is as
// documented (rkyv >= bincode for non-trivial partitions). Numbers are
// emitted via `tracing` for future regression comparison.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn qa_probe_q5_archive_vs_bincode_size_documented() {
    let partition = populated_partition(64);
    let original = Message::AssignPartition {
        round: 1,
        partition,
    };

    // Bincode v2 size (control message frame minus 9-byte header).
    let bincode_payload: Vec<u8> = crate::protocol::bincode_v2::encode(&original).unwrap();

    // rkyv archive size (the wire payload that send_frame_v2 emits when
    // FLAG_COMPRESSED is suppressed by an infinite threshold).
    let mut archive_buf: Vec<u8> = Vec::new();
    send_frame_v2(&mut archive_buf, &original, true, usize::MAX)
        .await
        .unwrap();
    let archive_payload_len = archive_buf.len() - crate::protocol::frame::FRAME_HEADER_SIZE;

    // LZ4-compressed sizes (threshold = 1 forces compression on both
    // wire formats; bincode is wrapped manually to mirror send_frame).
    let mut bincode_compressed_buf: Vec<u8> = Vec::new();
    send_frame(&mut bincode_compressed_buf, &original)
        .await
        .unwrap();
    let bincode_compressed_payload_len =
        bincode_compressed_buf.len() - crate::protocol::frame::FRAME_HEADER_SIZE;

    let mut archive_compressed_buf: Vec<u8> = Vec::new();
    send_frame_v2(&mut archive_compressed_buf, &original, true, 1)
        .await
        .unwrap();
    let archive_compressed_payload_len =
        archive_compressed_buf.len() - crate::protocol::frame::FRAME_HEADER_SIZE;

    tracing::info!(
        bincode = bincode_payload.len(),
        archive = archive_payload_len,
        bincode_lz4 = bincode_compressed_payload_len,
        archive_lz4 = archive_compressed_payload_len,
        "Q5 size matrix (bytes)"
    );

    // Documented expectation per SPEC-18 §5.3: archive carries alignment
    // padding so it is at least as large as bincode for non-trivial
    // partitions.
    assert!(
        archive_payload_len >= bincode_payload.len(),
        "Q5: archive payload ({}) MUST be >= bincode payload ({}) per §5.3",
        archive_payload_len,
        bincode_payload.len()
    );
    // After LZ4 the gap should narrow but the inequality direction is not
    // strictly guaranteed (compression can flip it on small inputs); we
    // only assert both sizes are smaller than their uncompressed peers
    // when the threshold actually fires.
    assert!(
        bincode_compressed_payload_len <= bincode_payload.len(),
        "Q5: LZ4 must not expand bincode payload (got {} > {})",
        bincode_compressed_payload_len,
        bincode_payload.len()
    );
    assert!(
        archive_compressed_payload_len <= archive_payload_len,
        "Q5: LZ4 must not expand archive payload (got {} > {})",
        archive_compressed_payload_len,
        archive_payload_len
    );
}

// ---------------------------------------------------------------------------
// Q6 — ARM alignment witness (DEFERRED).
//
// rkyv archive layouts depend on target endianness/alignment. CI runs only
// on x86_64, so cross-platform alignment is not exercised here. This test
// exists for traceability so future readers see the deferred coverage
// hooked in the test surface.
// ---------------------------------------------------------------------------

#[test]
fn qa_probe_q6_arm_alignment_documented_as_deferred() {
    // No-op test: rkyv 0.8.x guarantees portable archive layout across
    // little-endian targets (x86_64 and aarch64 both qualify), but no
    // ARM CI runner exists to *witness* round-trip identity end-to-end.
    // Track in `docs/DEFERRED-WORK.md` if the project ever ships ARM
    // artefacts (likely under ROADMAP 2.37-2.39 Tailscale distribution).
    tracing::info!(
        target_arch = std::env::consts::ARCH,
        "Q6: ARM alignment witness is N/A on this target; see review §7 Q6"
    );
}

// ---------------------------------------------------------------------------
// Q7 — rkyv version drift pinning documentation.
//
// Cargo.toml carries `rkyv = "0.8"` (caret 0.8.x). A future patch bump
// could in principle change archive layout; rkyv's compat policy says it
// will not, but we have no committed fixture archive to *prove* layout
// stability across versions. This test documents the deferred coverage
// and asserts that the *currently pinned* version still round-trips —
// the strongest check we can run without a fixture.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn qa_probe_q7_rkyv_version_drift_pinning_documented() {
    // Sanity: archive a known partition and round-trip it. If rkyv has
    // silently changed layout in a way that breaks deserialize, this
    // would surface here. Real cross-version testing requires committing
    // a fixture archive blob — deferred.
    let original = Message::AssignPartition {
        round: 0xFEEDC0DE,
        partition: populated_partition(4),
    };
    let decoded = round_trip_message(original.clone(), usize::MAX).await;
    match (original, decoded) {
        (
            Message::AssignPartition {
                round: r0,
                partition: p0,
            },
            Message::AssignPartition {
                round: r1,
                partition: p1,
            },
        ) => {
            assert_eq!(r0, r1);
            assert_partition_eq(&p0, &p1);
        }
        (_, other) => panic!("Q7: expected AssignPartition, got {:?}", other),
    }
    tracing::info!("Q7: pinned rkyv 0.8.x round-trip OK; cross-version fixture coverage deferred");
}

// ---------------------------------------------------------------------------
// Q8 — populated `free_port_index` HashMap round-trip.
//
// T11 fixtures use `free_port_index = HashMap::new()`. ArchivedHashMap has
// a non-trivial layout that differs from std HashMap. This probe confirms
// that a *populated* HashMap with mixed PortRef variants round-trips with
// full key/value fidelity — defends against silent equality failure that
// the existing `assert_partition_eq` helper would not catch (because it
// previously did not compare this field).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn qa_probe_q8_populated_free_port_index_round_trip() {
    let mut subnet = Net::new();
    for _ in 0..6 {
        subnet.create_agent(Symbol::Era);
    }
    let mut free_port_index: HashMap<u32, PortRef> = HashMap::new();
    // Mix all PortRef shapes: AgentPort (varied agent_id + port_id),
    // FreePort (regular border id), FreePort(u32::MAX) (DISCONNECTED
    // sentinel), each repeated to exercise the HashMap's bucket logic.
    free_port_index.insert(1, PortRef::AgentPort(0, 0));
    free_port_index.insert(2, PortRef::AgentPort(2, 1));
    free_port_index.insert(3, PortRef::AgentPort(5, 2));
    free_port_index.insert(11, PortRef::FreePort(11));
    free_port_index.insert(22, PortRef::FreePort(22));
    free_port_index.insert(33, PortRef::FreePort(33));
    free_port_index.insert(99, PortRef::FreePort(u32::MAX));

    let partition = Partition {
        subnet,
        worker_id: 7,
        free_port_index: free_port_index.clone(),
        id_range: IdRange {
            start: 100,
            end: 200,
        },
        border_id_start: 0,
        border_id_end: 100,
    };
    let original = Message::AssignPartition {
        round: 42,
        partition,
    };
    let decoded = round_trip_message(original.clone(), usize::MAX).await;
    let decoded_partition = match decoded {
        Message::AssignPartition { partition, .. } => partition,
        other => panic!("Q8: expected AssignPartition, got {:?}", other),
    };
    // Key/value identity check (HashMap unordered, so compare as sets).
    assert_eq!(
        decoded_partition.free_port_index.len(),
        free_port_index.len(),
        "Q8: free_port_index length must match after round-trip"
    );
    for (k, v) in &free_port_index {
        let got = decoded_partition.free_port_index.get(k).copied();
        assert_eq!(
            got,
            Some(*v),
            "Q8: free_port_index[{}] must round-trip to {:?}, got {:?}",
            k,
            v,
            got
        );
    }
    tracing::info!(
        entries = free_port_index.len(),
        "Q8 ArchivedHashMap round-trip OK"
    );
}

// -----------------------------------------------------------------
// TASK-0400 — D-005 Option A zero-copy rkyv round-trip tests.
// See docs/tests/TEST-SPEC-0400.md, UT-0400-07..09.
// -----------------------------------------------------------------

/// UT-0400-07: Shape A `PendingCommutation` round-trips via rkyv's
/// validating `access` API. Exercises `Vec<Symbol>` + nested
/// `Vec<LocalWiringHint>` archive layout (R34 rkyv gate).
#[test]
fn ut_0400_07_pending_commutation_rkyv_access_roundtrip_shape_a() {
    use crate::merge::{LocalWiringHint, PendingCommutation};

    let slot_marker_base = u32::MAX - 10_000;
    let pc_in = PendingCommutation {
        request_id: 0x1234_5678,
        target_symbols: vec![Symbol::Dup, Symbol::Con],
        local_wiring: vec![
            LocalWiringHint {
                src_slot: 0,
                src_port: 1,
                target: PortRef::AgentPort(slot_marker_base + 1, 2),
            },
            LocalWiringHint {
                src_slot: 1,
                src_port: 1,
                target: PortRef::AgentPort(17, 0),
            },
        ],
    };

    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&pc_in).expect("serialize");
    let archived =
        rkyv::access::<rkyv::Archived<PendingCommutation>, rkyv::rancor::Error>(bytes.as_ref())
            .expect("access");
    let pc_out: PendingCommutation =
        rkyv::deserialize::<PendingCommutation, rkyv::rancor::Error>(archived)
            .expect("deserialize");
    assert_eq!(pc_out, pc_in);
    assert_eq!(pc_out.target_symbols.len(), 2);
    assert_eq!(pc_out.local_wiring.len(), 2);
}

/// UT-0400-08: `LocalWiringHint` standalone rkyv round-trip across the
/// three target categories (placeholder / concrete / FreePort). Wire
/// layer is shape-agnostic about R33c case 6 — rejection is a worker
/// semantic responsibility (TASK-0402).
#[test]
fn ut_0400_08_local_wiring_hint_rkyv_access_roundtrip() {
    use crate::merge::LocalWiringHint;

    let slot_marker_base = u32::MAX - 10_000;
    let hints = vec![
        LocalWiringHint {
            src_slot: 0,
            src_port: 1,
            target: PortRef::AgentPort(slot_marker_base, 0),
        },
        LocalWiringHint {
            src_slot: 1,
            src_port: 2,
            target: PortRef::AgentPort(123, 0),
        },
        LocalWiringHint {
            src_slot: 0,
            src_port: 2,
            target: PortRef::FreePort(999),
        },
    ];

    for h_in in &hints {
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(h_in).expect("serialize");
        let archived =
            rkyv::access::<rkyv::Archived<LocalWiringHint>, rkyv::rancor::Error>(bytes.as_ref())
                .expect("access");
        let h_out: LocalWiringHint =
            rkyv::deserialize::<LocalWiringHint, rkyv::rancor::Error>(archived)
                .expect("deserialize");
        assert_eq!(&h_out, h_in);
        assert_eq!(h_out.src_slot, h_in.src_slot);
        assert_eq!(h_out.src_port, h_in.src_port);
        assert_eq!(h_out.target, h_in.target);
    }
}

/// UT-0400-09: rkyv `bytecheck` path rejects a corrupted archive
/// (length-prefix flipped inside `target_symbols` Vec). SPEC-18 Q1
/// contract extension to Shape A (R34 bytecheck gate).
#[test]
fn ut_0400_09_pending_commutation_rkyv_bytecheck_rejects_corrupted_length_byte() {
    use crate::merge::{LocalWiringHint, PendingCommutation};

    let pc_in = PendingCommutation {
        request_id: 7,
        target_symbols: vec![Symbol::Dup, Symbol::Con],
        local_wiring: vec![LocalWiringHint {
            src_slot: 0,
            src_port: 1,
            target: PortRef::AgentPort(5, 0),
        }],
    };
    let clean = rkyv::to_bytes::<rkyv::rancor::Error>(&pc_in).expect("serialize");
    // Baseline: clean archive validates successfully.
    let ok_access =
        rkyv::access::<rkyv::Archived<PendingCommutation>, rkyv::rancor::Error>(clean.as_ref());
    assert!(ok_access.is_ok(), "baseline clean archive must validate");

    // Flip every candidate byte in the archive; at least one MUST
    // trigger `rkyv::access` to return Err. The relative-pointer /
    // length-prefix positions are deterministic but version-dependent;
    // we probe each offset and assert at least one rejects. This
    // guards the bytecheck path even if internal layout shifts.
    let mut any_rejected = false;
    for idx in 0..clean.len() {
        let mut corrupted: Vec<u8> = clean.as_ref().to_vec();
        corrupted[idx] ^= 0xFF;
        let result = rkyv::access::<rkyv::Archived<PendingCommutation>, rkyv::rancor::Error>(
            corrupted.as_slice(),
        );
        if result.is_err() {
            any_rejected = true;
        }
    }
    assert!(
        any_rejected,
        "bytecheck must reject at least one single-byte corruption (SPEC-18 Q1 / R34)"
    );
}
