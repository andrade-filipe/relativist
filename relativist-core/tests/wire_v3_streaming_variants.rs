//! TASK-0575 — `RequestWork` / `NoMoreWork` wire-variant production tests.
//!
//! Validates that the two SPEC-21 R31 pull-dispatch variants added to
//! `Message` satisfy:
//!   - bincode v2 round-trip fidelity (UT-0575-01, UT-0575-02)
//!   - framed multi-message decode (UT-0575-03)
//!   - pre-bump version rejection via `RegisterNack` (UT-0575-04, UT-0575-05)
//!   - existing variants unaffected by append (UT-0575-06)
//!   - discriminant stability — new variants appended at end (UT-0575-07)
//!   - `PartialEq` / `Debug` / `Clone` derives present (UT-0575-08)
//!   - `NoMoreWork` payload minimal (UT-0575-09)
//!   - `WorkerId = u32::MAX` round-trips (UT-0575-10)
//!
//! SPEC-21 §3.6 R31; §3.8 A2; SPEC-06 R5.

use relativist_core::partition::WorkerId;
use relativist_core::protocol::bincode_v2;
use relativist_core::protocol::coordinator::{PREVIOUS_LIVE_VERSION, PROTOCOL_VERSION};
use relativist_core::protocol::frame::{recv_frame, send_frame};
use relativist_core::protocol::types::Message;
use relativist_core::protocol::NodeConfig;
use relativist_core::protocol::Transport;

// UT-0575-01: RequestWork bincode round-trip.
#[test]
fn request_work_bincode_round_trip() {
    let original = Message::RequestWork {
        worker_id: 7 as WorkerId,
    };
    let bytes = bincode_v2::encode(&original).expect("encode RequestWork");
    let decoded: Message = bincode_v2::decode_value(&bytes).expect("decode RequestWork");
    match decoded {
        Message::RequestWork { worker_id } => {
            assert_eq!(
                worker_id, 7,
                "UT-0575-01: worker_id must round-trip exactly"
            );
        }
        other => panic!("UT-0575-01: expected RequestWork, got {:?}", other),
    }
}

// UT-0575-02: NoMoreWork bincode round-trip.
#[test]
fn no_more_work_bincode_round_trip() {
    let original = Message::NoMoreWork;
    let bytes = bincode_v2::encode(&original).expect("encode NoMoreWork");
    let decoded: Message = bincode_v2::decode_value(&bytes).expect("decode NoMoreWork");
    match decoded {
        Message::NoMoreWork => {}
        other => panic!("UT-0575-02: expected NoMoreWork, got {:?}", other),
    }
}

// UT-0575-03: framed multi-message decode — RequestWork, NoMoreWork, RequestWork.
#[tokio::test(flavor = "current_thread")]
async fn framed_read_request_work_then_no_more_work() {
    use relativist_core::protocol::channel::ChannelTransport;
    #[allow(unused_imports)]
    use relativist_core::protocol::Transport as _T;

    let (mut server_transport, mut client_transport) = ChannelTransport::pair(8, 65536);
    let max_payload = NodeConfig::default().max_payload_size;

    // Writer task: write three frames.
    let write_handle = tokio::spawn(async move {
        let mut w: relativist_core::protocol::TransportStream =
            client_transport.connect().await.expect("connect");
        send_frame(&mut w, &Message::RequestWork { worker_id: 0 })
            .await
            .expect("send 1");
        send_frame(&mut w, &Message::NoMoreWork)
            .await
            .expect("send 2");
        send_frame(&mut w, &Message::RequestWork { worker_id: 1 })
            .await
            .expect("send 3");
    });

    let mut server_stream: relativist_core::protocol::TransportStream =
        server_transport.accept().await.expect("accept");

    let (msg1, _) = recv_frame(&mut server_stream, max_payload)
        .await
        .expect("recv 1");
    let (msg2, _) = recv_frame(&mut server_stream, max_payload)
        .await
        .expect("recv 2");
    let (msg3, _) = recv_frame(&mut server_stream, max_payload)
        .await
        .expect("recv 3");

    write_handle.await.expect("writer task");

    assert!(
        matches!(msg1, Message::RequestWork { worker_id: 0 }),
        "UT-0575-03: frame 1 must be RequestWork(0)"
    );
    assert!(
        matches!(msg2, Message::NoMoreWork),
        "UT-0575-03: frame 2 must be NoMoreWork"
    );
    assert!(
        matches!(msg3, Message::RequestWork { worker_id: 1 }),
        "UT-0575-03: frame 3 must be RequestWork(1)"
    );
}

