# TEST-SPEC-0604 — Tests for TASK-0604 — Bench harness path selection (eager vs streaming) + `ep_annihilation_stream` wiring

**Task:** TASK-0604 (Phase C-2 + C-4, P0 — central wiring task)
**Spec:** SPEC-09 R18a–R18g, R37c (commit `82b2d27`); SPEC-21 R23–R26, R37g; SPEC-01 G1 (fundamental property — distributed result equals local).
**Test floor delta:** **+10 default**.
**Prerequisites:**
- TASK-0602 (REQUIRED — `BenchmarkSuiteConfig` extended fields).
- TASK-0603 (RECOMMENDED for IT-0604-09 smoke, otherwise tests can build config in code).
- TASK-0597 (RECOMMENDED for streaming branch lifetime correctness; without it, IT-0604-04 may surface biased pending-store behavior).
- TASK-0596 (REQUIRED for `--mode tcp` integration; the local-path tests in this spec do not require it).

---

## Test inventory

| test_id | level | target file | prerequisite TASK | cfg gates |
|---|---|---|---|---|
| UT-0604-01 | unit | `relativist-core/src/bench/suite.rs::tests::path_selection_some_chunk_size_invokes_streaming_branch` | TASK-0602 | none |
| UT-0604-02 | unit | `relativist-core/src/bench/suite.rs::tests::path_selection_none_chunk_size_invokes_eager_branch` | TASK-0602 | none |
| UT-0604-03 | unit | `relativist-core/src/bench/suite.rs::tests::eager_branch_grid_config_carries_max_pending_lifetime` | TASK-0602, TASK-0597 | none |
| UT-0604-04 | unit | `relativist-core/src/bench/suite.rs::tests::streaming_branch_grid_config_carries_recycle_policy` | TASK-0602 | none |
| UT-0604-05 | unit | `relativist-core/src/bench/suite.rs::tests::ep_annihilation_streaming_dispatch_invokes_stream_impl` | TASK-0602 | none |
| IT-0604-06 | integration | `relativist-core/tests/spec09_bench_streaming_path.rs::streaming_path_produces_valid_csv_with_g1_passing` | TASK-0602, TASK-0596 | none |
| IT-0604-07 | integration | `relativist-core/tests/spec09_bench_streaming_path.rs::streaming_vs_eager_merged_net_isomorphism` | TASK-0602 | none |
| IT-0604-08 | integration | `relativist-core/tests/spec09_bench_eager_path_regression.rs::eager_path_output_bit_equivalent_to_baseline` | TASK-0602 | none |
| IT-0604-09 | integration | `relativist-core/tests/spec09_bench_streaming_path.rs::cli_smoke_chunk_size_100_completes_under_60s` | TASK-0602, TASK-0603 | none |
| PT-0604-10 | property | `relativist-core/tests/spec09_bench_streaming_path.rs::proptest_streaming_eager_isomorphic_for_random_chunk_sizes` | TASK-0602 | none |

Total: **10 default tests**.

---

## Per-test specifications

### UT-0604-01 — `path_selection_some_chunk_size_invokes_streaming_branch`

**Purpose.** Verify the path-selection branch in `bench/suite.rs:313-438`: `Some(N)` → streaming.
**Setup.**
- Construct a `BenchmarkSuiteConfig` with `chunk_size = Some(50)`, `max_pending_lifetime = 16`, `recycle_policy = DisableUnderDelta`, `representation = Dense`, benchmark = `EpAnnihilation`, size = 200, workers = 2.
- Inject a test-only branch tracker: a wrapper around `merge::generate_and_partition_chunked_with_chunk_size_and_lifetime` that records "called with chunk_size=N, lifetime=M". (Or: use a `tracing-test` subscriber capturing a span.)
**Action.** Invoke the bench dispatch entry point (the public API in `bench/mod.rs` or `bench/suite.rs` that today calls `run_grid` unconditionally).
**Assertions.**
- The streaming function is invoked exactly once with `chunk_size = 50`, `max_pending_lifetime = 16`.
- `run_grid` (eager) is NOT invoked in this run.
**Boundary case coverage.** Catches a buggy fix where the branch is added but inverted (`if chunk_size.is_none()` → streaming).
**Why it must exist.** Acceptance criterion #1 of TASK-0604.

---

### UT-0604-02 — `path_selection_none_chunk_size_invokes_eager_branch`

**Purpose.** Symmetric: `None` → eager `run_grid`.
**Setup.** Same as UT-0604-01 except `chunk_size = None`.
**Action.** Invoke bench dispatch.
**Assertions.**
- `run_grid` (eager) is invoked exactly once.
- The streaming generator is NOT invoked.
**Boundary case coverage.** Catches the silent regression where the eager path is accidentally rerouted through streaming, biasing `v1_local_baseline` cross-comparison.
**Why it must exist.** This is the headline regression guard called out in TASK-0604 §Notes ("the silent eager-path regression is the headline risk").

---

### UT-0604-03 — `eager_branch_grid_config_carries_max_pending_lifetime`

