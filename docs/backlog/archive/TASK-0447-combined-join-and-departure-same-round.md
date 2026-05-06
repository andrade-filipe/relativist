# TASK-0447: Combined join + departure in the same round — §4.2.3

**Spec:** SPEC-20 §4.2.3 (combined join and departure: single cycle, departures first, then joins).
**Requirements:** §4.2.3 composition rule; cross-ref R26.
**Priority:** P1 (edge-case; correctness-critical when churn is high).
**Status:** TODO
**Depends on:** TASK-0433 (v1 repartition on join), TASK-0440 (v1 reclaim), TASK-0446 (delta rejoin), TASK-0443 (delta reclaim).
**Blocked by:** TASK-0433, TASK-0440.
**Estimated complexity:** S (~70-100 LoC production + ~80 LoC tests)
**Bundle:** SPEC-20 Elastic Grid — Phase 2.2+2.3 Combined.

## Context

If workers both join AND depart between the same two rounds, the coordinator processes departures first (reclaim states per R24a/b), then registers joins per R10a-b, then computes `K_eff_new = (K_eff_old - D) + J`. A single `split()` (v1) or `reconstruct + split + InitialPartition` (delta) cycle handles both — no sequential repartitions.

## Acceptance Criteria

- [ ] In the `AcceptingMembershipChanges` exit handler, the composition is:
  1. Process all accumulated `WorkerLeft` / `PhaseTimeout` / `ConnLost` events → reclaim via TASK-0440 (v1) or TASK-0443 (delta).
  2. Process all accumulated `WorkerJoined` events → RegisterWorker for next round.
  3. Compute `K_eff_new = (K_eff_old - D) + J + (1 if hybrid)`.
  4. Single `split()` (v1) or `reconstruct + split + InitialPartition` (delta) cycle.
- [ ] Order constraint: departures FIRST, joins SECOND.
- [ ] No sequential re-partitions.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/protocol/coordinator.rs` | modify | `AcceptingMembershipChanges` exit orchestrator; enforce departures-first, then joins. |

## Test Expectations (forward-ref)

- EG-U11 `test_join_and_departure_same_round` (§4.2.3, R26).
- EG-I4 `test_elastic_churn_correctness` (R9-R30, G1 CONDITIONAL — now CLOSED via ARG-006 for v1 and delta-conservative).

## Invariants Touched

- D1, D3, D4 — preserved via single cycle.
- G1 — PRESERVED.

## Notes

- **Departures first rationale**: reclaim state must be included in the input of the subsequent join's reconstruct/split; if joins were processed first, the reclaim would have to re-do id-range computation.

## DAG Links

- **Predecessors:** TASK-0433, TASK-0440, TASK-0443, TASK-0446.
- **Successors:** EG-U11, EG-I4.
