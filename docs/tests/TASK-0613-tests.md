# TEST-SPEC-0613 — Tests for TASK-0613 — D-011 partition perf regression witness (TDD-RED integration test)

**Task:** TASK-0613 (D-011 BLOCKER 2026-05-04 — Stage 2 TDD-RED witness; ships BEFORE the fix in TASK-0612).
**Spec:** SPEC-22 v2.4 §3.4 R22 (effective_arena_size metric — operationalised at the partition-orchestration level), R30 (the rejection contract is NOT exercised here; this test pins the SUCCESS path on a healthy workload), R22a (M5 pathology still detected — implicitly: this test asserts that NON-M5 healthy workloads do NOT trigger the SPARSE escape). Anchor amendment landed in TASK-0611 (commits `e941273` / `972ce47` / `5cca7e6`; closure log `docs/spec-reviews/SPEC-22-amendment-2026-05-04-d011-blocker.md`).
**Origin:** `docs/plans/2026-05-04-d011-partition-perf-fix.md` §5 Task 2 Step 1 (verbatim test body, lines 169–247). Bug history and bisect transcript: `docs/next-steps.md` BLOCKER 2026-05-04 (lines 9–95 active; 37–155 historical context).
**Test floor delta:** **+1 default** (one new `#[test]` in a new integration-test binary `relativist-core/tests/d011_partition_perf_witness.rs`). Zero-copy and streaming-no-recycle floors unchanged.
**Prerequisites:**
- TASK-0611 (SPEC-22 v2.4 amendment) CLOSED — provides the normative anchor.
- No code prerequisite for compilation: the test references only existing public APIs (`Net::new`, `Net::create_agent`, `partition::split_with_config`, `FennelStrategy::new`, `PartitionConfig::default`, `PortRef::FreePort`, `PORTS_PER_SLOT`).
- **Sequencing:** this task ships BEFORE TASK-0612 in commit order. The test is intentionally **TDD RED** — it FAILS on pre-TASK-0612 HEAD, capturing the bug as committed state. TASK-0612 then flips it to PASS, providing the canonical bisect-replicable proof that the fix closes the bug class.

---

## Test inventory

| test_id | level | target file::test_name | prerequisite TASK | cfg gates |
|---|---|---|---|---|
| IT-0613-01 | integration | `relativist-core/tests/d011_partition_perf_witness.rs::d011_witness_partition_dense_branch_for_healthy_workload` | none | none |

**Totals:** 0 UT, 1 IT, 0 PT. Net floor delta: **+1 default**.

---

## Per-test specifications

### IT-0613-01 — `d011_witness_partition_dense_branch_for_healthy_workload`

**Purpose.** Regression witness for SPEC-22 v2.4 R22 metric correction. Asserts that a healthy workload (live agents densely packed within their `id_range`) routes EVERY non-empty partition through the **DENSE** branch of `partition::build_subnet`, not the SPARSE branch. The pre-fix HEAD (commit immediately preceding TASK-0612) FAILS this test with a panic message containing "took SPARSE branch"; the post-fix HEAD (after TASK-0612 lands) PASSES. This FAIL → PASS transition is the empirical proof that the bug class is closed.

The bisect transcript in `docs/next-steps.md` BLOCKER 2026-05-04 documents the field evidence: `ep_con 5M w=2 local` wall regressed from v1's 12 s to HEAD's 22 s (+83%), all of which was attributed to two partitions × ≈ 7 s per partition in the SPARSE-via-`to_dense` path. This test is the unit-of-correctness cause check that complements TASK-0614's bench verification (which measures wall-time but not branch-selection).

**Setup.** Per plan §5 Task 2 Step 1 (canonical source for the verbatim test body — do not duplicate code in this TEST-SPEC).

