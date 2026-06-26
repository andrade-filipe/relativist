# TEST-SPEC-0020: Scaffold reduction module structure

**Task:** TASK-0020
**Spec:** SPEC-03 (structural prerequisite)
**Generated:** 2026-04-07 (retroactive)

---

## Tests

### T1: Module compiles
`cargo check` passes with the new module structure (`src/reduction/mod.rs`, `dispatch.rs`, `rules.rs`, `engine.rs`).

### T2: Re-exports accessible
Public types from sub-modules are importable via `crate::reduction::*`.

## Notes

Scaffolding task — no behavioral tests. Compilation is the acceptance criterion.
