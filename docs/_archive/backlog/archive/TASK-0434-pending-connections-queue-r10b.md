# TASK-0434: `pending_connections_queue` buffering (R10b, SC-012)

**Spec:** SPEC-20 §3.2 R10b (boundary buffering, closes SC-012); partial R16 (mid-round joins queued).
**Requirements:** R10b (queue TCP accepts during non-AcceptingMembershipChanges states; FSM transitions for `WorkerJoined` during Partitioning / Dispatching / WaitingForResults / Merging — all `QueueWorkerForNextWindow(id)`).
**Priority:** P0 (correctness: prevents mid-round join chaos).
**Status:** TODO
**Depends on:** TASK-0422 (select arm (c) pushes to queue).
**Blocked by:** TASK-0422.
**Estimated complexity:** S (~60-90 LoC production + ~80 LoC tests)
**Bundle:** SPEC-20 Elastic Grid — Phase 2.2 Dynamic Joining.

## Context

TCP `accept()` completions during any non-`AcceptingMembershipChanges` state MUST be buffered in a coordinator-local `pending_connections_queue: VecDeque<TcpStream>`. Only the raw stream is queued — the `Register`/`JoinRequest` handshake does NOT process buffered connections until the FSM enters `AcceptingMembershipChanges` (R10a).

## Acceptance Criteria

- [ ] Add `pending_connections_queue: VecDeque<TransportStream>` to coordinator state.
- [ ] On select arm (c) (TASK-0422), push the accepted stream into the queue — do NOT start handshake yet.
- [ ] FSM transitions for `WorkerJoined(id)` in states {`Partitioning`, `Dispatching`, `WaitingForResults`, `Merging`} → same state + action `QueueWorkerForNextWindow(id)` (note: `WorkerJoined` here represents an already-handshake-completed join that happened *during* the AcceptingMembershipChanges window but whose registration into `W_active` is deferred to the next window; the raw-stream queue holds connections whose handshake has not even started).
- [ ] Drain logic is in TASK-0435 (R10a drain-then-arm).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/protocol/coordinator.rs` | modify | Add queue field; wire arm (c). |

## Test Expectations (forward-ref)

- EG-U6 `test_dynamic_join_mid_round_queued` (R10b, R16).
- EG-U6a `test_join_window_boundary_race` (R10a-b, SC-007).

## Invariants Touched

- D6 — preserved via BSP barrier respect.

## Notes

- **Terminology precision**: two separate "queues" — (i) the raw-stream `pending_connections_queue` (this task) holds TCP streams whose handshake has not yet begun; (ii) an internal "workers queued for next window" list (action `QueueWorkerForNextWindow`) holds already-handshaked WorkerIds awaiting registration into `W_active`. Keep them distinct.

## DAG Links

- **Predecessors:** TASK-0422.
- **Successors:** TASK-0435 (drain on AcceptingMembershipChanges entry).
