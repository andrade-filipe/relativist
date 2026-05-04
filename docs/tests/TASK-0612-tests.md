# TEST-SPEC-0612 — Tests for TASK-0612 — `build_subnet_with_config` effective_arena_size metric + error field rename + UT rewrites

**Task:** TASK-0612 (D-011 BLOCKER 2026-05-04 fix — Stage 3+).
**Spec:** SPEC-22 v2.4 §3.4 R22 (effective_arena_size metric), R30 (rejection-payload contract), R22a (M5 pathology still detected). Anchor amendment landed in TASK-0611 (commits `e941273` / `972ce47` / `5cca7e6`; closure log `docs/spec-reviews/SPEC-22-amendment-2026-05-04-d011-blocker.md`).
**Origin:** `docs/plans/2026-05-04-d011-partition-perf-fix.md` §5 Task 3 Steps 1–4 (verbatim test bodies). Bisect transcript and bug history: `docs/next-steps.md` BLOCKER 2026-05-04.
**Test floor delta:** **net 0** (5 tests rewritten in place; 1 of those renamed; the unchanged UT-0484-02 still counts toward the existing floor; no new tests added by this task).
**Prerequisites:**
- TASK-0611 (SPEC-22 v2.4 amendment) CLOSED. The rewritten tests pin the NEW metric; without R22 v2.4 they have no normative anchor.
- TASK-0613 (TDD-RED witness in `relativist-core/tests/d011_partition_perf_witness.rs`) lands BEFORE TASK-0612 in commit order so the bug is empirically captured before the fix flips it.
- The five rewritten tests + the renamed UT-0484-05 do **not** compile until the `error.rs` field rename in TASK-0612 lands; therefore the production change and the test rewrites must commit together (atomicity rationale in TASK-0612 "Atomicity note").

---

## Test inventory

| test_id | level | target file::test_name | prerequisite TASK | cfg gates |
|---|---|---|---|---|
| UT-0484-02 (unchanged) | unit | `relativist-core/src/partition/helpers.rs::tests::sparse_build_false_below_threshold_succeeds` | none | none |
| UT-0484-03 (REWRITE) | unit | `relativist-core/src/partition/helpers.rs::tests::sparse_build_false_above_threshold_rejects` | TASK-0611 | none |
| UT-0484-04 (REWRITE) | unit | `relativist-core/src/partition/helpers.rs::tests::sparse_build_true_above_threshold_uses_sparse_path` | TASK-0611 | none |
| UT-0484-05 (REWRITE + RENAME) | unit | `relativist-core/src/partition/helpers.rs::tests::error_field_effective_arena_size_correct` | TASK-0611 | none |
| UT-0492-01 (REWRITE) | unit | `relativist-core/src/partition/helpers.rs::tests::sparse_path_taken_above_threshold` | TASK-0611 | none |
| UT-0492-02 (REWRITE) | unit | `relativist-core/src/partition/helpers.rs::tests::dense_path_taken_below_threshold` | TASK-0611 | none |

**Totals:** 6 UT (5 rewritten, 1 unchanged-but-revalidated), 0 IT, 0 PT. Net floor delta: **0**.

UT-0484-06 (`error_variant_in_partition_error_enum`, lines 1476–1496) is in the same test mod and pattern-matches on the renamed field — it is updated as part of TASK-0612's mechanical field rename, not as a metric-semantics rewrite, and so is not enumerated as a per-test specification here. The reviewer should still verify it compiles after the rename.

---

## Per-test specifications

### UT-0484-02 — `sparse_build_false_below_threshold_succeeds` (UNCHANGED — confirm survives the metric switch)

**Purpose.** Validate that `sparse_build = false` returns `Ok(...)` when the threshold is not exceeded. Originally pinned the OLD metric at the boundary `id_range_size = 4 × live_count = 40` (no exceedance). Under the NEW metric the same setup yields `effective_arena_size = max_live_id + 1 = 9 + 1 = 10`, and `10 ≤ 4 × 10 = 40` so the threshold is still NOT exceeded — the test continues to pass without modification.

**Setup.** Lines 1344–1370 of `relativist-core/src/partition/helpers.rs` (HEAD). 10 live agents densely packed at IDs 0..9 (`max_live_id = 9`); principal ports wired to `FreePort(1_000_000 + i)` for T1; `id_range = 0..40`; `cfg.sparse_build = false`.

**Action.** `build_subnet_with_config(&cfg, 0, &net, &agents, &sigma, &[], 0, 0..40)`.

