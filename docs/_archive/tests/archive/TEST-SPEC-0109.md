# TEST-SPEC-0109: Define WorkerState enum and FSM types

**Task:** TASK-0109
**Spec:** SPEC-13
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: WorkerState serializes to JSON

**Type:** Unit test
**Input:** `serde_json::to_string(&WorkerState::Init).unwrap()`
**Expected:** Produces a valid JSON string (e.g., `"Init"`)
**Verifies:** `#[derive(serde::Serialize)]` on WorkerState

### T2: All 6 state variants are distinct

**Type:** Unit test
**Input:**
```
let states = vec![
    WorkerState::Init, WorkerState::Idle, WorkerState::Reducing,
    WorkerState::Returning, WorkerState::Error, WorkerState::Done,
];
```
**Expected:** All 6 variants constructible; `Init != Idle`, etc. (PartialEq)
**Verifies:** R24 -- 6 worker FSM states

### T3: WorkerEvent variants exist

**Type:** Unit test
**Input:** Construct each: `Connected`, `ReceivePartition(partition)`, `ReductionComplete(partition)`, `SendComplete`, `Shutdown`, `ReductionError("err".into())`, `ConnectionLost`
**Expected:** All 7 compile and format with Debug
**Verifies:** R25 -- all event types defined

### T4: WorkerAction variants exist

**Type:** Unit test
**Input:** Construct each: `SendMessage(Message::Shutdown)`, `CloseConnection`, `ShutdownSelf`, `LogTransition { from: Init, to: Idle }`
**Expected:** All compile and format with Debug
**Verifies:** R25 -- all action types defined

### T5: Worker module declared in lib.rs

**Type:** Compilation test
**Input:** `use relativist::worker::WorkerState;`
**Expected:** Compiles successfully
**Verifies:** Module is public and declared in lib.rs

---

## Edge Cases

### E1: No AttemptReconnect action

**Verify:** `WorkerAction::AttemptReconnect` does not exist.
**Why:** SPEC-06 R25 -- worker does NOT reconnect; coordinator aborts on connection loss.

### E2: WorkerEvent::ConnectionLost has no associated data

**Verify:** `WorkerEvent::ConnectionLost` is a unit variant (no fields).
**How:** `let e = WorkerEvent::ConnectionLost;` compiles without providing any data.
**Why:** Connection loss details are logged but not carried in the event.
