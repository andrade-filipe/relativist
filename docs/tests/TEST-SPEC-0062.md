# TEST-SPEC-0062: Define GridConfig struct

**Task:** TASK-0062
**Spec:** SPEC-05 (R25, R29)
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Create with max_rounds Some
Create `GridConfig { num_workers: 4, max_rounds: Some(100) }`. Assert `num_workers == 4` and `max_rounds == Some(100)`.

### T2: Create with max_rounds None
Create `GridConfig { num_workers: 2, max_rounds: None }`. Assert `num_workers == 2` and `max_rounds.is_none()`.

### T3: Derives Debug and Clone
Create `GridConfig { num_workers: 8, max_rounds: Some(50) }`. Call `format!("{:?}", config)` to verify Debug. Call `config.clone()` and assert the clone has identical field values.

### T4: Single worker config
Create `GridConfig { num_workers: 1, max_rounds: None }`. Assert `num_workers == 1`. This is the degenerate case for R26 (n==1 optimization).

## Edge Cases

### E1: max_rounds = Some(0)
Create `GridConfig { num_workers: 2, max_rounds: Some(0) }`. Assert `max_rounds == Some(0)`. This is a valid configuration that causes immediate termination in `run_grid`.

### E2: Large num_workers
Create `GridConfig { num_workers: 1024, max_rounds: Some(u32::MAX) }`. Assert fields are accessible and hold the expected values. No overflow at construction.

### E3: Module compiles
`cargo check` passes with the `GridConfig` struct in `src/merge.rs`.