**Assertions.**
- `result.is_ok()` (the test's existing assertion is sufficient).

**Boundary case coverage.** The original test was a *boundary* test for the OLD metric (`id_range_size == 4 × live_count` exactly). Under the NEW metric, this is no longer the boundary case (the new boundary case for the same workload would require `effective_arena_size == 40`, i.e., `max_live_id == 39`). The test still passes because the new metric is *strictly tighter* on this workload. Boundary-at-exact-4× under the new metric is not currently covered by the existing inline tests — see "Boundary-coverage analysis" below for whether a NEW test is required.

**Why it must exist.** Acts as the negative control: confirms that healthy densely-packed workloads continue to take the dense path under both metrics. This is exactly the workload class the fix restores to v1 behavior.

**Reviewer action.** Verify the test still compiles and passes after the field rename without modification. If clippy or fmt flags it, leave the body alone; the comment header may be optionally updated to note that the test now exercises the NEW metric below threshold (cosmetic only — not required for landing).

---

### UT-0484-03 (REWRITE) — `sparse_build_false_above_threshold_rejects`

**Purpose.** Validate that `sparse_build = false` rejects with `PartitionError::DenseAllocationExceedsThreshold` when `effective_arena_size > 4 × live_count`. Under the OLD metric the test relied on `id_range = 0..50` (50 > 40) to trip; under the NEW metric, `id_range.end` is irrelevant — the test must construct a workload where `max_live_id + 1` itself exceeds `4 × live_count`.

**Setup.** Per plan §5 Task 3 Step 1 (canonical source — do not duplicate code here). Construct a `Net` with 50 agents created sequentially (IDs 0..49), then call `net.remove_agent(id)` for every ID not in `{0, 5, 10, 15, 20, 25, 30, 35, 40, 45}`. Result: 10 live agents at scattered IDs with `max_live_id = 45`, `effective_arena_size = 46`, `live_count = 10`. Principal ports of the 10 live agents wired to `FreePort(1_000_000 + i)` for T1. `cfg.sparse_build = false`. The `id_range` argument passed to `build_subnet_with_config` is `0..50` — but its value is now irrelevant to the rejection decision.

**Action.** `build_subnet_with_config(&cfg, 0, &net, &agents, &sigma, &[], 0, 0..50)`.

**Assertions.**
- `matches!(result, Err(PartitionError::DenseAllocationExceedsThreshold { .. }))`.
- Failure message (in the `assert!` second argument) cites the scattered-ID setup: `"scattered live IDs (max=45, live=10): expected DenseAllocationExceedsThreshold under new metric, got {:?}"`.

**Boundary case coverage.** `effective_arena_size = 46`, threshold `= 40`, exceedance `= 6`. Just-above-threshold; tests the metric semantics, not the magnitude. Combined with UT-0484-02 (just-at-threshold-but-not-exceeded, eff_arena = 10, threshold = 40), the pair brackets the boundary on the *high side* (eff_arena = 46 vs threshold = 40). The pair does NOT bracket the *exact* boundary `eff_arena == 4 × live_count` — see "Boundary-coverage analysis" below.

**Why it must exist.** Operationalises R30 under the new metric. Without this test, a regression that re-introduces the OLD metric (`id_range_size`-based) would silently restore the bug because the densely-packed workload of pre-rewrite UT-0484-03 trips both old and new metrics; the scattered-ID setup is the *only* setup that distinguishes them.

**Rationale for "scattered IDs" in setup.** The new metric measures `max_live_id + 1`. To exercise the threshold the workload MUST have at least one live agent at a high enough ID to push `max_live_id + 1 > 4 × live_count`. Densely packing 10 live agents at IDs 0..9 yields `max_live_id + 1 = 10`, never exceeding `4 × 10 = 40`. The scattered setup `{0, 5, 10, …, 45}` yields `max_live_id + 1 = 46`, just above 40. The `step_by(5)` choice is the smallest power-of-2-adjacent stride that crosses the threshold with exactly 10 live agents.

---

### UT-0484-04 (REWRITE) — `sparse_build_true_above_threshold_uses_sparse_path`

**Purpose.** Validate that `sparse_build = true` returns `Ok(...)` (no rejection) when the threshold is exceeded — same workload as UT-0484-03 but with `sparse_build = true`. Confirms the SPARSE branch is reachable as the documented opt-in path.

**Setup.** Identical to UT-0484-03 (10 live agents at scattered IDs `{0, 5, …, 45}`, `effective_arena_size = 46`, threshold = 40). Only difference: `cfg.sparse_build = true`. Per plan §5 Task 3 Step 2.

**Action.** `build_subnet_with_config(&cfg, 0, &net, &agents, &sigma, &[], 0, 0..50)`.

**Assertions.**
- `result.is_ok()`.
- Failure message cites: `"sparse_build=true above threshold (scattered live IDs): must not reject, got {:?}"`.

**Boundary case coverage.** Companion to UT-0484-03. Together they form the matrix `{sparse_build ∈ {false, true}} × {threshold ∈ {below, above}}` — UT-0484-02 covers `{false, below}`, UT-0484-03 covers `{false, above}`, UT-0484-04 covers `{true, above}`. The fourth cell `{true, below}` is covered by UT-0492-02.

**Why it must exist.** Confirms `sparse_build = true` is honored as an escape hatch. Without it, a regression that hard-rejects above-threshold regardless of `sparse_build` would land silently.

---

### UT-0484-05 (REWRITE + RENAME) — `error_field_effective_arena_size_correct`

**Purpose.** Validate that the rejected `PartitionError::DenseAllocationExceedsThreshold` payload carries the renamed field `effective_arena_size` (NOT `id_range_size`) with the correct numeric value. This test is the field-rename gate per R30 — without it, the rename could silently pin the old field name in the variant's destructuring pattern.

**Setup.** Per plan §5 Task 3 Step 3. 500 agents created sequentially, then `step_by(5)` retains 100 live at IDs `{0, 5, 10, …, 495}`. `max_live_id = 495`, `effective_arena_size = 496`, `live_count = 100`. Principal ports for live agents wired to `FreePort(1_000_000 + i)` for T1. `cfg.sparse_build = false`. The `id_range` argument is `0..1000` — irrelevant to the new metric; included to also verify the rename does not accidentally re-introduce a code path that uses `id_range.end` for the payload.

**Action.** `build_subnet_with_config(&cfg, 0, &net, &agents, &sigma, &[], 0, 0..1000)`.

**Assertions.**
- `matches!(result, Err(PartitionError::DenseAllocationExceedsThreshold { partition_index, effective_arena_size, live_count }))` destructure succeeds (the test pattern-matches the renamed field by name).
- `partition_index == 0`.
- `effective_arena_size == 496` (= max_live_id + 1).
- `live_count == 100`.

**Boundary case coverage.** Larger-magnitude exceedance than UT-0484-03 (96 over the threshold of 400, vs 6 over 40 in UT-0484-03). Confirms the metric scales correctly with workload size and is not silently capped at some sentinel value.

**Why it must exist.** Two simultaneous gates:
1. **Field-rename correctness** — the destructuring `{ effective_arena_size, .. }` would fail to compile if the field is still named `id_range_size` (this is the trigger for the atomicity constraint between TASK-0612's production change and its test rewrites).
2. **Numeric-value correctness** — confirms the metric is computed as `max(worker_agents) + 1`, not as `worker_agents.len()` (a buggy implementation could use the wrong source).

**Rationale for renaming the test function.** The original name `error_field_id_range_size_correct` would be semantically misleading after the rename; the new name reflects what the test actually asserts. The rename is mechanical (replacement in the `#[test] fn ...` line) and does not affect test count.

---

### UT-0492-01 (REWRITE) — `sparse_path_taken_above_threshold`

**Purpose.** Validate that `sparse_build = true` (default config) returns `Ok(...)` and produces a subnet with the expected number of live agents when the new metric trips the SPARSE branch. Operationalises the SPARSE-path success contract of R22 v2.4.

**Setup.** Per plan §5 Task 3 Step 4 (first half). 55 agents created sequentially, `step_by(6).take(10)` retains 10 live at IDs `{0, 6, 12, 18, 24, 30, 36, 42, 48, 54}`. `max_live_id = 54`, `effective_arena_size = 55`, `live_count = 10`, threshold `= 40`. Stride is 6 (not 5) so that exactly 10 live agents fit within IDs 0..55 (using stride 5 would allow 11 within 0..50, drift the count). Principal ports for live agents wired to `FreePort(1_000_000 + i)` for T1. `cfg = PartitionConfig::default()` (sparse_build = true).

**Action.** `build_subnet_with_config(&cfg, 0, &net, &agents, &sigma, &[], 0, 0..55)`.

**Assertions.**
- `result.is_ok()`.
- `subnet.count_live_agents() == 10` — sparse branch correctly routed all 10 live agents into the resulting subnet.

**Boundary case coverage.** Just-above-threshold sparse path (eff_arena = 55, threshold = 40, exceedance = 15). Discriminator note (per plan §5 Task 3 Step 4): the SPARSE path produces an arena sized by `id_range.end - id_range.start` via `to_dense`, whereas the DENSE path produces `arena_len = max_live_id + 1`. For this specific test, `to_dense(0..55)` yields `arena_len = 55` and DENSE would also yield 55 — the two produce *the same arena size* for this particular setup, so we cannot discriminate by `arena_len`. The test therefore asserts only `count_live_agents() == 10`. The path-discrimination is the responsibility of TEST-SPEC-0613 / IT-0613-01 (which uses 1000 agents to maximise the discrimination signal).

**Why it must exist.** Sparse-path smoke under the new metric. Without it, a regression that breaks `to_dense` for the sparse path would not be caught by UT-0484-04 (which only checks `is_ok()`).

---

### UT-0492-02 (REWRITE) — `dense_path_taken_below_threshold`

**Purpose.** Validate that the DENSE path is taken when the new metric does NOT trip, AND that the resulting subnet's `agents` Vec is sized to `max_live_id + 1` (NOT to `id_range.end`). This is the **CRITICAL** test that catches the original bug class: a regression that re-introduces `id_range.end`-based sizing in the dense path would yield `subnet.agents.len() == 1000` (because `id_range = 0..1000`); the dense path under R22 v2.4 must yield `subnet.agents.len() == 10`.

**Setup.** Per plan §5 Task 3 Step 4 (second half). 10 agents densely packed at IDs 0..9 (`max_live_id = 9`, `effective_arena_size = 10`, threshold = 40 — NOT exceeded). Principal ports wired to `FreePort(1_000_000 + i)` for T1. `cfg = PartitionConfig::default()` (sparse_build = true; irrelevant since threshold not exceeded). The `id_range` argument is `0..1000` — deliberately *enormous* relative to `max_live_id` to surface any code path that mistakenly uses `id_range.end` for arena sizing.

**Action.** `build_subnet_with_config(&cfg, 0, &net, &agents, &sigma, &[], 0, 0..1000)`.

**Assertions.**
- `result.is_ok()`.
- `subnet.agents.len() == 10` — dense branch sizes arena to `max_live_id + 1 = 10`. Failure message: `"dense branch: arena = max_live_id + 1 = 10 (NOT id_range.end = 1000)"`.
- `subnet.count_live_agents() == 10`.

**Boundary case coverage.** This is the bug-class catch test. Pre-fix, the equivalent setup (with the OLD metric) produced `subnet.agents.len() == 1000` because `id_range_size = 1000 > 4 × 10 = 40` tripped SPARSE, and SPARSE-via-`to_dense(0..1000)` returns an arena of size 1000. Post-fix, `effective_arena_size = 10 ≤ 40` so DENSE is taken and the arena is correctly sized to 10. The 100× ratio between `id_range.end = 1000` and `max_live_id + 1 = 10` is large enough to be unambiguous in the failure message (no risk of an off-by-one ambiguity).

**Why it must exist.** The most important test in this suite. It is the inline-test analogue of TEST-SPEC-0613's IT-0613-01 (the integration regression witness). Both tests catch the same bug class; UT-0492-02 catches it at the unit level (single call to `build_subnet_with_config`), IT-0613-01 catches it at the partition-level integration (`split_with_config` orchestration). If UT-0492-02 fails but IT-0613-01 passes, the bug is in the unit; if both fail, the bug is in either. If both pass and the bisect's bench timing is still bad, the bug has shifted location.

---

## Boundary-coverage analysis (exact-4× edge case)

The new metric formula is `effective_arena_size > 4 × live_count` — *strict* inequality. The boundary `effective_arena_size == 4 × live_count` is NOT exceeded (test must succeed in this case).

| Test | live_count | max_live_id | effective_arena_size | 4 × live_count | Exceeded? | Branch |
|---|---|---|---|---|---|---|
| UT-0484-02 | 10 | 9 | 10 | 40 | No | dense (sparse_build=false, no rejection) |
| UT-0484-03 | 10 | 45 | 46 | 40 | Yes (+6) | rejected |
| UT-0484-04 | 10 | 45 | 46 | 40 | Yes (+6) | sparse |
| UT-0484-05 | 100 | 495 | 496 | 400 | Yes (+96) | rejected |
| UT-0492-01 | 10 | 54 | 55 | 40 | Yes (+15) | sparse |
| UT-0492-02 | 10 | 9 | 10 | 40 | No | dense |

**Gap analysis.** The exact boundary `effective_arena_size == 4 × live_count` (e.g., 10 live with `max_live_id = 39`, eff_arena = 40, threshold = 40, NOT exceeded) is NOT covered by any of the six rewritten tests. UT-0484-02 covers `eff_arena = 10` (10× below the threshold, comfortable margin), not `eff_arena = 40` (at the boundary).

**Recommendation.** A NEW test `boundary_exact_4x_not_exceeded` is **NOT required for TASK-0612** because (a) the original UT-0484-02 was the boundary test for the OLD metric and the plan does not call for adding new tests in TASK-0612, and (b) the strict inequality `>` is structurally simple and unlikely to drift independently. The reviewer MAY suggest adding this boundary test in a follow-up; if so, it would land outside this bundle and be specified separately. The TEST-GENERATOR notes the gap explicitly here so the reviewer/QA agents have it on record without acting on it.

**Decision: gap acknowledged, no new test added in TEST-SPEC-0612.** Scope discipline — the plan §5 Task 3 enumerates exactly 5 rewrites and TASK-0612 promises "net 0" floor delta. Adding a 7th test would inflate the floor beyond what the task contract specifies.

---

## Cross-references

- **Plan source of truth:** `docs/plans/2026-05-04-d011-partition-perf-fix.md` §5 Task 3 Steps 1–4 (verbatim test bodies). The TEST-SPEC describes WHAT each test asserts and WHY; the plan supplies the exact test code. The developer implements from the plan.
- **Companion TEST-SPEC:** `docs/tests/TEST-SPEC-0613-tests.md` (TASK-0613 integration witness — IT-0613-01 catches the same bug class at the partition-orchestration level).
- **Spec anchor:** SPEC-22 v2.4 §3.4 R22 (effective_arena_size formula), R30 (rejection-payload contract), R22a (M5 still detected).
- **Bisect transcript / bug history:** `docs/next-steps.md` BLOCKER 2026-05-04.
- **Closure log:** `docs/spec-reviews/SPEC-22-amendment-2026-05-04-d011-blocker.md`.

## Cfg gates

None. All six tests run on the default profile (`cargo test`). They are NOT gated on `zero-copy` or `streaming-no-recycle`. The zero-copy and streaming-no-recycle floors (1726 / 1680) are unchanged by this task.

## Coverage matrix

| test_id | R22 (metric) | R22 (formula) | R30 (rejection) | R30 (payload field) | R22a (M5 still caught) |
|---|---|---|---|---|---|
| UT-0484-02 | ✅ (below) | ✅ | ✅ (no rejection) | — | — |
| UT-0484-03 | ✅ (above) | ✅ | ✅ (rejection) | — | ✅ (scattered IDs) |
| UT-0484-04 | ✅ (above) | ✅ | ✅ (sparse opt-in) | — | ✅ (scattered IDs) |
| UT-0484-05 | ✅ (above) | ✅ | ✅ (rejection) | ✅ (renamed field) | ✅ (scattered IDs, large) |
| UT-0492-01 | ✅ (above) | ✅ | ✅ (sparse) | — | ✅ (scattered IDs) |
| UT-0492-02 | ✅ (below) | ✅ | ✅ (dense, no rejection) | — | — (negative case) |

Every R has ≥ 1 test. R22a is exercised by every scattered-ID test (the scattered-ID workload IS the M5 pathology proxy in unit-test form).

---

## Out-of-scope (explicitly NOT specified here)

- **TEST-SPEC-T16 / TEST-SPEC-0484 / TEST-SPEC-0492 / TEST-SPEC-0552** — historical TEST-SPEC documents for the original (pre-rewrite) tests. Their drift is intentional ship-time historical record; do NOT cross-reference or attempt to "supersede" them.
- **Integration test in `tests/d011_partition_perf_witness.rs`** — specified in `docs/tests/TEST-SPEC-0613-tests.md`.
- **Spec amendment paperwork (TASK-0611)** — no test deliverables.
- **Bench verification (TASK-0614)** — wall-time measurement, not unit-test material.
- **`compute_id_ranges` chunk multiplier** — separate concern (Option A in plan §2; rejected).
- **Cross-version downgrade or wire compatibility** — this fix is build-time-only; no wire-format implications.
- **UT-0484-06 (`error_variant_in_partition_error_enum`)** — mechanical pattern-match test that ships as part of TASK-0612's field rename; not a metric-semantics test, so not specified per-test here. Reviewer should verify it compiles after the rename.
