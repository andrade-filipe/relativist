# TEST-SPEC-0110: Implement worker FSM transition function

**Task:** TASK-0110
**Spec:** SPEC-13, SPEC-08
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Init + Connected -> Idle

**Type:** Unit test (sync)
**Input:** `worker_transition(WorkerState::Init, WorkerEvent::Connected)`
**Expected:** `state == WorkerState::Idle`; actions contain `LogTransition { from: Init, to: Idle }`
**Verifies:** R25 row 1

### T2: Idle + ReceivePartition -> Reducing

**Type:** Unit test (sync)
**Input:** `worker_transition(WorkerState::Idle, WorkerEvent::ReceivePartition(partition))`
**Expected:** `state == WorkerState::Reducing`; actions contain `LogTransition`
**Verifies:** R25 row 2

### T3: Reducing + ReductionComplete -> Returning

**Type:** Unit test (sync)
**Input:** `worker_transition(WorkerState::Reducing, WorkerEvent::ReductionComplete(reduced_partition))`
**Expected:** `state == WorkerState::Returning`; actions contain `LogTransition`
**Verifies:** R25 row 3

### T4: Returning + SendComplete -> Idle

**Type:** Unit test (sync)
**Input:** `worker_transition(WorkerState::Returning, WorkerEvent::SendComplete)`
**Expected:** `state == WorkerState::Idle`; actions contain `LogTransition`
**Verifies:** R25 row 4 -- worker ready for next round

### T5: Idle + Shutdown -> Done

**Type:** Unit test (sync)
**Input:** `worker_transition(WorkerState::Idle, WorkerEvent::Shutdown)`
**Expected:** `state == WorkerState::Done`; actions contain `CloseConnection` and `LogTransition`
**Verifies:** R25 row 5 -- clean shutdown

### T6: Reducing + ReductionError -> Error

**Type:** Unit test (sync)
**Input:** `worker_transition(WorkerState::Reducing, WorkerEvent::ReductionError("OOM".into()))`
**Expected:** `state == WorkerState::Error`; actions contain `SendMessage(Message::Error { .. })` and `LogTransition`
**Verifies:** R25 row 6 -- error during reduction

### T7: Any state + ConnectionLost -> Error with ShutdownSelf

**Type:** Unit test (sync)
**Input:** `worker_transition(WorkerState::Idle, WorkerEvent::ConnectionLost)`
**Expected:** `state == WorkerState::Error`; actions contain `ShutdownSelf` and `LogTransition`
**Verifies:** R25 row 7 -- no reconnection, just shutdown

### T8: Unhandled state+event -> Error

**Type:** Unit test (sync)
**Input:** `worker_transition(WorkerState::Done, WorkerEvent::ReceivePartition(partition))` (invalid: Done should not receive partitions)
**Expected:** `state == WorkerState::Error`
**Verifies:** Robustness against unexpected events

---

## Edge Cases

### E1: Transition function is pure (no I/O, no async)

**Verify:** `worker_transition` has no `async`, no `tokio` types, no side effects.
**How:** Code review; all tests are synchronous (no `#[tokio::test]`).
**Why:** Stimulus-response purity for testability.

### E2: ConnectionLost from Reducing state

**Verify:** `worker_transition(WorkerState::Reducing, WorkerEvent::ConnectionLost)` transitions to `Error` with `ShutdownSelf`.
**Why:** Connection can be lost during any active state, not just Idle.
