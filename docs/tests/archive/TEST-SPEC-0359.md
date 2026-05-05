# TEST-SPEC-0359: T11-T14 round-trip + corruption + archive-flag suite

**Task:** TASK-0359
**Spec:** SPEC-18 §7.2 (T11, T12, T13, T14), R27
**Generated:** 2026-04-16
**Baseline before this task:** 904 lib (default) / 930 lib (`--features zero-copy`, post-TASK-0358).

---

## Scope note

T11-T14 form the **D-002 acceptance gate** — when these four tests are
GREEN under `--features zero-copy`, DEFERRED-WORK row D-002 may be
closed. The suite lives in a single coherent file
`relativist-core/src/protocol/zero_copy_tests.rs` (recommended) or the
existing `frame.rs` test module (developer choice). All tests are
gated under `#[cfg(all(test, feature = "zero-copy"))]`.

The tuple-form match arms `Err(ProtocolError::ArchiveValidationFailed(_))`
are mandated by spec-critic DC-2 (2026-04-16); future flips to struct
form would break compilation.

T11/T12/T13 cover round-trip + LZ4 + corruption (R27, R26).
T14 is the wire-end of the chain; T14b is the R22 enforcement probe.

This TEST-SPEC ALSO covers the integration ends already enumerated as
QA candidates in earlier TEST-SPECs: alignment correctness (T13b ←
TEST-SPEC-0355) and hot-path-only enforcement on the wire (T14c ←
TEST-SPEC-0356 UT-03/UT-04). They appear here as the final acceptance
gate, even if duplicating per-task probes — D-002 closure depends on
them living together as a coherent block.

---

## T11-01: rkyv round-trip identity on a realistic Partition

**Target file:** `relativist-core/src/protocol/zero_copy_tests.rs`
(create new file under `#[cfg(all(test, feature = "zero-copy"))]`).
**Feature gate:** `#[cfg(all(test, feature = "zero-copy"))]`.
**R-mapping:** R27, T11.

```rust
#[test]
fn t11_rkyv_round_trip_partition_realistic() -> Result<(), Box<dyn std::error::Error>> {
    let p = sample_partition_realistic_10_agents_3_ranges();
    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&p)?;
    let archived = rkyv::access::<rkyv::Archived<Partition>, rkyv::rancor::Error>(
        bytes.as_slice(),
    )?;
    let round: Partition =
        rkyv::deserialize::<Partition, rkyv::rancor::Error>(archived)?;
    assert_eq!(p, round, "T11 R27: round-trip identity must hold");
    Ok(())
}
```

**Fixture spec:** `sample_partition_realistic_10_agents_3_ranges` —
~10 agents with mixed Symbols (3 CON + 4 DUP + 3 ERA), 3 IdRange
entries, 5 entries in `free_port_index` (HashMap<u32, PortRef>) with
mixed `AgentPort` and `Disconnected` values. The fixture lives in
`partition::tests` as `pub(crate)` (lifted by TASK-0353 if it's
private currently) or inlined in this test file.

---

## T11-02: rkyv round-trip across 5 representative partition shapes

**Target file:** same as T11-01.
**Feature gate:** `#[cfg(all(test, feature = "zero-copy"))]`.
**R-mapping:** R27, T11 (table-driven coverage).

```rust
#[test]
fn t11_rkyv_round_trip_across_representative_shapes() -> Result<(), Box<dyn std::error::Error>> {
    let shapes: Vec<(&str, Partition)> = vec![
        ("small_2_agents",      sample_partition_2_agents()),
        ("medium_10_agents",    sample_partition_realistic_10_agents_3_ranges()),
        ("deeply_nested_dup",   sample_partition_dup_chain(8)),
        ("single_agent_no_borders", sample_partition_single_agent()),
        ("all_symbols_present", sample_partition_one_of_each_symbol()),
    ];
    for (name, p) in shapes {
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&p)?;
        let archived = rkyv::access::<rkyv::Archived<Partition>, rkyv::rancor::Error>(
            bytes.as_slice(),
        )?;
        let round: Partition =
            rkyv::deserialize::<Partition, rkyv::rancor::Error>(archived)?;
        assert_eq!(p, round, "T11 fixture `{}` must round-trip", name);
    }
    Ok(())
}
```

