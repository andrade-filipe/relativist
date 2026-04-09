# TEST-SPEC-0180: Scaffold bench module structure

**Task:** TASK-0180
**Spec:** SPEC-09
**Requirements verified:** R1, R2 (module structure only)

---

## Test Strategy

This task creates empty module scaffolding. The only acceptance criterion is successful compilation. No runtime tests are needed.

## Tests

### T1: Compilation check

**Type:** Build
**Precondition:** All placeholder files created.
**Verification:** `cargo check` exits with code 0.
**Expected:** No compilation errors.

### T2: Module accessibility

**Type:** Build
**Precondition:** `pub mod bench;` added to `src/lib.rs`.
**Verification:** An external `use relativist::bench;` statement compiles.
**Expected:** The bench module is publicly accessible from the crate root.

## Edge Cases

1. **Empty sub-modules:** Placeholder files with only a doc comment should not trigger dead-code or unused-import warnings.
2. **Benchmark sub-modules:** All 8 benchmark sub-modules declared but empty should compile without errors.

## Notes

- No runtime tests. Verification is purely compilation-based.
- Subsequent tasks (TASK-0181+) will add types that exercise the module structure.
