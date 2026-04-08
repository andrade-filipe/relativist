# TEST-SPEC-0107: Define CoordinatorState enum and FSM types

**Task:** TASK-0107
**Spec:** SPEC-13
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: CoordinatorState serializes to JSON

**Type:** Unit test
**Input:** `serde_json::to_string(&CoordinatorState::Init).unwrap()`
**Expected:** Produces a valid JSON string (e.g., `"Init"`)
**Verifies:** `#[derive(serde::Serialize)]` on CoordinatorState

### T2: All 9 state variants are distinct

**Type:** Unit test
**Input:**
```
let states = vec![
    CoordinatorState::Init, CoordinatorState::WaitingForWorkers,
    CoordinatorState::Partitioning, CoordinatorState::Dispatching,
    CoordinatorState::WaitingForResults, CoordinatorState::Merging,
    CoordinatorState::CheckTermination, CoordinatorState::Done,
    CoordinatorState::Error,
];
```
**Expected:** All 9 variants can be constructed; `Init != WaitingForWorkers`, etc. (PartialEq)
**Verifies:** R19 -- 9 FSM states defined

### T3: CoordinatorEvent variants exist

**Type:** Unit test
**Input:** Construct each event variant: `ConfigLoaded`, `WorkerConnected(0)`, `SplitComplete(vec![])`, `AllDispatched`, `PartitionReturned { worker_id: 0, partition }`, `PhaseTimeout(0)`, `MergeComplete { net, is_normal_form: true }`, `FatalError("msg".into())`
**Expected:** All compile and can be formatted with Debug
**Verifies:** R20 -- all event types defined

### T4: CoordinatorAction variants exist

**Type:** Unit test
**Input:** Construct each action: `BindListener`, `SendMessage`, `InvokeSplit`, `InvokeMergeAndReduce`, `StartTimer`, `CancelTimer`, `LogTransition`, `WriteOutput`, `ShutdownAll`
**Expected:** All compile and can be formatted with Debug
**Verifies:** R20 -- all action types defined

### T5: Coordinator module declared in lib.rs

**Type:** Compilation test
**Input:** `use relativist::coordinator::CoordinatorState;`
**Expected:** Compiles successfully
**Verifies:** Module is public and declared in lib.rs

### T6: TimerId type alias exists

**Type:** Unit test
**Input:** `let id: TimerId = 42;`
**Expected:** Compiles; `id == 42u32`
**Verifies:** R20 -- `type TimerId = u32`

---

## Edge Cases

### E1: EmitMetric action does NOT exist

**Verify:** `CoordinatorAction::EmitMetric` does not compile.
**Why:** EmitMetric was removed from SPEC-13 R20 during review (SC-008).

### E2: CoordinatorEvent uses WorkerConnected, not WorkerRegistered

**Verify:** `CoordinatorEvent::WorkerConnected(0)` compiles; `CoordinatorEvent::WorkerRegistered` does not.
**Why:** SPEC-13 naming convention uses WorkerConnected; registration is handled at the runtime layer.
