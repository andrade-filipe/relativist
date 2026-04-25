# TASK-0446: Delta-mode rejoin cycle — mid-run `FinalStateRequest` + reconstruct + fresh `InitialPartition` (R12-delta, R14-delta)

**Spec:** SPEC-20 §3.2 R12-delta (rejoin cycle, closes alternative incremental-split optimization), R12a (acceptable one-time v1-equivalent wire cost), R14-delta (joining worker receives `InitialPartition`).
**Requirements:** R12-delta, R12a, R14-delta.
**Priority:** P0 (delta joining correctness).
**Status:** TODO
**Depends on:** TASK-0412 (reconstruct 3-arg — though for joining, `reclaimed_partitions = Vec::new()`), TASK-0432 (JoinRequest handler), TASK-0437 (delta self-worker), TASK-0435 (join window).
**Blocked by:** TASK-0432.
**Estimated complexity:** M (~130-180 LoC production + ~100 LoC tests)
**Bundle:** SPEC-20 Elastic Grid — Phase 2.2 Dynamic Joining (delta mode).

## Context

Delta-mode rejoin: at the join window, the coordinator instructs surviving workers to send `FinalStateResult` (mid-run `FinalStateRequest`), then `reconstruct(border_graph, worker_partitions)` into a single net, `split()` for K_eff_new slots, dispatch fresh `InitialPartition` to all members of new `W_active`. Previously-active workers discard old partition and adopt new one. Subsequent rounds revert to delta-mode wire cost. R12a: rejoin-round wire cost is v1-equivalent (acceptable one-time overhead per `c_o_join`).

## Acceptance Criteria

- [ ] On `MembershipWindowClosed` in delta mode with J > 0 joiners:
  - Broadcast `FinalStateRequest` to surviving workers.
  - Collect `FinalStateResult.partition` from each.
  - `merged = reconstruct(border_graph, surviving_partitions, Vec::new())` (3-arg; empty reclaimed).
  - `partitions = split(merged, K_eff_new)`.
  - Dispatch fresh `InitialPartition { round: N+1, partition: p }` to ALL members of new `W_active` (previously-active + new joiners).
  - Re-initialize `BorderGraph` from the new `PartitionPlan` (SPEC-19 R10).
- [ ] Workers receiving the fresh `InitialPartition` discard any previous partition/border state and adopt the new one.
- [ ] Subsequent rounds revert to delta wire cost.
- [ ] Metrics: `join_round_overhead_ms_per_round` records this round's overhead.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/protocol/coordinator.rs` | modify | Delta rejoin cycle orchestration. |
| `relativist-core/src/protocol/worker.rs` | modify | Handle fresh `InitialPartition` mid-session (discard state). |

## Test Expectations (forward-ref)

- EG-U5-delta `test_dynamic_join_repartition_delta` (R12-delta).
- EG-I2-delta `test_elastic_join_correctness_delta` (R9-R14, R12-delta, G1 CONDITIONAL).
- EG-B3 `bench_join_round_overhead_delta` (R12a).

## Invariants Touched

- D3, D4, D5 — preserved via full reconstruct + re-split.
- G1 delta — CONDITIONAL per R29a (ARG-005 now CLOSED so no longer a blocker).

## Notes

- **Alternative optimization OUT OF SCOPE**: incremental "split-the-BorderGraph" optimization is deferred to a future spec; R12-delta mandates the conservative full-reconstruct path.

## DAG Links

- **Predecessors:** TASK-0412, TASK-0432, TASK-0437, TASK-0435.
- **Successors:** EG-U5-delta, EG-I2-delta, EG-B3.
