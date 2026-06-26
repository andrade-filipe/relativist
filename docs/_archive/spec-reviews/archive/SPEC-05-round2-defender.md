# SPEC-05 -- Round 2: Defender Response

**Date:** 2026-04-05
**Defender:** Especialista em Specs
**Target:** SPEC-05-merge.md
**Critic review:** SPEC-05-round1-critic.md
**Spec version:** Revised v2 -> Revised v3

---

## Summary

| Response | Count |
|----------|-------|
| ACCEPTED | 12 |
| PARTIALLY ACCEPTED | 2 |
| NOT ADDRESSED | 0 |
| **Total issues** | **14** |

---

## Responses

### SC-001: WorkerRoundStats definition is stale -- missing SPEC-11 extensions
**Severity:** CRITICAL
**Response:** ACCEPTED
**Action taken:** Updated R37 from SHOULD to MUST and added the two fields from SPEC-11 OQ-1:
- `reduce_duration_secs: f64` -- wall-clock duration of reduce_all
- `interactions_by_rule: [u64; 6]` -- per-rule interaction counts [CON-CON, CON-DUP, CON-ERA, DUP-DUP, DUP-ERA, ERA-ERA]

Added `serde::Serialize, serde::Deserialize` derives to the struct definition (Section 4.1), as required by SPEC-06 R12 for wire transmission.

Added an explanatory note on the tension between SPEC-03's 4 rule categories (anni, comm, eras, void) and the 6-element array. The reduction engine's `dispatch` function already identifies the specific `(Symbol, Symbol)` pair, so tracking 6 counters is a straightforward extension. Raised OQ-4 as a cross-spec concern for SPEC-03 R17.

The grid loop pseudocode (Section 4.5) now shows how `ReductionStats` flows from `reduce_all` into `WorkerRoundStats.interactions_by_rule` via a `reduction_stats_to_by_rule` helper.

SPEC-11 OQ-1 is declared resolved in the R37 text.
**Spec sections modified:** Section 3.7 (R37), Section 4.1 (WorkerRoundStats struct), Section 4.5 (pseudocode), Section 7 (OQ-4 added)

### SC-002: Termination condition in run_grid uses only redex_queue.is_empty(), inconsistent with SPEC-13 is_normal_form
**Severity:** CRITICAL
**Response:** ACCEPTED
**Action taken:** Restructured the grid loop pseudocode (Section 4.5) to check termination after merge + reduce_all (Phase 4), not at the top of the loop. The termination check now appears as step [7] in the cycle diagram.

The initial Normal Form check (before any rounds) is preserved as a separate step [0] with an explanatory note: "In distributed context (SPEC-13), the coordinator MAY perform this check before entering the BSP loop, or MAY execute one round and detect Normal Form during CheckTermination. Both approaches are correct."

Added a note below the pseudocode explicitly connecting the post-merge termination check to SPEC-13's `CheckTermination` FSM state: "The `is_normal_form` field in SPEC-13's `MergeComplete` event is logically equivalent to the `current_net.redex_queue.is_empty()` check after `drain_stale_redexes`."

Updated Section 4.7 (Grid Cycle Diagram) to reflect the new structure with step [7] at the end of the loop body.

The informal proof (Section 4.6) was updated: the statement about termination "at the beginning of the next iteration" now reads "at the termination check (step [7]) at the end of the current round."
**Spec sections modified:** Section 4.5 (pseudocode restructured), Section 4.6 (informal proof), Section 4.7 (diagram)

### SC-003: merge() return type inconsistency with SPEC-13 InvokeMergeAndReduce
**Severity:** HIGH
**Response:** ACCEPTED
**Action taken:** Three changes:

(a) Updated R1 to accept `PartitionPlan` by value instead of separate `Vec<Partition>` + `&HashMap`. The rationale: `PartitionPlan` already bundles partitions and borders; accepting it by value avoids Rust partial-move issues; and it aligns with SPEC-04's `PartitionPlan` as the canonical interchange type.

