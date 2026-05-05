# TEST-SPEC-0618 — Tests for TASK-0618 — D-011-FU-MIPS: decide implement-or-drop for `mips_*` and `total_interactions` columns

**Task:** TASK-0618 (D-012 Instrumentation Restore — Stage 3 DEV scope; LOW severity; independent).

> **D-012 Stage 6 REFACTOR amendment (2026-05-05).** Two amendments:
>
> 1. **Wrong-layer correction (QA-D012-002).** The Stage 3 commit body
>    described "Closes RF-07" but the literal `mips_mean = 0.000` failure
>    mode shipped from `scripts/bench_docker_v2.sh:283`, where a Python
>    embedded-in-bash literal hardcoded the column to `0.0`. The original
>    IT-0618-A1 witness ran `run_grid` and stopped at the in-memory
>    `GridMetrics` — it could never observe the bash hardcode. The Stage 6
>    refactor (a) fixes `bench_docker_v2.sh:283` to recompute `mips_mean`
>    from the per-rep `total_interactions` values (matching detail.csv's
>    formula), and (b) adds IT-0618-A4 in `tests/d012_mips_witness.rs`
>    that exercises `bench::suite::run_benchmark_suite` end-to-end through
>    the CSV writer. Path-(a) is closed at both layers (Rust + bash).
> 2. **Coverage gap (reviewer SF-004).** A new IT-0618-A3
>    (`single_worker_path_records_nonzero_total_interactions`) covers the
>    `run_single_worker` aggregation site at `merge/grid.rs:1466+` (which
>    uses `=` instead of `+=` and pushes 0 to `border_interactions_per_round`).
>    Many bench profiles use `workers=1` for a parallel-execution
>    baseline; without A3, that path was unwitnessed.
> 3. **Naming honesty (reviewer SF-003).** IT-0618-A1 is renamed from
>    `tcp_round_records_nonzero_total_interactions_and_mips` to
>    `metrics_struct_records_nonzero_total_interactions_and_mips` because
>    the body uses `run_grid` (in-process), not a TCP round.
>
> See `docs/qa/QA-D012-instrumentation-restore-2026-05-05.md` QA-D012-002
> and `docs/reviews/REVIEW-D012-instrumentation-restore-2026-05-05.md`
> MF-002 / SF-003 / SF-004 for full rationale.
**Spec:** none for path (b). Path (a) is purely additive instrumentation reusing TASK-0616's payload extension if both pick path (a).
**Closes red flag:** RF-07 (`docs/analysis/D011-final-baseline-analysis-2026-05-04.md` §3 RF-07, lines 174–179) — `mips_mean = 0.000` and `total_interactions = 0` everywhere on **both** v1 and v2 (symmetric pre-existing dead column).
**Origin:** `docs/handoffs/2026-05-05-D012-instrumentation-restore-handoff.md` §3 D-011-FU-MIPS + `docs/backlog/TASK-0618-d011-fu-mips-decide-implement-or-drop.md` Decision-required section.
**Test floor delta:** **+1 default** (one new integration-test binary; the developer picks the appropriate test file name based on path choice — `d012_mips_witness.rs` for path (a) or `d012_mips_columns_dropped.rs` for path (b)). Zero-copy and streaming-no-recycle floors unchanged.
**Prerequisites:**
- Path (a): if TASK-0616 also picks path (a), coordinate to extend `PartitionResult` ONCE with both `compute_duration` and `total_interactions` (per TASK-0618 Implementation hint 1). Sequencing: TASK-0616 lands first, this task piggybacks.
- Path (b): no code prerequisites.
- TASK-0617 not strictly required.

---

## Decision contract

The developer MUST pick path (a) "implement" OR path (b) "drop" and document the choice in the implementation commit body (TASK-0618 Acceptance criterion 1). The TEST-SPEC covers BOTH paths so neither implementation goes uncovered:

| Path | Test files | Test IDs |
|---|---|---|
| (a) — implement total_interactions end-to-end | `relativist-core/tests/d012_mips_witness.rs` | IT-0618-A1, IT-0618-A2 |
| (b) — drop the columns from CSVs | `relativist-core/tests/d012_mips_columns_dropped.rs` | IT-0618-B1, IT-0618-B2 |

