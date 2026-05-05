# TEST-SPEC-0607 — Tests for TASK-0607 — `sparse_construction_memory.csv` sub-writer

**Task:** TASK-0607 (Phase D-3, P1)
**Spec:** SPEC-09 R18a–R18g + §3.4.5 (committed `82b2d27`).
**Origin:** D-011 plan §D-3 — output channel for the Phase D micro-bench.
**Test floor delta:** **+3 default** (1 schema + 1 ratio computation + 1 isolated-row convention).
**Prerequisites:** TASK-0606 (sparse path produces values), TASK-0605 (probe).

---

## Test inventory

| test_id | level | target file | prerequisite TASK | cfg gates |
|---|---|---|---|---|
| UT-0607-01 | unit | `relativist-core/src/bench/csv_writers.rs::tests::sparse_csv_header_matches_spec_09_section_3_4_5` | none | none |
| IT-0607-02 | integration | `relativist-core/tests/spec09_sparse_csv_writer.rs::full_run_emits_correct_rows_and_ratio` | TASK-0606 | `#[cfg(target_os = "linux")]` |
| IT-0607-03 | integration | `relativist-core/tests/spec09_sparse_csv_writer.rs::sparse_only_run_leaves_ratio_blank` | TASK-0606 | none |
| IT-0607-04 | integration | `relativist-core/tests/spec09_sparse_csv_writer.rs::existing_csv_outputs_unchanged_when_sparse_csv_path_set` | none | none |

Total: **3 default tests + 1 Linux-gated** (effective +3 floor).

Conservative floor delta: **+3 default**.

---

## Per-test specifications

### UT-0607-01 — `sparse_csv_header_matches_spec_09_section_3_4_5`

**Purpose.** Lock the CSV header line to the EXACT column order mandated by SPEC-09 §3.4.5 (committed `82b2d27`). The columns are: `benchmark, size, representation, peak_memory_during_construction, ratio_to_dense`.
**Setup.** Construct an empty `SparseConstructionMemoryCsvWriter` (or whatever the production type is named) writing to an in-memory buffer (`Vec<u8>`).
**Action.** Write the header only (no data rows); flush; read back the buffer as UTF-8.
**Assertions.**
- The first line is **literally** `benchmark,size,representation,peak_memory_during_construction,ratio_to_dense\n` (column names per SPEC-09 R18a; canonical metric name `peak_memory_during_construction` per line 482 of SPEC-09 amendment commit `82b2d27`).
- Column count == 5.
- No leading BOM, no trailing whitespace before `\n`.
- The columns parse correctly via `csv::ReaderBuilder::new().has_headers(true).from_reader(...)`.
**Boundary case coverage.** Catches a header drift where someone uses the legacy `peak_construction_bytes` column name (the task description originally uses this name in §CSV schema, but the SPEC-09 R18a canonical name supersedes — they are the SAME metric, but the column header MUST be the SPEC-09 canonical name).
**Why it must exist.** Acceptance criterion #6 (CSV is parseable; schema confirmed). Locks the exact column names against drift.

**Critical implementation note.** The task description's §CSV schema example uses `peak_construction_bytes` as the column name. **The SPEC-09 R18a canonical name (`peak_memory_during_construction`) wins** per the dispatch brief explicit instruction ("Use the canonical metric name `peak_memory_during_construction` per SPEC-09 R18a, line 482"). Stage 3 developer must use the SPEC-09 name in the production code; this test enforces it.

---

### IT-0607-02 — `full_run_emits_correct_rows_and_ratio`

**Purpose.** End-to-end: a bench suite invocation with both `Dense` and `Sparse` runs at `dual_tree(5000)` produces a sub-CSV with 2 rows, correct memory values, and a correctly computed `ratio_to_dense`.
**Setup.**
- Create a temp file path for the sub-CSV (`tempfile::NamedTempFile` or equivalent).
- Configure `BenchmarkSuiteConfig` with `benchmark = DualTree`, `sizes = [5000]`, `representations = [Dense, Sparse]`, `sparse_construction_memory_csv_path = Some(temp_path)`.
**Action.** Invoke the suite; read and parse the temp file via the `csv` crate.
**Assertions.**
- The CSV has exactly 2 data rows.
- Row 1: `benchmark = "dual_tree"`, `size = 5000`, `representation = "dense"`, `peak_memory_during_construction > 0`, `ratio_to_dense == 1.0`.
- Row 2: `benchmark = "dual_tree"`, `size = 5000`, `representation = "sparse"`, `peak_memory_during_construction > 0`, `ratio_to_dense ≈ sparse_peak / dense_peak` (assert with `(ratio - sparse/dense).abs() < 1e-6`).
- Row 2's `ratio_to_dense < 0.80` (the headline acceptance gate; consistency with TASK-0606 IT-0606-04).
- No extra columns, no missing columns.
**Boundary case coverage.** Catches a buggy ratio computation (e.g. dividing in the wrong direction, or using the wrong dense value when multiple sizes are present).
**cfg gate.** `#[cfg(target_os = "linux")]` (the probe returns 0 on non-Linux, which would make the ratio meaningless).
**Why it must exist.** Acceptance criterion #2 + #3 (one row per (benchmark, size, representation) tuple; `ratio_to_dense` correctly computed).