- Helper function `build_dense_packed_net(n: u32) -> Net` extracted at the top of the test file:
  - `Net::new()` then `n` calls to `net.create_agent(Symbol::Con)` (creates IDs `0..n`, `max_live_id = n - 1`, dense).
  - For each `i in 0..n`, set `net.ports[i as usize * PORTS_PER_SLOT] = PortRef::FreePort(1_000_000 + i)`. This wires the principal port of every agent to a unique FreePort to satisfy T1 (every principal port occupied) without forming any active redex.
- Test body: `let net = build_dense_packed_net(1000);` — 1000 CON agents, IDs 0..999, `max_live_id = 999`, all FreePort-anchored.
- Strategy: `FennelStrategy::new()`. With 2 workers and FENNEL, each partition receives ≈ 500 live agents (the exact split is deterministic per FENNEL but the test does not pin it).
- Config: `PartitionConfig::default()` (sparse_build = true; this is irrelevant for the assertion because the new metric does NOT trip the SPARSE branch on this workload).

The chunk arithmetic that drives the bug-witness logic: `compute_id_ranges(workers = 2, base_next_id = 1000)` allocates per-worker chunks of `max(100_000, base_next_id × 10) = max(100_000, 10_000) = 100_000` IDs each. So `id_range[worker 0] = [1000, 101_000)` and `id_range[worker 1] = [101_000, u32::MAX)`. Under the OLD R22 metric (pre-TASK-0612): `id_range_size (≥ 100_000) > 4 × live_count (≈ 500 × 4 = 2000)` → SPARSE branch (BUG). Under the NEW R22 metric (post-TASK-0612): `effective_arena_size (max_live_id + 1, ≤ 1000 in this workload) ≤ 4 × live_count (≈ 2000)` → DENSE branch (CORRECT).

> **2026-05-04 TEST-SPEC correction (post-developer-Phase-A abort):** the original draft used the discriminator `arena_len < id_range_size`. **This no longer works** because QA-D009-005 (already landed on `v2-development`) changed `SparseNet::to_dense` to size the dense arena by `max_id + 1` regardless of the supplied `id_range` (see `relativist-core/src/net/sparse.rs:325-336`). Both branches therefore produce identical `arena_len`. The reliable discriminator is `subnet.next_id` after `partition::split_with_config`. From `partition/split.rs:93-98` (`subnet.next_id = max(subnet.next_id, max_agent_id + 1)`):
>
> | branch | initial `next_id` | post-override `next_id` |
> |---|---|---|
> | DENSE (`build_subnet`) | `0` | `max(0, max_agent_id + 1) = max_agent_id + 1` |
> | SPARSE (`build_subnet_sparse`) | `id_range.start` | `max(id_range.start, max_agent_id + 1) = id_range.start` (whenever `id_range.start > max_agent_id`, which holds for every partition `i ≥ 1` and even `i = 0` here because `compute_id_ranges` sets `id_range[0].start = base_next_id = 1000 > max_agent_id_in_partition_0 ≈ 499`) |
>
> Discriminator: `partition.subnet.next_id == max_agent_id_in_partition + 1` (DENSE invariant) vs `== id_range.start` (SPARSE invariant). For our test workload the gap is ~500 vs ~1000 (worker 0) and ~1000 vs ~101_000 (worker 1) — unambiguous and bisect-replicable.

**Action.** `let plan = partition::split_with_config(net, 2, &strategy, &cfg);`

**Assertions.** For each partition `i` in `plan.partitions`, with `live_count = partition.subnet.count_live_agents()`:

1. **Skip empty partitions:** if `live_count == 0`, `continue` (both DENSE and SPARSE branches return `Net::new()` for empty workers — neither is wrong, and asserting on empty partitions would be a false positive).

2. **Compute discriminator inputs:** `max_live_id` = highest occupied slot index in `partition.subnet.agents`; `expected_dense_next_id = max_live_id + 1`; `id_range_start = partition.id_range.start`.