The unused path's tests are NOT created (this differs from TEST-SPEC-0616's `#[ignore]` approach because for path (b) the fields they reference no longer exist, so the test wouldn't compile). The TEST-SPEC documents both for **future migration symmetry**: if a future bundle reverses the decision, the TEST-SPEC of the now-needed path is already written and ready to implement.

---

## Test inventory — Path (a): implement

| test_id | level | target file::test_name | prerequisite TASK | cfg gates |
|---|---|---|---|---|
| IT-0618-A1 | integration | `relativist-core/tests/d012_mips_witness.rs::tcp_round_records_nonzero_total_interactions_and_mips` | TASK-0616 (if path-(a)) for payload coordination | none |
| IT-0618-A2 | integration | `relativist-core/tests/d012_mips_witness.rs::total_interactions_equals_sum_of_per_round_interactions` | none | none |

## Test inventory — Path (b): drop

| test_id | level | target file::test_name | prerequisite TASK | cfg gates |
|---|---|---|---|---|
| IT-0618-B1 | integration | `relativist-core/tests/d012_mips_columns_dropped.rs::summary_csv_headers_omit_dropped_columns` | none | none |
| IT-0618-B2 | integration | `relativist-core/tests/d012_mips_columns_dropped.rs::no_test_or_struct_field_references_dropped_names` | none | none |

**Totals (per path):** 0 UT, 2 IT, 0 PT. Net floor delta per path: **+1 default** (single new integration binary with 2 tests).

---

## Per-test specifications — Path (a)

### IT-0618-A1 — `tcp_round_records_nonzero_total_interactions_and_mips` (path (a))

**Purpose.** Headline acceptance witness for path (a) closure of RF-07. After a TCP-mode benchmark completes a non-trivial workload, `summary.csv::mips_mean > 0` AND `detail.csv::total_interactions > 0`. Pre-fix HEAD: BOTH zero. Post-fix HEAD on path (a): BOTH non-zero.

**Setup.**
- Coordinator + 1 worker over localhost TCP.
- Workload: `dual_tree(depth = 7)` or `ep_annihilation(256)` — guarantees ≥ 100 reductions so `total_interactions` is comfortably non-zero and `mips` is meaningful.
- Run a complete bench (not just 1 round) so the `summary.csv` and `detail.csv` writers fire.
- Use the existing `bench/suite.rs` harness with `repetitions >= 2` (the median computation needs at least 2 reps).

**Action.**
1. Run the bench end-to-end.
2. Read the produced `summary.csv` and `detail.csv` from the bench's output directory.
3. Parse the rows. Use `csv::Reader` or simple line parsing.

**Assertions.**
1. `summary.csv` exists and has at least 1 data row.
2. `summary.csv::mips_mean > 0.0` for the row matching the test's bench slot.
3. `detail.csv::total_interactions > 0` for at least one row matching the test's bench slot.
4. `mips_mean ≈ total_interactions / wall_clock_secs * 1e-6` within 5% — the formula `bench/suite.rs::aggregate` uses must yield consistent values. (TASK-0618 Implementation hint 3: "Do NOT touch `bench/suite.rs::aggregate`'s formula" under path (a); this assertion guards that the formula is still applied.)
5. **Sanity bounds on mips:** `mips_mean ∈ [0.001, 1000.0]` — at least 1 KOP/s, at most 1 GOP/s, on a representative laptop CPU. Guards against unit-confusion bugs (e.g., reporting interactions/s instead of mega-interactions/s, off by 1e6).

**Failure message contract.** On (2)/(3) failure, panic message MUST cite "RF-07 path (a)" and `docs/analysis/D011-final-baseline-analysis-2026-05-04.md §3 RF-07`. On (4) failure, panic must report `mips_mean`, `total_interactions`, `wall_clock_secs`, and the computed-vs-stored ratio.

