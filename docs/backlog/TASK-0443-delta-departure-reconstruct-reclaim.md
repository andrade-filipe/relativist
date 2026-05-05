# TASK-0443: Delta-mode departure reclaim + `reconstruct` + re-`split` (R24a-delta, R24b-delta, R27 hybrid-delta fallback)

**Spec:** SPEC-20 §3.3.4 R24a-delta (catastrophic, uses retained_initial), R24b-delta (CONDITIONAL on ARG-005; uses retained_last_acked optimized), R24b-delta conservative fallback (always use retained_initial per R29a), R25, R26, R27, R28 (WARN logging).
**Requirements:** R24a-delta (CLOSED via ARG-006), R24b-delta (CLOSED-conservative / CONDITIONAL-optimized per R29a), R25, R26, R27, R28.
**Priority:** P0 (delta departure correctness).
**Status:** TODO
**Depends on:** TASK-0410 (Net::union), TASK-0411 (remap + allocate_border_ids), TASK-0412 (reconstruct 3-arg), TASK-0437 (delta self-worker), TASK-0439 (retained state), TASK-0436 Phase C.
**Blocked by:** TASK-0412, TASK-0437, TASK-0439.
**Estimated complexity:** M (~180-250 LoC production + ~150 LoC tests) — *if exceeds 200 LoC, split into 0443a (R24a conservative) / 0443b (R24b optimized behind feature flag)*.
**Bundle:** SPEC-20 Elastic Grid — Phase 2.3 Dynamic Departure (delta mode).

## Context

Delta-mode reclaim: different materialization from v1. Reclaimed `Partition` is obtained by applying snapshot's deltas to `retained_initial` (lightweight) OR by reading the checkpointed `Partition` (if `checkpoint_partitions = true`). Subsequent steps:
1. Broadcast `FinalStateRequest` to surviving workers → collect `FinalStateResult.partition`.
2. `reconstruct(border_graph, surviving_partitions, reclaimed_partitions)` (3-arg per A8, TASK-0412).
3. `split(merged_net, K_eff_new)`.
4. Fresh `InitialPartition` to all members of new `W_active`; new `BorderGraph` from new `PartitionPlan`.

**Conservative vs Optimized (R24b path)**:
- **Conservative (CLOSED via ARG-006)**: always materialize from `retained_initial[w]` (skip deltas). Safe unconditionally.
- **Optimized (CONDITIONAL on ARG-005)**: use `retained_last_acked` = `(border_graph_snapshot, last_deltas)` — cheaper, but requires SPEC-19 border-completeness guarantees.

**Default ship behavior per R29a**: feature-flag optimized path; default to conservative.

## Acceptance Criteria

- [ ] On worker departure in delta mode, emit `ReclaimPartition(w, auto)` → reclaim orchestrator routes to delta branch.
- [ ] **Conservative path (default)**: materialize reclaimed `Partition` from `retained_initial[w]` only — no delta application.
- [ ] **Optimized path (feature-gated behind `--delta-optimized-reclaim` or similar)**:
  - If `checkpoint_partitions = true`: use `retained_last_acked.DeltaCheckpoint(Partition)` directly.
  - Else: apply `(border_graph_snapshot, last_deltas)` to `retained_initial` to materialize current state.
- [ ] Broadcast `FinalStateRequest` to surviving workers (if any); collect `FinalStateResult.partition`.
- [ ] `remap_partition_ids` on each reclaimed partition (TASK-0411).
- [ ] `reconstruct(border_graph, surviving_partitions, reclaimed_partitions)` (3-arg, TASK-0412).
- [ ] `split(merged_net, K_eff_new)`.
- [ ] `InitialPartition` (SPEC-19 R31) to all new W_active members; `BorderGraph` re-initialized from new `PartitionPlan`.
- [ ] R26 multi-departure: single cycle.
- [ ] R27: if all remote depart + hybrid → `SoloReducing` with self-partition still progressing (delegated to TASK-0442).
- [ ] R28 WARN log: worker_id, departure type, round, slot consumed.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/protocol/coordinator.rs` | modify | Delta reclaim branch; optimized path feature-gate. |
| `relativist-core/src/partition/departure_recovery.rs` | modify | Delta materialization helpers. |

## Test Expectations (forward-ref)

- EG-U7c `test_departure_reclaim_last_acked_delta` (R23c-delta, R24b).
- EG-U10b `test_graceful_leave_urgent_delta` (R22b).
- EG-I3-delta `test_elastic_departure_correctness_delta` (R18-R26, G1 CLOSED-conservative / CONDITIONAL-optimized).
- EG-P5 CON-DUP heavy × churn (delta mode) — CONDITIONAL on ARG-005 for optimized variant.
- EG-P6 `prop_delta_elastic_correctness` (R0a, G1 CONDITIONAL).

## Invariants Touched

- D3, D4, D5, D6 — preserved via the clean-boundary re-introduction rule (R24c).
- G1 — CLOSED via ARG-006 for conservative path; CONDITIONAL on ARG-005 for optimized R24b path.

## Notes

- **Default ship**: conservative path (R24a-style) is unconditionally safe. Optimized path is opt-in until ARG-005 formally closes (ARG-005 is already CLOSED per §9 Open Issue 1, so the feature flag can be flipped once implementation stabilizes).
- **ARG-006 closed**: v1 and delta-conservative no longer conditional.

## DAG Links

- **Predecessors:** TASK-0410, TASK-0411, TASK-0412, TASK-0437, TASK-0439.
- **Successors:** TASK-0442 (D==K_eff delta branch), EG-I3-delta, EG-P5, EG-P6.
