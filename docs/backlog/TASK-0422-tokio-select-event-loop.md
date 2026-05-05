# TASK-0422: Coordinator event loop — `tokio::select!` 4-arm pattern (R3)

**Spec:** SPEC-20 §3.1 R3 (concurrency model, closes SC-002), R3b (events processed during self-partition in-flight).
**Requirements:** R3 (4-arm select), R3b (process membership/timeout events concurrently with self-reduce).
**Priority:** P0 (foundational for the entire FSM event-processing model).
**Status:** TODO
**Depends on:** TASK-0414 (FSM enums), TASK-0418 (wire messages to route), TASK-0415 (GridConfig).
**Blocked by:** TASK-0414.
**Estimated complexity:** M (~150-200 LoC production + ~80 LoC tests)
**Bundle:** SPEC-20 Elastic Grid — Phase 2.1 Hybrid Coordinator (concurrency model).

## Context

R3 mandates a `tokio::select!` over four arms so the FSM processes events from: (a) worker messages (remote OR self via channel), (b) timer wheel, (c) new TCP accepts (queued for R10b), (d) self-partition task panic signal. The FSM transition function remains pure and single-threaded; only the I/O event sources are concurrent. This directly closes SC-002 (mid-reduce event loss) and satisfies R3b (`LeaveRequest`, `WorkerJoined`, `PhaseTimeout` processed while self-partition reduces).

## Acceptance Criteria

- [ ] Refactor coordinator's main loop to `tokio::select!` with 4 arms:
  - `msg = workers.next_message()` → emit `WorkerMessage(id, msg)` event.
  - `timer_id = timer_wheel.next()` → emit `TimerFired(timer_id)` event.
  - `Some(stream) = listener.accept()` → push to `pending_connections_queue` (TASK-0434 drains).
  - `panic = self_join_handle.panic_signal()` → emit `SelfPartitionPanic(reason)` event.
- [ ] All 4 arms produce events fed into the pure FSM transition function (TASK-0436).
- [ ] Self-partition runs on `tokio::task::spawn_blocking`; its `JoinHandle` panic translates to `SelfPartitionPanic(String)` via a dedicated `oneshot` channel wired to arm (d).
- [ ] No event is dropped or reordered: select! fairness + FSM determinism required.
- [ ] Loop exit on `Done` or `Error` states (per SPEC-13 R21).
- [ ] FSM state transitions logged via `tracing::info!` with `trace_id`.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/protocol/coordinator.rs` | modify | Replace existing main loop with 4-arm select. |
| `relativist-core/src/protocol/self_worker.rs` *(new)* | create | Self-partition task spawn helper (spawn_blocking + panic signal). |

## Test Expectations (forward-ref)

- EG-U1 (`test_hybrid_coordinator_single_machine` — R1, R5).
- EG-U16 (`test_self_partition_panic_to_error` — R3a) — injects panic in spawn_blocking task.
- Property coverage in EG-P1.

## Invariants Touched

- D6 (Protocol Termination) — preserved via deterministic event ordering.

## Notes

- **No blocking in an arm**: every arm returns immediately; FSM transition is sync and fast.
- **Channel backpressure**: workers' message stream must not block on a slow consumer; use bounded channels with sensible capacity.

## DAG Links

- **Predecessors:** TASK-0414, TASK-0418, TASK-0415.
- **Successors:** TASK-0423 (self-worker spawn), TASK-0436 (FSM transitions), TASK-0434 (pending_connections_queue drain).
