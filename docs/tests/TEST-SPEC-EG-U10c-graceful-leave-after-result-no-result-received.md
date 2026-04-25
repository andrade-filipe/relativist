# TEST-SPEC EG-U10c: graceful leave AfterResult upgraded to Urgent (R22c)

**SPEC-20 §7.1 ID:** EG-U10c
**Owning task(s):** TASK-0441, TASK-0436.
**Type:** unit.
**Test name:** `test_graceful_leave_after_result_no_result_received`.

---

## Inputs / Fixtures

- Hybrid; K_remote=2.
- Worker `w1` sends `LeaveRequest { kind: AfterResult }` BUT the coordinator never received `PartitionResult` from w1 for the current round (e.g., because it crashed mid-reduction or the result message was lost).

## Expected behaviour

R22c (lenient upgrade): coordinator silently UPGRADES the `AfterResult` semantic to `Urgent` because the prerequisite (a successfully-received result) is missing. The departure path then mirrors EG-U10a (urgent leave with reclaim).

## Assertions

| # | Assertion |
|---|-----------|
| A1 | A WARN-level log line records the upgrade: "AfterResult upgraded to Urgent for WorkerId X (no result received)". |
| A2 | `LeaveAck` is sent. |
| A3 | The coordinator triggers the reclaim path (R24a or R24b) for the current round, NOT a clean leave. |
| A4 | `metrics.workers_departed_per_round` records the departure with `DepartureKind::LeaveUrgent` (NOT `LeaveAfter`) — reflecting the upgrade. |
| A5 | `canonicalise(final) == canonicalise(reduce_all(input))`. |

## Edge / negative cases

- EC-1: result arrives AFTER the LeaveRequest is processed — late result is dropped (R31 atomic refresh; cross-link to EG-U13).
- EC-2: 2 workers send AfterResult, one with result and one without — first is clean (R22a), second is upgraded (R22c); separate paths per worker.

## Invariants asserted

- D6 (Protocol Termination).
- G1 PRESERVED via ARG-006.

## ARG/DISC/REF citation

ARG-006.

## Determinism notes

`#[tokio::test(flavor = "current_thread", start_paused = true)]`. Worker scripted to send `LeaveRequest{AfterResult}` without ever sending `PartitionResult`.

## Cross-test dependencies

- EG-U10 (clean AfterResult).
- EG-U10a (Urgent v1 reclaim path).