---

### IT-0607-03 — `sparse_only_run_leaves_ratio_blank`

**Purpose.** Convention test: when the suite invocation includes ONLY a sparse run for a (benchmark, size) pair (no dense pair to compare against), the `ratio_to_dense` column is blank or `NaN` per the test-generator's chosen convention.
**Setup.**
- Configure `representations = [Sparse]` only (no Dense).
- Same temp-file mechanism.
**Action.** Run; parse the CSV.
**Assertions.**
- The CSV has exactly 1 data row.
- Row 1: `representation = "sparse"`, `peak_memory_during_construction > 0` (Linux) or `== 0` (non-Linux).
- **Convention chosen by test-generator:** the `ratio_to_dense` field is the empty string `""` (blank), NOT `NaN`. Rationale: blanks are forward-compatible with downstream CSV consumers (pandas/polars/Excel) without requiring NaN-aware parsing; `NaN` requires the consumer to know the column is float and handle missing data explicitly.
- Test asserts: `row.get("ratio_to_dense") == Some("")`.
- The `csv` crate parses the row without error.
**Boundary case coverage.** Catches a buggy implementation that emits `0.0` (which would be indistinguishable from "sparse used zero memory" — a false success signal) or `inf` (division-by-zero from missing dense baseline).
**Why it must exist.** Acceptance criterion #3 ("left blank or `NaN` when no dense pair is in the run; test-generator picks; document"). This test BOTH picks the convention (blank) AND enforces it.

**Convention rationale (locked here):** blank string `""` is canonical for missing `ratio_to_dense`. Stage 3 developer must implement accordingly.

---

### IT-0607-04 — `existing_csv_outputs_unchanged_when_sparse_csv_path_set`

**Purpose.** Side-effect isolation: setting `sparse_construction_memory_csv_path = Some(...)` MUST NOT affect the existing detail/rounds/summary CSV outputs.
**Setup.**
- Configure a bench run that emits all four CSVs: detail, rounds, summary, sparse.
- Run twice with identical seeds: once WITH `sparse_construction_memory_csv_path = Some(...)`, once WITHOUT (`None`).
- Capture detail/rounds/summary CSV bytes from both runs.
**Action.** Compare the detail/rounds/summary CSV bytes from the two runs.
**Assertions.**
- `detail_with == detail_without` (byte-for-byte).
- `rounds_with == rounds_without`.
- `summary_with == summary_without`.
- The sparse CSV exists in the WITH run, does NOT exist in the WITHOUT run.
**Boundary case coverage.** Catches a buggy refactor that accidentally writes sparse-related columns into the detail CSV (cross-contamination).
**Why it must exist.** Acceptance criterion #5 (existing CSV outputs unchanged).

---

## Coverage matrix

| test_id | AC-1 (path field) | AC-2 (one row per tuple) | AC-3 (ratio computation) | AC-4 (csv parseable) | AC-5 (existing unchanged) | AC-6 (parse-from-CSV) |
|---|---|---|---|---|---|---|
| UT-0607-01 | | | | ✅ | | ✅ |
| IT-0607-02 | ✅ | ✅ | ✅ | ✅ | | ✅ |
| IT-0607-03 | ✅ | ✅ | ✅ | ✅ | | ✅ |
| IT-0607-04 | ✅ | | | | ✅ | |

Every acceptance criterion has ≥1 test.

---

## Out-of-scope tests (deferred to other tasks)

- The probe itself — TASK-0605.
- The bench branch — TASK-0606.
- Documentation in `docs/DATA-COLLECTION-PLAN.md` describing the sub-CSV — Phase F-3 close-out per task §Notes.

---

## Known spec ambiguity (adversarial flag)

- **CRITICAL — column name conflict.** The task description's §CSV schema example uses `peak_construction_bytes`, but SPEC-09 R18a (committed `82b2d27`, line 482) uses the canonical name `peak_memory_during_construction`. The dispatch brief is explicit: use the SPEC-09 canonical name. UT-0607-01 enforces this. **Flag for SDD-pipeline:** if the developer uses the task description's name verbatim, UT-0607-01 fails — this is intentional (the SPEC supersedes the task description on this exact point per the spec-amendment commit).
- The "blank vs NaN" convention is genuinely undecided by the spec. This test-spec **picks "blank"** and locks it. Stage 3 developer must implement blank-string emission, not NaN. If a downstream consumer reports they need NaN, this is a separate task to revisit the convention.
- SPEC-09 §3.4.5 lists the columns but does not specify quoting / escaping rules for the `representation` field (which contains a string). The test assumes lowercase unquoted `dense` / `sparse`. If production code emits `"Dense"` / `"Sparse"` (capitalized) or quoted strings, IT-0607-02 fails — flag for Stage 3 to use lowercase unquoted to match the SPEC-09 convention used in other CSVs.
