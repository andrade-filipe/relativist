# TEST-SPEC EG-U10b: graceful leave urgent delta (R22b)

**SPEC-20 §7.1 ID:** EG-U10b
**Owning task(s):** TASK-0441, TASK-0443, TASK-0436.
**Type:** unit.
**Test name:** `test_graceful_leave_urgent_delta`.

---

## Inputs / Fixtures

Same as EG-U10a but in delta mode (`delta_mode = true`):
- Hybrid; K_remote=2; delta mode.
- Worker `w1` sends `LeaveRequest { kind: Urgent }` mid-round.

## Expected behaviour

Same FSM transitions as EG-U10a but the reclaim invokes the delta-mode departure path (TASK-0443):
- If `retained_last_acked[w1]` exists, reclaim its `(BorderGraph snapshot, last RoundResult deltas)` pair.
- Conservative path: invoke `reconstruct(&bg_snapshot, surviving, vec![reclaimed])`, then re-split.
- Optimized path (CONDITIONAL on ARG-005): replay deltas onto live bg, then continue.

This test exercises the conservative path.

## Assertions

| # | Assertion |
|---|-----------|
| A1 | `LeaveAck` sent before close. |
| A2 | Reclaim invokes `reconstruct` (TEST-SPEC-0412 properties hold). |
| A3 | K_eff_new = K_eff - 1 for the restarted round. |
| A4 | `metrics.retained_last_acked_reclaims_per_round` increments by 1. |
| A5 | `canonicalise(final) == canonicalise(reduce_all(input))`. |

## Edge / negative cases

- EC-1: optimized path enabled — additional sub-test asserts the same final result.
- EC-2: w1 has 0 successful rounds → reclaim from `retained_initial`; same correctness.

## Invariants asserted

- D3 (Border Completeness) via SPEC-19 R39.
- G1 CLOSED for conservative; CONDITIONAL on ARG-005 for optimized.

## ARG/DISC/REF citation

ARG-005, ARG-006.

## Determinism notes

`#[tokio::test(flavor = "current_thread", start_paused = true)]`.

## Cross-test dependencies

- EG-U7c, EG-U10a.
- EG-I3-delta integration counterpart.
