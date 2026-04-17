# TEST-SPEC-0354: `ProtocolError::ArchiveValidationFailed(String)` tuple variant

**Task:** TASK-0354
**Spec:** SPEC-18 §3.5 (R26), §3.8 (R35)
**Generated:** 2026-04-16
**Baseline before this task:** 890 lib (default) / 899 lib (`--features zero-copy`, post-TASK-0353).

---

## Scope note

R35 mandates the variant **shape** verbatim as
`ArchiveValidationFailed(String)` — a tuple variant, NOT a struct
variant `{ reason: String }` (spec-critic DC-2, 2026-04-16). This
TEST-SPEC encodes that mandate so the developer cannot accidentally
implement the struct form. Test UT-04 is the load-bearing source-grep
that pins the tuple discriminator.

The variant is unconditional in the enum (no `cfg` gate per TASK-0354
acceptance criteria) so that exhaustive `match` on `ProtocolError`
compiles in both feature configurations. UT-01..03 therefore run in
both builds; UT-04 also runs in both builds.

---

## UT-0354-01: variant constructible with arbitrary string

**Target file:** `relativist-core/src/protocol/error.rs` (test module).
**Feature gate:** none.
**R-mapping:** R26, R35.

```rust
#[test]
fn archive_validation_failed_constructs_with_arbitrary_string() {
    let e1 = ProtocolError::ArchiveValidationFailed(String::new());
    let e2 = ProtocolError::ArchiveValidationFailed("alignment".into());
    let e3 = ProtocolError::ArchiveValidationFailed(
        "a".repeat(4096), // long reason — no length cap
    );
    // Pattern-match must succeed on the tuple form.
    matches!(e1, ProtocolError::ArchiveValidationFailed(_));
    matches!(e2, ProtocolError::ArchiveValidationFailed(_));
    matches!(e3, ProtocolError::ArchiveValidationFailed(_));
    // Field access via tuple destructuring.
    if let ProtocolError::ArchiveValidationFailed(s) = &e2 {
        assert_eq!(s, "alignment");
    } else {
        panic!("destructuring failed");
    }
}
```

**Asserts:**
- The variant accepts empty, short, and long Strings.
- Pattern destructuring uses tuple syntax `(s)`, not struct syntax `{ reason }`.

---

## UT-0354-02: Display impl renders canonical phrase + reason

**Target file:** `relativist-core/src/protocol/error.rs` (test module).
**Feature gate:** none.
**R-mapping:** R26 (Display contract).

```rust
/// SPEC-18 R26 — `ArchiveValidationFailed` Display preserves the
/// inner reason (parallel to `decompression_failed_error_renders`).
#[test]
fn archive_validation_failed_error_renders() {
    let e = ProtocolError::ArchiveValidationFailed(
        "alignment violation at offset 32".into(),
    );
    let s = e.to_string();
    assert!(s.contains("rkyv archive validation failed"),
        "canonical phrase missing; got: {}", s);
    assert!(s.contains("alignment violation at offset 32"),
        "inner reason missing; got: {}", s);
}
```

**Asserts:** Display format matches `"rkyv archive validation failed: {reason}"`.

---

## UT-0354-03: variant reachable via `recv_frame` rejection path

**Target file:** `relativist-core/src/protocol/error.rs` (test module)
or `relativist-core/src/protocol/frame.rs` test module — developer
chooses based on test-helper accessibility.
**Feature gate:** **flagged** — see Notes. Default proposal: this test
is feature-gated under `#[cfg(feature = "zero-copy")]` if it requires
the rkyv send path; otherwise it can be a synthetic frame fed to
`recv_frame` (which works in both builds because TASK-0357 makes
`recv_frame` reject `FLAG_ARCHIVED` in both configurations).

```rust
/// R26 reachability — a frame with FLAG_ARCHIVED set + garbage payload
/// surfaces as `ArchiveValidationFailed` (in BOTH build configurations:
/// `zero-copy` -> rkyv access fails; `not(zero-copy)` -> feature-disabled).
#[tokio::test]
async fn recv_frame_with_archive_flag_yields_archive_validation_failed() {
    let (mut tx, mut rx) = tokio::io::duplex(4096);
    // Forge a frame: 9-byte header with FLAG_ARCHIVED + arbitrary payload.
    let payload = b"not a valid rkyv archive".to_vec();
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
        Err(ProtocolError::ArchiveValidationFailed(_)) => {} // ok
        other => panic!("expected ArchiveValidationFailed, got {:?}", other),
    }
}
```

**Asserts:** the err variant is `ArchiveValidationFailed(_)` regardless
of feature configuration.

**NOTE on placement:** if writing this test in `error.rs` is
inconvenient because `recv_frame` lives in `frame.rs`, move it to
`frame.rs` test module. The R-mapping stays the same.

---

## UT-0354-04: variant is TUPLE, NOT struct (DC-2 mandate)

**Target file:** `relativist-core/src/protocol/error.rs` (test module).
**Feature gate:** none.
**R-mapping:** R35 (verbatim — DC-2 spec-critic mandate).

