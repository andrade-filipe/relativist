# TEST-SPEC-0483: `merge` free-list reconciliation across partitions (R12 / A8)

**SPEC-22 §7 ID:** T16 (joint integration coverage with TEST-SPEC-0492) plus this plumbing file.
**Owning task:** TASK-0483.
**Parent spec:** SPEC-22 §3.1 R12; §3.8 A8.
**Type:** unit.

---

## Inputs / Fixtures

- A `PartitionPlan` with 2 partitions:
  - Partition 0: range `0..100`, post-reduction free-list `[50, 75]`, agents at `{0..49, 51..74, 76..99}` minus consumed.
  - Partition 1: range `100..200`, post-reduction free-list `[125, 175]`, agents at `{100..124, 126..174, 176..199}` minus consumed.
- The merged net's arena is the concatenation; agents from both partitions placed in their own ID slots.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0483-01 | `merge_combines_partition_free_lists_correctly` | 2 partitions with free-lists `[50, 75]` and `[125, 175]` | `let (merged, next_id) = merge(plan);` | `merged.free_list.iter().copied().collect::<HashSet<_>>() == {50, 75, 125, 175}`. |
| UT-0483-02 | `merge_post_condition_validate_free_list_passes` | UT-0483-01 result | `merged.validate_free_list()` | `Ok(())` — every ID in free-list corresponds to a `None` slot. |
| UT-0483-03 | `merge_discards_filled_slots` | partition 0 free-list contains `[50]` BUT the merge boundary resolution fills slot 50 with a new agent (synthetic test setup) | merge | `!merged.free_list.contains(&50)`. (Discard branch in §3.8 A8 algorithm.) |
| UT-0483-04 | `merge_preserves_no_duplicates` | both partition free-lists are duplicate-free; D4 disjointness enforced | merge | `merged.free_list` has no duplicates. (Auto-guaranteed by D4; smoke check.) |
| UT-0483-05 | `merge_resets_id_range_to_none` | both partitions have `id_range = Some(...)` | merge | `merged.id_range == None`. (Whole-net context post-merge.) |
| UT-0483-06 | `merge_resets_border_entries_shadow_to_none` | both partitions have `border_entries_shadow = Some(...)` | merge | `merged.border_entries_shadow == None`. |
| UT-0483-07 | `merge_drains_protected_tombstones` (debug-only) | one partition has `protected_tombstones = Some({47})` and `agents[47] == None` post-reduction | merge | `merged.free_list.contains(&47) == true` (the tombstone is reclaimable since the slot is `None`). `merged.protected_tombstones` cleared. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Single partition (no reconciliation needed) | Merged free-list equals the single partition's free-list verbatim. |
| EC-2 | Both partitions empty free-list | Merged free-list empty. |
| EC-3 | One partition's free-list contains an ID outside its declared range (D4 violation) | The merge proceeds (no defensive D4 check at merge time); the resulting free-list MAY contain the violating ID. (Documents that merge doesn't re-validate D4; that's `build_subnet`'s responsibility.) |

## Invariants asserted

- R12 (merge handles free-lists correctly).
- §3.8 A8 (algorithm specified).
- I3' uniqueness on merged net.
- D4 (consumed; partition disjointness is the no-duplicate guarantor).

## ARG/DISC/REF citation

- ARG-002 P5 (merge identity).

## Determinism notes

The merge algorithm is deterministic given a fixed partition order. Pure synchronous; no tokio.

## Cross-test dependencies

- T16 covers the integration-level merge correctness via G1.
- TEST-SPEC-0467 (SPEC-05 amendment) is forward-referenced; A8 is recorded in SPEC-22 §3.8.
- TEST-SPEC-0475's `validate_free_list` helper is consumed.
