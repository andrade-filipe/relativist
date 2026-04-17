# TEST-SPEC-0356: `send_frame` archive path (hot-path only, optional LZ4)

**Task:** TASK-0356
**Spec:** SPEC-18 §3.5 (R22, R23)
**Generated:** 2026-04-16
**Baseline before this task:** 896 lib (default) / 909 lib (`--features zero-copy`, post-TASK-0355).

---

## Scope note

R22 says only `AssignPartition` and `PartitionResult` MAY take the rkyv
path; control messages (`Shutdown`, `Error`, `Register`, `RegisterAck`,
`RegisterNack`) MUST always use bincode v2 even when
`use_zero_copy = true`. R23 says LZ4 compression MAY be applied on top
of the archived bytes when their size is `>= compression_threshold`.

DC-4 (spec-critic 2026-04-16) mandates that send-side `rkyv::to_bytes`
failures map to `ProtocolError::ArchiveValidationFailed(format!(
"serialize: {}", e))` — the `"serialize: "` prefix is load-bearing
(source-grep verifiable, log scrapers split on it).

All tests are gated under `#[cfg(all(test, feature = "zero-copy"))]`
EXCEPT UT-06 which runs in the default build to prove the
`use_zero_copy` parameter is a no-op when the feature is OFF.

---

## UT-0356-01: hot-path AssignPartition sets FLAG_ARCHIVED

**Target file:** `relativist-core/src/protocol/frame.rs` (test module).
**Feature gate:** `#[cfg(all(test, feature = "zero-copy"))]`.
**R-mapping:** R22, R23 (flag set on hot-path send).

```rust
#[cfg(feature = "zero-copy")]
#[tokio::test]
async fn send_frame_v2_sets_archive_flag_on_assign_partition() {
    let (mut tx, mut rx) = tokio::io::duplex(64 * 1024);
    let msg = Message::AssignPartition {
        round: 7,
        partition: sample_partition_with_borders(),
    };
    send_frame_v2(&mut tx, &msg, usize::MAX, true).await
        .expect("send_frame_v2");
    drop(tx);
    // Read the 9-byte header and inspect the flags byte.
    use tokio::io::AsyncReadExt;
    let mut header_buf = [0u8; FRAME_HEADER_SIZE];
    rx.read_exact(&mut header_buf).await.expect("read header");
    let header = FrameHeader::from_bytes(&header_buf).expect("parse header");
    assert_eq!(header.flags & FLAG_ARCHIVED, FLAG_ARCHIVED,
        "FLAG_ARCHIVED must be set on hot-path send (R22+R23); flags={:08b}",
        header.flags);
    assert_eq!(header.flags & FLAG_COMPRESSED, 0,
        "FLAG_COMPRESSED must NOT be set when threshold = usize::MAX");
}
```

**Asserts:** flag bit 1 (FLAG_ARCHIVED) is set; bit 0 (FLAG_COMPRESSED) is not.

---

## UT-0356-02: hot-path PartitionResult sets FLAG_ARCHIVED

**Target file:** `relativist-core/src/protocol/frame.rs` (test module).
**Feature gate:** `#[cfg(all(test, feature = "zero-copy"))]`.
**R-mapping:** R22, R23.

```rust
#[cfg(feature = "zero-copy")]
#[tokio::test]
async fn send_frame_v2_sets_archive_flag_on_partition_result() {
    let (mut tx, mut rx) = tokio::io::duplex(64 * 1024);
    let msg = Message::PartitionResult {
        round: 7,
        partition: sample_partition_with_borders(),
        stats: WorkerRoundStats {
            worker_id: WorkerId(0),
            round: 7,
            local_redexes: 0,
            interactions_by_rule: [0; 6],
            reduce_duration_secs: 0.0,
            has_border_activity: false,
        },
    };
    send_frame_v2(&mut tx, &msg, usize::MAX, true).await
        .expect("send_frame_v2");
    drop(tx);
    use tokio::io::AsyncReadExt;
    let mut header_buf = [0u8; FRAME_HEADER_SIZE];
    rx.read_exact(&mut header_buf).await.expect("read header");
    let header = FrameHeader::from_bytes(&header_buf).expect("parse header");
    assert_eq!(header.flags & FLAG_ARCHIVED, FLAG_ARCHIVED,
        "FLAG_ARCHIVED must be set on PartitionResult");
}
```

---

## UT-0356-03: control message Shutdown does NOT set FLAG_ARCHIVED

**Target file:** `relativist-core/src/protocol/frame.rs` (test module).
**Feature gate:** `#[cfg(all(test, feature = "zero-copy"))]`.
**R-mapping:** R22 (silent fallback to bincode for control variants).