```rust
/// SPEC-18 R35 + spec-critic DC-2 (2026-04-16) — variant MUST be the
/// TUPLE form `ArchiveValidationFailed(String)`, NOT struct
/// `ArchiveValidationFailed { reason: String }`. This test pins the
/// shape via a pattern match that ONLY compiles for the tuple form.
#[test]
fn archive_validation_failed_is_tuple_variant_per_dc2() {
    let e = ProtocolError::ArchiveValidationFailed("test".into());
    // The following pattern only compiles on a tuple variant.
    // If a future refactor flips it to struct form
    // `{ reason: String }`, this test fails to compile, alerting the
    // developer that DC-2 has been violated.
    let captured = match e {
        ProtocolError::ArchiveValidationFailed(s) => s,
        _ => panic!("variant unreachable"),
    };
    assert_eq!(captured, "test");
}
```

**Asserts:** compile-time + runtime — the destructuring uses `(s)`, not
`{ reason: s }`.

**Notes:** this test is the canonical regression-guard against DC-2
violation. If TASK-0354 is implemented as `{ reason: String }`, this
test fails compilation with a clear error message pointing at
`docs/spec-reviews/SPEC-18-section-3.5-design-choices-2026-04-16.md`
DC-2 (developer can grep for `archive_validation_failed_is_tuple_variant_per_dc2`).

---

## UT-0354-05: existing `test_all_variants_debug` is extended

**Target file:** `relativist-core/src/protocol/error.rs` (test module).
**Feature gate:** none.
**R-mapping:** R35 (variant exhaustiveness).

```rust
// EXTEND existing test (not a new test). Add to the existing variant
// enumeration so the new variant is exercised by Debug.
#[test]
fn test_all_variants_debug_includes_archive_validation_failed() {
    let e = ProtocolError::ArchiveValidationFailed(
        "any reason".into(),
    );
    let dbg = format!("{:?}", e);
    assert!(!dbg.is_empty());
    assert!(dbg.contains("ArchiveValidationFailed"),
        "Debug output must name the variant; got: {}", dbg);
}
```

**Asserts:** Debug output includes the variant name.

**Note:** if `test_all_variants_debug` already exists as a single test
that loops over variants, the developer should extend it in place;
otherwise add this test as a sibling. Net test count unchanged either
way (the extension does not add a new `#[test]` if folded in-place).
For the count below, we assume it adds **+1** new test
(`test_all_variants_debug_includes_archive_validation_failed`).

---

## Test count

| Test | Default build | `--features zero-copy` build |
|------|---------------|------------------------------|
| UT-0354-01 | ✅ runs | ✅ runs |
| UT-0354-02 | ✅ runs | ✅ runs |
| UT-0354-03 | ✅ runs | ✅ runs |
| UT-0354-04 | ✅ runs | ✅ runs |
| UT-0354-05 | ✅ runs | ✅ runs |
| **Total new tests** | **+5** | **+5** |

(All tests are unconditional — the variant exists in both builds; UT-03
exercises the recv path which routes through different cfg branches but
yields the same error type either way.)

---

## Adversarial probes (QA candidates for Stage 5)

| # | Scenario | Why dangerous | Stage |
|---|----------|---------------|-------|
| QA-0354-A | Construct with a 1 MiB reason string | Verify no panic, no truncation in Display | QA |
| QA-0354-B | Construct with a String containing embedded `\n` and `\0` | Display must not break log scrapers | QA |
| QA-0354-C | Construct with non-UTF-8 (impossible — String is UTF-8) — verify the assumption | Sanity probe | QA / spec-doc |
| QA-0354-D | Send `recv_frame` 1-byte payload with FLAG_ARCHIVED set, malformed CRC | Verify CRC check happens BEFORE the rkyv access (R12), so the err is `ChecksumMismatch`, NOT `ArchiveValidationFailed` | QA |
| QA-0354-E | Match arm pattern `ArchiveValidationFailed { reason }` in a downstream caller | Compile error proves DC-2 enforcement reaches consumers | QA / static |
| QA-0354-F | `std::error::Error::source()` returns `None` | Verify no ErrorKind chaining promised | QA |

---

## Hard-to-write-deterministically tests (FLAGGED)

- None. All 5 tests are pure value construction + Display formatting.
  UT-03 uses `tokio::io::duplex` which is deterministic.

---

## Acceptance gate

1. Default build: `cargo test --workspace` count: 890 → **895** (+5).
2. Feature build: `cargo test --workspace --features zero-copy` count:
   899 → **904** (+5).
3. `cargo clippy --workspace --all-targets -- -D warnings` clean both ways.
4. `cargo fmt --check` clean.
5. Existing `test_all_variants_debug` (or equivalent) still green.

---

## Out of scope

- Send-side rkyv error mapping (`"serialize: "` prefix per DC-4) →
  TEST-SPEC-0356.
- Recv-side rkyv discrimination (Assign-first per DC-3) → TEST-SPEC-0357.
- CLI `--use-zero-copy` flag → TEST-SPEC-0358.
- T13 corruption rejection end-to-end → TEST-SPEC-0359.