3. **DENSE branch invariant (catches "wrong branch taken" — the original D-011 bug):**
   ```text
   partition.subnet.next_id == expected_dense_next_id
   ```
   This MUST hold for every non-empty partition. If it fails, the SPARSE branch was taken: `partition.subnet.next_id` will equal `id_range.start` (much larger than `max_live_id + 1` in this workload).
   - **Failure message** (verbatim, per plan §5 Task 2 Step 1): `"partition {i} took SPARSE branch: subnet.next_id = {} (= id_range.start = {}); expected DENSE branch: subnet.next_id = max_agent_id + 1 = {}. See docs/next-steps.md BLOCKER 2026-05-04."` — the message MUST cite "SPARSE branch" and reference `docs/next-steps.md BLOCKER 2026-05-04` so future debuggers immediately find the historical context.

4. **Discriminator-collapse sanity check (catches "test setup degenerated"):**
   ```text
   partition.subnet.next_id != id_range_start
   ```
   This MUST also hold. If `expected_dense_next_id == id_range_start` (i.e., `max_live_id + 1 == id_range.start`) the discriminator collapses and assertion (3) cannot tell the branches apart. For our workload (1000 agents, base_next_id ≥ 1000, FENNEL split) the two values are always distinct (DENSE: ≤ 1000; SPARSE: ≥ 1000 with gap ≥ 100_000 for worker 1). The sanity check guards against future test-setup drift (e.g., if `compute_id_ranges` changes its formula such that `id_range.start ≈ max_agent_id + 1`, this assertion fires with a clear "test setup must scatter live IDs differently" message rather than producing a false PASS).

**Boundary case coverage.** Two distinct failure modes are caught by the two assertions:
- **Mode A — wrong branch taken** (the original D-011 bug): SPARSE was taken when DENSE was expected. Caught by assertion (3): `subnet.next_id == id_range.start` (instead of `max_agent_id + 1`). With base_next_id = 1000 and live_count ≈ 500 per worker, the SPARSE branch yields `next_id ∈ {1000, 101_000}` while DENSE yields `next_id ∈ {≈ 500, ≈ 1000}`. The gap is unambiguous and worker-1's gap (~100×) is bisect-replicable across reasonable variations of FENNEL output.
- **Mode B — degenerate test setup** (future-proofing): the discriminator collapsed because `max_agent_id + 1 == id_range.start` post-override. Caught by assertion (4) with an explicit "test setup must scatter live IDs differently" message.

The combination provides defense-in-depth: Mode A catches the original regression; Mode B catches a future regression where a `compute_id_ranges` change accidentally invalidates this test's discriminator (the test would otherwise silently pass).

The empty-partition skip in (1) is a deliberate boundary case: FENNEL may produce one empty partition out of two on adversarial inputs (rare but possible). For an empty partition both branches return `Net::new()` with `next_id = 0`, and the override sets it to `max(0, 0 + 1) = 1` — a value that doesn't match either branch's "characteristic" next_id, so asserting on empty partitions would be a false positive. The test correctly skips them.

**Why it must exist.**
1. **R22 v2.4 correctness witness at the orchestration level.** TEST-SPEC-0612's UT-0492-02 catches the same bug class at the unit level (single `build_subnet_with_config` call); IT-0613-01 catches it at the integration level (full `split_with_config` orchestration including FENNEL strategy + `compute_id_ranges` interaction). Both layers must be tested because a regression could be introduced at either: a unit-level revert of the metric (caught by UT-0492-02), or an orchestration-level bug where `split_with_config` passes the wrong `id_range` to `build_subnet_with_config` (caught only by IT-0613-01).
2. **The bisect proved the bug exists at this level.** The bench bisect (in BLOCKER 2026-05-04) ran the actual `local` mode CLI path which goes through `split_with_config`, not directly through `build_subnet_with_config`. The empirical evidence is at the integration level; the witness must mirror that.
3. **Without it, a future unrelated change to `compute_id_ranges`** (e.g., raising the chunk multiplier from 10× to 20×, or removing the `max(100_000, …)` floor) could re-introduce the same failure mode silently. The metric correction in TASK-0612 makes the threshold check robust to *any* `compute_id_ranges` formula because the threshold is now decoupled from the planning range. This test pins that decoupling against the *current* `compute_id_ranges` formula — if a future change accidentally re-couples them (e.g., by passing `id_range.end` as a metric input), this test catches it.

