//! TASK-0596 — SPEC-19 §3.4 R35a integration tests.
//!
//! Validates that the new `CompactSubnet.free_list` wire suffix flows end-to-end
//! through every relevant transport path:
//!
//!   - IT-0596-08: pre-bump (`PREVIOUS_LIVE_VERSION`) Register payload is
//!     rejected by the v_current coordinator with a structured RegisterNack —
//!     i.e. the version gate exists and fires (the spec calls this an
//!     "UnsupportedVersion-class" rejection; the project's concrete error
//!     pathway is `Message::RegisterNack` carrying a `protocol version
//!     mismatch` reason — see `coordinator.rs::accept_workers`).
//!
//!   - PT-0596-09: property-test on randomly generated nets exercising the
//!     full empty/populated/sparse/order-permuted free_list combination space.
//!
//!   - IT-0596-10: in-process worker/coordinator partition-transfer harness
//!     using `ChannelTransport` (the canonical loopback for relativist tests
//!     since SPEC-17). Coordinator sends `AssignPartition` whose
//!     `subnet.free_list = [7, 3, 1]`; worker receives and asserts the list
//!     and `next_id` are intact, AND that the next `create_agent` on the
//!     received side pops `AgentId(1)` — the LAST element of the Vec, which
//!     is the LIFO top per SPEC-22 R5/R10c (push/pop at end). Coordinator
//!     and worker MUST agree on this allocation.
//!
//!   - IT-0596-11: minimal historical witness for QA-D009-001. Performs a
//!     direct `bincode_v2` round-trip of a `Partition` whose
//!     `subnet.free_list = [42]`. Pre-fix this fails because `into_net`
//!     hard-coded `Vec::new()`; post-fix the assertion passes. This test
//!     stays in the suite as the regression sentinel.
//!
//! Spec dependencies: SPEC-19 R35a (commit c4c80b8); SPEC-22 R9a, R10b/R12a,
//! R10c; SPEC-18 R28, R31, R33; SPEC-04 §A7.

use std::collections::HashMap;
use std::time::Duration;

use proptest::prelude::*;

use relativist_core::net::{AgentId, Net, Symbol};
use relativist_core::partition::compact::CompactSubnet;
use relativist_core::partition::{IdRange, Partition};
use relativist_core::protocol::bincode_v2;
use relativist_core::protocol::channel::ChannelTransport;
use relativist_core::protocol::coordinator::{
    accept_workers, PREVIOUS_LIVE_VERSION, PROTOCOL_VERSION,
};
use relativist_core::protocol::frame::{recv_frame, send_frame};
use relativist_core::protocol::types::{Message, RegisterPayload};
use relativist_core::protocol::{NodeConfig, Transport, TransportStream};

// ---------------------------------------------------------------------------
// Test fixtures
// ---------------------------------------------------------------------------

/// Build a fresh `Net` with a populated `free_list`. Helper used by IT-0596-10
/// and IT-0596-11.
///
/// CAVEAT: `create_agent` recycles via `free_list`, so a `create + remove`
/// CYCLE does not advance `next_id` — it just bounces the same id forever.
/// Grow the arena via fresh creates first, THEN remove the lot, THEN
/// overwrite `free_list` with the test-prescribed value.
fn make_net_with_free_list(ids: Vec<AgentId>, arena_grow_to: u32) -> Net {
    let mut net = Net::new();
    let allocated: Vec<AgentId> = (0..arena_grow_to)
        .map(|_| net.create_agent(Symbol::Era))
        .collect();
    for id in allocated {
        net.remove_agent(id);
    }
    net.free_list.clear();
    net.free_list = ids;
    net
}

// ---------------------------------------------------------------------------
// IT-0596-08 — pre-bump version rejected (UnsupportedVersion-class).
// ---------------------------------------------------------------------------