**Fixture specs:** see T11-01 fixture; the additional 4 fixtures
(small / nested-dup / single-agent / one-of-each-symbol) are inlined
in the test file or lifted from existing test modules.

---

## T12-01: rkyv + LZ4 round-trip via `decompress_payload_aligned`

**Target file:** same.
**Feature gate:** `#[cfg(all(test, feature = "zero-copy"))]`.
**R-mapping:** R23, T12, R12 (CRC ordering preserved end-to-end).

```rust
#[test]
fn t12_rkyv_with_lz4_round_trip() -> Result<(), Box<dyn std::error::Error>> {
    let p = sample_partition_realistic_10_agents_3_ranges();
    let archived = rkyv::to_bytes::<rkyv::rancor::Error>(&p)?;
    // Compress -> Decompress (back into AlignedVec) -> rkyv access -> deserialize.
    let compressed = crate::protocol::compression::compress_payload(archived.as_slice());
    let decompressed = crate::protocol::compression::decompress_payload_aligned(&compressed)
        .map_err(|s| -> Box<dyn std::error::Error> { s.into() })?;
    let view = rkyv::access::<rkyv::Archived<Partition>, rkyv::rancor::Error>(
        decompressed.as_slice(),
    )?;
    let round: Partition =
        rkyv::deserialize::<Partition, rkyv::rancor::Error>(view)?;
    assert_eq!(p, round, "T12: rkyv + LZ4 round-trip identity");
    Ok(())
}
```

**Note:** `decompress_payload_aligned` is the helper introduced by
TASK-0357 (returns `AlignedVec`). If the developer chose a different
name, update the import here.

---

## T13-01: rkyv validation rejects single-byte corruption near root pointer

**Target file:** same.
**Feature gate:** `#[cfg(all(test, feature = "zero-copy"))]`.
**R-mapping:** R26, T13.

```rust
#[test]
fn t13_rkyv_validation_rejects_corruption_at_root_pointer() {
    let p = sample_partition_realistic_10_agents_3_ranges();
    let mut bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&p)
        .expect("test fixture serializes");
    // T13 caveat (per TASK-0359 Notes): flip a byte in the rkyv root
    // pointer (last 4 bytes — the relative offset). Mid-data flips may
    // not be detected.
    let len = bytes.len();
    assert!(len >= 4, "archive too small for root-pointer flip");
    bytes[len - 1] ^= 0xFF;
    let result = rkyv::access::<rkyv::Archived<Partition>, rkyv::rancor::Error>(
        bytes.as_slice(),
    );
    assert!(result.is_err(),
        "T13 R26: rkyv validation MUST reject root-pointer corruption");
}
```

**Note:** the byte to flip MUST be the LAST byte (rkyv puts the root
pointer at the tail). Flipping middle-of-data may yield a valid
re-interpretable u32 / etc., which rkyv does not detect (round-trip
correctness, not byte-level integrity).

---

## T13-02: corrupted archive surfaces as `ArchiveValidationFailed` via `recv_frame`

**Target file:** same.
**Feature gate:** `#[cfg(all(test, feature = "zero-copy"))]`.
**R-mapping:** R26, T13 (end-to-end via wire path).

```rust
#[tokio::test]
async fn t13_corrupted_archive_yields_archive_validation_failed_via_recv() -> Result<(), Box<dyn std::error::Error>> {
    let original = Message::AssignPartition {
        round: 1,
        partition: sample_partition_realistic_10_agents_3_ranges(),
    };
    // Capture send_frame_v2 output bytes.
    let (mut tx, mut sniff) = tokio::io::duplex(64 * 1024);
    send_frame_v2(&mut tx, &original, usize::MAX, true).await?;
    drop(tx);
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut buf = Vec::new();
    sniff.read_to_end(&mut buf).await?;
    // Flip the last payload byte (rkyv root pointer area).
    let last = buf.len() - 1;
    buf[last] ^= 0xFF;
    // Recompute header CRC so the failure surfaces at rkyv access, NOT CRC check.
    let new_crc = crc32fast::hash(&buf[FRAME_HEADER_SIZE..]);
    buf[4..8].copy_from_slice(&new_crc.to_le_bytes());
    let (mut tx2, mut rx2) = tokio::io::duplex(64 * 1024);
    tx2.write_all(&buf).await?;
    drop(tx2);
    let result = recv_frame(&mut rx2, 1 << 20).await;
    match result {
        Err(ProtocolError::ArchiveValidationFailed(_)) => Ok(()),
        other => panic!("expected ArchiveValidationFailed, got {:?}", other),
    }
}
```