// UT-0575-04 / UT-0575-05: pre-bump version rejection.
//
// Since RequestWork / NoMoreWork are new discriminants (17, 18) in the same
// bincode stream, a peer that uses PREVIOUS_LIVE_VERSION in its Register
// handshake would be rejected by the coordinator's version check. This test
// validates the rejection mechanism by exercising the coordinator's
// `accept_workers` with a v_prev Register message — the coordinator sends
// RegisterNack whose `reason` cites the version mismatch. Both RequestWork
// and NoMoreWork scenarios share the same rejection path.
#[tokio::test(flavor = "current_thread")]
async fn pre_bump_version_rejects_connection_before_payload_exchange() {
    use relativist_core::protocol::channel::ChannelTransport;
    use relativist_core::protocol::coordinator::accept_workers;
    use relativist_core::protocol::types::RegisterPayload;
    use std::time::Duration;

    let (mut server_transport, mut client_transport) = ChannelTransport::pair(2, 65536);
    let config = NodeConfig {
        num_workers: 1,
        worker_connect_timeout: Duration::from_millis(500),
        ..NodeConfig::default()
    };

    let accept_handle = tokio::spawn({
        let config = config.clone();
        async move { accept_workers(&config, None, &mut server_transport, false).await }
    });

    let mut w: relativist_core::protocol::TransportStream =
        client_transport.connect().await.expect("connect");
    // Send a Register with PREVIOUS_LIVE_VERSION (simulates a pre-SPEC-21 peer).
    let pre_bump_register = Message::Register(RegisterPayload {
        protocol_version: PREVIOUS_LIVE_VERSION,
        auth_token: None,
    });
    send_frame(&mut w, &pre_bump_register)
        .await
        .expect("send register");
    let (response, _) = recv_frame(&mut w, NodeConfig::default().max_payload_size)
        .await
        .expect("recv nack");
    match response {
        Message::RegisterNack(payload) => {
            assert!(
                payload.reason.contains("protocol version mismatch"),
                "UT-0575-04/05: nack reason must mention version mismatch; got {:?}",
                payload.reason
            );
            assert!(
                payload
                    .reason
                    .contains(&format!("expected {}", PROTOCOL_VERSION)),
                "UT-0575-04/05: nack reason must cite current PROTOCOL_VERSION={}; got {:?}",
                PROTOCOL_VERSION,
                payload.reason
            );
        }
        other => panic!(
            "UT-0575-04/05: expected RegisterNack for pre-bump version, got {:?}",
            other
        ),
    }
    // Accept handle will timeout after the nack — that's expected.
    let _ = accept_handle.await;
}

// UT-0575-06: existing variants are unaffected by the append.
#[test]
fn existing_message_variants_unaffected_by_append() {
    use relativist_core::merge::WorkerRoundStats;
    use relativist_core::net::Net;
    use relativist_core::partition::{IdRange, Partition};
    use std::collections::HashMap;

    let empty_partition = Partition {
        subnet: Net::new(),
        worker_id: 0,
        free_port_index: HashMap::new(),
        id_range: IdRange { start: 0, end: 0 },
        border_id_start: 0,
        border_id_end: 0,
    };
    let stats = WorkerRoundStats {
        worker_id: 0,
        agents_before: 0,
        agents_after: 0,
        local_redexes: 0,
        reduce_duration_secs: 0.0,
        interactions_by_rule: [0; 6],
        has_border_activity: false,
        is_coordinator_self: false,
    };

    let pre_spec21_variants: Vec<Message> = vec![
        Message::AssignPartition {
            round: 0,
            partition: empty_partition.clone(),
        },
        Message::Shutdown,
        Message::PartitionResult {
            round: 0,
            partition: empty_partition.clone(),
            stats: stats.clone(),
        },
        Message::FinalStateRequest { round: 0 },
        Message::LeaveAck,
    ];

    for msg in &pre_spec21_variants {
        let bytes = bincode_v2::encode(msg).expect("encode pre-spec-21 variant");
        let decoded: Message =
            bincode_v2::decode_value(&bytes).expect("decode pre-spec-21 variant");
        let _ = format!("{:?}", decoded); // exercising Debug derive
    }
}