**Failure message specification.** As detailed in assertion (2) above, the assertion failure MUST cite "SPARSE branch" and reference `docs/next-steps.md BLOCKER 2026-05-04`. The plan §5 Task 2 Step 1 has the exact `assert!` macro with the right message; the developer MUST preserve it verbatim — the strings are part of the test's contract because future debuggers will grep for them when a regression fires.

**Pre-fix expected output (when the test runs on the commit immediately preceding TASK-0612):**
```
---- d011_witness_partition_dense_branch_for_healthy_workload stdout ----
panicked at 'partition 0 took SPARSE branch: subnet.next_id = 1000 (= id_range.start = 1000); expected DENSE branch: subnet.next_id = max_agent_id + 1 = ~500. See docs/next-steps.md BLOCKER 2026-05-04.'
```
The exact `subnet.next_id` and `max_agent_id + 1` figures depend on FENNEL's deterministic output for this seed-free workload; what matters is the panic message structure (cites "SPARSE branch", quotes the equality `subnet.next_id == id_range.start`, references `BLOCKER 2026-05-04`).

**Post-fix expected output (TASK-0612 landed):**
```
test d011_witness_partition_dense_branch_for_healthy_workload ... ok
```

---

## Notes

### Commit ordering (TDD-RED sequencing)

TASK-0613 commits BEFORE TASK-0612 in the bundle:

1. **Commit N** — TASK-0613: `tests/d011_partition_perf_witness.rs` lands. CI shows the test FAILING. Commit message (per plan §5 Task 2 Step 3) explicitly notes: "FAILS on HEAD … will pass after TASK-0612". This is the bug-as-committed-state.
2. **Commit N+1** — TASK-0612: `error.rs` field rename + `helpers.rs` metric rewrite + 5 inline UT rewrites. CI shows IT-0613-01 now PASSING. The combined TASK-0612 commit message documents the FAIL→PASS transition as verification.

If commit ordering is inverted (TASK-0612 first, then TASK-0613), the test still functions as a regression guard but loses the empirical "FAIL → PASS" record on the git log. The test's value is preserved either way; the ordering is preferred but not strictly required for correctness.

The CI floor briefly accommodates the failure: between commit N and commit N+1, the integration test FAILS while the rest of the suite remains green. If CI runs on every commit (not just on PR), commit N will be reported as red. This is acceptable for this bundle because the BLOCKER fix is being landed on a feature branch, not directly on `v2-development`'s tip; the merge-to-main gate is the combined two-commit state, which is fully green.

### What this test does NOT cover

- **Wall-time measurement.** TASK-0614 (bench verification) is the wall-time gate. IT-0613-01 is purely a correctness/branch-selection witness; it would pass even if the DENSE branch were 100× slower than v1, as long as the right branch is taken. The two layers (correctness here, performance in TASK-0614) are complementary.
- **M5 pathology (the original R22 motivation).** This test pins the *non-M5* (healthy) case. The M5 case (`max_live_id ≫ live_count`) is exercised by the rewritten unit tests in TEST-SPEC-0612 (UT-0484-03, UT-0484-05, UT-0492-01 all use scattered IDs that simulate the M5 pathology proxy). Together TEST-SPEC-0612 + TEST-SPEC-0613 cover both sides of the new metric.
- **Multi-worker scaling beyond 2.** The test uses `workers = 2` to match the pre-fix bisect's `ep_con 5M w=2` setup. Larger worker counts may exhibit different `compute_id_ranges` behaviour, but the metric correction is independent of worker count, so the W=2 case is sufficient evidence.
- **Auxiliary-port routing.** `build_dense_packed_net` only wires principal ports — adequate for a partition-branch witness because the branch decision is taken before any auxiliary-port wiring is consulted. NOT adequate for a reduction smoke test (TASK-0613 explicitly defers reduction-correctness to existing test coverage).

