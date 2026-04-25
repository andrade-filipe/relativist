# TEST-SPEC EG-U10: graceful leave after round (R20, R22a)

**SPEC-20 §7.1 ID:** EG-U10
**Owning task(s):** TASK-0441 (graceful leave), TASK-0436 (FSM), TASK-0451 (logging).
**Type:** unit.
**Test name:** `test_graceful_leave_after_round`.

---

## Inputs / Fixtures

- Hybrid; K_remote=2; v1 mode.
- Worker `w1` sends `PartitionResult` for round N successfully, then sends `LeaveRequest { kind: AfterResult }`.

## Expected behaviour

R22a (clean leave):
1. Coordinator processes the `PartitionResult` normally (round N completes per spec).
2. On `LeaveRequest{AfterResult}`, coordinator sends `LeaveAck` BEFORE closing the TCP stream (R35a).
3. Worker is removed from `ActiveWorkerSet` for round N+1.
4. K_eff_new = K_eff - 1 for the next round.
5. `retained_last_acked[w1]` is RELEASED (not reclaimed; clean leave does not produce a phantom partition).

## Assertions

| # | Assertion |
|---|-----------|
| A1 | The `PartitionResult` for round N from w1 is fully processed (verify via metrics or BorderGraph state change). |
| A2 | A `LeaveAck` is observed on w1's stream BEFORE the stream's read half observes EOF. |
| A3 | After round N completes, `ActiveWorkerSet` no longer contains w1's WorkerId. |
| A4 | Round N+1's K_eff_new == K_eff_old - 1. |
| A5 | `metrics.workers_departed_per_round[N]` increments by 1 (or N+1 boundary; document the convention). |
| A6 | `retained_last_acked[w1]` is released (no leak; assert `retained_last_acked.contains_key(&w1) == false`). |
| A7 | The departure log line includes `WorkerId(w1)` and `DepartureKind::LeaveAfter`. |

## Edge / negative cases

- EC-1: w1 sends `LeaveRequest{AfterResult}` BEFORE its `PartitionResult` arrives — covered by EG-U10c (silent upgrade to Urgent).
- EC-2: 2 workers send `LeaveRequest{AfterResult}` in the same round — both processed; K_eff_new = K_eff - 2.
- EC-3: w1 sends `LeaveRequest` but the coordinator's stream-write fails (e.g., TCP RST from the worker before LeaveAck delivery) — coordinator logs WARN; reclaim treated as `LeaveUrgent` fallback.

## Invariants asserted

- D6 (Protocol Termination) via R35a ack semantics.
- D5 (state ownership) — clean release; no phantom retained state.
- G1 PRESERVED (clean leave does not alter the reduction trace's correctness).

## ARG/DISC/REF citation

None.

## Determinism notes

`#[tokio::test(flavor = "current_thread", start_paused = true)]`. Worker actions scripted via duplex stream message scripting.

## Cross-test dependencies

- EG-U10a/b/c (other leave variants).
- EG-U19 (LeaveAck before close — the critical R35a invariant).