/// IT-0596-08: `Register` carrying `PREVIOUS_LIVE_VERSION` MUST be rejected.
/// The "UnsupportedVersion-class" error in the project's protocol is a
/// `Message::RegisterNack` with reason "protocol version mismatch" — the
/// coordinator path enforces this via `accept_workers` before any payload
/// (let alone a `CompactSubnet` without `free_list`) is decoded. This test
/// pins that the version gate is wired AT THE RECEIVE PATH, not just the
/// constant declaration site.
#[tokio::test(flavor = "current_thread")]
async fn wire_v3_payload_is_rejected_with_unsupported_version() {
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

    let mut w: TransportStream = client_transport.connect().await.expect("connect");
    let pre_bump = Message::Register(RegisterPayload {
        protocol_version: PREVIOUS_LIVE_VERSION,
        auth_token: None,
    });
    send_frame(&mut w, &pre_bump).await.expect("send pre-bump");

    let (response, _) = recv_frame(&mut w, NodeConfig::default().max_payload_size)
        .await
        .expect("recv");

    match response {
        Message::RegisterNack(p) => {
            assert!(
                p.reason.contains("protocol version mismatch"),
                "IT-0596-08: nack must cite protocol version mismatch (got {:?})",
                p.reason,
            );
            assert!(
                p.reason.contains(&format!("expected {}", PROTOCOL_VERSION)),
                "IT-0596-08: nack must cite current PROTOCOL_VERSION (got {:?})",
                p.reason,
            );
            assert!(
                !p.reason.is_empty(),
                "IT-0596-08: rejection MUST NOT be silent / empty",
            );
        }
        other => panic!(
            "IT-0596-08: pre-bump Register MUST be NACKed (UnsupportedVersion-class); \
             got {:?}",
            other
        ),
    }
    let _ = accept_handle.await;
}

// ---------------------------------------------------------------------------
// PT-0596-09 — proptest on random nets with free_list
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 64,
        ..ProptestConfig::default()
    })]

    /// PT-0596-09: for any well-formed `Net`, the round-trip
    /// `from_net -> into_net` preserves `free_list`, `next_id`, and the
    /// agent arena.
    #[test]
    fn proptest_round_trip_arbitrary_net_with_free_list(
        arena_grow_to in 1u32..=64,
        free_ids in proptest::collection::vec(0u32..64, 0..=16usize),
    ) {
        // Materialise a Net with the requested arena size then install the
        // free_list. We trim ids to be in-range and dedup for a well-formed
        // Net (the upstream invariant; we are testing the wire form, not
        // upstream invariants). Order is preserved.
        let mut trimmed = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for id in free_ids {
            if id < arena_grow_to && seen.insert(id) {
                trimmed.push(id);
            }
        }
        let net = make_net_with_free_list(trimmed.clone(), arena_grow_to);

        let compact = CompactSubnet::from_net(&net);
        let back = compact.into_net();

        prop_assert_eq!(&back.free_list, &trimmed,
            "PT-0596-09: free_list must round-trip verbatim (Vec equality)");
        prop_assert_eq!(back.free_list.len(), trimmed.len());
        prop_assert_eq!(back.next_id, net.next_id);
        prop_assert_eq!(back.agents.len(), net.agents.len());
    }
}

// ---------------------------------------------------------------------------
// IT-0596-10 — in-process partition transfer over ChannelTransport
// ---------------------------------------------------------------------------