### API surface verification

The test imports the following symbols from `relativist_core`:
- `relativist_core::net::Net` — public.
- `relativist_core::net::Symbol` — public (specifically `Symbol::Con`).
- `relativist_core::net::PortRef` — public (specifically `PortRef::FreePort(u32)`).
- `relativist_core::net::PORTS_PER_SLOT` — public constant.
- `relativist_core::partition::{split_with_config, FennelStrategy, PartitionConfig, PartitionStrategy}` — public.

If any of these has been narrowed (e.g., `pub(crate)` instead of `pub`) at the time TASK-0613 lands, the test will fail to compile. The fallback (per TASK-0613 Key risks) is to `grep -n "pub use" relativist-core/src/net/mod.rs` and verify the re-exports. The plan §5 Task 2 Step 2 explicitly anticipates this and includes a `cargo test --release --test d011_partition_perf_witness -- --nocapture` smoke check.

### Cfg gates

None. The test runs on the default profile (`cargo test`). It is NOT gated on `zero-copy` or `streaming-no-recycle`. The zero-copy and streaming-no-recycle floors (1726 / 1680) are unchanged by this task.

---

## Cross-references

- **Plan source of truth:** `docs/plans/2026-05-04-d011-partition-perf-fix.md` §5 Task 2 Step 1 (verbatim test body, lines 169–247) and Step 3 (commit message, lines 264–282).
- **Companion TEST-SPEC:** `docs/tests/TEST-SPEC-0612-tests.md` (TASK-0612 unit-level rewrites — UT-0492-02 catches the same bug class at the unit level).
- **Spec anchor:** SPEC-22 v2.4 §3.4 R22 (effective_arena_size formula).
- **Bisect transcript / bug history:** `docs/next-steps.md` BLOCKER 2026-05-04 (active block lines 9–95; historical context lines 37–155).
- **Closure log:** `docs/spec-reviews/SPEC-22-amendment-2026-05-04-d011-blocker.md`.
- **Downstream:** TASK-0614 — bench verification (wall-time gate, complements this correctness gate).

---

## Coverage matrix

| test_id | R22 (metric) | R22 (orchestration) | R22a (M5 negative case) | Bug-class branch-selection | Discriminator-collapse guard |
|---|---|---|---|---|---|
| IT-0613-01 | ✅ | ✅ | ✅ (healthy = no SPARSE) | ✅ (assertion 3 — `next_id` discriminator) | ✅ (assertion 4 — `next_id != id_range.start`) |

R22 is exercised at the orchestration level. R30 is intentionally NOT exercised by this test (the healthy workload does not trigger rejection); R30 coverage lives in TEST-SPEC-0612's UT-0484-03 and UT-0484-05.

---

## Out-of-scope (explicitly NOT specified here)

- Unit-level rewrites in `relativist-core/src/partition/helpers.rs` test mod — specified in `docs/tests/TEST-SPEC-0612-tests.md`.
- Spec amendment paperwork (TASK-0611) — no test deliverables.
- Bench verification (TASK-0614) — wall-time measurement, not unit-test material.
- Reduction-correctness smoke (the test only wires principal ports to FreePorts; auxiliary-port routing is not exercised).
- M5 pathology positive case — exercised by TEST-SPEC-0612's scattered-ID unit tests.
- Cross-version downgrade or wire compatibility — this fix is build-time-only; no wire-format implications.
- TEST-SPEC-T16 / TEST-SPEC-0484 / TEST-SPEC-0492 / TEST-SPEC-0552 (historical TEST-SPEC documents) — drift by design; no cross-reference.