---

## T14-01: legitimate AssignPartition round-trip via `send_frame_v2` + `recv_frame`

**Target file:** same.
**Feature gate:** `#[cfg(all(test, feature = "zero-copy"))]`.
**R-mapping:** T14a, R22, R24.

```rust
#[tokio::test]
async fn t14_archive_flag_legitimate_round_trip() -> Result<(), Box<dyn std::error::Error>> {
    let (mut tx, mut rx) = tokio::io::duplex(64 * 1024);
    let original = Message::AssignPartition {
        round: 7,
        partition: sample_partition_realistic_10_agents_3_ranges(),
    };
    send_frame_v2(&mut tx, &original, usize::MAX, true).await?;
    drop(tx);
    let (received, _) = recv_frame(&mut rx, 1 << 20).await?;
    assert_eq!(original, received, "T14a: end-to-end round-trip identity");
    Ok(())
}
```

---

## T14-02: forged FLAG_ARCHIVED on Shutdown payload is rejected (R22 enforcement)

**Target file:** same.
**Feature gate:** `#[cfg(all(test, feature = "zero-copy"))]`.
**R-mapping:** T14b, R22 (forge non-hot-path archive — receiver MUST reject).

```rust
#[tokio::test]
async fn t14_archive_flag_on_control_message_rejected() {
    let (mut tx, mut rx) = tokio::io::duplex(4096);
    // Forge the bytes since send_frame_v2 would refuse to set
    // FLAG_ARCHIVED for Shutdown per R22.
    let archived_bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&42u32)
        .expect("test fixture serializes");
    let payload = archived_bytes.as_slice().to_vec();
    let header = FrameHeader {
        length: payload.len() as u32,
        checksum: crc32fast::hash(&payload),
        flags: FLAG_ARCHIVED,
    };
    use tokio::io::AsyncWriteExt;
    tx.write_all(&header.to_bytes()).await.expect("write header");
    tx.write_all(&payload).await.expect("write payload");
    drop(tx);
    let result = recv_frame(&mut rx, 1 << 20).await;
    match result {
        Err(ProtocolError::ArchiveValidationFailed(_)) => {} // ok per DC-2
        other => panic!("expected ArchiveValidationFailed, got {:?}", other),
    }
}
```

---

## T14-03: integration end of TEST-SPEC-0356 hot-path-only enforcement

**Target file:** same.
**Feature gate:** `#[cfg(all(test, feature = "zero-copy"))]`.
**R-mapping:** T14, R22.

```rust
/// T14 (integration end of TEST-SPEC-0356 UT-03/UT-04): with
/// use_zero_copy = true, control messages NEVER carry FLAG_ARCHIVED
/// on the wire. Sweep all 5 control variants.
#[tokio::test]
async fn t14_hot_path_only_enforcement_sweeps_control_variants() {
    let control_msgs: Vec<Message> = vec![
        Message::Shutdown,
        Message::Register(RegisterPayload {
            protocol_version: 2,
            worker_id: WorkerId(0),
            capabilities: default_caps(),
        }),
        Message::RegisterAck(RegisterAckPayload { worker_id: WorkerId(0) }),
        Message::RegisterNack(RegisterNackPayload { reason: "test".into() }),
        // Message::Error(_) — include if the variant exists in the codebase
    ];
    for msg in control_msgs {
        let (mut tx, mut rx) = tokio::io::duplex(4096);
        send_frame_v2(&mut tx, &msg, usize::MAX, true).await
            .expect("send_frame_v2");
        drop(tx);
        use tokio::io::AsyncReadExt;
        let mut header_buf = [0u8; FRAME_HEADER_SIZE];
        rx.read_exact(&mut header_buf).await.expect("read header");
        let header = FrameHeader::from_bytes(&header_buf).expect("parse");
        assert_eq!(header.flags & FLAG_ARCHIVED, 0,
            "R22: control variant {:?} MUST NOT carry FLAG_ARCHIVED", msg);
    }
}
```

