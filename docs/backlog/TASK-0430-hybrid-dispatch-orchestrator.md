# TASK-0430: Hybrid dispatch orchestration — `K_eff = K+1` partitioning + self-spawn wiring

**Spec:** SPEC-20 §3.1 R1 (hybrid reduction), R2 (K_eff = K + 1), R3b (continue processing events while self-reduce in-flight), R4-v1 (merge includes self).
**Requirements:** R1, R2, R3b, R4-v1.
**Priority:** P0 (wires hybrid mode end-to-end for v1).
**Status:** TODO
**Depends on:** TASK-0420 (WorkerId reservation), TASK-0421 (id ranges), TASK-0422 (select loop), TASK-0423 (self-spawn).
**Blocked by:** TASK-0423.
**Estimated complexity:** M (~150-200 LoC production + ~100 LoC tests)
**Bundle:** SPEC-20 Elastic Grid — Phase 2.1 Hybrid Coordinator.

## Context

With hybrid enabled, `Partitioning` step produces `K_eff = K + 1` partitions via `split(net, K_eff)`. `partition_index = 0` (self), `1..K_eff` (remote). Dispatching sends `AssignPartition` (v1) to remotes AND spawns the self-worker via `spawn_self_partition` (TASK-0423). `WaitingForResults` awaits `K_eff` results (K remote + 1 self). R3b: membership/timeout events are processed concurrently while self-reduce is in-flight.

## Acceptance Criteria

- [ ] `Partitioning` state: `let partitions = split(net, K_eff)` where `K_eff = W_active.len() + (1 if hybrid)`.
- [ ] For hybrid: self-partition = `partitions[0]`; remote partitions = `partitions[1..]`.
- [ ] `Dispatching`: send `AssignPartition(p_i)` to each remote worker (sorted by WorkerId); then `SpawnSelfPartition(partitions[0])` action triggers TASK-0423's spawn helper.
- [ ] Arm `StartTimer(Collect)` at end of Dispatching.
- [ ] `WaitingForResults` counts `K_eff` expected results — the self-worker's `PartitionResult` flows through the channel identically to remote.
- [ ] `Merging`: pass all `K_eff` results to `merge()` (SPEC-05 R1-R11). No special self-branch (R4-v1 MUST).
- [ ] FSM transitions as listed in §4.1.4 for `Partitioning`, `Dispatching`, `WaitingForResults`, `Merging`.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/protocol/coordinator.rs` | modify | Hybrid branches in Partitioning/Dispatching/Merging; K_eff bookkeeping. |
| `relativist-core/src/merge/` | modify (minor) | Ensure merge handles K_eff partitions uniformly. |

## Test Expectations (forward-ref)

- EG-U1 `test_hybrid_coordinator_single_machine` (R1, R5).
- EG-U2 `test_hybrid_partition_count` (R2).
- EG-U3 `test_hybrid_self_partition_id_range` (R8).
- EG-U4 `test_hybrid_merge_includes_self` (R4-v1).
- EG-I1 integration `test_hybrid_grid_correctness_v1` (R1, R4-v1, G1).

## Invariants Touched

- D1 (Split/Merge Identity) — preserved: K_eff partitions re-merged uniformly.
- D2 — preserved.
- G1 (Fundamental Property, v1) — PRESERVED via R39-G1-v1.

## Notes

- **No special self-branch in merge**: the correctness argument of R4-v1 depends on the self-partition flowing through the identical code path.

## DAG Links

- **Predecessors:** TASK-0420, TASK-0421, TASK-0422, TASK-0423.
- **Successors:** TASK-0437 (delta self-worker symmetry), TASK-0436 (FSM wiring), EG-I1.
