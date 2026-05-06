# TASK-0467: [SPEC-05 amendment A8] Extend §4.2 `merge` — free-list reconciliation across partitions

**Spec:** SPEC-22 §3.8 A8 (closes SC-003 SPEC-05 amendment scope).
**Requirements:** A8 (formal SPEC-05 §4.2 amendment); enables SPEC-22 R12.
**Priority:** P0 (blocker for TASK-0483 merge free-list reconciliation).
**Status:** TODO
**Depends on:** TASK-0460 (I3'), TASK-0461 (R2 reuse).
**Blocked by:** none
**Estimated complexity:** S (~30 LoC SPEC-05 next-revision diff; no production code)
**Bundle:** SPEC-22 Arena Management — predecessor-spec amendment cluster (Phase A).
**Tag:** `[SPEC-05 amendment]`

## Context

SPEC-05 §4.2 `fn merge(plan: PartitionPlan) -> (Net, u32)` (line 322) currently does not touch free-lists. SPEC-22 R12 says "merge MUST handle free-lists from multiple partitions" but is silent on *how*. The amendment specifies the reconciliation algorithm: walk every input partition's `free_list`; for each ID, check whether the ID is occupied in the merged arena; if `None`, push to merged free-list; if `Some`, discard. Complexity: O(sum of |partition.free_list|). The post-merge free-list satisfies SPEC-22 R6 no-duplicates automatically because pre-merge each partition's free-list is duplicate-free and partitions own disjoint ID ranges (D4).

## Acceptance Criteria

- [ ] ESPECIALISTA EM SPECS lands the SPEC-05 next-revision diff amending §4.2 with the SPEC-22 §3.8 A8 *New text* verbatim.
- [ ] §4.2 specifies the reconciliation algorithm: walk → check occupancy → push-or-discard.
- [ ] §4.2 cross-references SPEC-22 R12, R6 (no duplicates), and SPEC-01 D4 (partition disjointness) as the no-duplicate guarantor.
- [ ] §4.2 declares the complexity: O(sum of |partition.free_list|).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `codigo/relativist/specs/SPEC-05-merge.md` | modify (by ESPECIALISTA EM SPECS only) | §4.2 `merge` amended per SPEC-22 §3.8 A8. |

## Test Expectations (forward-ref)

TEST-SPEC-0467 — covered by T16 (sparse build_subnet → reduce → merge → G1 isomorphism, TASK-0492) and the existing merge-identity tests (TASK-0074 / TASK-0075).

## Invariants Touched

- I3' (preserved; merged free-list is duplicate-free by partition disjointness).
- D4 (consumed; partition disjointness is the no-duplicate guarantor).
- G1 (preserved; merged net is structurally equivalent to sequential reduction).

## Notes

- Without this amendment, R12 ("merge MUST handle free-lists") is unimplementable.
- The reconciliation must NOT reorder live agents; it only constructs the merged free-list.

## DAG Links

- **Predecessors:** TASK-0460, TASK-0461.
- **Successors:** TASK-0483 (merge free-list reconciliation in `src/merge/engine.rs`).