**Boundary case coverage.**
- A workload that produces ≥ 100 reductions ensures `total_interactions` is non-trivially > 0; smaller workloads risk timer-resolution noise.
- Sanity bounds (5) catch unit-confusion at order-of-magnitude.

**Why it must exist.** Direct closure of TASK-0618 path (a) acceptance criterion: "`summary.csv::mips_mean > 0` for any non-trivial bench."

**Constraint coverage.** Operator prompt: "For (a): assert `mips_mean > 0` for any non-trivial bench" — IT-0618-A1 (load-bearing).

---

### IT-0618-A2 — `total_interactions_equals_sum_of_per_round_interactions` (path (a))

**Purpose.** Path-(a)-specific aggregation correctness. The coordinator-side `BenchmarkResult.total_interactions` MUST equal the sum of per-round interaction counts across all workers, all rounds. If aggregation is broken (e.g., overwrites instead of sums; or per-round count is double-counted), this test fires.

**Setup.**
- Coordinator + 2 workers over localhost TCP.
- Workload: `ep_annihilation(256)` — known per-redex count from the generator (the `ep_annihilation(N)` benchmark is documented to produce N redexes deterministically; verify against `relativist-core/src/bench/` source for the exact formula).
- 1 repetition (so the bench has a single `BenchmarkResult`).

