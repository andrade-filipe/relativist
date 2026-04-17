# TEST-SPEC-0357: `recv_frame` archive path (decompress → CRC → `rkyv::access`)

**Task:** TASK-0357
**Spec:** SPEC-18 §3.5 (R12, R22, R24, R26)
**Generated:** 2026-04-16
**Baseline before this task:** 897 lib (default) / 915 lib (`--features zero-copy`, post-TASK-0356).

---

## Scope note

`recv_frame` becomes the load-bearing receive-side dispatch. The R12
ordering invariant (decompress → CRC → rkyv access) MUST hold, and DC-3
(spec-critic 2026-04-16) mandates Assign-first try-then-try
discrimination plus a literal `// SPEC-18 R22 discrimination` comment
in source. UT-09 is the source-grep regression guard for the comment
mandate, mirroring UT-0351-09's self-grep mitigation pattern from the
SPEC-19 §3.1 bundle.

All tuple-form match arms `Err(ProtocolError::ArchiveValidationFailed(_))`
follow DC-2 (spec-critic 2026-04-16); any future flip back to struct
form `{ .. }` would break compilation, alerting the developer.

---

## UT-0357-01: round-trip AssignPartition through full pipeline (uncompressed)

**Target file:** `relativist-core/src/protocol/frame.rs` (test module).
**Feature gate:** `#[cfg(all(test, feature = "zero-copy"))]`.
**R-mapping:** R22, R24 (round-trip via the full send→recv pipeline).

```rust
#[cfg(feature = "zero-copy")]
#[tokio::test]
async fn recv_frame_round_trips_assign_partition_archive() {
    let (mut tx, mut rx) = tokio::io::duplex(64 * 1024);
    let original = Message::AssignPartition {
        round: 7,
        partition: sample_partition_with_borders(),
    };
    send_frame_v2(&mut tx, &original, usize::MAX, true).await
        .expect("send_frame_v2");
    drop(tx);
    let (decoded, n) = recv_frame(&mut rx, 1 << 20).await
        .expect("recv_frame");
    assert!(n > FRAME_HEADER_SIZE);
    assert_eq!(decoded, original, "AssignPartition must round-trip");
}
```

---

## UT-0357-02: round-trip PartitionResult through full pipeline (uncompressed)

**Target file:** `relativist-core/src/protocol/frame.rs` (test module).
**Feature gate:** `#[cfg(all(test, feature = "zero-copy"))]`.
**R-mapping:** R22, R24.

```rust
#[cfg(feature = "zero-copy")]
#[tokio::test]
async fn recv_frame_round_trips_partition_result_archive() {
    let (mut tx, mut rx) = tokio::io::duplex(64 * 1024);
    let original = Message::PartitionResult {
        round: 7,
        partition: sample_partition_with_borders(),
        stats: WorkerRoundStats {
            worker_id: WorkerId(2),
            round: 7,
            local_redexes: 5,
            interactions_by_rule: [3, 2, 0, 0, 0, 0],
            reduce_duration_secs: 0.001,
            has_border_activity: true,
        },
    };
    send_frame_v2(&mut tx, &original, usize::MAX, true).await
        .expect("send_frame_v2");
    drop(tx);
    let (decoded, _) = recv_frame(&mut rx, 1 << 20).await
        .expect("recv_frame");
    assert_eq!(decoded, original, "PartitionResult must round-trip");
}
```

---

## UT-0357-03: round-trip with FLAG_COMPRESSED + FLAG_ARCHIVED

**Target file:** `relativist-core/src/protocol/frame.rs` (test module).
**Feature gate:** `#[cfg(all(test, feature = "zero-copy"))]`.
**R-mapping:** R12 (decompress → CRC → access ordering), R22, R24.

```rust
#[cfg(feature = "zero-copy")]
#[tokio::test]
async fn recv_frame_round_trips_archive_with_lz4_compression() {
    let (mut tx, mut rx) = tokio::io::duplex(64 * 1024);
    let original = Message::AssignPartition {
        round: 1,
        partition: sample_partition_with_borders(),
    };
    // threshold = 0 forces both flags.
    send_frame_v2(&mut tx, &original, 0, true).await.expect("send");
    drop(tx);
    let (decoded, _) = recv_frame(&mut rx, 1 << 20).await.expect("recv");
    assert_eq!(decoded, original,
        "compressed-archived round-trip must yield bit-equal Message");
}
```

---

## UT-0357-04: R12 ordering proof — corrupt CRC AFTER LZ4 yields ChecksumMismatch (NOT ArchiveValidationFailed)

**Target file:** `relativist-core/src/protocol/frame.rs` (test module).
**Feature gate:** `#[cfg(all(test, feature = "zero-copy"))]`.
**R-mapping:** R12 (CRC over decompressed payload, BEFORE rkyv access).

