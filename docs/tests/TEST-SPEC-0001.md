# TEST-SPEC-0001: Convert net module to directory structure

**Task:** TASK-0001
**Spec:** SPEC-02 (structural prerequisite)
**Generated:** 2026-04-06

---

## Test Summary

This task is purely structural (file reorganization, no logic). Tests verify that the module structure compiles correctly and that the public API surface is preserved.

---

## Tests

### T1: Smoke test — cargo check succeeds

**Type:** Build verification
**Input:** Run `cargo check` after converting `src/net.rs` to `src/net/` directory
**Expected:** Compilation succeeds with zero errors
**Verifies:** Module declarations in `mod.rs` are syntactically correct, `lib.rs` still resolves `pub mod net;`

### T2: Module structure exists

**Type:** Filesystem verification
**Input:** Check that the following files exist after conversion:
- `src/net/mod.rs`
- `src/net/types.rs`
- `src/net/core.rs` (renamed from `net.rs` to avoid clippy `module_inception`)
- `src/net/debug.rs`

**Expected:** All 4 files exist
**Verifies:** Directory structure matches TASK-0001 acceptance criteria

### T3: Old file removed

**Type:** Filesystem verification
**Input:** Check that `src/net.rs` does NOT exist
**Expected:** File does not exist (replaced by directory)
**Verifies:** No conflict between file and directory module resolution

### T4: Sub-modules declared in mod.rs

**Type:** Source verification
**Input:** Read `src/net/mod.rs`
**Expected:** Contains `pub mod types;`, `pub mod core;`, `pub mod debug;`
**Verifies:** Sub-module declarations present

### T5: Re-exports in mod.rs

**Type:** Source verification
**Input:** Read `src/net/mod.rs`
**Expected:** Contains `pub use types::*;` and `pub use core::*;`
**Verifies:** External code can still import from `relativist::net::` without knowing internal structure

---

## Edge Cases

### E1: lib.rs unchanged

**Verify:** `src/lib.rs` still has `pub mod net;` — the line MUST NOT change.
**Why:** Ensures the conversion is transparent to the rest of the crate.

### E2: Doc comment preserved

**Verify:** `src/net/mod.rs` contains the original doc comment from `src/net.rs`:
```
//! Core types for Interaction Combinator networks (SPEC-02).
```
**Why:** Preserves module-level documentation across the restructuring.