---

## T13-03: integration end of TEST-SPEC-0355 alignment

**Target file:** same.
**Feature gate:** `#[cfg(all(test, feature = "zero-copy"))]`.
**R-mapping:** R25, T13 (integration end).

```rust
/// T13 integration end (per TASK-0359 acceptance criteria): assert
/// that the read buffer used by recv_frame on FLAG_ARCHIVED frames is
/// 16-byte aligned at the call site. We do this indirectly by asserting
/// that the round-trip succeeds for several payload sizes — if the
/// alignment broke, rkyv::access would fail with an alignment error
/// surfaced as ArchiveValidationFailed.
#[tokio::test]
async fn t13_alignment_integration_end_to_end() -> Result<(), Box<dyn std::error::Error>> {
    let partitions = vec![
        sample_partition_2_agents(),
        sample_partition_realistic_10_agents_3_ranges(),
        sample_partition_dup_chain(8),
    ];
    for p in partitions {
        let original = Message::AssignPartition { round: 0, partition: p.clone() };
        let (mut tx, mut rx) = tokio::io::duplex(64 * 1024);
        send_frame_v2(&mut tx, &original, usize::MAX, true).await?;
        drop(tx);
        let (received, _) = recv_frame(&mut rx, 1 << 20).await?;
        assert_eq!(received, original,
            "T13: alignment-correctness end-to-end for partition shape");
    }
    Ok(())
}
```

---

## Test count

| Test | Default build | `--features zero-copy` build |
|------|---------------|------------------------------|
| T11-01, T11-02 | ⏭ skipped | ✅ runs (2 tests) |
| T12-01 | ⏭ skipped | ✅ runs (1 test) |
| T13-01, T13-02, T13-03 | ⏭ skipped | ✅ runs (3 tests) |
| T14-01, T14-02, T14-03 | ⏭ skipped | ✅ runs (3 tests) |
| **Total new tests** | **+0** | **+9** |

Note: TASK-0359's acceptance criterion says "+4 tests (one per
T11..T14)". This TEST-SPEC delivers 9 tests because T11/T13/T14 each
yield 2-3 sub-cases per the orchestrator brief's "T11-T14 each yields
4-6 tests". The conservative target was +4, the orchestrator target
was +18 across the bundle; this lands inside both bands.

---

## Adversarial probes (QA candidates for Stage 5)

| # | Scenario | Why dangerous | Stage |
|---|----------|---------------|-------|
| QA-0359-A | T11 over a partition with `f64::NAN` in `WorkerRoundStats` | NaN bit pattern preservation through rend::f64_le | QA |
| QA-0359-B | T12 with a payload that does NOT compress (random bytes) | LZ4 compress may produce LARGER output; verify decompress recovers | QA |
| QA-0359-C | T13 flipping a byte in the middle of a u32 field | Verify rkyv DOES NOT detect (documented caveat — round-trip correctness only) | QA |
| QA-0359-D | T14b with a forged FLAG_ARCHIVED on `Message::Error` (control variant) | R22 must reject; sweep over every control variant | QA |
| QA-0359-E | T11 over an EMPTY partition (`Partition::new()`) | Edge case for the rkyv archive of an empty arena | QA |
| QA-0359-F | T11 over a partition with `free_port_index` containing 1000 entries | Stress the HashMap rkyv archive | QA |
| QA-0359-G | T14 with `use_zero_copy = true` AND `compression_threshold = 0` AND large partition | Both flags + large payload — exercise the full path | QA |
| QA-0359-H | T14b with FLAG_ARCHIVED + FLAG_RESERVED-bit-also-set (e.g., 0b1110) | UnknownFlags must reject FIRST; the err is `UnknownFlags`, NOT `ArchiveValidationFailed` | QA |
| QA-0359-I | Sender forges FLAG_ARCHIVED on a bincode-encoded Shutdown payload (adversarial misuse) | Receiver must reject; R22 enforcement on the recv side catches what the sender contract is supposed to prevent | QA |
| QA-0359-J | T13 corruption in the position table (rkyv stores root pointer at tail, but archive headers may have other metadata) | Verify validation rejects corruption in any structural position | QA |