```rust
#[cfg(feature = "zero-copy")]
#[tokio::test]
async fn r12_archive_compression_corrupted_crc_yields_checksum_mismatch() {
    // Send a legitimate compressed-archived frame, capture the bytes,
    // tamper the CRC field of the header, replay through recv_frame.
    let (mut tx, mut sniff) = tokio::io::duplex(64 * 1024);
    let msg = Message::AssignPartition {
        round: 1,
        partition: sample_partition_with_borders(),
    };
    send_frame_v2(&mut tx, &msg, 0, true).await.expect("send");
    drop(tx);
    use tokio::io::AsyncReadExt;
    let mut buf = Vec::new();
    sniff.read_to_end(&mut buf).await.expect("drain");
    // Tamper the CRC bytes (bytes 4..8 in the 9-byte header — verify
    // exact offset against FrameHeader::to_bytes layout).
    buf[4] ^= 0xFF;
    let (mut tx2, mut rx2) = tokio::io::duplex(64 * 1024);
    use tokio::io::AsyncWriteExt;
    tx2.write_all(&buf).await.expect("replay");
    drop(tx2);
    let result = recv_frame(&mut rx2, 1 << 20).await;
    match result {
        Err(ProtocolError::ChecksumMismatch { .. }) => {} // ok — R12
        other => panic!(
            "R12 ordering broken: expected ChecksumMismatch (CRC checked \
             BEFORE rkyv access on the decompressed payload), got {:?}",
            other
        ),
    }
}
```

**Asserts:** the receiver reports CRC mismatch on the decompressed
bytes, not a rkyv access error — proves CRC verification happens before
rkyv access.

---

## UT-0357-05: feature OFF — receive FLAG_ARCHIVED yields `ArchiveValidationFailed("zero-copy feature disabled")`

**Target file:** `relativist-core/src/protocol/frame.rs` (test module).
**Feature gate:** `#[cfg(all(test, not(feature = "zero-copy")))]`.
**R-mapping:** R26 (cfg-disabled rejection — Option B per DC-1).

```rust
#[cfg(not(feature = "zero-copy"))]
#[tokio::test]
async fn recv_frame_rejects_archive_flag_when_feature_off() {
    let (mut tx, mut rx) = tokio::io::duplex(4096);
    // Forge: 9-byte header with FLAG_ARCHIVED + arbitrary payload.
    let payload = b"any bytes — never read in feature-off build".to_vec();
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
        Err(ProtocolError::ArchiveValidationFailed(reason)) => {
            assert!(
                reason.contains("zero-copy feature disabled"),
                "DC-1 mandate: feature-off rejection must include the \
                 literal phrase 'zero-copy feature disabled'; got: {}",
                reason
            );
        }
        other => panic!(
            "expected ArchiveValidationFailed, got {:?}", other
        ),
    }
}
```

**Asserts:** `ArchiveValidationFailed` reason contains the literal
`"zero-copy feature disabled"` (per Option B in bundle index +
spec-critic DC-1).

---

## UT-0357-06: feature ON + FLAG_ARCHIVED + adversarial bincode-shaped payload yields ArchiveValidationFailed

**Target file:** `relativist-core/src/protocol/frame.rs` (test module).
**Feature gate:** `#[cfg(all(test, feature = "zero-copy"))]`.
**R-mapping:** R22 (non-hot-path rejection).

```rust
#[cfg(feature = "zero-copy")]
#[tokio::test]
async fn recv_frame_rejects_archive_flag_with_non_hot_path_payload() {
    // Synthesize an archive of a non-Partition type (bare u32) and
    // wrap it in a FLAG_ARCHIVED frame. This is the impossible-by-
    // sender-contract case (R22 enforces send-side); recv side MUST
    // also reject (R22 enforced on recv via try-then-try discrimination).
    let archived = rkyv::to_bytes::<rkyv::rancor::Error>(&42u32)
        .expect("u32 serializes via rkyv");
    let payload = archived.as_slice().to_vec();
    let header = FrameHeader {
        length: payload.len() as u32,
        checksum: crc32fast::hash(&payload),
        flags: FLAG_ARCHIVED,
    };
    let (mut tx, mut rx) = tokio::io::duplex(4096);
    use tokio::io::AsyncWriteExt;
    tx.write_all(&header.to_bytes()).await.expect("write header");
    tx.write_all(&payload).await.expect("write payload");
    drop(tx);
    let result = recv_frame(&mut rx, 1 << 20).await;
    match result {
        Err(ProtocolError::ArchiveValidationFailed(_)) => {} // ok — R22
        other => panic!(
            "R22: non-hot-path archive must surface as \
             ArchiveValidationFailed, got {:?}", other
        ),
    }
}
```

---

## UT-0357-07: feature ON + corrupt rkyv archive yields ArchiveValidationFailed

