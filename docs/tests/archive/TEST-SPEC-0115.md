# TEST-SPEC-0115: Align Cargo.toml with SPEC-13 dependency map

**Task:** TASK-0115
**Spec:** SPEC-13
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: cargo check passes with default features

**Type:** Build verification
**Input:** `cargo check`
**Expected:** Compilation succeeds with zero errors
**Verifies:** All always-on dependencies resolve correctly

### T2: cargo check with tls feature

**Type:** Build verification
**Input:** `cargo check --features tls`
**Expected:** Compilation succeeds (may have warnings if stubs are not yet implemented)
**Verifies:** R37 -- tls feature compiles

### T3: cargo check with full feature

**Type:** Build verification
**Input:** `cargo check --features full`
**Expected:** Compilation succeeds
**Verifies:** R37 -- full feature includes tls, metrics, otel

### T4: Always-on dependencies present

**Type:** Source verification
**Input:** Read `Cargo.toml` `[dependencies]` section
**Expected:** Contains: `tokio` (1.x), `serde` (1.x), `bincode`, `clap` (4.x), `tracing` (0.1), `tracing-subscriber` (0.3), `thiserror`, `rand` (0.8)
**Verifies:** R11 -- required always-on dependencies

### T5: Feature flags use dep: syntax

**Type:** Source verification
**Input:** Read `Cargo.toml` `[features]` section
**Expected:** Contains `tls = ["dep:rustls", "dep:tokio-rustls"]` (or equivalent with `dep:` prefix)
**Verifies:** R12 -- feature-gated dependencies use dep: syntax

### T6: Dev-dependencies present

**Type:** Source verification
**Input:** Read `Cargo.toml` `[dev-dependencies]` section
**Expected:** Contains `proptest`, `criterion`, `tokio-test`
**Verifies:** R13 -- dev-dependencies for testing

---

## Edge Cases

### E1: No duplicate dependency entries

**Verify:** Each crate appears at most once in `[dependencies]`.
**How:** Scan Cargo.toml for duplicate crate names.
**Why:** Duplicate entries cause cargo warnings or build failures.

### E2: rand dependency is added

**Verify:** `rand = "0.8"` (or compatible) is present in `[dependencies]`.
**How:** Check Cargo.toml.
**Why:** Needed for token generation and test data but was previously missing.