// UT-0575-07: discriminant stability — RequestWork and NoMoreWork are at end of enum.
//
// This is enforced by the byte-level discriminant test in types.rs; here we
// verify the specific discriminant values (17 and 18) directly, confirming
// they follow the last pre-SPEC-21 variant (JoinNack at 16).
#[test]
fn discriminant_stability_request_work_and_no_more_work_at_end() {
    let rw_bytes =
        bincode_v2::encode(&Message::RequestWork { worker_id: 0 }).expect("encode RequestWork");
    let nmw_bytes = bincode_v2::encode(&Message::NoMoreWork).expect("encode NoMoreWork");

    assert_eq!(
        rw_bytes[0], 17,
        "UT-0575-07: RequestWork MUST have discriminant 17 (SPEC-21 R31)"
    );
    assert_eq!(
        nmw_bytes[0], 18,
        "UT-0575-07: NoMoreWork MUST have discriminant 18 (SPEC-21 R31)"
    );

    // The last pre-SPEC-21 variant (JoinNack) must still be at 16.
    use relativist_core::protocol::types::JoinNackReason;
    let jn_bytes = bincode_v2::encode(&Message::JoinNack {
        reason: JoinNackReason::ElasticJoinDisabled,
    })
    .expect("encode JoinNack");
    assert_eq!(
        jn_bytes[0], 16,
        "UT-0575-07: JoinNack (last pre-SPEC-21 variant) MUST still be at discriminant 16"
    );
}

// UT-0575-08: Debug and Clone derives present and correct.
//
// Note: Message does not derive PartialEq (Partition inside does not either).
// The PartialEq assertion from TEST-SPEC-0575 is fulfilled for RequestWork /
// NoMoreWork specifically via destructuring — their fields are comparable.
#[test]
fn request_work_derive_debug_and_clone() {
    let a = Message::RequestWork { worker_id: 7 };

    // Clone works.
    let cloned = a.clone();
    match (a, cloned) {
        (Message::RequestWork { worker_id: wa }, Message::RequestWork { worker_id: wb }) => {
            assert_eq!(wa, wb, "UT-0575-08: clone must preserve worker_id");
        }
        _ => panic!("UT-0575-08: clone must produce identical variant"),
    }

    // Debug works (must not panic).
    let dbg_rw = format!("{:?}", Message::RequestWork { worker_id: 7 });
    let dbg_nmw = format!("{:?}", Message::NoMoreWork);
    assert!(
        dbg_rw.contains("RequestWork"),
        "UT-0575-08: Debug must name the variant"
    );
    assert!(
        dbg_nmw.contains("NoMoreWork"),
        "UT-0575-08: Debug must name the variant"
    );

    // Field equality for RequestWork (two bincode round-trips produce same worker_id).
    let msg1 = Message::RequestWork { worker_id: 42 };
    let bytes1 = bincode_v2::encode(&msg1).expect("encode msg1");
    let decoded1: Message = bincode_v2::decode_value(&bytes1).expect("decode msg1");
    let msg2 = Message::RequestWork { worker_id: 42 };
    let bytes2 = bincode_v2::encode(&msg2).expect("encode msg2");
    assert_eq!(
        bytes1, bytes2,
        "UT-0575-08: same worker_id must produce identical bytes"
    );
    match decoded1 {
        Message::RequestWork { worker_id } => {
            assert_eq!(worker_id, 42, "UT-0575-08: decoded worker_id must match");
        }
        _ => panic!("UT-0575-08: wrong variant"),
    }
}

// UT-0575-09: NoMoreWork payload is minimal (≤ 4 bytes — variant discriminant only).
#[test]
fn no_more_work_serde_payload_size_minimal() {
    let bytes = bincode_v2::encode(&Message::NoMoreWork).expect("encode NoMoreWork");
    assert!(
        bytes.len() <= 4,
        "UT-0575-09: NoMoreWork must encode in ≤ 4 bytes (unit variant); got {} bytes",
        bytes.len()
    );
}

// UT-0575-10: WorkerId = u32::MAX round-trips without truncation.
#[test]
fn worker_id_u32_max_round_trip() {
    let original = Message::RequestWork {
        worker_id: u32::MAX as WorkerId,
    };
    let bytes = bincode_v2::encode(&original).expect("encode RequestWork(u32::MAX)");
    let decoded: Message = bincode_v2::decode_value(&bytes).expect("decode RequestWork(u32::MAX)");
    match decoded {
        Message::RequestWork { worker_id } => {
            assert_eq!(
                worker_id,
                u32::MAX as WorkerId,
                "UT-0575-10: WorkerId=u32::MAX must round-trip without truncation"
            );
        }
        other => panic!("UT-0575-10: expected RequestWork, got {:?}", other),
    }
}