**Target file:** `relativist-core/src/protocol/frame.rs` (test module).
**Feature gate:** `#[cfg(all(test, feature = "zero-copy"))]`.
**R-mapping:** R26 (rkyv validation surfaces as `ArchiveValidationFailed`).

```rust
#[cfg(feature = "zero-copy")]
#[tokio::test]
async fn recv_frame_rejects_corrupt_rkyv_archive() {
    let original = Message::AssignPartition {
        round: 1,
        partition: sample_partition_with_borders(),
    };
    let (mut tx, mut sniff) = tokio::io::duplex(64 * 1024);
    send_frame_v2(&mut tx, &original, usize::MAX, true).await.expect("send");
    drop(tx);
    use tokio::io::AsyncReadExt;
    let mut buf = Vec::new();
    sniff.read_to_end(&mut buf).await.expect("drain");
    // Tamper a byte deep in the rkyv root pointer (last 4-8 bytes are
    // the archive's relative pointer; flipping them causes
    // out-of-bounds access detected by validating API).
    let last = buf.len() - 1;
    buf[last] ^= 0xFF;
    // Recompute the header CRC so the failure surfaces at rkyv access,
    // NOT at CRC check.
    let new_payload_crc = crc32fast::hash(&buf[FRAME_HEADER_SIZE..]);
    buf[4..8].copy_from_slice(&new_payload_crc.to_le_bytes());
    let (mut tx2, mut rx2) = tokio::io::duplex(64 * 1024);
    use tokio::io::AsyncWriteExt;
    tx2.write_all(&buf).await.expect("replay");
    drop(tx2);
    let result = recv_frame(&mut rx2, 1 << 20).await;
    match result {
        Err(ProtocolError::ArchiveValidationFailed(reason)) => {
            assert!(
                reason.contains("access") || reason.contains("non-hot-path"),
                "expected reason to mention access failure; got: {}",
                reason
            );
        }
        other => panic!(
            "R26: corrupt archive must surface as ArchiveValidationFailed; \
             got {:?}", other
        ),
    }
}
```

---

## UT-0357-08: DC-3 Assign-first ordering — PartitionResult archive falls through Assign attempt and is accepted

**Target file:** `relativist-core/src/protocol/frame.rs` (test module).
**Feature gate:** `#[cfg(all(test, feature = "zero-copy"))]`.
**R-mapping:** R22, DC-3 (Assign-first try-then-try).

```rust
#[cfg(feature = "zero-copy")]
#[tokio::test]
async fn dc3_partition_result_archive_falls_through_assign_attempt() {
    // A PartitionResult archive, when fed to
    // rkyv::access::<ArchivedAssignPayload>, MUST fail validation
    // (different layout). recv_frame's try-then-try then attempts
    // ArchivedPartitionResultPayload, which succeeds. We verify by
    // sending PartitionResult and asserting recv_frame returns the
    // matching variant.
    let (mut tx, mut rx) = tokio::io::duplex(64 * 1024);
    let original = Message::PartitionResult {
        round: 99,
        partition: sample_partition_with_borders(),
        stats: WorkerRoundStats {
            worker_id: WorkerId(1),
            round: 99,
            local_redexes: 1,
            interactions_by_rule: [0, 1, 0, 0, 0, 0],
            reduce_duration_secs: 0.5,
            has_border_activity: false,
        },
    };
    send_frame_v2(&mut tx, &original, usize::MAX, true).await.expect("send");
    drop(tx);
    let (decoded, _) = recv_frame(&mut rx, 1 << 20).await.expect("recv");
    match decoded {
        Message::PartitionResult { .. } => {} // ok — DC-3 fallback worked
        other => panic!(
            "DC-3: PartitionResult must be accepted via the second try; \
             got {:?}", other
        ),
    }
    assert_eq!(decoded, original, "byte-equal round-trip");
}
```

---

## UT-0357-09: DC-3 mandated source comment grep (regression guard)

**Target file:** `relativist-core/src/protocol/frame.rs` (test module).
**Feature gate:** none — runs in both builds (the comment must exist
regardless of feature; if the rkyv branch is gated out, the comment
lives inside the gated block, but the source-grep finds it either way
via `include_str!`).
**R-mapping:** DC-3 (spec-critic mandated comment).