**Purpose.** Per TASK-0604 acceptance criterion #2: `config.max_pending_lifetime` propagates into `GridConfig` in BOTH branches (no `u32::MAX` in the bench path).
**Setup.** `BenchmarkSuiteConfig { chunk_size: None, max_pending_lifetime: 64, ... }`.
**Action.** Invoke bench dispatch; capture the `GridConfig` actually passed to `run_grid` (test-only hook or instrumented `tracing` event).
**Assertions.**
- The captured `GridConfig.max_pending_lifetime == 64`.
- It is NOT `u32::MAX`.
- It is NOT the `GridConfig::default()` value (unless the default itself is now correctly = `BenchmarkSuiteConfig`'s default of 16, which is a separate test).
**Boundary case coverage.** Catches a buggy fix that updates only the streaming branch.
**Why it must exist.** Acceptance criterion #2 of TASK-0604; pairs with TASK-0597's coverage at the legacy-caller level.

---

### UT-0604-04 — `streaming_branch_grid_config_carries_recycle_policy`

**Purpose.** Per TASK-0604 acceptance criterion #3: `config.recycle_policy` propagates into the streaming branch's `GridConfig`.
**Setup.** `BenchmarkSuiteConfig { chunk_size: Some(100), recycle_policy: RecyclePolicy::BorderClean, ... }`.
**Action.** Invoke; capture the `GridConfig` passed to the streaming generator.
**Assertions.**
- Captured `GridConfig.recycle_policy == BorderClean` (or whatever field the project uses to expose this — verify via SPEC-22 R10b).
**Boundary case coverage.** Catches a buggy fix that hard-codes `DisableUnderDelta` in the streaming branch.
**Why it must exist.** Acceptance criterion #3.

---

### UT-0604-05 — `ep_annihilation_streaming_dispatch_invokes_stream_impl`

**Purpose.** Per TASK-0604 §C-4: when benchmark id = `EpAnnihilation` AND `chunk_size.is_some()`, the dispatch table calls `ep_annihilation_stream` (the stream-impl override), NOT the default `default_chunked_iter` adapter.
**Setup.**
- `BenchmarkId::EpAnnihilation`, `chunk_size = Some(100)`.
- Test-only marker injected into `ep_annihilation_stream` (e.g., a per-call counter).
**Action.** Run dispatch.
**Assertions.**
- The `ep_annihilation_stream` override is invoked at least once.
- The default chunked-iter is NOT invoked for this benchmark id (= 0 calls in this run).
**Boundary case coverage.** Catches a wiring miss where C-4 is forgotten and ep_annihilation falls through to the default adapter (not necessarily incorrect, but not what the spec mandates for ep_annihilation_stream specifically).
**Why it must exist.** Acceptance criterion #4 of TASK-0604.

---

### IT-0604-06 — `streaming_path_produces_valid_csv_with_g1_passing`

**Purpose.** End-to-end: streaming bench produces a valid CSV row with G1 (full or weak per `skip_g1`) passing.
**Setup.**
- Run the bench harness in-process for ep_annihilation, size=1000, workers=2, chunk_size=Some(100), max_pending_lifetime=16.
- Output to a temp CSV path.
**Action.** Read CSV.
**Assertions.**
- Exactly one row corresponding to (ep_annihilation, 1000, 2).
- `g1_pass` column is `"true"` (or the project's encoding for G1 success).
- `final_agent_count` is non-zero and matches the eager path's value (cross-check via the eager run done in IT-0604-07).
- The R18a–R18g columns are populated with finite, non-NaN values (or appropriate sentinels per SPEC-09 §4.9).
**Boundary case coverage.** Catches a streaming-path bug where CSV emission omits new columns or fills them with `0.0`.
**Why it must exist.** Acceptance criterion #5 of TASK-0604.

---

### IT-0604-07 — `streaming_vs_eager_merged_net_isomorphism`

**Purpose.** SPEC-01 G1 + SPEC-21 R37c: the merged net produced by streaming MUST be agent-isomorphic to the eager-path counterpart (same final reduction).
**Setup.**
- Run bench dispatch twice on the SAME size and SAME random seed:
  - Run 1: `chunk_size = None` (eager), capture final merged Net.
  - Run 2: `chunk_size = Some(100)` (streaming), capture final merged Net.
**Action.** Compare via the project's existing `Net` agent-isomorphism helper (e.g., `net::tests::nets_agent_isomorphic` or equivalent — if absent, define a test-only helper that compares (live agent count, redex count, root, final reduction normal form fingerprint)).
**Assertions.**
- Final `live_agent_count` equal across paths.
- Final `redex_count` equal across paths (likely both 0 after `reduce_all`).
- Final reduced root structure isomorphic.
**Boundary case coverage.** This is the strongest invariant — catches any silent semantic divergence introduced by the streaming construction order.
**Why it must exist.** Acceptance criterion #7 of TASK-0604; SPEC-01 G1; SPEC-21 R37c.

---

### IT-0604-08 — `eager_path_output_bit_equivalent_to_baseline`

**Purpose.** Regression: bench WITHOUT `--chunk-size` produces output bit-equivalent to a known pre-D-011 baseline (the v1_local_baseline summary or a frozen check-in CSV).
**Setup.**
- Run ep_annihilation, size=1000, workers=2, chunk_size=None, deterministic seed.
- Compare the bench result row (relevant columns: `final_agent_count`, `redex_total`, `mean_reduction_us` within ±10%, exact match for non-time fields).
**Action.** Read produced CSV; load baseline row.
**Assertions.**
- `final_agent_count` exact match.
- `redex_total` exact match.
- `g1_pass` exact match.
- `wall_time_us` within ±10% of baseline (loose tolerance — system noise).
**Boundary case coverage.** This is the silent-regression headline guard from TASK-0604 §Notes. If a refactor accidentally reroutes the eager path, this test fires.
**Why it must exist.** Acceptance criterion #6 of TASK-0604 + plan §C-2 mitigation ("non-negotiable").

**Implementation note.** Where the baseline lives: `results/locked/v1_local_baseline/` (per CLAUDE.md). Pin the specific row and seed at test-spec time; if pinning is infeasible because the v1 baseline does not run with v2's deterministic seed, document the chosen anchor and mark the test as a soft-floor.

---

### IT-0604-09 — `cli_smoke_chunk_size_100_completes_under_60s`

**Purpose.** End-to-end CLI smoke per the D-011 plan.
**Setup.** None (uses subprocess via `assert_cmd::Command`).
**Action.** Exec: `cargo run --bin relativist -- bench --benchmark ep_annihilation --sizes 1000 --workers 2 --chunk-size 100`. Time the run.
**Assertions.**
- Exit code == 0.
- Wall-clock < 60 s.
- Generated CSV contains a row for (ep_annihilation, 1000, 2).
- The CSV row's `chunk_size` column = 100 (verifies the flag wiring all the way to CSV).
**Boundary case coverage.** Catches CI-environment-specific failures (e.g., docker permission, missing /proc).
**Why it must exist.** Plan §C-2 smoke; promotes IT-0603-09 from `#[ignore]` to active once TASK-0604 lands.

---

### PT-0604-10 — `proptest_streaming_eager_isomorphic_for_random_chunk_sizes`

**Purpose.** Property-test on randomized chunk sizes to surface boundary failures.
**Generator strategy.**
```
arb_size() -> u32 in 100..=2000 (small enough for fast tests)
arb_chunk_size() -> u32 in 1..=size_total (extremes: chunk_size=1, chunk_size=size_total)
arb_workers() -> u32 in 1..=4
```
**Property.** For all `(size, chunk_size, workers)`: the final merged net produced by streaming-path bench is agent-isomorphic to the eager-path bench on the same `size` + `workers`.
**Specific sub-assertions.**
- `prop_assert_eq!(stream_result.final_agent_count, eager_result.final_agent_count)`.
- `prop_assert_eq!(stream_result.g1_pass, eager_result.g1_pass)`.
**Shrinking note.** Minimal counterexample should narrow to either (a) chunk_size=1 (extreme: every chunk is a single agent), (b) chunk_size=size_total (extreme: streaming degenerates to eager), or (c) workers=1 (trivial partition). Each shape diagnoses a distinct failure mode.
**Why it must exist.** Catches edge cases not enumerated by IT-0604-07 (which uses a single chunk_size).

**Implementation note.** Keep the proptest budget low (`PROPTEST_CASES=16`) — each case spins up a full bench run.

---

## Coverage matrix

| test_id | §AC-1 | §AC-2 | §AC-3 | §AC-4 | §AC-5 | §AC-6 | §AC-7 | §AC-8 | G1 | R37c |
|---|---|---|---|---|---|---|---|---|---|---|
| UT-0604-01 | ✅ | | | | | | | | | |
| UT-0604-02 | ✅ | | | | | | | | | |
| UT-0604-03 | | ✅ | | | | | | | | |
| UT-0604-04 | | | ✅ | | | | | | | |
| UT-0604-05 | | | | ✅ | | | | | | |
| IT-0604-06 | | | | | ✅ | | | ✅ | | |
| IT-0604-07 | | | | | | | ✅ | ✅ | ✅ | ✅ |
| IT-0604-08 | | | | | | ✅ | | ✅ | | |
| IT-0604-09 | | | | | ✅ | | | | | |
| PT-0604-10 | | | | | | | ✅ | ✅ | ✅ | ✅ |

Every acceptance criterion + G1 + R37c has ≥1 test.

---

## Out-of-scope tests (deferred to other tasks)

- Memory probe at construction-complete → **TASK-0605**.
- Sparse-net path in the bench harness → **TASK-0606** (Phase D, deferred from this dispatch).
- TCP-mode bench-rodada smoke → **TASK-0610** (Phase E-4, deferred).
- Streaming impls for benchmarks OTHER than `ep_annihilation` → out of scope per task §Files OUT of scope (the other 12 use `default_chunked_iter` already).
