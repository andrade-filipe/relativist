# TASK-0421: ID-range recomputation on `K_eff` change via `compute_id_ranges(K_eff)`

**Spec:** SPEC-20 R8 (self-partition id-range via `partition_index = 0`), R13 (recomputation when K_eff changes, closes SC-014), R30 (ID uniqueness preservation after reclaim).
**Requirements:** R8, R13, R30 (preserves D4 via `partition_index` indexing per R11a).
**Priority:** P0 (hybrid foundation; every re-partition cycle calls this).
**Status:** TODO
**Depends on:** TASK-0411 (`remap_partition_ids`), TASK-0420 (`partition_index_of`).
**Blocked by:** TASK-0420.
**Estimated complexity:** S (~70-100 LoC production + ~60 LoC tests)
**Bundle:** SPEC-20 Elastic Grid — Phase 2.1 Hybrid Coordinator (foundation).

## Context

SPEC-20 R8 clarifies that the self-partition's `IdRange` is obtained by `compute_id_ranges(K_eff)[0]` (partition_index = 0). R13 reaffirms the signature (unchanged from SPEC-04 R18); SPEC-20 only changes *when* it is called (every round where `K_eff` changes) and *what* is passed. R30 ensures that after a reclaimed partition re-enters the system via `remap_partition_ids` + fresh `IdRange`, no ID collision occurs.

## Acceptance Criteria

- [ ] Add `fn compute_round_id_ranges(config: &GridConfig, active: &BTreeSet<WorkerId>) -> Vec<IdRange>` that:
  - Computes `K_eff = active.len() + (1 if hybrid_coordinator else 0)`.
  - Calls SPEC-04 `compute_id_ranges(K_eff)`.
  - Indexes by `partition_index` per R11a.
- [ ] For hybrid mode, index `[0]` is the self-partition's range; remote workers consume `[1..K_eff]` by ascending `WorkerId`.
- [ ] Post-call: for each partition, set sub-net `next_id = max(range.start, max_agent_id_in_partition + 1)` (SPEC-04 R18 step).
- [ ] Recomputation is triggered by the FSM `AcceptingMembershipChanges -> Partitioning` transition (TASK-0436) with the updated `K_eff`.
- [ ] After a reclaim + re-split (TASK-0440, TASK-0443), `remap_partition_ids` (TASK-0411) renumbers the reclaimed partition into one of the new disjoint ranges.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/partition/id_ranges.rs` *(new or existing)* | modify | Add `compute_round_id_ranges`. |

## Test Expectations (forward-ref)

- EG-U3 `test_hybrid_self_partition_id_range` (R8).
- EG-U5 `test_dynamic_join_repartition_v1` (R12-v1, R13).
- EG-U12 `test_id_ranges_no_collision_after_repartition` (R13, R30, R11a).

## Invariants Touched

- D4 (ID Uniqueness) + D4-elastic — preserved via dense `partition_index` indexing.

## Notes

- **Signature fidelity**: `compute_id_ranges(K_eff) -> Vec<IdRange>` — does NOT take `next_id` (that was a wording error in SPEC-20 v1 draft, fixed in R8).

## DAG Links

- **Predecessors:** TASK-0411, TASK-0420.
- **Successors:** TASK-0430 (hybrid dispatch), TASK-0433 (joining re-partition), TASK-0440 (departure re-split).