(b) Clarified in R1 that `merge()` is the pure merge step and `reduce_all()` is called separately by the caller. SPEC-13's `InvokeMergeAndReduce` is a coordinator-level convenience action that encapsulates both. The `is_normal_form` field in `MergeComplete` is computed after `reduce_all`, not by `merge()` itself.

(c) Updated the Section 4.2 pseudocode signature to `fn merge(plan: PartitionPlan) -> (Net, u32)` with a note explaining the destructuring pattern. Updated the grid loop pseudocode to call `merge(plan)` instead of `merge(plan.partitions, &plan.borders)`.
**Spec sections modified:** Section 3.1 (R1), Section 4.2 (merge signature and note), Section 4.5 (pseudocode)

### SC-004: GridMetrics lacks SPEC-11's per-rule interaction tracking
**Severity:** HIGH
**Response:** ACCEPTED
**Action taken:** Added `total_interactions_by_rule: [u64; 6]` to both the R35 requirements list and the `GridMetrics` struct (Section 4.1). Each round, the coordinator accumulates worker-reported per-rule counts (from `WorkerRoundStats.interactions_by_rule`) plus border-resolution per-rule counts (from the coordinator's own `reduce_all` ReductionStats).

The grid loop pseudocode (Section 4.5) now shows the per-rule accumulation loop:
```
for i in 0..6:
    metrics.total_interactions_by_rule[i] +=
        local_by_rule[i] + border_by_rule[i]
```

This ensures both the benchmark CSV output (SPEC-09) and Prometheus metrics (SPEC-11 R12 `interactions_by_rule_total`) have access to per-rule breakdowns from the same source of truth.
**Spec sections modified:** Section 3.7 (R35), Section 4.1 (GridMetrics struct), Section 4.5 (pseudocode)

### SC-005: Merge discards partition redex queues -- potential correctness concern
**Severity:** HIGH
**Response:** ACCEPTED
**Action taken:** Adopted option (b) from the critic's suggestions, with the addition of a debug assertion:

(a) Updated R9 to explicitly state that partition redex queues are discarded because `reduce_all` (SPEC-03 R13) guarantees that after full local reduction, the partition queue contains only stale entries. This replaces the previous R9 which ambiguously mentioned "any internal redexes remaining from each partition's local reduction."

(b) Updated R18 to be consistent with the new R9: it now clarifies that the merged queue contains only border-origin redexes, since partition queues are guaranteed stale after `reduce_all`.

(c) Added a debug assertion in Section 4.2 (Step 2 note) that verifies no non-stale redexes exist in any partition queue at merge time. This catches any violation of the invariant that would indicate a bug in the reduction engine.

The internal inconsistency between R9 and R18 (original) is fully resolved. Both requirements now agree: partition queues after `reduce_all` contain only stale entries, so discarding them is safe and correct.
**Spec sections modified:** Section 3.1 (R9), Section 3.3 (R18), Section 4.2 (Step 2 note with debug assertion)

### SC-006: run_grid signature uses &dyn PartitionStrategy but SPEC-13 uses usize for num_workers
**Severity:** MEDIUM
**Response:** ACCEPTED
**Action taken:** Added an explicit note in Section 4.5 confirming that `GridConfig.num_workers` is `u32`, consistent with SPEC-04's `WorkerId` type. SPEC-13's `run_grid_local` (Section 4.5 of that spec) already uses `u32` after the SPEC-13 Round 2 revision (SC-015). The remaining discrepancy (SPEC-13's `InvokeSplit { net: Net, num_workers: usize }` using `usize`) is a SPEC-13 internal concern: the coordinator FSM action type is not part of the SPEC-05 contract.
**Spec sections modified:** Section 4.5 (added note on num_workers type)

### SC-007: No specification for handling FreePort (Lafont) during merge
**Severity:** MEDIUM
**Response:** ACCEPTED
**Action taken:** Updated merge Step 2 (Section 4.2) to distinguish between Boundary FreePorts and Lafont FreePorts. The border map serves as the discriminator:
- If `FreePort(fid)` where `fid` is in the border map: this is a Boundary FreePort. Temporarily mark as `DISCONNECTED` (handled in Step 3).
- If `FreePort(fid)` where `fid` is NOT in the border map: this is a Lafont FreePort (pre-existing interface port). Copy directly to the result net.

Added an explanatory note referencing SPEC-04 R15 and SPEC-00 Sections 6.1/6.2, explaining why both FreePort types share the same variant and how the border map disambiguates.

This fix ensures that the merge correctly preserves the interface of the net for nets with pre-existing Lafont FreePorts, satisfying D1 (split/merge identity).
**Spec sections modified:** Section 4.2 (Step 2 pseudocode and note)

### SC-008: compute_time_per_round conflates reduction time and free_port_index reconstruction time
**Severity:** MEDIUM
**Response:** PARTIALLY ACCEPTED
**Action taken:** Added R35a as a SHOULD requirement for `index_rebuild_time_per_round: Vec<Duration>` in GridMetrics. Added the field to the `GridMetrics` struct. The grid loop pseudocode shows `t_index = Instant::now()` around `rebuild_free_port_index`.

The fix differs from the critic's suggestion in that R35a is SHOULD rather than MUST. In the local simulation mode, the index rebuild happens in-process and may be negligible compared to reduce_all. The ENGINEER may choose to omit the field in v1 if it adds complexity without measurable benefit. The `compute_time_per_round` field now has an updated doc comment noting that it includes rebuild time unless separately tracked.

The timing infrastructure is specified; the decision to populate it is left to the ENGINEER.
**Spec sections modified:** Section 3.7 (added R35a), Section 4.1 (GridMetrics struct), Section 4.5 (pseudocode timing)

### SC-009: merge_time_per_round conflates merge and border resolution
**Severity:** MEDIUM
**Response:** PARTIALLY ACCEPTED
**Action taken:** Split the timing in two:
- `merge_time_per_round: Vec<Duration>` -- structural merge time only (Phase 4 in the pseudocode)
- `border_reduce_time_per_round: Vec<Duration>` -- time for `reduce_all` after merge (Phase 5)

Updated both R35 and the `GridMetrics` struct (Section 4.1). The grid loop pseudocode now times merge and border resolution separately:
```
let t_merge = Instant::now()
let (merged_net, border_redex_count) = merge(plan)
metrics.merge_time_per_round.push(t_merge.elapsed())

let t_border = Instant::now()
let border_stats = reduce_all(&mut merged_net)
metrics.border_reduce_time_per_round.push(t_border.elapsed())
```

This enables SPEC-09 benchmarks to distinguish protocol overhead (structural merge) from useful computation (border reduction), as the critic correctly identified.

The fix differs from the critic's suggestion in that the original `merge_time_per_round` is redefined to exclude border resolution (rather than keeping it as-is and adding a second field). This is cleaner since the field name now accurately describes what it measures.
**Spec sections modified:** Section 3.7 (R35), Section 4.1 (GridMetrics struct), Section 4.5 (pseudocode timing)

### SC-010: drain_stale_redexes approach may mask lost-redex bugs
**Severity:** MEDIUM
**Response:** ACCEPTED
**Action taken:** Promoted OQ-3 to R41 as a SHOULD requirement: "After the final `reduce_all` in each round, and before declaring Normal Form, the grid loop SHOULD perform a full scan of the port array to detect any redexes not present in the queue. This scan SHOULD be enabled by default in debug mode (`#[cfg(debug_assertions)]`) and MAY be disabled in release mode via a configuration option (e.g., `GridConfig.verify_normal_form: bool`)."

Added the full scan call (`verify_no_redexes_full_scan`) to the termination check in the grid loop pseudocode (Section 4.5), guarded by `#[cfg(debug_assertions)]`.

OQ-3 is now marked as resolved in Section 7. The rationale in Section 4.4 preserves both the drain approach (baseline) and the full scan (defense-in-depth), with the latter now mandated as SHOULD.
**Spec sections modified:** Section 4.4 (R41 added, alternative note updated), Section 4.5 (pseudocode), Section 7 (OQ-3 resolved)

### SC-011: No explicit specification for initial round's normal form check
**Severity:** LOW
**Response:** ACCEPTED
**Action taken:** The restructured grid loop (Section 4.5) now has an explicit initial Normal Form check (step [0]) before the main loop, with a note explaining the discrepancy with SPEC-13's FSM:

"In distributed context (SPEC-13), the coordinator MAY perform this check before entering the BSP loop, or MAY execute one round and detect Normal Form during CheckTermination. Both approaches are correct."

The cycle diagram (Section 4.7) shows step [0] as a separate pre-loop check that short-circuits to "NORMAL FORM (0 rounds)" if the net is already reduced. This clarifies the behavior for all cases: empty net, non-empty but already reduced net, and normal net with redexes.
**Spec sections modified:** Section 4.5 (pseudocode), Section 4.7 (diagram)

### SC-012: Border map is passed by reference but partitions are consumed by merge
**Severity:** LOW
**Response:** ACCEPTED
**Action taken:** Resolved by SC-003's change: `merge()` now accepts `PartitionPlan` by value, consuming both partitions and borders in a single move. Added a note in Section 4.2 explaining the internal destructuring: `let PartitionPlan { partitions, borders, .. } = plan;`. This eliminates the partial-move issue entirely.
**Spec sections modified:** Section 4.2 (merge signature note) -- same change as SC-003

### SC-013: R19 (border_interactions_per_round) is SHOULD, but the grid loop pseudocode always collects it
**Severity:** LOW
**Response:** ACCEPTED
**Action taken:** Upgraded R19 from SHOULD to MUST. This is consistent with R35 which lists `border_interactions_per_round` as a MUST field, and with the grid loop pseudocode which unconditionally populates it.
**Spec sections modified:** Section 3.3 (R19)

### SC-014: No specification for how reduce_all reports per-rule interaction counts
**Severity:** LOW
**Response:** ACCEPTED
**Action taken:** The grid loop pseudocode (Section 4.5) now shows the complete data flow:

1. `reduce_all(&mut partition.subnet)` returns `ReductionStats` (SPEC-03 Section 4.6.2)
2. `reduction_stats_to_by_rule(&reduction_stats)` maps the 4-category stats to the 6-element `[u64; 6]` array
3. The per-rule array is stored in `WorkerRoundStats.interactions_by_rule`
4. The coordinator accumulates per-rule counts from all workers and from border resolution into `GridMetrics.total_interactions_by_rule`

Added an informative note explaining the `reduction_stats_to_by_rule` helper and the dependency on SPEC-03's evolution (OQ-4). If SPEC-03 is extended with 6 direct counters, the helper becomes trivial.

This is a cross-spec gap (SPEC-03 -> SPEC-05 -> SPEC-11). The gap is documented as OQ-4 and bridged by the helper function in the pseudocode.
**Spec sections modified:** Section 4.5 (pseudocode), Section 7 (OQ-4)

---

## Changes Made to SPEC-05

### Header
- Status changed from "Revised v2" to "Revised v3"
- Added review history line

### Section 3.1 (Partition Merge)
- R1: Changed merge signature to accept `PartitionPlan` by value. Added clarification on relationship to SPEC-13's `InvokeMergeAndReduce` (SC-003)
- R9: Rewritten to clarify that partition queues are discarded (only stale entries after reduce_all). Added debug assertion SHOULD. Removed ambiguous "any internal redexes remaining" language (SC-005)

### Section 3.3 (Border Redex Resolution)
- R18: Rewritten for consistency with R9. Now explicitly states merged queue contains only border-origin redexes (SC-005)
- R19: Upgraded from SHOULD to MUST (SC-013)

### Section 3.7 (Metrics)
- R35: Added `total_interactions_by_rule: [u64; 6]` field. Split `merge_time_per_round` into `merge_time_per_round` (structural) + `border_reduce_time_per_round`. Updated `compute_time_per_round` doc comment (SC-004, SC-008, SC-009)
- R35a: New SHOULD requirement for `index_rebuild_time_per_round` (SC-008)
- R37: Upgraded from SHOULD to MUST. Added `reduce_duration_secs: f64` and `interactions_by_rule: [u64; 6]`. Added serde derives. Added explanatory note on SPEC-03 mapping. Declared SPEC-11 OQ-1 resolved (SC-001, SC-014)

### Section 4.1 (Types -- GridMetrics)
- Added `total_interactions_by_rule: [u64; 6]` field with doc comment (SC-004)
- Changed `merge_time_per_round` doc to "Structural merge time" (SC-009)
- Added `border_reduce_time_per_round: Vec<Duration>` field (SC-009)
- Added `index_rebuild_time_per_round: Vec<Duration>` field (SC-008)

### Section 4.1 (Types -- WorkerRoundStats)
- Added `serde::Serialize, serde::Deserialize` derives (SC-001)
- Added `reduce_duration_secs: f64` field (SC-001)
- Added `interactions_by_rule: [u64; 6]` field (SC-001)
- Added doc comment declaring canonical source and SPEC-11 OQ-1 resolution

### Section 4.2 (Merge Algorithm)
- Changed merge signature to `fn merge(plan: PartitionPlan) -> (Net, u32)` (SC-003, SC-012)
- Added note on API design and internal destructuring (SC-012)
- Step 2: Added FreePort (Lafont) vs FreePort (Boundary) distinction using border map as discriminator (SC-007)
- Step 2: Added explanatory note on FreePort distinction referencing SPEC-04 R15 and SPEC-00 (SC-007)
- Step 2: Added debug assertion on partition queue staleness (SC-005)
- Step 3: Changed `borders` to `&borders` (consistent with by-value plan consumption)

### Section 4.4 (Stale Redex Draining)
- Added R41: SHOULD requirement for full-scan Normal Form verification (SC-010)
- Renamed "Alternative" to "Alternative (informative)" and updated rationale (SC-010)

### Section 4.5 (Grid Loop Algorithm)
- Added note on `num_workers` type consistency (SC-006)
- Restructured pseudocode: termination check now after merge+reduce_all instead of at top of loop (SC-002)
- Added initial Normal Form check before the main loop with explanatory note (SC-011)
- Split timing: merge and border resolution timed separately (SC-009)
- Added `reduction_stats_to_by_rule` helper and per-rule accumulation (SC-001, SC-004, SC-014)
- WorkerRoundStats construction now includes `reduce_duration_secs` and `interactions_by_rule` (SC-001)
- Added `#[cfg(debug_assertions)] verify_no_redexes_full_scan` call at termination check (SC-010)
- Added note connecting termination check to SPEC-13's CheckTermination FSM state (SC-002)
- Added note on `reduction_stats_to_by_rule` informative helper (SC-014)

### Section 4.6 (Informal Proof)
- Updated "at the beginning of the next iteration" to "at the termination check (step [7]) at the end of the current round" (SC-002)

### Section 4.7 (Grid Cycle Diagram)
- Complete rewrite to reflect new loop structure with step [0] initial check and step [7] termination check (SC-002, SC-011)

### Section 7 (Open Questions)
- OQ-3: Marked as resolved, promoted to R41 (SC-010)
- OQ-4: New cross-spec concern for SPEC-03 per-rule tracking (SC-001, SC-014)

---

## Cross-Spec Impact

| Spec | Impact | Action needed |
|------|--------|---------------|
| SPEC-03 | OQ-4: `ReductionStats` SHOULD be extended with 6 per-rule counters | During SPEC-03's next revision |
| SPEC-06 | R12: `WorkerRoundStats` now has 6 fields (was 4). Serialized struct size increases. No structural change needed in SPEC-06 since R12 references SPEC-05's canonical definition | None (automatic) |
| SPEC-11 | OQ-1 resolved: SPEC-05 R37 now contains the extended fields. SPEC-11 Section 4.4's definition is no longer normative; SPEC-05 is canonical | SPEC-11 OQ-1 can be closed |
| SPEC-13 | Termination check alignment confirmed. `InvokeSplit.num_workers: usize` is a SPEC-13 internal concern, not a SPEC-05 contract issue | None required |
| SPEC-09 | GridMetrics now has `total_interactions_by_rule`, `border_reduce_time_per_round`, and `index_rebuild_time_per_round` -- richer data for benchmark analysis | SPEC-09 may reference new fields |
