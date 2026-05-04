# QA-D011-POST-FIX-AUDIT — Adversarial audit of the D-011 fix bundle

**Date:** 2026-05-04
**Branch:** v2-development
**Commit audited:** `62de30f` — `fix(d-011): TASK-0612 + Bug1/Bug2 — SPEC-22 v2.4 metric + dense build_subnet correctness`
**Auditor:** QA agent (Stage 5)
**Scope:** Adversarial bug hunt on the fix itself (Bug 1, Bug 2, AF-2, AF-3, metric correction, test rewrites). Bug 2 root-cause already covered in `QA-D011-BUG2-i1-violation-2026-05-04.md` — this audit looks for **NEW** regressions or **ADDITIONAL** latent bugs.
**Test baseline:** 1621 lib tests pass (debug, default features). 17 D-011 regression witnesses pass. 328 merge tests pass. 108 streaming tests pass.

---

## Verdict

**`ACCEPT_WITH_FOLLOW_UP`**

- 0 CRITICAL
- 1 HIGH (latent — currently dormant; not blocking merge)
- 3 MEDIUM
- 2 LOW

The fix correctly addresses Bug 1 + Bug 2 and the AF-2/AF-3 guards do not misfire on any existing test path. **No regression introduced** that would block the merge. The HIGH finding is a pre-existing latent surface that AF-2 makes louder rather than silenter — the right direction. The MEDIUMs are test-quality / documentation issues that should be tracked but do not threaten correctness today.

---

## Findings

### F-001 [HIGH — latent / dormant] Streaming `PartitionAccumulator::finalize` produces subnets that violate AF-2 if subsequently reduced

**File:** `relativist-core/src/partition/streaming.rs:819-858` (`PartitionAccumulator::finalize`) + `relativist-core/src/net/sparse.rs:308-415` (`SparseNet::to_dense`).

**Test attempted:** Trace the streaming pipeline finalize → to_dense path and check whether the resulting `Net.next_id` is guaranteed to satisfy AF-2 (`range.start <= next_id < range.end`) for every worker.

**Result — bug found, but currently unreachable in production.**

**Trace.** Streaming `PartitionAccumulator` accumulates agents into a `SparseNet` (`new` initializes `next_id = 0`, then `create_agent_at` updates `next_id = max(next_id, id+1)`). At finalize, `s.to_dense(Some(id_range))` is called; `to_dense` copies `self.next_id` verbatim (`sparse.rs:396`) into `Net.next_id` and sets `Net.id_range = Some(id_range)`. **There is no analog of the dense fix (`next_id = id_range.start`) and no analog of the `split.rs:103` widening step.**

This produces two concrete AF-2 violation scenarios on the resulting subnet:

  1. **Empty streaming partition with non-zero range** (e.g. worker 1 of a 4-worker round-robin with global_max=99 → range = [25..50)). `max_assigned_id = None`, `SparseNet.next_id = 0`, so `Net.next_id = 0` while `Net.id_range = Some(25..50)`. Calling `create_agent` panics on AF-2 lower bound: `next_id (0) < range.start (25)`.

  2. **Worker holding the maximum-ID agent in its band** (round-robin always produces this on the last worker; can also occur on intermediate workers depending on chunk_size and assignment skew). `max_assigned_id == range.end - 1` → `SparseNet.next_id == range.end` → `Net.next_id == range.end`. AF-2 upper bound panics on the first allocation: `next_id (range.end) >= range.end`.

**Reproduction (analytical — not currently executable in test suite):**
```text
1. generate_and_partition_chunked(stream of N agents, num_workers=2, RoundRobin)
2. Result has a worker whose accumulator's max_assigned_id == band_end - 1.
3. PartitionPlan::from(result) → run_grid → reduce_all(&mut partition.subnet)
4. First CON-DUP rule fires → create_agent(...) → debug_assert panic at AF-2.
```

