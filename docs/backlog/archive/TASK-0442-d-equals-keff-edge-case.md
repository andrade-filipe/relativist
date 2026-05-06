# TASK-0442: `D == K_eff` edge case — solo fallback (hybrid) / Error (non-hybrid) (R26a, R27, NF-007)

**Spec:** SPEC-20 §3.3.4 R26a (closes NF-007), R27 (refined fallback rules).
**Requirements:** R26a (both hybrid and non-hybrid branches), R27.
**Priority:** P0 (defensive; prevents deadlock on FinalStateRequest broadcast to empty recipient set).
**Status:** TODO
**Depends on:** TASK-0440 (v1 reclaim), TASK-0443 (delta reclaim), TASK-0425 (SoloReducing).
**Blocked by:** TASK-0440.
**Estimated complexity:** S (~60-90 LoC production + ~80 LoC tests)
**Bundle:** SPEC-20 Elastic Grid — Phase 2.3 Dynamic Departure.

## Context

When `D == K_eff` (every active remote worker departs in the same round), standard R26 cycle has degenerate inputs (no survivors). R26a:
- **Hybrid mode**: self-partition still progressing (self is K_eff_total = K_remote + 1 but never departs gracefully — panic path is `SelfPartitionPanic → Error` per R3a). Discard `retained_last_acked` for all D reclaimed slots; fall back per R27 (solo with self-partition); reclaimed partitions queued for re-introduction on next `AcceptingMembershipChanges` window via R5a + R15. Delta-mode `FinalStateRequest` step SKIPPED (no survivors).
- **Non-hybrid mode**: no executor remains. Transition to `Error`. Reclaimed snapshots released in `Error` cleanup.

## Acceptance Criteria

- [ ] Detect `D == K_eff` condition in the reclaim orchestrator.
- [ ] **Hybrid branch**:
  - Discard `retained_last_acked` for all D workers; fall back to `retained_initial[w]` (R24a conservative).
  - Skip `FinalStateRequest` broadcast (no recipients).
  - Enter `SoloReducing` (or continue self-partition progress) via R27.
  - Reclaimed partitions queued for next AcceptingMembershipChanges window.
- [ ] **Non-hybrid branch**:
  - Transition to `Error` (the v1 fatal path).
  - Release reclaimed state as part of Error cleanup.
- [ ] Log the edge-case occurrence at WARN level with explicit diagnostic (which branch, D, K_eff).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/protocol/coordinator.rs` | modify | R26a branch handler. |

## Test Expectations (forward-ref)

- EG-U9 `test_departure_all_workers_solo_fallback` — BOTH branches (hybrid AND non-hybrid) covered.

## Invariants Touched

- D6 — preserved by explicit Error transition in non-hybrid branch.

## Notes

- **Defensive design**: D==K_eff is rare but pathological; explicit handling prevents silent deadlock.

## DAG Links

- **Predecessors:** TASK-0440, TASK-0443, TASK-0425.
- **Successors:** EG-U9.