```rust
#[cfg(feature = "zero-copy")]
#[tokio::test]
async fn send_frame_v2_does_not_archive_shutdown_per_r22() {
    let (mut tx, mut rx) = tokio::io::duplex(4096);
    send_frame_v2(&mut tx, &Message::Shutdown, usize::MAX, true).await
        .expect("send_frame_v2");
    drop(tx);
    use tokio::io::AsyncReadExt;
    let mut header_buf = [0u8; FRAME_HEADER_SIZE];
    rx.read_exact(&mut header_buf).await.expect("read header");
    let header = FrameHeader::from_bytes(&header_buf).expect("parse header");
    assert_eq!(header.flags & FLAG_ARCHIVED, 0,
        "R22: control messages MUST NOT take rkyv path; flags={:08b}",
        header.flags);
}
```

---

## UT-0356-04: control message Register does NOT set FLAG_ARCHIVED

**Target file:** `relativist-core/src/protocol/frame.rs` (test module).
**Feature gate:** `#[cfg(all(test, feature = "zero-copy"))]`.
**R-mapping:** R22.

```rust
#[cfg(feature = "zero-copy")]
#[tokio::test]
async fn send_frame_v2_does_not_archive_register_per_r22() {
    let (mut tx, mut rx) = tokio::io::duplex(4096);
    let msg = Message::Register(RegisterPayload {
        protocol_version: 2,
        worker_id: WorkerId(0),
        capabilities: default_caps(),
    });
    send_frame_v2(&mut tx, &msg, usize::MAX, true).await
        .expect("send_frame_v2");
    drop(tx);
    use tokio::io::AsyncReadExt;
    let mut header_buf = [0u8; FRAME_HEADER_SIZE];
    rx.read_exact(&mut header_buf).await.expect("read header");
    let header = FrameHeader::from_bytes(&header_buf).expect("parse header");
    assert_eq!(header.flags & FLAG_ARCHIVED, 0,
        "R22: Register MUST NOT take rkyv path");
}
```

---

## UT-0356-05: hot-path + threshold = 0 sets BOTH FLAG_ARCHIVED + FLAG_COMPRESSED

**Target file:** `relativist-core/src/protocol/frame.rs` (test module).
**Feature gate:** `#[cfg(all(test, feature = "zero-copy"))]`.
**R-mapping:** R23 (LZ4 wrap on top of rkyv archive).

```rust
#[cfg(feature = "zero-copy")]
#[tokio::test]
async fn send_frame_v2_combines_archive_and_compress_flags() {
    let (mut tx, mut rx) = tokio::io::duplex(64 * 1024);
    let msg = Message::AssignPartition {
        round: 1,
        partition: sample_partition_with_borders(),
    };
    // threshold = 0 forces compression on every payload size > 0.
    send_frame_v2(&mut tx, &msg, 0, true).await.expect("send_frame_v2");
    drop(tx);
    use tokio::io::AsyncReadExt;
    let mut header_buf = [0u8; FRAME_HEADER_SIZE];
    rx.read_exact(&mut header_buf).await.expect("read header");
    let header = FrameHeader::from_bytes(&header_buf).expect("parse header");
    assert_eq!(header.flags & (FLAG_ARCHIVED | FLAG_COMPRESSED),
               FLAG_ARCHIVED | FLAG_COMPRESSED,
        "R23: BOTH flags must be set; got flags={:08b}", header.flags);
}
```

---

## UT-0356-06: feature OFF — `use_zero_copy = true` is a no-op (no FLAG_ARCHIVED)

**Target file:** `relativist-core/src/protocol/frame.rs` (test module).
**Feature gate:** `#[cfg(all(test, not(feature = "zero-copy")))]`.
**R-mapping:** R22 (cfg-disabled fallback).

```rust
#[cfg(not(feature = "zero-copy"))]
#[tokio::test]
async fn send_frame_v2_falls_back_to_bincode_when_feature_off() {
    let (mut tx, mut rx) = tokio::io::duplex(64 * 1024);
    let msg = Message::AssignPartition {
        round: 1,
        partition: sample_partition_with_borders(),
    };
    // use_zero_copy = true, but feature is OFF -> should silently use bincode.
    send_frame_v2(&mut tx, &msg, usize::MAX, true).await.expect("send_frame_v2");
    drop(tx);
    use tokio::io::AsyncReadExt;
    let mut header_buf = [0u8; FRAME_HEADER_SIZE];
    rx.read_exact(&mut header_buf).await.expect("read header");
    let header = FrameHeader::from_bytes(&header_buf).expect("parse header");
    assert_eq!(header.flags & FLAG_ARCHIVED, 0,
        "FLAG_ARCHIVED must NOT appear in feature-off build");
}
```

---

## UT-0356-07: rkyv send-side error maps via DC-4 mandated `"serialize: "` prefix

**Target file:** `relativist-core/src/protocol/frame.rs` (test module).
**Feature gate:** `#[cfg(all(test, feature = "zero-copy"))]`.
**R-mapping:** R26, DC-4 (mandatory `"serialize: "` prefix — source-grep verifiable).

**HARD-TO-WRITE-DETERMINISTICALLY:** `rkyv::to_bytes` failure is
pathological (alignment-padding OOM) and not reproducible without an
allocator hook. Two test strategies:

