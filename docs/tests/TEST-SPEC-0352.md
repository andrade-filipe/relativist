# TEST-SPEC-0352: Cargo.toml + `zero-copy` feature gate

**Task:** TASK-0352
**Spec:** SPEC-18 §3.5 (R20)
**Generated:** 2026-04-16
**Baseline before this task:** 887 lib + 4 integration (post-SPEC-19 §3.1 ship)

---

## Scope note

This task is a Cargo.toml-only edit. There is no `src/` change. The tests
verify three properties: (a) the `zero-copy` feature is opt-in (NOT in
`default`), (b) building with `--features zero-copy` succeeds and the
feature flag is visible at compile time, (c) building without the feature
also succeeds and rkyv is not pulled into the dep tree.

Because the change is build-system only, two of the three test patterns
below are integration-style (they live under `tests/feature_gate.rs`,
not inside `relativist-core/src/`) so they exercise the actual cargo
metadata and feature compilation behavior. The third is a pure
compile-time `cfg!()` predicate inside a unit test in
`relativist-core/src/lib.rs`.

---

## UT-0352-01: feature flag is opt-in (default features)

**Target file:** `relativist-core/src/lib.rs` (root, inside the existing
`#[cfg(test)] mod tests` block).
**Feature gate:** none (default features).
**R-mapping:** R20 (zero-copy MUST NOT be a default feature).

```rust
/// SPEC-18 R20 — `zero-copy` MUST NOT be a default feature. This test
/// runs in the default-features build and asserts the feature is OFF.
#[test]
fn zero_copy_feature_is_off_by_default() {
    assert!(
        !cfg!(feature = "zero-copy"),
        "SPEC-18 R20: 'zero-copy' must NOT be a default feature; \
         re-check Cargo.toml [features] table"
    );
}
```

**Asserts:** `cfg!(feature = "zero-copy") == false` in the default build.

---

## UT-0352-02: feature flag activates rkyv dependency

**Target file:** `relativist-core/src/lib.rs` (same module as UT-01).
**Feature gate:** `#[cfg(feature = "zero-copy")]`.
**R-mapping:** R20 (the feature gate `dep:rkyv` works as wired).

```rust
/// SPEC-18 R20 — When `--features zero-copy` is active, the rkyv crate
/// is in the dep tree. We probe for a known rkyv symbol that only
/// compiles with the dep present.
#[cfg(feature = "zero-copy")]
#[test]
fn zero_copy_feature_activates_rkyv() {
    // Probe: AlignedVec is the symbol used by TASK-0355 and TASK-0357.
    // If rkyv is not in the dep tree, this would not compile.
    let v: rkyv::util::AlignedVec = rkyv::util::AlignedVec::with_capacity(16);
    assert_eq!(v.len(), 0);
    assert_eq!(v.capacity() % 16, 0,
        "AlignedVec must yield 16-aligned capacity (R25 precondition)");
}
```

**Asserts:**
- The test compiles ONLY under `--features zero-copy`.
- `AlignedVec::with_capacity(16).len() == 0`.
- `AlignedVec` capacity is a multiple of 16 (precondition for R25).

---

## UT-0352-03: Cargo.toml does not list `zero-copy` in `default`

**Target file:** `relativist-core/src/lib.rs` (parser test).
**Feature gate:** none (default features; runs in any build).
**R-mapping:** R20.

```rust
/// SPEC-18 R20 — Static parse of `Cargo.toml` confirms `default = []`
/// (or any list that does NOT contain `"zero-copy"`).
#[test]
fn cargo_toml_default_features_excludes_zero_copy() {
    let manifest = include_str!("../Cargo.toml");
    // Find the `[features]` section.
    let features_idx = manifest.find("[features]")
        .expect("Cargo.toml must have a [features] table");
    let after = &manifest[features_idx..];
    // Look for the `default = ` line.
    let default_idx = after.find("default = ")
        .expect("[features] must declare a `default = ` entry");
    // Take that line up to the next newline.
    let default_line_end = after[default_idx..].find('\n').unwrap_or(after.len());
    let default_line = &after[default_idx..default_idx + default_line_end];
    assert!(
        !default_line.contains("zero-copy"),
        "SPEC-18 R20 violated: `default` features list contains \
         'zero-copy'. Got line: {}",
        default_line
    );
}
```

**Asserts:** the substring `"zero-copy"` does not appear inside the
`default = ...` line of `Cargo.toml`.

**Notes:** uses `include_str!` so the test runs at compile time on the
embedded manifest text. If the manifest layout shifts, the test logs the
exact line for debug.

---

## Test count

| Test | Default build | `--features zero-copy` build |
|------|---------------|------------------------------|
| UT-0352-01 | ✅ runs | ✅ runs |
| UT-0352-02 | ⏭ skipped (cfg-gated) | ✅ runs |
| UT-0352-03 | ✅ runs | ✅ runs |
| **Total new tests** | **+2** | **+3** |

---

## Adversarial probes (QA candidates for Stage 5)

| # | Scenario | Why dangerous | Stage |
|---|----------|---------------|-------|
| QA-0352-A | `cargo build --release --features zero-copy` succeeds | Release builds may surface trait bound issues that debug elides | QA |
| QA-0352-B | `cargo build --workspace --no-default-features` succeeds | If `default = []` is not literally empty, this would fail | QA |
| QA-0352-C | `cargo metadata --format-version 1 \| jq '.packages[].name'` does NOT contain `rkyv` in default build | Confirms rkyv is truly optional, not pulled in transitively | QA |
| QA-0352-D | `cargo build --features full` does NOT pull rkyv | Confirms `full` does not include `zero-copy` (acceptance criterion #4) | QA |
| QA-0352-E | `cargo build --features "tls metrics otel zero-copy"` succeeds | Combined-feature smoke: rkyv coexists with TLS/metrics/otel | QA |

---

## Acceptance gate

1. `cargo test --workspace` count: 887 → **889** (+2: UT-01 + UT-03).
2. `cargo test --workspace --features zero-copy` count: 887 → **890**
   (+3: UT-01 + UT-02 + UT-03).
3. `cargo build --workspace` clean.
4. `cargo build --workspace --features zero-copy` clean.
5. `cargo clippy --workspace --all-targets -- -D warnings` clean both ways.
6. `cargo fmt --check` clean.

---

## Out of scope (deferred to other TEST-SPECs)

- Rkyv derives on the 8 hot-path types → TEST-SPEC-0353.
- `ProtocolError::ArchiveValidationFailed` variant → TEST-SPEC-0354.
- Aligned receive buffer → TEST-SPEC-0355.
- send/recv archive paths → TEST-SPEC-0356/0357.
- CLI `--use-zero-copy` flag → TEST-SPEC-0358.
- T11-T14 round-trip suite → TEST-SPEC-0359.
