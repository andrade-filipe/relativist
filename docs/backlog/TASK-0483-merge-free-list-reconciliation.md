# TASK-0483: `merge` free-list reconciliation across partitions (R12 — consumer of A8)

**Spec:** SPEC-22 §3.1 R12; §3.8 A8 (consumes SPEC-05 §4.2 amendment).
**Requirements:** R12 (`merge` MUST handle free-lists from multiple partitions; resulting net's free-list MUST contain only IDs corresponding to `None` slots in the merged arena; IDs whose slots are now occupied MUST NOT appear).
**Priority:** P0 (post-merge correctness; preserves I3' uniqueness on the merged net).
**Status:** TODO
**Depends on:** TASK-0467 (SPEC-05 §4.2 amendment), TASK-0471 (free_list field on Net).
**Blocked by:** none
**Estimated complexity:** S (~50 LoC production + ~80 LoC tests)
**Bundle:** SPEC-22 Arena Management — Phase C (distributed integration).

## Context

`merge(plan: PartitionPlan) -> (Net, u32)` (SPEC-05 §4.2 line 322) currently does not touch free-lists. SPEC-22 R12 mandates the reconciliation algorithm: walk every input partition's `free_list`; for each ID, check whether the ID is occupied in the merged arena; if `None`, push to merged free-list; if `Some`, discard. Complexity: O(sum of |partition.free_list|). The post-merge free-list satisfies SPEC-22 R6 no-duplicates automatically because pre-merge each partition's free-list is duplicate-free and partitions own disjoint ID ranges (D4).

## Acceptance Criteria

- [ ] Modify `merge` in `relativist-core/src/merge/engine.rs` (or current canonical location) to construct the merged net's `free_list` per the A8 algorithm:
  ```
  for each partition in plan.partitions:
      for each id in partition.net.free_list:
          if merged.agents[id as usize].is_none():
              merged.free_list.push(id);
          // else discard (slot was filled by a different partition or by merge boundary resolution)
  ```
- [ ] The merged free-list MUST satisfy R6 no-duplicates (auto-guaranteed by D4 partition disjointness).
- [ ] Post-merge: call `merged.validate_free_list()` (helper from TASK-0475) in debug builds to assert the post-condition.
- [ ] The merged net's `id_range` is set to `None` (whole-net context after merge; partition-locality dissolves).
- [ ] The merged net's `border_entries_shadow` is reset to `None` (delta state cleared; reconstruct will repopulate per SPEC-19 R38).
- [ ] The merged net's `protected_tombstones` (debug) is drained — any tombstones still alive at merge time are turned back into free-list candidates IF their slots are still `None`.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/merge/engine.rs` | modify | Extend `merge` per SPEC-22 §3.8 A8 — free-list reconciliation. |

## Key Types / Signatures

```rust
pub fn merge(plan: PartitionPlan) -> (Net, u32) {
    // ... existing merge logic to construct merged.agents, merged.ports ...
    let mut merged = /* constructed Net */;

    // SPEC-22 R12 / §3.8 A8: free-list reconciliation
    for partition in &plan.partitions {
        for &id in &partition.net.free_list {
            if merged.agents[id as usize].is_none() {
                merged.free_list.push(id);
            }
            // else: slot was filled by another partition — discard
        }
    }

    debug_assert!(merged.validate_free_list().is_ok());
    merged.id_range = None;
    merged.border_entries_shadow = None;

    (merged, merged_next_id)
}
```

## Test Expectations (forward-ref)

TEST-SPEC-0483:
- `merge_combines_partition_free_lists_correctly` — 2 partitions, free-lists `{50, 75}` and `{125, 175}`; merged `{50, 75, 125, 175}` (in some order).
- `merge_discards_filled_slots` — partition free-list contains an ID that the other partition's merge boundary resolution fills; assert it's NOT in the merged free-list.
- `merge_preserves_no_duplicates` — D4 disjointness guarantees this; smoke check.
- T16 (sparse build_subnet → reduce → merge → G1 isomorphism, joint with TASK-0492).

## Invariants Touched

- I3' uniqueness — preserved on the merged net.
- D4 (consumed; partition disjointness is the no-duplicate guarantor).
- G1 — merged net is structurally equivalent to sequential reduction.

## Notes

- The "discard" path in the algorithm is a safety net. In practice, with D4 disjointness, the discard branch should fire rarely (only if merge boundary resolution fills a slot that was previously `None` in a partition's free-list — possible during border-redex resolution that creates a new agent at a recycled ID).

## DAG Links

- **Predecessors:** TASK-0467, TASK-0471.
- **Successors:** TASK-0492 (sparse build_subnet → merge integration), TASK-0500 (regression gate).