/// IT-0596-10: end-to-end partition transfer preserves `subnet.free_list`.
///
/// Uses `ChannelTransport` (SPEC-17 in-process loopback — the existing test
/// scaffold for transport regression). The coordinator-side hand-builds a
/// `Message::AssignPartition` whose `subnet.free_list = [7, 3, 1]`, sends it
/// via `send_frame` (so it is bincode-encoded with the v_current
/// PROTOCOL_VERSION), and the worker-side decodes via `recv_frame` and
/// asserts every concretely-observable property of SPEC-22 R10b/R10c/R12a.
#[tokio::test(flavor = "current_thread")]
async fn tcp_two_worker_partition_transfer_preserves_free_list() {
    // Build the partition with the canonical free_list fixture from the test spec.
    let net = make_net_with_free_list(vec![7u32, 3u32, 1u32], 10);
    let sent_next_id = net.next_id;
    let partition = Partition {
        subnet: net,
        worker_id: 0,
        free_port_index: HashMap::new(),
        id_range: IdRange { start: 0, end: 100 },
        border_id_start: 0,
        border_id_end: 0,
    };

    // Open the loopback. We don't need accept_workers here — we are exercising
    // the framing + bincode path directly with a single channel pair.
    let (mut server_transport, mut client_transport) = ChannelTransport::pair(1, 1 << 20);
    let server_handle = tokio::spawn(async move {
        let mut s = server_transport.accept().await.expect("server accept");
        // Coordinator -> worker: AssignPartition.
        let assign = Message::AssignPartition {
            round: 0,
            partition,
        };
        send_frame(&mut s, &assign).await.expect("send assign");
    });

    let mut c: TransportStream = client_transport.connect().await.expect("connect");
    let (msg, _) = recv_frame(&mut c, NodeConfig::default().max_payload_size)
        .await
        .expect("recv assign");

    let received = match msg {
        Message::AssignPartition { partition, .. } => partition,
        other => panic!("IT-0596-10: expected AssignPartition, got {:?}", other),
    };

    assert_eq!(
        received.subnet.free_list,
        vec![7u32, 3u32, 1u32],
        "IT-0596-10: subnet.free_list must round-trip across the wire",
    );
    assert_eq!(
        received.subnet.next_id, sent_next_id,
        "IT-0596-10: SPEC-22 R10b/R12a — next_id must be coordinator/worker-consistent",
    );

    // SPEC-22 R10c: the LIFO stack is encoded as a `Vec` with push/pop at the
    // END — the last element of `free_list` is the next id `create_agent`
    // returns. For `[7, 3, 1]` that is `1`. Coordinator and worker MUST agree
    // on this allocation, otherwise SPEC-22 R10b/R12a is violated and the
    // round-N+1 reduction diverges.
    let mut received_net = received.subnet;
    let next_alloc = received_net.create_agent(Symbol::Era);
    assert_eq!(
        next_alloc, 1,
        "IT-0596-10: SPEC-22 R10c — first create_agent on receiver MUST pop \
         AgentId(1) (last element of `free_list = [7, 3, 1]`, which is the \
         LIFO top per SPEC-22 R5/R10c push/pop-at-end semantics).",
    );

    server_handle.await.unwrap();
}

// ---------------------------------------------------------------------------
// IT-0596-11 — historical regression witness for QA-D009-001
// ---------------------------------------------------------------------------

/// IT-0596-11: pre-R35a bug-witness. Direct bincode round-trip of a
/// `Partition` whose `subnet.free_list = [42]`.
///
/// Pre-fix (commit c4c80b8 — SPEC-19 R35a) this assertion fails because
/// `CompactSubnet::into_net` hard-codes `free_list: Vec::new()`. Post-fix
/// (this task — TASK-0596) the round-trip is loss-free. This test is the
/// historical witness for QA-D009-001 and stays in the suite as a regression
/// sentinel: any future "simplification" that drops the field again will
/// trip this single-element minimal case immediately.
#[test]
fn regression_witness_pre_r35a_bug_repro() {
    let net = make_net_with_free_list(vec![42u32], 50);
    let partition = Partition {
        subnet: net,
        worker_id: 0,
        free_port_index: HashMap::new(),
        id_range: IdRange { start: 0, end: 100 },
        border_id_start: 0,
        border_id_end: 0,
    };

    // Round-trip through Partition's own serde adapters — this is the exact
    // path used by the wire protocol (`Partition::subnet` uses the
    // serialize_with/deserialize_with adapters from compact.rs).
    let bytes = bincode_v2::encode(&partition).expect("encode partition");
    let back: Partition = bincode_v2::decode_value(&bytes).expect("decode partition");

    assert_eq!(
        back.subnet.free_list,
        vec![42u32],
        "IT-0596-11 (QA-D009-001 witness): SPEC-19 R35a (commit c4c80b8) — \
         without this fix `into_net` hard-codes `free_list: Vec::new()` and \
         this single-element case fails. Post-fix it passes.",
    );
}
