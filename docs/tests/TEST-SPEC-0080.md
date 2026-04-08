# TEST-SPEC-0080: Convert protocol module to directory structure

**Task:** TASK-0080
**Spec:** SPEC-06 (structural prerequisite)
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Smoke test -- cargo check succeeds

**Type:** Build verification
**Input:** Run `cargo check` after converting `src/protocol.rs` to `src/protocol/` directory
**Expected:** Compilation succeeds with zero errors
**Verifies:** Module declarations in `mod.rs` are syntactically correct, `lib.rs` still resolves `pub mod protocol;`

### T2: Module structure exists

**Type:** Filesystem verification
**Input:** Check that the following files exist after conversion:
- `src/protocol/mod.rs`
- `src/protocol/types.rs`
- `src/protocol/frame.rs`
- `src/protocol/error.rs`
- `src/protocol/config.rs`
- `src/protocol/coordinator.rs`
- `src/protocol/worker.rs`

**Expected:** All 7 files exist
**Verifies:** Directory structure matches TASK-0080 acceptance criteria

### T3: Old file removed

**Type:** Filesystem verification
**Input:** Check that `src/protocol.rs` does NOT exist
**Expected:** File does not exist (replaced by directory)
**Verifies:** No conflict between file and directory module resolution

### T4: Sub-modules declared in mod.rs

**Type:** Source verification
**Input:** Read `src/protocol/mod.rs`
**Expected:** Contains `pub mod types;`, `pub mod frame;`, `pub mod error;`, `pub mod config;`, `pub mod coordinator;`, `pub mod worker;`
**Verifies:** All 6 sub-module declarations present

### T5: Re-exports in mod.rs

**Type:** Source verification
**Input:** Read `src/protocol/mod.rs`
**Expected:** Contains `pub use types::*;`, `pub use frame::*;`, `pub use error::*;`, `pub use config::*;`
**Verifies:** External code can import from `relativist::protocol::` without knowing internal structure

### T6: Doc comment on mod.rs

**Type:** Source verification
**Input:** Read `src/protocol/mod.rs`
**Expected:** Contains `//! Wire protocol for coordinator-worker communication (SPEC-06).`
**Verifies:** Module-level documentation is present

---

## Edge Cases

### E1: lib.rs unchanged

**Verify:** `src/lib.rs` still has `pub mod protocol;` -- the line MUST NOT change.
**Why:** Ensures the conversion is transparent to the rest of the crate.

### E2: Each sub-module has a doc comment placeholder

**Verify:** Each of the 6 sub-module files contains at least one `//!` doc comment line.
**Why:** Ensures placeholder documentation exists for future implementers.