---

## Hard-to-write-deterministically tests (FLAGGED)

- None of T11-T14 are non-deterministic. T13's "single byte flip" has
  a documented caveat (mid-data flips may not be detected) — the test
  targets the root pointer (last byte) where corruption is reliably
  detected.

---

## Acceptance gate

1. Default build: `cargo test --workspace` count: 904 → **904** (+0;
   all T11-T14 are feature-gated).
2. Feature build: `cargo test --workspace --features zero-copy` count:
   930 → **939** (+9: T11×2 + T12×1 + T13×3 + T14×3).
3. `cargo clippy --workspace --all-targets -- -D warnings` clean both ways.
4. `cargo fmt --check` clean.
5. Round-trip identity `deserialize(access(to_bytes(p))) == p` holds
   for every test partition (R27 acceptance signal).
6. **D-002 acceptance signal:** T14a (legitimate AssignPartition with
   `--features zero-copy` produces a frame with FLAG_ARCHIVED and the
   receiver decodes via `rkyv::access`) PASSES.
7. NO `unwrap()` in production-path test code (test fixtures may use
   `.expect("test fixture")` per CLAUDE.md tradition).

---

## Bundle-wide cumulative test count

| TEST-SPEC | Default delta | Feature delta | Cumulative default | Cumulative feature |
|-----------|---------------|---------------|--------------------|--------------------|
| Baseline  | —             | —             | 887                | 887                |
| TEST-SPEC-0352 | +2 | +3 | 889 | 890 |
| TEST-SPEC-0353 | +1 | +9 | 890 | 899 |
| TEST-SPEC-0354 | +5 | +5 | 895 | 904 |
| TEST-SPEC-0355 | +1 | +5 | 896 | 909 |
| TEST-SPEC-0356 | +1 | +6 | 897 | 915 |
| TEST-SPEC-0357 | +2 | +8 | 899 | 923 |
| TEST-SPEC-0358 | +5 | +7 | 904 | 930 |
| TEST-SPEC-0359 | +0 | +9 | **904** | **939** |
| **Bundle total** | **+17** | **+52** | — | — |

**Default-features target:** 887 → **904** (+17; orchestrator hint
was +4; we exceed by 13 because the variant + CLI tests are
unconditional per TASK-0354/0358 design rationale, which is by design
and consistent with hard-floor preservation).

**Feature-build target:** 887 → **939** (+52; orchestrator hint was
+24; we exceed because each per-task TEST-SPEC enumerates 5-9 sub-cases
covering R-numbers, edge cases, and DC-mandates explicitly).

Both numbers exceed the orchestrator hints, but the conservative
floors (887 default, 905+ feature per TASK-0359 hint) are met with
margin. The orchestrator may decide to trim during Stage 3 DEV;
preferred trim path is to fold UT-0354-04 (DC-2 tuple-pin) and
UT-0357-09 (DC-3 comment-pin) into helper assertions invoked from
existing tests rather than standalone `#[test]`s, but the verdict
should rest with the developer + reviewer.

---

## Out of scope

- D-002 row update in `docs/DEFERRED-WORK.md` — TASK-UPDATER's job
  after Stage 6 REFACTOR sign-off.
- Performance benchmarks for the rkyv path vs bincode v2 — separate
  bench suite (ROADMAP item 2.24 §benchmarks).
- Rkyv 0.7 → 0.8 API migration probes — TASK-0352's `version = "0.8"`
  pin is the authority.
