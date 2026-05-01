# TASK-0605 — Add `get_peak_memory_at_construction_complete` probe (Phase C-5)

**Phase:** C-5 (D-011 bench harness wiring — pre-reduction memory measurement)
**Bundle:** D-011 — Tier 3 Hardening + Bench Enablement
**Status:** PENDING
**Priority:** P0 (required to validate M5 — peak coordinator memory metric in SPEC-09 R18d)
**Spec:** SPEC-09 R18a–R18g (committed `82b2d27` — specifically R18d / R18e on construction-phase peak memory).
**Origin:** D-011 plan §C-5.
**Estimated complexity:** S (~50 LoC production + ~30 LoC tests)
**Estimated stages duration:** Stages 2→3→4→5→6 over ~0.5 day.

---

## Context

Today's memory probe (`relativist-core/src/bench/memory.rs:8-30`) provides only `get_peak_memory_bytes()` — a process-RSS reading. SPEC-09 R18a–R18g (the new Tier 3 metrics committed `82b2d27`) require `peak_memory_during_construction` — a measurement *between* net construction completion and `reduce_all` invocation. The coarse process-RSS at end-of-run is biased by reduction-phase allocations and can't isolate construction.

The fix: add a *checkpoint*-style probe `get_peak_memory_at_construction_complete()` that takes the snapshot at the right pipeline moment, called from:
- `bench/suite.rs:102` (sequential path, between `make_net()` and `reduce_all`)
- `bench/suite.rs:~221` (grid path, between net build and `run_grid`)

Implementation note: on Linux the probe still reads `/proc/self/status` VmHWM (peak resident set since process start), so the *peak* during construction is recovered by snapshotting at construction-complete and reporting that snapshot value (the post-reduction reading would be ≥ this, so the difference between the two snapshots is also informative — possibly worth reporting as a derived metric).

## Dependencies

- **TASK-0602 (C-1)** — RECOMMENDED — the new metric needs a place to land in `BenchmarkSuiteConfig` and the CSV row struct. (Strictly speaking, it can run earlier if the row struct is extended directly.)
- **TASK-0604 (C-2/C-4)** — RECOMMENDED for the smoke flow but not required for unit-level correctness.
- **SPEC commit `82b2d27`** — already landed.

## Files in scope

| File | Change |
|------|--------|
| `relativist-core/src/bench/memory.rs` (~lines 8-30) | Add `pub fn get_peak_memory_at_construction_complete() -> u64` (Linux: `/proc/self/status` VmHWM read; non-Linux: 0). |
| `relativist-core/src/bench/suite.rs:~102` (sequential) | Call probe between `make_net()` and `reduce_all`; record into the result row. |
| `relativist-core/src/bench/suite.rs:~221` (grid) | Call probe between net construction and `run_grid`; record into the result row. |
| `relativist-core/src/bench/mod.rs` (`BenchmarkResult` or row struct) | Add field `peak_memory_at_construction_complete: u64` (per SPEC-09 R18d). |
| CSV writer (existing infrastructure) | Emit the new column. (May be just a struct-derive change if csv crate is used.) |
| `relativist-core/tests/spec09_bench_construction_memory_probe.rs` (new) | Linux-gated test: probe at construction-complete returns a non-zero value for a non-trivial benchmark; probe value MUST be ≤ end-of-run peak. |

## Files explicitly OUT of scope

- The sparse-net measurement (Phase D / TASK-0606) — separate sub-CSV.
- Cross-OS memory accounting (macOS / Windows) — explicitly returns 0 per current behaviour; not adding new platform support.
- Any change to existing `get_peak_memory_bytes()` callers.

## Key signature

```rust
// In relativist-core/src/bench/memory.rs

/// Returns the peak resident memory (in bytes) at the moment of call. On Linux,
/// reads `/proc/self/status` VmHWM. Per SPEC-09 R18d, callers MUST invoke this
/// AFTER net construction completes and BEFORE reduction begins to obtain the
/// "peak_memory_during_construction" metric.
pub fn get_peak_memory_at_construction_complete() -> u64;
```

## Acceptance criteria

1. New probe function added with the exact signature above.
2. Probe is invoked in BOTH the sequential and grid paths in `bench/suite.rs` between net construction and reduction.
3. New field `peak_memory_at_construction_complete: u64` added to the result row struct and emitted to CSV.
4. Linux-gated test asserts the probe returns a non-zero value for `ep_annihilation` size 10000 and is ≤ the end-of-run peak (monotonicity).
5. Non-Linux test asserts the probe returns 0 (matches existing OS-availability convention).
6. CSV column is added without breaking the existing `v1_local_baseline` schema (new column is appended as a tail column).
7. All existing tests pass with zero regression.

## Test floor delta expected

**+3 to +5 tests** added.

## Notes

- The "monotonicity" assertion (construction ≤ end-of-run) is the simplest discriminating test; stronger acceptance gates (e.g., construction memory < total memory by some margin) belong in Phase F-2 baseline analysis, not here.
- Coordinate the CSV column ordering with the existing schema: append to the end, do not insert in the middle (defends `v1_local_baseline` cross-comparison).
