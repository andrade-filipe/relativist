# TASK-0433: v1 re-partition on join (`K_eff_new = K_eff_old + J`) — R12-v1

**Spec:** SPEC-20 §3.2 R12-v1 (re-partition merged net), R13 (recompute id ranges), R14-v1 (joining worker receives full `Partition`).
**Requirements:** R12-v1, R13, R14-v1.
**Priority:** P0 (v1 joining correctness).
**Status:** TODO
**Depends on:** TASK-0421 (id-range recomputation), TASK-0432 (JoinRequest handled), TASK-0435 (join window timing), TASK-0436 (FSM wiring).
**Blocked by:** TASK-0432.
**Estimated complexity:** M (~100-150 LoC production + ~80 LoC tests)
**Bundle:** SPEC-20 Elastic Grid — Phase 2.2 Dynamic Joining.

## Context

v1 re-partition on join: after the current round's `merge()` completes and the FSM closes the join window, `split(merged_net, K_eff_new)` re-partitions with the updated slot count. `compute_id_ranges(K_eff_new)` runs. Joining workers receive `AssignPartition(partition)` (discriminant from SPEC-06 R2), identical to how remote workers receive their round 0 payload.

## Acceptance Criteria

- [ ] Wire the FSM transition `AcceptingMembershipChanges × MembershipWindowClosed → Partitioning` action `InvokeSplitAndDispatch(K_eff_new)`.
- [ ] The split is over the MERGED net (output of the previous round's `merge()`), not over individual partitions.
- [ ] `compute_id_ranges(K_eff_new)` (via TASK-0421).
- [ ] Partitions are dispatched in `partition_index` order: `partitions[0]` → self (if hybrid); `partitions[1..]` → remote workers sorted by WorkerId.
- [ ] Each joining worker receives `AssignPartition` (v1 — SPEC-06 R2).
- [ ] Existing workers receive their new partition as `AssignPartition` (they discard the previous partition state).
- [ ] Round counter increments.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/protocol/coordinator.rs` | modify | `InvokeSplitAndDispatch` action implementation for v1. |

## Test Expectations (forward-ref)

- EG-U5 `test_dynamic_join_repartition_v1` (R12-v1, R13).
- EG-I2 `test_elastic_join_correctness_v1` (R9-R14, R12-v1, G1).

## Invariants Touched

- D1, D2 — preserved via uniform split/merge.
- G1 v1 — PRESERVED via R39-G1-v1.

## Notes

- **v1 behavior only**: delta-mode rejoin is TASK-0446.

## DAG Links

- **Predecessors:** TASK-0421, TASK-0432, TASK-0435.
- **Successors:** EG-U5, EG-I2.
