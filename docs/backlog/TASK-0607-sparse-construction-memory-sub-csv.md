# TASK-0607 — `sparse_construction_memory.csv` sub-writer (Phase D-3)

**Phase:** D-3 (D-011 SparseNet micro-bench — output)
**Bundle:** D-011 — Tier 3 Hardening + Bench Enablement
**Status:** PENDING
**Priority:** P1 (output channel for the Phase D micro-bench)
**Spec:** SPEC-09 R18a–R18g (committed `82b2d27`).
**Origin:** D-011 plan §D-3.
**Estimated complexity:** S (~40 LoC production + ~25 LoC tests)
**Estimated stages duration:** Stages 2→3→4→5→6 over ~0.25 day.

---

## Context

Per D-011 plan: the sparse micro-bench output goes to its **own** CSV (`sparse_construction_memory.csv`) so it does not pollute the main detail/rounds/summary CSVs. The columns are: `benchmark, size, representation, peak_construction_bytes, ratio_to_dense`.

The "ratio_to_dense" column is computed only when both runs (sparse + dense) are in the same suite invocation; otherwise it is left blank or NaN per the plan author's discretion (test-generator should pick a convention and document).

## Dependencies

- **TASK-0606 (D-1+D-2)** — REQUIRED. The sparse path produces the values this CSV emits.
- **TASK-0605 (C-5)** — REQUIRED. The `peak_construction_bytes` value comes from the C-5 probe.

## Files in scope

| File | Change |
|------|--------|
| `relativist-core/src/bench/suite.rs` (CSV emit path) | Add a `sparse_construction_memory_csv_path: Option<String>` field consumer; emit one row per (benchmark, size, representation) tuple to the path when configured. |
| `relativist-core/src/bench/mod.rs` (`BenchmarkSuiteConfig`) | Add field `sparse_construction_memory_csv_path: Option<String>`. (Note: this is *additional* to the C-1 fields; flag this as a dependency on TASK-0602.) |
| `relativist-core/src/config.rs` (`BenchArgs`) | Add CLI flag `--sparse-csv-path <PATH>`. |
| `relativist-core/tests/spec09_sparse_csv_writer.rs` (new) | Integration: run sparse + dense for dual_tree, write to a temp file, parse with csv crate, assert columns and values. |

## Files explicitly OUT of scope

- Existing detail/rounds/summary CSV writers — unchanged.
- The probe itself — TASK-0605.
- The bench branch — TASK-0606.

## CSV schema

```csv
benchmark,size,representation,peak_construction_bytes,ratio_to_dense
dual_tree,5000,dense,123456789,1.0
dual_tree,5000,sparse,98765432,0.80
dual_tree,50000,dense,9876543210,1.0
dual_tree,50000,sparse,7890123456,0.80
```

## Acceptance criteria

1. New optional CSV path on `BenchmarkSuiteConfig` and corresponding CLI flag.
2. When the path is configured AND the bench run includes a sparse representation row, one row per (benchmark, size, representation) tuple is emitted.
3. `ratio_to_dense` is `1.0` for dense rows and `(sparse_bytes / dense_bytes)` for sparse rows in the same (benchmark, size) pair; left blank or `NaN` when no dense pair is in the run (test-generator picks; document).
4. CSV is parseable by the `csv` crate (no schema drift).
5. Existing CSV outputs unchanged.
6. New parse-from-CSV test confirms schema.

## Test floor delta expected

**+2 to +3 tests** added.

## Notes

- The "ratio_to_dense" field is the headline metric for Phase F-2 narrative; correctness of the ratio computation is the QA Stage 5 focus.
- ~1 page in `docs/DATA-COLLECTION-PLAN.md` describing the sub-CSV — this docs entry is part of the Phase F-3 close-out, not this task.
