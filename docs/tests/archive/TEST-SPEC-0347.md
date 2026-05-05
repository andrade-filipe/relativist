# TEST-SPEC-0347: PROTOCOL_VERSION bump 1 → 2

**Task:** TASK-0347
**Spec:** SPEC-18 R28, R29, R30, R31, R32, R35 partial (item 2.23, §3.6)
**Generated:** 2026-04-16
**Baseline before this task:** 815+ (post-TASK-0346)

---

## Scope note

This is the **atomic commit** of the v2 wire break. Prior tasks
accumulated wire-incompatible changes (bincode v2 + compact PortRef +
9-byte header + LZ4) under the still-v1 `PROTOCOL_VERSION`. This task
flips the constant 1 → 2 and adds handshake handling so a v1 worker
trying to register against a v2 coordinator (or vice versa) gets a
clear, fast rejection instead of a corrupted-frame parse error.

After TASK-0347 lands, SPEC-18 §3.1-3.4 + §3.6 + §3.7 + §3.8 (the
non-rkyv portion of v2 wire format) is fully shipped. §3.5 (rkyv)
remains deferred to ROADMAP item 2.24 per DEFERRED-WORK D-002.

---

## R1: constant updated

```rust
#[test]
fn protocol_version_is_two() {
    assert_eq!(PROTOCOL_VERSION, 2, "v2 wire format requires PROTOCOL_VERSION = 2");
}
```

This test is the canary against accidental rollback during merge
conflicts.

## R2: coordinator rejects v1 worker via RegisterNack (R29)

```rust
#[tokio::test]
async fn coordinator_rejects_v1_worker_with_register_nack() {
    let (mut transport, mut coordinator) = ChannelTransport::new_pair_with_coordinator();
    let v1_register = Message::Register(RegisterPayload {
        protocol_version: 1,
        worker_id: WorkerId(7),
        capabilities: default_caps(),
    });
    transport.send(v1_register).await.unwrap();

    let response = transport.recv().await.unwrap();
    let nack = match response {
        Message::RegisterNack(p) => p,
        other => panic!("expected RegisterNack, got {:?}", other),
    };
    assert!(nack.reason.contains("protocol version mismatch"),
            "reason missing version-mismatch phrase: {}", nack.reason);
    assert!(nack.reason.contains("expected 2"), "expected version absent: {}", nack.reason);
    assert!(nack.reason.contains("got 1"), "received version absent: {}", nack.reason);

    // Coordinator should also close / drop the worker registration.
    assert!(transport.is_closed_after_recv().await);
}
```

## R3: worker handles RegisterNack and terminates (R30)

```rust
#[tokio::test]
async fn worker_terminates_on_version_mismatch_nack() {
    let (mut transport, worker_handle) = ChannelTransport::new_pair_with_worker();
    // Worker sends Register; coordinator (us, simulated) replies with NACK.
    let _ = transport.recv().await.unwrap();  // drain the worker's Register
    transport.send(Message::RegisterNack(RegisterNackPayload {
        reason: "protocol version mismatch: expected 2, got 1".into(),
    })).await.unwrap();

    let outcome = worker_handle.await.unwrap();
    assert!(
        matches!(outcome, Err(WorkerExit::VersionMismatch { .. })),
        "worker did not terminate with VersionMismatch, got {:?}",
        outcome,
    );
}
```

## R4: `ProtocolError::VersionMismatch` formatting

```rust
#[test]
fn version_mismatch_error_renders() {
    let e = ProtocolError::VersionMismatch { expected: 2, received: 1 };
    let s = e.to_string();
    assert!(s.contains("expected 2"), "got: {}", s);
    assert!(s.contains("received 1"), "got: {}", s);
}
```

## R5: full v2 round-trip identity (R32) — every Message variant through complete pipeline

This is the **final acceptance test for SPEC-18 §3.1-3.4 + §3.6**:
every `Message` variant survives the complete v2 wire pipeline
(bincode v2 + compact PortRef + 9-byte header + LZ4 + frame).

```rust
#[tokio::test]
async fn v2_pipeline_round_trip_all_message_variants() {
    let tuning = TransportTuning::default();  // production defaults
    for msg in sample_all_message_variants() {
        let mut buf: Vec<u8> = Vec::new();
        send_frame_with_tuning(&mut buf, &msg, &tuning).await.unwrap();
        let mut cur = std::io::Cursor::new(buf);
        let back = recv_frame(&mut cur).await.unwrap();
        assert_eq!(back, msg, "variant {:?} failed v2 pipeline round-trip", msg);
    }
}
```

This test must enumerate **every** variant via exhaustive `match`, so
adding a new `Message` variant in the future without updating this
test breaks the build.

## R6: integration — full coordinator + simulated v1 worker over loopback

```rust
#[tokio::test]
async fn integration_v1_worker_rejected_by_v2_coordinator_over_tcp() {
    let coord = spawn_coordinator_for_test().await;
    let mut socket = TcpStream::connect(coord.addr()).await.unwrap();

    // Hand-craft a v1-shaped Register message (8-byte header, no flags byte,
    // protocol_version = 1). This bypasses the new send_frame to simulate
    // a true v1 client.
    let payload = legacy_v1_register_bytes(WorkerId(99));
    socket.write_all(&payload).await.unwrap();

    // Read the coordinator's response — must arrive promptly and indicate
    // version mismatch (either as a v2 RegisterNack frame, which the v1
    // worker can't parse, or as a clean disconnect).
    let mut buf = vec![0u8; 1024];
    let n = tokio::time::timeout(Duration::from_secs(2), socket.read(&mut buf))
        .await.unwrap().unwrap_or(0);
    assert!(n == 0 || buf[..n].windows(8).any(|w| w == b"mismatch"),
            "coordinator did not reject v1 worker promptly");
}
```

If `legacy_v1_register_bytes` is non-trivial to construct, gate this
test under `#[ignore]` and rely on R2's in-memory ChannelTransport
test to cover the contract.

## R7: smoke — coordinator + worker both v2 still works

```rust
#[tokio::test]
async fn smoke_v2_coordinator_v2_worker_handshake_succeeds() {
    let (mut transport, _coordinator) = ChannelTransport::new_pair_with_coordinator();
    transport.send(Message::Register(RegisterPayload {
        protocol_version: PROTOCOL_VERSION,
        worker_id: WorkerId(1),
        capabilities: default_caps(),
    })).await.unwrap();

    let response = transport.recv().await.unwrap();
    assert!(matches!(response, Message::RegisterAck(_)),
            "v2/v2 handshake should ACK, got {:?}", response);
}
```

## Acceptance

1. `cargo test --workspace` count: 815 → **819+** (≥ +4 covering R1,
   R2, R4, R5; R3/R6/R7 add coverage but may share test counts with
   helpers).
2. All previously passing tests still pass.
3. `cargo clippy --workspace --all-targets -- -D warnings` clean.
4. `cargo fmt --check` clean.
5. Manual smoke (release): coordinator + worker both v2 →
   `relativist compute add 3 5` distributed run produces `Result: 8`.
6. Documentation: `pipeline-state.md` marks SPEC-18 wire-format-v2
   (item 2.23) as COMPLETE; `V2-FEATURE-MATRIX.md` row 2.23
   IN PROGRESS → DONE; `DEFERRED-WORK.md` D-002 already records the
   rkyv portion deferred to 2.24.

## Out of Scope

- rkyv (deferred to 2.24, see DEFERRED-WORK D-002).
- Anything in §3.5 of SPEC-18.
- Coordinator-Free Round (item 2.34, next on Tier 1 after 2.23).
