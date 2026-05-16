# TASK-0703 — D-014-CSV: extend CSV schema with `vmrss_*`, `stop_reason`, `cv_above_gate`

**Phase:** D-014 (Stress Curve Campaign) — Stage 3 DEV scope
**Bundle:** D-014 — Stress Curve Campaign
**Status:** TODO
**Priority:** P1 (consumed by plot generator and aggregator; non-blocking for descriptor smoke).
**Spec:** none.
**Depends on:** TASK-0700 (`MemoryProbe`), TASK-0701 (`StopRule` defines `StopReason`).
**Estimated complexity:** S (~30 LoC production + ~30 LoC unit test).

---

## Context

Existing bench CSVs (post-D-012) have ~25 columns covering wall, mips, network/compute time decomposition, and the SPEC-09 R18a peak-at-construction. The stress-curve campaign needs 4 additional columns — and only those, no removals or renames:

| Column | Type | Source |
|---|---|---|
| `vmrss_peak_mb` | f64 (MiB) | `MemoryProbe::peak_bytes() / (1024 * 1024)` at end-of-rep |
| `vmrss_current_end_mb` | f64 (MiB) | `MemoryProbe::current_bytes() / (1024 * 1024)` at end-of-rep |
| `stop_reason` | string (`""` if normal, `"WallTimeExceeded"` / `"MemoryExceeded"` / `"Oom"` for sentinel) | `StopReason` via `serde::Serialize` |
| `cv_above_gate` | bool (`true`/`false`) | `cv > 0.05` flag, computed in the existing post-rep aggregator |

The 4 columns are **append-only**: they go at the end of the row. Existing baselines (D-010, D-011, D-012) read fixed-position columns and MUST NOT break. Verify by re-running the existing CSV-roundtrip tests after the change.

## Files in scope (file:line pointers)

| File | Change |
|------|--------|
| `relativist-core/src/bench/csv.rs` | **MODIFY.** Append 4 fields to the row struct. Update the `csv::Writer` invocation to serialize them. ~30 LoC. (Note: SPEC-09 / D-012 baseline file is `bench/csv.rs`, not `csv_writer.rs` as design doc names it. The design doc filename is aspirational; honor existing layout.) |
| `relativist-core/src/bench/suite.rs` | **MODIFY.** Plumb the 4 new fields into the row struct's construction site (post-rep, end of bench loop). ~10 LoC. |
| `relativist-core/tests/d014_csv_schema_roundtrip.rs` | **CREATE.** Roundtrip test: write a row with all 4 new fields populated, read it back, verify values + types preserved. Plus a "legacy reader" test that reads a row written by THIS code with a struct that ignores the 4 new fields (forward compatibility). ~30 LoC. |

## Files explicitly OUT of scope

- The plot generator — TASK-0705 reads the new columns by name.
- Frozen `results/locked/v2_post_d012_baseline_2026-05-05/` — read-only forever.
- Any change to existing column names or order.
- Header comments in CSVs (cosmetic; not part of the data contract).

## Required schema delta (additive, end of row)

```text
... existing 25 columns from D-012 ... ,
vmrss_peak_mb,vmrss_current_end_mb,stop_reason,cv_above_gate
```

`stop_reason` is the empty string when the row is a normal rep; non-empty when the row is the sentinel emitted by the harness after a `StopRule::check` fires.

`cv_above_gate` is computed at the same call site as the existing `mips`/`wall` aggregation: `(stddev / mean) > 0.05`.

## Acceptance criteria

1. New rows have exactly 4 additional columns at the end of the row, matching the names above.
2. `stop_reason` serializes as `""` for normal reps and `"WallTimeExceeded" / "MemoryExceeded" / "Oom"` for sentinel rows.
3. `cv_above_gate` serializes as `true` / `false`.
4. Roundtrip test writes a row with `vmrss_peak_mb = 123.4` and reads `123.4` back (within `f64::EPSILON * 16`).
5. **Legacy reader test:** a struct deserializing only the original 25 columns reads any row produced by this code without error (csv crate's default is forward-compatible — verify explicitly).
6. Existing pre-D-014 CSV-related tests still pass unmodified — zero regression on the existing rows produced for D-010/D-011/D-012 datasets.
7. `cargo test` floor: **+2 default = ≥ 1811** (cumulative TASK-0700..0703).
8. `cargo test --features zero-copy` floor: **+2 = ≥ 1855**.
9. `cargo test --features streaming-no-recycle` floor: **+2 = ≥ 1802**.
10. `cargo test --release` floor: **+2 = ≥ 1753**.
11. v1 floor (690) inviolable.
12. `cargo clippy --all-features -- -D warnings` clean.
13. `cargo fmt --check` clean.

## Test floor delta

**+2 default** (one integration test file with 2 tests). Cumulative after TASK-0700..0703:
- default ≥ 1811
- zero-copy ≥ 1855
- streaming-no-recycle ≥ 1802
- release ≥ 1753

## Implementation hints

1. The existing `bench/csv.rs` uses `csv::Writer<File>` plus `serde::Serialize` derive on the row struct. Just add the 4 fields with `#[serde(rename = "...")]` if naming differs from the field name. Cargo handles the rest.
2. For `stop_reason: Option<StopReason>`, derive serialization to empty string for `None`. The simplest path: use `Option<String>` in the row struct and stringify at the call site; serde defaults `None` → empty in the csv crate.
3. Keep the existing column order. Adding to the end is the only safe change.
4. For `cv_above_gate`, derive in the row builder, not at write time — that way the value is also visible in the BenchmarkResult struct for in-memory tests.
5. Document the schema change in a doc-comment on the row struct ("`vmrss_*` and `stop_reason` added 2026-05-06 for D-014 stress-curve campaign").
6. DO NOT delete the SPEC-09 R18a `peak_memory_at_construction_mb` column — it remains separate from `vmrss_peak_mb`. Different call site, different semantics; the campaign uses the new column, but the baseline regression bench keeps the old.

## Estimated LoC

- Production: ~30 LoC (4 fields + serialization + plumbing).
- Tests: ~30 LoC (roundtrip + legacy reader).
- Total: ~60 LoC. Trivially under 200 LoC.

## Cross-references

- Design doc: `docs/superpowers/specs/2026-05-05-stress-test-large-nets-design.md` §4.2 row 4, §4.4 (`metrics:` enumeration).
- Consumes: TASK-0700 (`MemoryProbe::peak_bytes`/`current_bytes`), TASK-0701 (`StopReason`).
- Consumed by: TASK-0705 (plot generator reads by column name), TASK-0708 (campaign aggregator).
- Existing CSV peer module: `relativist-core/src/bench/csv.rs` (D-012 baseline).