```rust
/// DC-3 mandate (spec-critic 2026-04-16) — the discrimination call site
/// MUST carry a literal `// SPEC-18 R22 discrimination` comment in
/// source so a future reader knows the strategy was reviewed and
/// approved (cite docs/spec-reviews/SPEC-18-section-3.5-design-choices-2026-04-16.md
/// DC-3). This test self-greps the source. Mirror of UT-0351-09's
/// self-grep mitigation from the SPEC-19 §3.1 bundle.
#[test]
fn dc3_recv_frame_carries_spec_18_r22_discrimination_comment() {
    let src = include_str!("frame.rs");
    assert!(
        src.contains("SPEC-18 R22 discrimination"),
        "DC-3 mandate violated: `// SPEC-18 R22 discrimination` comment \
         missing in frame.rs. See docs/spec-reviews/\
         SPEC-18-section-3.5-design-choices-2026-04-16.md DC-3."
    );
    // Stronger probe: the comment must appear in the same neighbourhood
    // as `rkyv::access` (the discrimination call site).
    if cfg!(feature = "zero-copy") {
        let access_idx = src.find("rkyv::access");
        if let Some(idx) = access_idx {
            let neighbourhood = &src[idx.saturating_sub(400)..(idx + 1200).min(src.len())];
            assert!(
                neighbourhood.contains("SPEC-18 R22 discrimination"),
                "DC-3 mandate violated: comment must be near `rkyv::access`; \
                 not found in neighbourhood"
            );
        }
    }
}
```

**Self-grep mitigation:** the comment must NOT be inside this test's
own assertion string (else the grep would match the test, not the
production call site). Use a sentinel string like
`SPEC-18 R22 discrimination` in the production path and reference it
via `contains` here. This is the exact pattern used by UT-0351-09.

---

## Test count

| Test | Default build | `--features zero-copy` build |
|------|---------------|------------------------------|
| UT-0357-01..04 | ⏭ skipped (cfg-gated) | ✅ runs (4 tests) |
| UT-0357-05 | ✅ runs | ⏭ skipped (cfg-gated) |
| UT-0357-06..08 | ⏭ skipped (cfg-gated) | ✅ runs (3 tests) |
| UT-0357-09 | ✅ runs | ✅ runs |
| **Total new tests** | **+2** | **+8** |

---

## Adversarial probes (QA candidates for Stage 5)

| # | Scenario | Why dangerous | Stage |
|---|----------|---------------|-------|
| QA-0357-A | Frame with FLAG_ARCHIVED + FLAG_COMPRESSED but a 1-byte payload (impossible to be valid LZ4) | Decompress fails → must return `DecompressionFailed`, NOT `ArchiveValidationFailed` (R12 ordering) | QA |
| QA-0357-B | Frame with FLAG_ARCHIVED + length = 0 | rkyv access on empty buffer → `ArchiveValidationFailed`; verify no panic | QA |
| QA-0357-C | Frame with FLAG_ARCHIVED + max_payload_size + 1 length | Length check rejects BEFORE allocation | QA |
| QA-0357-D | Frame with both FLAG_ARCHIVED + a reserved bit set (bits 2..7) | Unknown-flag check rejects FIRST (R19 retained); err is `UnknownFlags`, NOT `ArchiveValidationFailed` | QA |
| QA-0357-E | Replay of UT-04 with bytes 8 (flags) tampered: flip FLAG_ARCHIVED → 0 | CRC re-check passes → falls into bincode path → bincode decode fails on rkyv bytes → `Deserialize` error | QA |
| QA-0357-F | Send legitimate AssignPartition archive, recv with `max_payload_size = header.length - 1` | Length check rejects with `MessageTooLarge` | QA |
| QA-0357-G | Two consecutive archives in the same stream | Verify recv_frame yields the first, leaves the second buffered for the next call | QA |

---

## Hard-to-write-deterministically tests (FLAGGED)

- None of UT-01..09 are non-deterministic. UT-09 is brittle to source
  refactors but the sentinel string is stable (it cites the spec-review
  filename in production, which is itself stable). Mitigation: same as
  UT-0351-09 — flagged for future migration to a parsed-AST check if
  `syn` becomes a dev-dep.

---

## Acceptance gate

1. Default build: `cargo test --workspace` count: 897 → **899** (+2: UT-05 + UT-09).
2. Feature build: `cargo test --workspace --features zero-copy` count:
   915 → **923** (+8: UT-01..04, UT-06..09).
3. `cargo clippy --workspace --all-targets -- -D warnings` clean both ways.
4. `cargo fmt --check` clean.
5. R12 ordering preserved (UT-04 PASS in feature build).
6. **DC-3 source-grep mandate (UT-09):** the literal `SPEC-18 R22 discrimination`
   appears in `frame.rs` near `rkyv::access`.
7. NO `unwrap()`, NO `unsafe`, NO `println!` in new code.
8. **NO `rkyv::access_unchecked` in production code** (R24 step 3 — the
   developer should source-grep `src/` to confirm).

---

## Out of scope

- CLI `--use-zero-copy` flag → TEST-SPEC-0358.
- T11-T14 end-to-end suite → TEST-SPEC-0359.
- Existing QA Probe 3 update — captured in TEST-SPEC-0357 by UT-05/06.
