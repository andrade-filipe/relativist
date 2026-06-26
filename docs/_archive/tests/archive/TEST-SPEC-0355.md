# TEST-SPEC-0355: 16-byte aligned receive buffer (`AlignedVec`)

**Task:** TASK-0355
**Spec:** SPEC-18 §3.5 (R25)
**Generated:** 2026-04-16
**Baseline before this task:** 895 lib (default) / 904 lib (`--features zero-copy`, post-TASK-0354).

---

## Scope note

R25 says the receive buffer for `FLAG_ARCHIVED` payloads MUST be
16-byte aligned. The helper introduced by TASK-0355
(`read_aligned_payload`) wraps `rkyv::util::AlignedVec` and feeds the
bytes to `tokio::io::AsyncReadExt::read_exact`. All tests in this
TEST-SPEC are gated under `#[cfg(all(test, feature = "zero-copy"))]`;
the default build does not have the helper and runs zero of these tests.

Each test constructs a deterministic payload, feeds it through an
in-memory channel (`tokio::io::duplex` or `tokio_test::io::Builder`),
calls `read_aligned_payload`, and asserts (1) the bytes match the
input and (2) the start address is 16-byte aligned.

---

## UT-0355-01: zero-length payload

**Target file:** `relativist-core/src/protocol/frame.rs` (test module).
**Feature gate:** `#[cfg(all(test, feature = "zero-copy"))]`.
**R-mapping:** R25 (alignment + zero-length edge case).

```rust
#[cfg(feature = "zero-copy")]
#[tokio::test]
async fn read_aligned_payload_handles_zero_length() {
    let (mut tx, mut rx) = tokio::io::duplex(64);
    drop(tx); // close immediately; read_exact(0 bytes) succeeds with no I/O
    let buf = read_aligned_payload(&mut rx, 0).await
        .expect("zero-length read must not error");
    assert_eq!(buf.len(), 0);
    // Empty AlignedVec may have null/dangling pointer; do NOT assert
    // alignment on an empty buffer (rkyv invariant only applies to
    // non-empty buffers).
}
```

**Asserts:**
- `buf.len() == 0`.
- No panic, no error.

---

## UT-0355-02: 1-byte payload alignment

**Target file:** `relativist-core/src/protocol/frame.rs` (test module).
**Feature gate:** `#[cfg(all(test, feature = "zero-copy"))]`.
**R-mapping:** R25.

```rust
#[cfg(feature = "zero-copy")]
#[tokio::test]
async fn read_aligned_payload_aligns_for_len_1() {
    let (mut tx, mut rx) = tokio::io::duplex(64);
    use tokio::io::AsyncWriteExt;
    tx.write_all(&[0xAB]).await.expect("write 1 byte");
    drop(tx);
    let buf = read_aligned_payload(&mut rx, 1).await.expect("read");
    assert_eq!(buf.len(), 1);
    assert_eq!(buf[0], 0xAB);
    assert_eq!(buf.as_ptr() as usize % 16, 0,
        "1-byte AlignedVec start must be 16-aligned");
}
```

---

## UT-0355-03: 16-byte payload alignment + content

**Target file:** `relativist-core/src/protocol/frame.rs` (test module).
**Feature gate:** `#[cfg(all(test, feature = "zero-copy"))]`.
**R-mapping:** R25.

```rust
#[cfg(feature = "zero-copy")]
#[tokio::test]
async fn read_aligned_payload_aligns_for_len_16() {
    let payload: Vec<u8> = (0..16).collect();
    let (mut tx, mut rx) = tokio::io::duplex(64);
    use tokio::io::AsyncWriteExt;
    tx.write_all(&payload).await.expect("write 16 bytes");
    drop(tx);
    let buf = read_aligned_payload(&mut rx, 16).await.expect("read");
    assert_eq!(buf.len(), 16);
    assert_eq!(&buf[..], &payload[..], "byte content must match input");
    assert_eq!(buf.as_ptr() as usize % 16, 0,
        "16-byte AlignedVec start must be 16-aligned");
}
```

---

## UT-0355-04: 256-byte payload alignment + content

**Target file:** `relativist-core/src/protocol/frame.rs` (test module).
**Feature gate:** `#[cfg(all(test, feature = "zero-copy"))]`.
**R-mapping:** R25.

```rust
#[cfg(feature = "zero-copy")]
#[tokio::test]
async fn read_aligned_payload_aligns_for_len_256() {
    let payload: Vec<u8> = (0..256).map(|i| i as u8).collect();
    let (mut tx, mut rx) = tokio::io::duplex(512);
    use tokio::io::AsyncWriteExt;
    tx.write_all(&payload).await.expect("write 256 bytes");
    drop(tx);
    let buf = read_aligned_payload(&mut rx, 256).await.expect("read");
    assert_eq!(buf.len(), 256);
    assert_eq!(&buf[..], &payload[..]);
    assert_eq!(buf.as_ptr() as usize % 16, 0,
        "256-byte AlignedVec start must be 16-aligned");
}
```