**Strategy A (preferred — source-grep regression guard):**

```rust
#[cfg(feature = "zero-copy")]
#[test]
fn send_side_error_mapping_uses_dc4_serialize_prefix() {
    // DC-4 mandates the literal `"serialize: "` prefix on send-side
    // ProtocolError::ArchiveValidationFailed construction. Source-grep
    // is the canonical pin: the literal MUST appear in frame.rs near
    // `rkyv::to_bytes`.
    let src = include_str!("frame.rs");
    let occurrences = src.matches(r#""serialize: ""#).count();
    assert!(
        occurrences >= 1,
        "DC-4 mandates the literal \"serialize: \" prefix on the \
         send-side rkyv error mapping. Source grep found 0 occurrences \
         in frame.rs — see docs/spec-reviews/\
         SPEC-18-section-3.5-design-choices-2026-04-16.md DC-4."
    );
    // Stronger assertion: the prefix must appear in the same neighbourhood
    // as `rkyv::to_bytes` (within ~200 chars).
    let to_bytes_idx = src.find("rkyv::to_bytes")
        .expect("send path must call rkyv::to_bytes");
    let neighbourhood = &src[to_bytes_idx..src.len().min(to_bytes_idx + 800)];
    assert!(
        neighbourhood.contains(r#""serialize: ""#),
        "DC-4 mandates `\"serialize: \"` near `rkyv::to_bytes`; not found"
    );
}
```

**Strategy B (best-effort dynamic — flag for QA):** stub a wrapper
function that calls `rkyv::to_bytes::<_, rkyv::rancor::Error>` on a
type that triggers a serialization error (e.g., a deeply nested
recursive structure that exceeds rkyv's stack limit). This is
**flagged as hard-to-write-deterministically** and is enumerated as a
QA candidate, NOT implemented as `#[test]`. See QA-0356-A below.

---

## Test count

| Test | Default build | `--features zero-copy` build |
|------|---------------|------------------------------|
| UT-0356-01..05 | ⏭ skipped (cfg-gated) | ✅ runs (5 tests) |
| UT-0356-06 | ✅ runs | ⏭ skipped (cfg-gated) |
| UT-0356-07 | ⏭ skipped (cfg-gated) | ✅ runs (1 test) |
| **Total new tests** | **+1** | **+6** |

---

## Adversarial probes (QA candidates for Stage 5)

| # | Scenario | Why dangerous | Stage |
|---|----------|---------------|-------|
| QA-0356-A | Trigger `rkyv::to_bytes` failure dynamically (deep nesting / OOM) | Verifies the `"serialize: "` prefix at runtime, not just via source-grep | QA |
| QA-0356-B | Send `Message::Error(ErrorPayload)` with `use_zero_copy = true` | Error is a control variant per R22; FLAG_ARCHIVED MUST NOT set | QA |
| QA-0356-C | Send `Message::PartitionResult` with `WorkerRoundStats { reduce_duration_secs: f64::NAN }` | Verify NaN survives the rkyv round trip; verify CRC computed on archived bytes still matches | QA |
| QA-0356-D | `use_zero_copy = true`, threshold > archived size | FLAG_COMPRESSED must NOT set even with `use_zero_copy = true`; FLAG_ARCHIVED only | QA |
| QA-0356-E | `bytes_written` return value equals `FRAME_HEADER_SIZE + payload.len()` | Regression with existing `test_bytes_count` | QA |
| QA-0356-F | Adversarial: send a frame with archived bytes whose internal CRC32C disagrees with the header field (forged via direct write) | Recv side must reject with `ChecksumMismatch`; this exercises the R12 ordering invariant on the rkyv path | QA |

---

## Hard-to-write-deterministically tests (FLAGGED)

- **UT-0356-07 dynamic side (Strategy B)** — `rkyv::to_bytes` failure
  is **pathological and not reproducible without allocator stubs**. The
  source-grep test (Strategy A) is the canonical pin; QA-0356-A is the
  best-effort dynamic counterpart.

---

## Acceptance gate

1. Default build: `cargo test --workspace` count: 896 → **897** (+1: UT-06).
2. Feature build: `cargo test --workspace --features zero-copy` count:
   909 → **915** (+6: UT-01..05, UT-07).
3. `cargo clippy --workspace --all-targets -- -D warnings` clean both ways.
4. `cargo fmt --check` clean.
5. Existing `send_frame` tests (T6-T8 round-trips, R5 CRC-on-uncompressed
   etc.) all stay green in BOTH builds.
6. NO `unwrap()`, NO `unsafe`, NO `println!` in new code.
7. **DC-4 source-grep mandate (UT-07):** the literal `"serialize: "`
   appears in `frame.rs` near `rkyv::to_bytes`.

---

## Out of scope

- `recv_frame` archive path → TEST-SPEC-0357.
- CLI `--use-zero-copy` flag → TEST-SPEC-0358.
- T11-T14 end-to-end suite → TEST-SPEC-0359.