**Action.**
1. Run the bench.
2. Capture `BenchmarkResult.total_interactions` (the coordinator's aggregate).
3. Capture per-round, per-worker interaction counts (this requires the test to either intercept worker messages or read from a debug field on `GridMetrics`; the developer must expose at minimum `metrics.interactions_per_round: Vec<u64>` summed across workers).

**Assertions.**
1. `result.total_interactions > 0`.
2. **Sum invariant:** `result.total_interactions == metrics.interactions_per_round.iter().sum::<u64>()`. This is **exact equality**; interaction counts are integers, not floats. No tolerance band.
3. **Per-worker symmetry (sanity):** for each round, the per-round total equals the sum of per-worker contributions for that round (also exact equality). If the test cannot easily access per-worker counts, this assertion can be marked `// TODO: requires per-worker debug field` and deferred to a separate task; the developer should still satisfy assertion (2).
4. **Workload sanity:** `result.total_interactions` matches the workload's expected redex count within ±5% (the small drift accounts for any commutation that produces additional redexes mid-run; for `ep_annihilation` the value should be exact at depth-N).

**Failure message contract.** On (2) failure, panic message MUST report both sides of the equation and the difference.

**Boundary case coverage.**
- W=2 workers ensures the aggregation isn't trivially correct via "single worker passes through".
- Multi-round workload (depth-7 dual_tree gives ~30 rounds) ensures the per-round vec is properly accumulated.

**Why it must exist.** Constraint coverage: "For (a): ... assert `total_interactions == sum_of_per_round_interactions`" — IT-0618-A2 (load-bearing).

---

## Per-test specifications — Path (b)

### IT-0618-B1 — `summary_csv_headers_omit_dropped_columns` (path (b))

**Purpose.** Headline acceptance witness for path (b) closure of RF-07. After running a benchmark, the produced `summary.csv` and `detail.csv` no longer contain the dropped columns (`mips`, `mips_mean`, `mips_std`, `total_interactions`, etc.).

**Setup.**
- Coordinator + 1 worker over localhost TCP (or in-process; either is fine — the CSV writer is mode-agnostic).
- Workload: any small benchmark (e.g., `ep_annihilation(64)`); the test only inspects the CSV schema, not the data.
- Run with `repetitions = 1`.

**Action.**
1. Run the bench.
2. Read `summary.csv` first line (header row). Parse via `csv::Reader::headers()` or simple `.split(',')`.
3. Read `detail.csv` first line (header row).

**Assertions.**
1. `summary.csv` headers do NOT contain ANY of: `mips`, `mips_mean`, `mips_median`, `mips_std`, `mips_min`, `mips_max`, `mips_p50`, `mips_p95`, `mips_p99`, `total_interactions`, `total_interactions_mean`, etc. The test enumerates the exact column names being dropped (developer fills them in based on the actual pre-fix headers; the spec just mandates "no `mips_*` and no `total_interactions` substring").
2. `detail.csv` headers likewise do NOT contain `total_interactions` or `mips`.
3. **Positive control:** at least one EXPECTED header is still present (e.g., `wall_clock_secs`, `bytes_sent`, `network_time_secs`). This guards against a bug where the developer accidentally truncates the entire header row.
4. **Test stability:** the test reads the actual produced CSV file, not a hardcoded copy of the header. If the CSV writer changes its column ordering or adds new columns, the test still passes as long as the dropped columns remain absent.

**Failure message contract.** On (1)/(2) failure, panic message MUST list the offending column name and the full header row, so the developer can see at a glance which writer site they missed.

**Boundary case coverage.** Substring matching ("contains `mips_`" rather than "exact equals `mips_mean`") catches partial-removal bugs (e.g., dropping `mips_mean` but leaving `mips_median`).

**Why it must exist.** Direct closure of TASK-0618 path (b) acceptance criterion: "`summary.csv` headers no longer contain `mips_mean` / `total_interactions` / related; `detail.csv` likewise."

**Constraint coverage.** Operator prompt: "For (b): assert `summary.csv` headers no longer contain the dropped columns" — IT-0618-B1 (load-bearing).

---

### IT-0618-B2 — `no_test_or_struct_field_references_dropped_names` (path (b))

**Purpose.** Path-(b)-specific completeness witness. After dropping columns from the CSV writers, the corresponding fields on `BenchmarkResult` and `AggregatedStats` (and any tests asserting on them) must also be removed (or `Option`-ized + skipped). Compile-time verification covers most of this (Rust would error on a removed field), but a residual `#[ignore]`-gated test or a struct field renamed-but-unused could escape.

**Setup.**
- This is a **CI lint test**, not a runtime test. Implement it as either:
  - **Option A** (preferred, lower-friction): a `#[test]` function that uses `include_str!` to read the project's source files and asserts via simple string scanning.
  - **Option B**: a `build.rs` or CI shell-out that runs `grep -rn "mips_mean\|total_interactions" relativist-core/src` and fails the build if any matches are found in production code (test code may legitimately reference the old names if they're in commit-history regression assertions; scope to `src/` only).

**Setup details (Option A):** in the test file, hard-code the path to `relativist-core/src/bench/mod.rs`, `relativist-core/src/bench/csv.rs`, `relativist-core/src/bench/validate.rs`, and any other suspected sites. Read each via `include_str!` (compile-time embedded). Search via `.contains("mips_mean")` etc.

**Action.** Static string scan over the embedded source.

**Assertions.**
1. None of `relativist-core/src/bench/{mod,csv,validate}.rs` contain the substring `"mips_mean"`, `"mips_median"`, `"mips_std"`, `"total_interactions"`, etc. (same enumeration as IT-0618-B1).
2. No struct definition in those files declares a field named `total_interactions`, `mips_mean`, etc. (heuristic: `"total_interactions:"` followed by a Rust type would catch a field; `.contains` is fine for the spec's purposes).
3. **Positive control:** the test's source-scan harness itself works — assert that at least one expected substring (e.g., `"BenchmarkResult"`) IS found in `bench/mod.rs`. Guards against `include_str!` returning empty due to a path bug.

**Allowed exceptions.** Test files (`relativist-core/tests/`) and historical comments (e.g., `// removed mips_mean in TASK-0618`) are exempt from the scan. The developer may add `// allowed: TASK-0618 history` markers if any test legitimately references the old names; the scan SHOULD respect those markers (or the test simply scopes to `src/` only and ignores comments — cleaner).

**Failure message contract.** On (1)/(2) failure, panic message reports the file:line where the offending substring was found.

**Boundary case coverage.** Comments and string literals that mention the dropped names (in commit history or `tracing::warn!` messages) are explicitly allowed via the `// allowed: TASK-0618 history` marker convention; this prevents the test from flaking when the developer adds a deprecation log.

**Why it must exist.** Constraint coverage: "For (b): ... assert no test references the removed fields." IT-0618-B2 (load-bearing). Without this test, a `BenchmarkResult.total_interactions` field that's no longer written-to but still present (and zero-valued) silently passes IT-0618-B1.

**Note for reviewer.** This kind of "scan source for forbidden substrings" test is brittle if the codebase grows. The developer may prefer to wire it as a `cargo deny`-style lint or a `clippy` lint config rather than a `#[test]`. Either implementation is acceptable as long as the assertion bullets above are satisfied; the TEST-SPEC does not pin the mechanism.

---

## Notes

### Path coordination with TASK-0616

If TASK-0616 picks path (a), the same `PartitionResult` extension carries `compute_duration` (TASK-0616) and `total_interactions` (this task). Coordinate so only ONE wire-format extension happens. Sequencing: TASK-0616 lands first, this task piggybacks. Reviewer should verify the developer didn't extend the payload twice.

### Determinism strategy

Path (a) tests use real-clock measurement for the `mips` denominator; the 5% sanity band absorbs scheduler jitter. Path (b) tests are static-string scans — fully deterministic, no flake risk.

### What this test does NOT cover

- **Network time / compute time** — TEST-SPECs 0615 / 0616.
- **v1's frozen `summary.csv`** — explicitly OUT of scope per TASK-0618 ("v1's `summary.csv` (frozen at `results/locked/v1_local_baseline/`) retains `mips_mean = 0.000` columns. This is expected and not in scope."). No test asserts on v1's frozen CSV.
- **TCC artigo column drops** — REDATOR territory (§6 verdict item 2 of the analysis is TCC-side guidance, not DEV-side).
- **Wire format compatibility** — if path (a) requires a PROTOCOL_VERSION bump, escalate to ESPECIALISTA EM SPECS first.

### Coverage of constraints from the operator prompt

| Constraint | Path | Where |
|---|---|---|
| For (a): assert `mips_mean > 0` for any non-trivial bench | (a) | IT-0618-A1 (load-bearing) |
| For (a): assert `total_interactions == sum_of_per_round_interactions` | (a) | IT-0618-A2 (load-bearing) |
| For (b): assert `summary.csv` headers no longer contain the dropped columns | (b) | IT-0618-B1 (load-bearing) |
| For (b): assert no test references the removed fields | (b) | IT-0618-B2 (load-bearing) |

### Cfg gates

None. Recommend re-running under `cargo test --release` post-TASK-0617.

---

## Cross-references

- **Source task:** `docs/backlog/TASK-0618-d011-fu-mips-decide-implement-or-drop.md`.
- **Bundle handoff:** `docs/handoffs/2026-05-05-D012-instrumentation-restore-handoff.md` §2 row 4, §3 D-011-FU-MIPS subsection.
- **Red flag:** `docs/analysis/D011-final-baseline-analysis-2026-05-04.md` §3 RF-07 (lines 174–179), §6 verdict item 2 (TCC-side framing).
- **Companion TEST-SPEC:** TEST-SPEC-0616 (compute-time companion; payload coordination opportunity if both pick path (a)).

---

## Coverage matrix

| test_id | Path | RF-07 closure | Aggregation correctness | Schema removal | Source-cleanliness |
|---|---|---|---|---|---|
| IT-0618-A1 | (a) | ✅ (load-bearing) | partial | — | — |
| IT-0618-A2 | (a) | ✅ | ✅ (load-bearing) | — | — |
| IT-0618-B1 | (b) | ✅ (load-bearing) | — | ✅ (load-bearing) | — |
| IT-0618-B2 | (b) | ✅ | — | partial | ✅ (load-bearing) |

---

## Out-of-scope (explicitly NOT specified here)

- Network time / compute time — TEST-SPECs 0615 / 0616.
- Release-mode test compilation — TEST-SPEC-0617.
- TCC artigo edits (REDATOR territory).
- v1 frozen `summary.csv` patching — explicitly preserved as "dead column" expected.
- Frozen baselines under `results/locked/`.
- PROTOCOL_VERSION bump (escalate to ESPECIALISTA EM SPECS if path (a) requires it).
