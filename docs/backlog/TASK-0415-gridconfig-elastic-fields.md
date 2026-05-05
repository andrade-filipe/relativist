# TASK-0415: [SPEC-05 amendment A5] Extend `GridConfig` with 9 elastic-grid fields

**Spec:** SPEC-20 §3.0 M0 (4-mode matrix), R0a (per-feature mode coverage), R0b (v1↔delta interpretation), R0c (mode immutability per run); §3.4 R33 + R33a (defaults table); §3.8 A5 (formal SPEC-05 amendment).
**Requirements:** M0 (run is one of A/B/C/D), R0a (config fields enable per-feature mode coverage), R0b (interpretation of v1/delta terms is config-driven), R0c (mode fixed at run start; validator rejects mid-run mutation), R33 (field list), R33a (defaults: `hybrid_coordinator=false`, `elastic_departure=false`, `retain_partitions=derived`, `elastic_join=derived`, `checkpoint_partitions=false`, `initial_wait_timeout=30s`, `join_window_min=50ms`, `join_window_max=500ms`, `solo_budget=10_000`).
**Priority:** P0 (foundational; blocker for virtually every SPEC-20 implementation task).
**Status:** TODO
**Depends on:** none.
**Blocked by:** none
**Estimated complexity:** S (~50-80 LoC production + ~50 LoC tests + CLI wiring in TASK-0416)
**Bundle:** SPEC-20 Elastic Grid — config + wire foundations.
**Tag:** `[SPEC-05 amendment]`

## Context

SPEC-20 introduces 9 new `GridConfig` fields governing hybrid coordinator, dynamic joining, dynamic departure, retention, and timing. Defaults preserve v1 baseline (backward compatibility). Some defaults are *derived* (`retain_partitions` auto-true when `elastic_departure = true`; `elastic_join` auto-true when `hybrid_coordinator || elastic_departure`).

## Acceptance Criteria

- [ ] Extend `GridConfig` struct in `relativist-core/src/` (`merge/config.rs` or wherever SPEC-05 defines it) with the 9 new fields per R33 schema.
- [ ] Implement `Default` for `GridConfig` with R33a defaults. Derived defaults (`retain_partitions`, `elastic_join`) computed in a `GridConfig::normalize(&mut self)` helper invoked after CLI parse.
- [ ] Add a `GridConfig::validate(&self) -> Result<(), ConfigError>` method that rejects:
  - `retain_partitions = false` AND `elastic_departure = true` (per R32, the validator rejects this combination).
  - `join_window_min > join_window_max`.
  - `solo_budget == 0`.
- [ ] Add a `GridConfig::active_mode(&self) -> ExecutionMode` accessor that returns one of `{V1Lenient, V1Strict, DeltaLenient, DeltaStrict}` (M0 mode A/B/C/D) computed from `delta_mode × strict_bsp`.
- [ ] R0c (mode immutability): document in `GridConfig` Rustdoc that `delta_mode` and `strict_bsp` MUST NOT mutate after `run_grid` enters `WaitingForWorkers`. The pure config struct does not enforce this on its own; the runtime path captures `active_mode()` once at `run_grid` entry and passes it as a `&ExecutionMode` parameter (no `&mut`) thereafter. Validator emits a hard-coded comment hint, no runtime check.
- [ ] Add `#[cfg_attr(feature = "zero-copy", derive(rkyv::Archive, ...))]` where the struct already derives rkyv.
- [ ] Serialize/deserialize cleanly via serde + bincode (SPEC-18 wire compatibility, though GridConfig is not on the wire — this is for snapshot/replay parity).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/merge/config.rs` *(SPEC-05 GridConfig site)* | modify | Add 9 fields + `Default` impl + `normalize` + `validate`. |
| `relativist-core/src/error.rs` *(or wherever ConfigError lives)* | modify | Add `ConfigError::RetainRequiredForDeparture`, `ConfigError::JoinWindowOrdering`, `ConfigError::SoloBudgetZero` variants. |

## Key Types / Signatures

```rust
pub struct GridConfig {
    // ... existing v1 fields ...
    // ... existing SPEC-19 fields ...
    pub hybrid_coordinator: bool,        // default false (R33a; closes SC-010)
    pub elastic_departure: bool,         // default false
    pub retain_partitions: bool,         // derived; default false, auto-true when elastic_departure
    pub elastic_join: bool,              // derived; auto-true when hybrid||elastic_departure
    pub checkpoint_partitions: bool,     // default false
    pub initial_wait_timeout: Duration,  // default 30 s
    pub join_window_min: Duration,       // default 50 ms
    pub join_window_max: Duration,       // default 500 ms
    pub solo_budget: u32,                // default 10_000
}
```

## Test Expectations (forward-ref)

TEST-SPEC-0415:

- `grid_config_defaults_match_r33a` — every default matches the R33a table.
- `grid_config_derived_retain_partitions` — normalize flips `retain_partitions` true when `elastic_departure` is true.
- `grid_config_derived_elastic_join` — normalize flips `elastic_join` true when either antecedent is set.
- `validate_rejects_retain_false_with_departure_true`.
- `validate_rejects_inverted_join_window_bounds`.
- `validate_rejects_zero_solo_budget`.
- `grid_config_backward_compat_v1_baseline` — a `GridConfig::default()` struct reproduces v1 behavior under all 1181 existing tests (regression).

## Invariants Touched

- None (configuration-surface change).

## Notes

- **Serde stability**: all new fields need `#[serde(default)]` so older serialized `GridConfig` blobs continue to deserialize.
- **No CLI in this task**: TASK-0416 owns the clap wiring for R34 flags.

## DAG Links

- **Predecessors:** none.
- **Successors:** TASK-0416 (CLI flags), TASK-0430 (hybrid FSM), TASK-0431 (solo), and every elastic feature task.