**Why it doesn't fire today:** the bench harness (`bench/suite.rs:217-`) uses an **assembly-streaming** path (single monolithic `SparseNet`, then convert) — NOT the `generate_and_partition_chunked` partition-streaming path. The four production reduction call-sites (`worker.rs:647`, `merge/grid.rs:112`, `protocol/self_worker.rs:62`, `protocol/worker.rs:230`) consume `Partition`s sourced from `split_with_config` (which goes through `build_subnet_with_config` — already fixed), not from `PartitionAccumulator::finalize`. So the buggy partitions are constructed by tests and inspected for structure but never reduced. SPEC-21 §6.2 explicitly anticipates wiring `generate_and_partition_chunked → run_grid` as a follow-up; **this latent bug will trigger immediately when that wiring lands**.

**Severity rationale:** HIGH (because the violation is a panic, not a silent wrong result, and the panic is in a debug guard that release builds elide — so a release-mode integration would silently allocate either OOB into another worker's range OR panic on the assert at line 527 only if next_id reaches u32::MAX). LATENT/DORMANT (because no production caller currently exercises the unsafe combination).

**Recommended action (follow-up task, NOT blocking merge):**
- Add `Net.next_id = id_range.start` (and a widen analogous to split.rs:103 if pre-split agent IDs ≥ id_range.start are possible in the streaming context) to `PartitionAccumulator::finalize` immediately AFTER `to_dense`, e.g.:
  ```rust
  let mut dense = match self.subnet { ... };
  dense.next_id = std::cmp::max(dense.next_id, id_range.start);
  // Or, if max_assigned_id may collide with band_end-1:
  dense.next_id = id_range.start.max(self.max_assigned_id.map(|m| m+1).unwrap_or(0));
  // If dense.next_id == id_range.end, the partition is FULL — caller must know.
  ```
- Add a regression witness mirroring `qa_d011_bug2_dense_build_subnet_next_id_in_range` for the streaming finalize path.
- Update SPEC-21 §6.2 with the next_id-seed contract.

---

### F-002 [MEDIUM] UT-0492-01 / UT-0492-02 lost their "which path was taken" discriminator

**File:** `relativist-core/src/partition/helpers.rs:1594-1663`.

**Test attempted:** Verify the two rewritten Category A tests still discriminate the SPARSE vs DENSE branch they claim to test.

**Result — discriminator collapsed.**

- **UT-0492-01 (`sparse_path_taken_above_threshold`)** asserts `subnet.id_range == Some(0..55)` to "verify SPARSE path was taken". But the post-fix DENSE path (`build_subnet` line 399) ALSO sets `id_range: Some(id_range)`. Both branches now satisfy this assertion. The test only verifies "didn't error" + "id_range propagated".
- **UT-0492-02 (`dense_path_taken_below_threshold`)** asserts `subnet.agents.len() == 10` to "verify DENSE path was taken". But `SparseNet::to_dense` (sparse.rs:336) also sizes the dense arena to `max_id + 1 = 10`. Both branches produce the same arena length for this input. Test only verifies "didn't error" + "agent count correct".

**Impact.** Future bugs that route the wrong branch (e.g. a regression in the threshold computation that flips dense ↔ sparse) will not be caught by these named-intent tests. The witness `tests/d011_partition_perf_witness.rs` mitigates the perf-direction case (sparse-when-should-dense) but not the opposite (dense-when-should-sparse).

**Recommended action:**
- Add an explicit branch counter (e.g. `#[cfg(debug_assertions)] static SPARSE_PATH_TAKEN: AtomicBool`) or refactor `build_subnet_with_config` to return a `(Net, BuildPath)` pair in test builds.
- Or, replace the discriminator with one that genuinely differs, e.g. `next_id` or `free_list` length, depending on which fields actually diverge between branches now.
- Note: the post-fix invariant is that the two branches converge on most observable fields — that's the design — so a clean discriminator may require instrumentation.

---

### F-003 [MEDIUM] Empty dense partition under `sparse_build=false` still drops `id_range` and `next_id`

**File:** `relativist-core/src/partition/helpers.rs:297-299` (`build_subnet` empty branch returns `Net::new()`).

**Test attempted:** Trace `build_subnet_with_config` with an empty `worker_agents` and `sparse_build=false`.

**Result — pre-existing bug not addressed by this fix; AF-2 makes it more brittle.**

The QA-D009-009 carve-out at `helpers.rs:499` only forces SPARSE for empty partitions when `sparse_build=true`. When `sparse_build=false`:
- `effective_arena_size = 0`, threshold not exceeded, falls through to dense path.
- `build_subnet` returns `Net::new()` which has `id_range: None` and `next_id: 0`.
- `split.rs:117-118` then wraps with `Partition { ..., id_range: id_ranges[i] }` — but the inner `subnet.id_range` remains `None`.

If anyone later sets `partition.subnet.id_range = Some(non_zero_range)` and calls `create_agent`, AF-2 fires (`next_id (0) < range.start`). This is observed in the `id_range_some_traps_out_of_range_pop` test rewrite — it had to "defer id_range installation until after agent creation" precisely because of this AF-2 interaction.

**Impact.** Pre-existing contract gap (QA-D009-009 only fixes the `sparse_build=true` half). The fix doesn't worsen it directly, but AF-2 means any future caller relying on "empty dense subnet then install id_range then allocate" will now panic in debug.

**Recommended action:**
- Either extend the QA-D009-009 carve-out to fire regardless of `sparse_build` (always preserve id_range/next_id for empty partitions), OR
- Document loudly in `build_subnet`'s rustdoc that empty workers return a barebones `Net::new()` and the inner `id_range` will be None — callers MUST set both `id_range` and `next_id = id_range.start` before `create_agent`.

---

### F-004 [MEDIUM] Threshold boundary `effective_arena_size == 4 * live_count` not explicitly tested

**File:** `relativist-core/src/partition/helpers.rs:1409-1435` (`sparse_build_false_below_threshold_succeeds`) and `:1444-1479` (`sparse_build_false_above_threshold_rejects`).

**Test attempted:** Verify the strict-greater-than (`>`) boundary semantics.

**Result — boundary not directly exercised.**

- The "below_threshold" test uses `live=10, max_live_id=9, eff=10, threshold=40` → comfortably below.
- The "above_threshold" test uses `live=10, max_live_id=45, eff=46, threshold=40` → 6 above the boundary.
- **No test pins the exact `eff == 4*live` case** (e.g. `live=4, max_live_id=15, eff=16, threshold=16` → 16 > 16 is false → must succeed). A future change from `>` to `>=` would not be caught by current tests.

**Recommended action:** Add a 1-line test asserting that `effective_arena_size == 4 * live_count` succeeds (boundary, not exceeded). Mirror in `streaming.rs::tests`.

---

### F-005 [LOW] `freeport_redirects` cloned in full to every partition — memory cost at scale

**File:** `relativist-core/src/partition/helpers.rs:397` (dense) and `:621` (sparse).

**Observation.** Both branches clone `net.freeport_redirects` fully into every worker's subnet. At scale (many workers, dense redirect map), this multiplies redirect-map memory by `num_workers`. The fix is correct (SC-001 second-surface closure requires preserving redirects) and consistent across branches, but the memory bound is `O(workers * |redirects|)` instead of `O(|redirects|)`.

**Recommended action:** None for this audit — flagged for future SPEC-22 perf work. Could be optimized to filter to only the FreePort IDs visible to this worker, but that requires deeper analysis of merge's `rebuild_free_port_index` consumer.

---

### F-006 [LOW] AF-2 / AF-3 are debug-only — release-mode behavior change is null on purpose, but documentation should make this explicit

**Files:** `net/core.rs:536-548` (AF-2), `merge/core.rs:121-125` (AF-3).

**Observation.** Both guards are `#[cfg(debug_assertions)]`. In release builds:
- AF-2: zero behavior change. `create_agent` proceeds with whatever `next_id` is set, potentially allocating outside `id_range`. The downstream impact is the same as pre-fix (cross-partition collision → I1 violation eventually).
- AF-3: zero behavior change. `result.agents[i] = Some(*agent)` overwrites silently as it did before, producing the merge concatenation collision that Bug 2 demonstrated.

The fix's correctness in release mode rests entirely on Bug 1 + Bug 2 source-level fixes (helpers.rs:382, :397). The guards are valuable as **regression detectors in CI/test** but provide no production safety. Commit message and code comments are mostly clear about this, but the release-mode semantics deserve a one-liner in the rustdoc for both guards.

**Recommended action:** Add to AF-2 comment: "Debug-only — release builds rely on `build_subnet`/`build_subnet_sparse` correctness for the underlying invariant." Same for AF-3.

---

## Attacks attempted — full coverage matrix

| # | Attack vector | Result | Severity if found |
|---|---|---|---|
| 1 | Empty partition (dense empty branch) | Pre-existing gap surfaced via AF-2 brittleness | F-003 MEDIUM |
| 2 | id_range start == end | Cannot occur via `compute_id_ranges` (min chunk = 100k); banding clamps to non-degenerate; test code uses Net::new() so AF-2 inactive | None |
| 3 | id_range up to u32::MAX | AF-2 upper bound `next_id < range.end` accepts `next_id = u32::MAX-1`; the existing top-of-function `assert! (next_id < u32::MAX)` catches the overflow first. ✓ | None |
| 4 | Boundary `eff == 4 * live` | Implicit only; no explicit test | F-004 MEDIUM |
| 5 | Recycle path interaction | AF-2 only on fresh allocation; recycle path has its own range check that exempts pre-split IDs (id < range.start). Symmetric and correct. ✓ | None |
| 6 | Merge AF-3 spurious in elastic-grid rejoin | All 9 `grid_delta_integration_tests` PASS; 328 total merge tests PASS. AF-3 only fires on actual D3 violation. ✓ | None |
| 7 | Streaming `PartitionAccumulator::finalize` analog of Bug 2 | Bug present, currently dormant (no production reduction call-site) | F-001 HIGH (latent) |
| 8 | Release-mode behavior of debug guards | Zero behavior change in release; correctness rests on source fixes alone | F-006 LOW |
| 9 | Test brittleness of regression witnesses | Witness assertions are stable across plausible `compute_id_ranges` formula tweaks (chunk_size floor protects). ✓ | None |
| 10 | Category A test intent loss after rewrite | UT-0492-01 / UT-0492-02 lost their branch discriminator | F-002 MEDIUM |
| 11 | Debug-mode perf regression from AF-3 | 328 merge tests in 0.17s; no measurable slowdown. ✓ | None |
| 12 | Other latent bugs in dense `build_subnet` | freeport_redirects fully cloned per worker (memory cost, not correctness) | F-005 LOW |

---

## Verification commands run

```text
cargo test --lib                                  → 1621 passed; 0 failed
cargo test --lib qa_d011                          → 17 passed; 0 failed
cargo test --lib merge::                          → 328 passed; 0 failed (0.17s)
cargo test --lib streaming::                      → 108 passed; 0 failed
cargo test --lib t5_full_integration_*            → 1 passed
cargo build --lib --tests                         → clean (22.9s, debug profile)
```

## Working tree state

Pre-audit and post-audit working trees are byte-identical. No production source, test, spec, plan, or backlog file was modified by this audit. The only file added by this audit is the present QA report at `docs/qa/QA-D011-POST-FIX-AUDIT-2026-05-04.md`. No `dbg!`, `tracing::trace!`, or other temporary instrumentation was added at any point.
