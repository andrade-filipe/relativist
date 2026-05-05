# TEST-SPEC-0120: Convert security module to directory structure

**Task:** TASK-0120
**Spec:** SPEC-10 (structural prerequisite)
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Cargo check succeeds after conversion

**Type:** Build verification
**Input:** Run `cargo check` after converting `src/security.rs` to `src/security/` directory
**Expected:** Compilation succeeds with zero errors
**Verifies:** Module declarations in `mod.rs` are syntactically correct

### T2: Cargo check with TLS feature succeeds

**Type:** Build verification
**Input:** Run `cargo check --features tls`
**Expected:** Compilation succeeds with zero errors
**Verifies:** Feature-gated `tls` module compiles correctly

### T3: Module structure files exist

**Type:** Filesystem verification
**Input:** Check that the following files exist:
- `src/security/mod.rs`
- `src/security/token.rs`
- `src/security/error.rs`
- `src/security/tls.rs`
**Expected:** All 4 files exist
**Verifies:** Directory structure matches acceptance criteria

### T4: Old file removed

**Type:** Filesystem verification
**Input:** Check that `src/security.rs` does NOT exist
**Expected:** File does not exist (replaced by directory)

### T5: Sub-modules declared in mod.rs

**Type:** Source verification
**Input:** Read `src/security/mod.rs`
**Expected:** Contains `pub mod token;`, `pub mod error;`, and `#[cfg(feature = "tls")] pub mod tls;`

---

## Edge Cases

### E1: lib.rs unchanged

**Verify:** `src/lib.rs` still has `pub mod security;` -- the line MUST NOT change.
**Why:** Ensures the conversion is transparent to the rest of the crate.

### E2: TLS module is feature-gated

**Verify:** `src/security/tls.rs` contains `#[cfg(feature = "tls")]` at the module level or `mod.rs` gates the module declaration.
**Why:** TLS code must not compile when the `tls` feature is disabled.
