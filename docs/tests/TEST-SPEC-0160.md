# TEST-SPEC-0160: Convert io module to directory structure

**Task:** TASK-0160
**Spec:** SPEC-12 (structural prerequisite)
**Generated:** 2026-04-08 (retroactive)

---

## Unit Tests

### T1: Smoke test -- cargo check succeeds

**Type:** Build verification
**Input:** Run `cargo check` after converting `src/io.rs` to `src/io/` directory
**Expected:** Compilation succeeds with zero errors
**Verifies:** Module declarations in `mod.rs` are syntactically correct, `lib.rs` still resolves `pub mod io;`

### T2: Module structure exists

**Type:** Filesystem verification
**Input:** Check that the following files exist after conversion:
- `src/io/mod.rs`
- `src/io/types.rs`
- `src/io/binary.rs`
- `src/io/text_dsl.rs` (or `text.rs`)
- `src/io/generators.rs` (or `examples.rs`)

**Expected:** All files exist
**Verifies:** Directory structure matches TASK-0160 acceptance criteria and SPEC-12 Section 4.3

### T3: Old file removed

**Type:** Filesystem verification
**Input:** Check that `src/io.rs` does NOT exist
**Expected:** File does not exist (replaced by directory)
**Verifies:** No conflict between file and directory module resolution

### T4: Sub-modules declared in mod.rs

**Type:** Source verification
**Input:** Read `src/io/mod.rs`
**Expected:** Contains `pub mod types;`, `pub mod binary;`, `pub mod text_dsl;` (or `text`), `pub mod generators;` (or `examples`)
**Verifies:** All sub-module declarations present

### T5: Re-exports in mod.rs

**Type:** Source verification
**Input:** Read `src/io/mod.rs`
**Expected:** Contains `pub use` statements that re-export key types (`NetFormat`, `NetSummary`, `ReductionSummary`, `detect_format`, `net_summary`, etc.)
**Verifies:** External code can import from `relativist::io::` without knowing internal structure

### T6: Doc comment on mod.rs

**Type:** Source verification
**Input:** Read `src/io/mod.rs`
**Expected:** Contains `//!` module-level doc comment referencing SPEC-12 and User I/O
**Verifies:** Module-level documentation is present

---

## Edge Cases

### E1: lib.rs unchanged

**Verify:** `src/lib.rs` still has `pub mod io;` -- the line MUST NOT change.
**Why:** Ensures the conversion is transparent to the rest of the crate.

### E2: Each sub-module has a doc comment

**Verify:** Each sub-module file (`types.rs`, `binary.rs`, `text_dsl.rs`, `generators.rs`) contains at least one `//!` doc comment line.
**Why:** Ensures documentation exists for every sub-module.