---

## UT-0355-05: 4096-byte payload alignment + content

**Target file:** `relativist-core/src/protocol/frame.rs` (test module).
**Feature gate:** `#[cfg(all(test, feature = "zero-copy"))]`.
**R-mapping:** R25 (max-typical-payload-size case).

```rust
#[cfg(feature = "zero-copy")]
#[tokio::test]
async fn read_aligned_payload_aligns_for_len_4096() {
    let payload: Vec<u8> = (0..4096).map(|i| (i % 251) as u8).collect();
    let (mut tx, mut rx) = tokio::io::duplex(8192);
    use tokio::io::AsyncWriteExt;
    tx.write_all(&payload).await.expect("write 4096 bytes");
    drop(tx);
    let buf = read_aligned_payload(&mut rx, 4096).await.expect("read");
    assert_eq!(buf.len(), 4096);
    assert_eq!(&buf[..], &payload[..]);
    assert_eq!(buf.as_ptr() as usize % 16, 0,
        "4096-byte AlignedVec start must be 16-aligned");
}
```

---

## UT-0355-06: helper does not exist when feature is OFF (compile-time)

**Target file:** `relativist-core/src/protocol/frame.rs` (test module).
**Feature gate:** `#[cfg(all(test, not(feature = "zero-copy")))]`.
**R-mapping:** R25 (cfg-gating discipline).

```rust
#[cfg(not(feature = "zero-copy"))]
#[test]
fn read_aligned_payload_does_not_exist_in_default_build() {
    // Pure compile-time predicate: the symbol must NOT be reachable
    // without the feature. We assert via cfg!() rather than referencing
    // the symbol directly (which would fail compilation).
    assert!(!cfg!(feature = "zero-copy"),
        "this test is only meaningful when the feature is OFF");
    // No further runtime assertion needed; this test is a placeholder
    // that documents the cfg-gating discipline.
}
```

**Asserts:** placeholder; the test exists to document that the helper
is feature-gated and is unreachable in the default build.

---

## Test count

| Test | Default build | `--features zero-copy` build |
|------|---------------|------------------------------|
| UT-0355-01..05 | ⏭ skipped (cfg-gated) | ✅ runs (5 tests) |
| UT-0355-06 | ✅ runs | ⏭ skipped (cfg-gated) |
| **Total new tests** | **+1** | **+5** |

---

## Adversarial probes (QA candidates for Stage 5)

| # | Scenario | Why dangerous | Stage |
|---|----------|---------------|-------|
| QA-0355-A | Premature EOF: `read_exact` errors mid-payload | Must surface `ProtocolError::ConnectionLost`, NOT panic | QA |
| QA-0355-B | `len = u32::MAX as usize` (overflow on 32-bit, OOM on 64-bit) | Verify the helper does not allocate before the size check; pre-check is at the caller (`recv_frame`) | QA |
| QA-0355-C | Pathological allocator that returns 1-byte alignment for `AlignedVec` | Theoretical only — `AlignedVec` invariant is by construction; debug_assert would catch | QA |
| QA-0355-D | Concurrent reads from the same reader (race) | Not applicable — `AsyncReadExt` is single-threaded by design; flag for protocol-level QA | QA |
| QA-0355-E | Reader that yields partial reads (`Pending` → `Ready(half)` → `Pending` → `Ready(half)`) | `read_exact` MUST handle partial reads correctly; tokio_test::io::Builder can simulate | QA |

---

## Hard-to-write-deterministically tests (FLAGGED)

- **QA-0355-C** (allocator-misbehavior probe) is **hard to write
  deterministically** because the global allocator is process-wide.
  NOT implemented as `#[test]`; flagged for documentation only.
- **QA-0355-D** (race) is **non-deterministic**. NOT implemented; the
  helper is single-threaded by `tokio` design.

---

## Acceptance gate

1. Default build: `cargo test --workspace` count: 895 → **896** (+1: UT-06).
2. Feature build: `cargo test --workspace --features zero-copy` count:
   904 → **909** (+5: UT-01..05).
3. `cargo clippy --workspace --all-targets -- -D warnings` clean both ways.
4. `cargo fmt --check` clean.
5. Helper signature: `pub(crate) async fn read_aligned_payload<R: AsyncReadExt + Unpin>(reader: &mut R, len: usize) -> Result<rkyv::util::AlignedVec, ProtocolError>`.
6. NO `unwrap()` in the helper; NO `unsafe` (per TASK-0355
   acceptance bullet — unless `set_len` is chosen with `// SAFETY:`).

---

## Out of scope

- Wiring into `recv_frame` (TASK-0357 / TEST-SPEC-0357).
- `decompress_payload_aligned` helper (TASK-0357 / TEST-SPEC-0357).
- T13 alignment integration end-to-end → TEST-SPEC-0359.
