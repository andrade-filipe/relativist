# TEST-SPEC-0602 — Tests for TASK-0602 — Extend `BenchmarkSuiteConfig` with Tier 3 fields

**Task:** TASK-0602 (Phase C-1, P0)
**Spec:** SPEC-09 R18a–R18g, R37c (commit `82b2d27`); SPEC-21 §3.8 A3 (GridConfig chunk_size + max_pending_lifetime); SPEC-22 R10b (recycle_under_delta).
**Test floor delta:** **+4 default**.
**Prerequisites:** SPEC commit `82b2d27` already landed.

---

## Test inventory

| test_id | level | target file | prerequisite TASK | cfg gates |
|---|---|---|---|---|
| UT-0602-01 | unit | `relativist-core/src/bench/mod.rs::tests::benchmark_suite_config_default_values` | none | none |
| UT-0602-02 | unit | `relativist-core/src/bench/mod.rs::tests::recycle_policy_enum_traits_and_variants` | none | none |
| UT-0602-03 | unit | `relativist-core/src/bench/mod.rs::tests::net_representation_enum_traits_and_variants` | none | none |
| UT-0602-04 | unit | `relativist-core/tests/bench_suite_config_serde.rs::struct_clone_and_debug_round_trip_preserves_tier3_fields` | none | none |

Total: **4 default tests**.

---

## Per-test specifications

### UT-0602-01 — `benchmark_suite_config_default_values`

**Purpose.** Lock the exact default values of the 4 new Tier 3 fields. Defaults must preserve the eager-path status quo so existing bench runs are unchanged.
**Setup.** Construct a `BenchmarkSuiteConfig` via the project's canonical default constructor (currently a struct-literal pattern with `..Default::default()` or an explicit `BenchmarkSuiteConfig::default()` if one exists — verify in `relativist-core/src/bench/mod.rs:231-251`).
**Action.** Read the 4 new fields.
**Assertions.**
- `config.chunk_size == None` (eager path is the default).
- `config.max_pending_lifetime == 16` (matches coordinator CLI default per task spec).
- `config.recycle_policy == RecyclePolicy::DisableUnderDelta`.
- `config.representation == NetRepresentation::Dense`.
**Boundary case coverage.** Catches a silent default drift (e.g., a future refactor that flips `representation` to `Sparse` and breaks all `v1_local_baseline` comparisons).
**Why it must exist.** Acceptance criterion #2 of TASK-0602 ("Defaults match the eager-path status quo"). Single most important regression guard for this task.

---

### UT-0602-02 — `recycle_policy_enum_traits_and_variants`

**Purpose.** Verify the new enum has exactly 2 variants in the spec-mandated order with the required derives.
**Setup.** None (compile-time + value-level).
**Action.** Construct each variant; compare; format.
**Assertions.**
- `RecyclePolicy::DisableUnderDelta != RecyclePolicy::BorderClean` (PartialEq derive present).
- `RecyclePolicy::DisableUnderDelta == RecyclePolicy::DisableUnderDelta` (Eq + PartialEq).
- `let _ = RecyclePolicy::DisableUnderDelta.clone()` (Clone + Copy compile).
- `format!("{:?}", RecyclePolicy::BorderClean) == "BorderClean"` (Debug derive renders variant name).
- An exhaustive `match` over `RecyclePolicy` compiles with exactly 2 arms (verified by negative test: removing one arm fails compilation; documented as a doc comment, not asserted at runtime).
**Boundary case coverage.** Catches a buggy implementation that adds a third variant without spec amendment.
**Why it must exist.** Acceptance criterion #3 of TASK-0602 ("the two new enums derive `Debug, Clone, Copy, PartialEq, Eq`").

---

### UT-0602-03 — `net_representation_enum_traits_and_variants`

**Purpose.** Same as UT-0602-02 but for `NetRepresentation`.
**Setup.** None.
**Action.** Construct variants; compare; format.
**Assertions.**
- `NetRepresentation::Dense != NetRepresentation::Sparse`.
- `NetRepresentation::Dense.clone() == NetRepresentation::Dense` (Copy + Clone).
- `format!("{:?}", NetRepresentation::Sparse) == "Sparse"`.
**Boundary case coverage.** Same as UT-0602-02.
**Why it must exist.** Same acceptance criterion #3.

---

### UT-0602-04 — `struct_clone_and_debug_round_trip_preserves_tier3_fields`

**Purpose.** Verify that `Clone` and `Debug` (which are the available "round-trip" traits since `BenchmarkSuiteConfig` does not derive `Serialize`) preserve the 4 new fields. This is a sanity test that the fields are wired into `#[derive(Debug, Clone)]` correctly (not skipped).
**Setup.** Build a `BenchmarkSuiteConfig` with all 4 new fields set to non-default values:
```
config.chunk_size = Some(123);
config.max_pending_lifetime = 42;
config.recycle_policy = RecyclePolicy::BorderClean;
config.representation = NetRepresentation::Sparse;
```
**Action.** `let cloned = config.clone(); let debug_str = format!("{:?}", config);`
**Assertions.**
- `cloned.chunk_size == Some(123)`.
- `cloned.max_pending_lifetime == 42`.
- `cloned.recycle_policy == RecyclePolicy::BorderClean`.
- `cloned.representation == NetRepresentation::Sparse`.
- `debug_str.contains("chunk_size: Some(123)")`.
- `debug_str.contains("max_pending_lifetime: 42")`.
- `debug_str.contains("BorderClean")`.
- `debug_str.contains("Sparse")`.
**Boundary case coverage.** Catches a buggy `#[derive(Clone)]` invocation where a manual `impl Clone` exists that forgets the new fields. (This is a real failure mode in projects with mixed manual + derived Clone.)
**Why it must exist.** Acceptance criterion #6 of TASK-0602 ("regression guard against accidental default drift") + ensures the field is wired into the derive pipeline, not orphaned.

---

## Coverage matrix

| test_id | TASK §AC-1 | TASK §AC-2 | TASK §AC-3 | TASK §AC-4 | TASK §AC-5 | TASK §AC-6 |
|---|---|---|---|---|---|---|
| UT-0602-01 | ✅ | ✅ | | | ✅ | ✅ |
| UT-0602-02 | ✅ | | ✅ | | | |
| UT-0602-03 | ✅ | | ✅ | | | |
| UT-0602-04 | ✅ | | | ✅ | ✅ | ✅ |

Every acceptance criterion has ≥1 test.

---

## Out-of-scope tests (deferred to other tasks)

- Path selection on `chunk_size = Some(N)` → **TASK-0604**.
- CLI parsing of the 4 new fields → **TASK-0603**.
- Memory probe wiring → **TASK-0605**.
- Sparse-path specifics → **TASK-0606** (deferred).
- `serde::{Serialize, Deserialize}` round-trip → not in scope; struct does not derive serde traits.
