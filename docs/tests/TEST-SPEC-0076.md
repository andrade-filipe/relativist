# TEST-SPEC-0076: Implement merge module exports and wiring

**Task:** TASK-0076
**Spec:** SPEC-05 (structural wiring)
**Generated:** 2026-04-07 (retroactive)

---

## Tests

### T1: Module compiles
`cargo check` passes with `src/merge.rs` containing all imports and exports.

### T2: Public types importable
`GridMetrics`, `WorkerRoundStats`, and `GridConfig` are importable via `use relativist::merge::GridMetrics;` (or equivalent crate path).

### T3: Public functions importable
`merge`, `run_grid`, `rebuild_free_port_index`, and `drain_stale_redexes` are importable from the merge module.

### T4: Required imports present
The module imports from `std::collections::HashMap`, `std::collections::VecDeque`, `std::time::{Duration, Instant}`, `crate::net`, `crate::partition`, and `crate::reduction`.

## Edge Cases

### E1: Unused import warnings
Verify `cargo check` produces no warnings for unused imports (or that `#[allow(unused_imports)]` is used temporarily for dependencies not yet implemented).

### E2: Module doc comment preserved
The module-level doc comment (`//! Merge and grid cycle (SPEC-05).`) is present at the top of `src/merge.rs`.

## Notes

Scaffolding/wiring task -- compilation is the primary acceptance criterion. This task can be done incrementally as dependent types become available.
