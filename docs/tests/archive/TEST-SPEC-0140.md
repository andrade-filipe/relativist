# TEST-SPEC-0140: Convert observability module to directory structure and add dependencies

**Task:** TASK-0140
**Spec:** SPEC-11 (structural prerequisite)
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: cargo check succeeds with default features

**Input:** Run `cargo check`
**Expected:** Compilation succeeds
**Verifies:** Module structure compiles without optional features

### T2: cargo check succeeds with metrics feature

**Input:** Run `cargo check --features metrics`
**Expected:** Compilation succeeds
**Verifies:** Metrics sub-modules and axum dependency compile

### T3: cargo check succeeds with otel feature

**Input:** Run `cargo check --features otel`
**Expected:** Compilation succeeds
**Verifies:** OTel sub-modules and dependencies compile

### T4: cargo check succeeds with all features

**Input:** Run `cargo check --all-features`
**Expected:** Compilation succeeds
**Verifies:** All feature combinations are compatible

### T5: Module structure files exist

**Input:** Check for `src/observability/mod.rs`, `src/observability/config.rs`, `src/observability/tracing_init.rs`
**Expected:** All files exist
**Verifies:** Directory conversion complete

### T6: Old file removed

**Input:** Check that `src/observability.rs` does NOT exist
**Expected:** File does not exist

---

## Edge Cases

### E1: Feature gates in Cargo.toml are correct

**Verify:** `metrics` feature includes `["prometheus-client", "dep:axum"]` and `otel` feature includes the OTel crates.
**Why:** Incorrect feature gates cause compile errors or unintended code inclusion.

### E2: tracing-subscriber has json feature

**Verify:** `tracing-subscriber` dependency includes `features = ["env-filter", "json"]`.
**Why:** R3 requires JSON log output format.
