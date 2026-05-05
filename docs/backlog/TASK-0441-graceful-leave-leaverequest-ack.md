# TASK-0441: Graceful `LeaveRequest`/`LeaveAck` flow (R20, R21, R22a, R22b, R22c, R35a)

**Spec:** SPEC-20 §3.3.2 R20 (graceful departure), R21 (LeaveRequest variant + LeaveKind), R22a (clean leave after result), R22b (urgent leave composes timeout path), R22c (coordinator silent upgrade of AfterResult→Urgent when no result received), R35a (LeaveAck before TCP close, SC-017).
**Requirements:** R20, R21, R22a, R22b, R22c, R35a.
**Priority:** P0 (correct shutdown semantics).
**Status:** TODO
**Depends on:** TASK-0418 (LeaveRequest/LeaveAck variants), TASK-0436 Phase C (FSM transitions for WorkerLeft), TASK-0440 (Urgent path uses reclaim).
**Blocked by:** TASK-0418, TASK-0440.
**Estimated complexity:** S (~90-130 LoC production + ~100 LoC tests)
**Bundle:** SPEC-20 Elastic Grid — Phase 2.3 Dynamic Departure.

## Context

Graceful departure: worker sends `LeaveRequest { kind: AfterResult | Urgent }`. Coordinator sends `LeaveAck` before closing TCP (R35a). FSM handling:
- `AfterResult`: worker completed round; coordinator stores result, removes worker for NEXT round only.
- `Urgent`: worker cannot complete round; coordinator treats as timeout for current round AND graceful for future (composes R24a/b reclaim + RemoveWorker).
- `AfterResult` received but no result yet → silent upgrade to Urgent (R22c, logs WARN).

## Acceptance Criteria

- [ ] Worker-side: send `LeaveRequest { kind: AfterResult }` ONLY after successfully emitting `PartitionResult`/`RoundResult` for the current round. Block on `LeaveAck` before closing TCP (R35a).
- [ ] Worker-side urgent path: send `LeaveRequest { kind: Urgent }` WITHOUT current-round result.
- [ ] Coordinator-side handler:
  - `LeaveRequest.AfterResult` received AND result already stored → FSM action `StoreResult(id, prev) + RemoveWorkerForNextRound(id) + LogDeparture(LeaveAfter) + Send LeaveAck(id)` (R22a).
  - `LeaveRequest.AfterResult` received AND result NOT yet stored → upgrade to Urgent semantics + log WARN (R22c).
  - `LeaveRequest.Urgent` received → FSM action `ReclaimPartition(id, auto) + RemoveWorker(id) + LogDeparture(LeaveUrgent) + Send LeaveAck(id)` (R22b).
- [ ] `LeaveAck` sent BEFORE TCP close (R35a explicit); workers MUST NOT close before receiving LeaveAck (worker-side enforcement).
- [ ] `Shutdown` (SPEC-06) remains reserved for coordinator-initiated termination ONLY — NOT used as leave ack (SC-017).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/protocol/worker.rs` | modify | Worker send-LeaveRequest helper; block on LeaveAck. |
| `relativist-core/src/protocol/coordinator.rs` | modify | Coordinator LeaveRequest handler with R22a/b/c branching. |

## Test Expectations (forward-ref)

- EG-U10 `test_graceful_leave_after_round` (R20, R22a).
- EG-U10a `test_graceful_leave_urgent_v1` (R22b, SC-008).
- EG-U10b `test_graceful_leave_urgent_delta` (R22b).
- EG-U10c `test_graceful_leave_after_result_no_result_received` (R22c).
- EG-U19 `test_leave_ack_before_close` (R35a, SC-017).

## Invariants Touched

- D6 — preserved by explicit ack semantics.

## Notes

- **Shutdown vs LeaveAck**: distinct semantics; do NOT overload.
- **Urgent composability**: R22b's action set is literally the timeout path's action set plus graceful-future bookkeeping.

## DAG Links

- **Predecessors:** TASK-0418, TASK-0436, TASK-0440.
- **Successors:** EG-U10..U10c, U19.
