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
use crate::net::{Net, Symbol};
use crate::partition::{IdRange, Partition};
use crate::protocol::error::ProtocolError;
use crate::protocol::frame::{
    recv_frame, send_frame_v2, FrameHeader, DEFAULT_MAX_PAYLOAD_SIZE, FLAG_ARCHIVED,
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
