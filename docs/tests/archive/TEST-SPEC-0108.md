# TEST-SPEC-0108: Implement coordinator FSM transition function

**Task:** TASK-0108
**Spec:** SPEC-13, SPEC-08
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Init + ConfigLoaded -> WaitingForWorkers

**Type:** Unit test (sync)
**Input:**
```
let mut ctx = CoordinatorContext { expected_workers: 2, registered_workers: vec![], received_partitions: vec![], round: 0 };
let (state, actions) = transition(CoordinatorState::Init, CoordinatorEvent::ConfigLoaded, &mut ctx);
```
**Expected:** `state == CoordinatorState::WaitingForWorkers`; actions contain `BindListener(..)` and `LogTransition { from: Init, to: WaitingForWorkers }`
**Verifies:** R21 row 1 -- Init to WaitingForWorkers

### T2: WaitingForWorkers + WorkerConnected (count < expected) stays in WaitingForWorkers

**Type:** Unit test (sync)
**Input:**
```
let mut ctx = CoordinatorContext { expected_workers: 3, registered_workers: vec![], .. };
let (state, actions) = transition(CoordinatorState::WaitingForWorkers, CoordinatorEvent::WorkerConnected(0), &mut ctx);
```
**Expected:** `state == CoordinatorState::WaitingForWorkers`; `ctx.registered_workers == vec![0]`; actions contain `LogTransition`
**Verifies:** R21 row 2 -- not enough workers yet

### T3: WaitingForWorkers + WorkerConnected (count == expected) -> Partitioning

**Type:** Unit test (sync)
**Input:**
```
let mut ctx = CoordinatorContext { expected_workers: 2, registered_workers: vec![0], .. };
let (state, actions) = transition(CoordinatorState::WaitingForWorkers, CoordinatorEvent::WorkerConnected(1), &mut ctx);
```
**Expected:** `state == CoordinatorState::Partitioning`; actions contain `InvokeSplit { .. }` and `LogTransition`
**Verifies:** R21 row 3 -- all workers connected, start partitioning

### T4: WaitingForResults + all PartitionReturned -> Merging

**Type:** Unit test (sync)
**Input:**
```
let mut ctx = CoordinatorContext { expected_workers: 2, received_partitions: vec![(0, p0)], .. };
let (state, actions) = transition(CoordinatorState::WaitingForResults, CoordinatorEvent::PartitionReturned { worker_id: 1, partition: p1 }, &mut ctx);
```
**Expected:** `state == CoordinatorState::Merging`; actions contain `CancelTimer(..)` and `InvokeMergeAndReduce(..)`
**Verifies:** R21 row 7 -- all results received, start merging

### T5: WaitingForResults + PhaseTimeout -> Error

**Type:** Unit test (sync)
**Input:**
```
let (state, actions) = transition(CoordinatorState::WaitingForResults, CoordinatorEvent::PhaseTimeout(0), &mut ctx);
```
**Expected:** `state == CoordinatorState::Error`; actions contain `ShutdownAll` and `LogTransition`
**Verifies:** R21 row 8 -- timeout causes error and shutdown

### T6: Merging + MergeComplete -> CheckTermination

**Type:** Unit test (sync)
**Input:**
```
let (state, actions) = transition(CoordinatorState::Merging, CoordinatorEvent::MergeComplete { net, is_normal_form: true }, &mut ctx);
```
**Expected:** `state == CoordinatorState::CheckTermination`; actions contain `LogTransition`
**Verifies:** R21 row 9 -- merge complete, check termination

### T7: CheckTermination with is_normal_form == true -> Done

**Type:** Unit test (sync)
**Input:** Context with `is_normal_form = true` from the preceding MergeComplete event
**Expected:** `state == CoordinatorState::Done`; actions contain `WriteOutput(..)`, `ShutdownAll`, `LogTransition`
**Verifies:** R21 row 10 -- normal form reached, done

### T8: CheckTermination with is_normal_form == false -> Partitioning

**Type:** Unit test (sync)
**Input:** Context with `is_normal_form = false`
**Expected:** `state == CoordinatorState::Partitioning`; actions contain `InvokeSplit { .. }` and `LogTransition`
**Verifies:** R21 row 11 -- not converged, another round

### T9: Any state + FatalError -> Error

**Type:** Unit test (sync)
**Input:** `transition(CoordinatorState::Dispatching, CoordinatorEvent::FatalError("crash".into()), &mut ctx)`
**Expected:** `state == CoordinatorState::Error`; actions contain `ShutdownAll`
**Verifies:** R21 row 12 -- fatal error from any state

### T10: Unhandled state+event -> Error

**Type:** Unit test (sync)
**Input:** `transition(CoordinatorState::Done, CoordinatorEvent::WorkerConnected(0), &mut ctx)` (invalid: Done should not receive new connections)
**Expected:** `state == CoordinatorState::Error` (or a FatalError transition)
**Verifies:** Robustness against unexpected events

---

## Edge Cases

### E1: Transition function is pure (no I/O)

**Verify:** The `transition` function signature has no `async`, no `tokio` types, no file I/O.
**How:** Code review of the function signature and body.
**Why:** R20 -- stimulus-response purity for testability.

### E2: Round counter increments on new partition cycle

**Verify:** After `CheckTermination [not normal form] -> Partitioning`, `ctx.round` is incremented by 1.
**Why:** Each BSP cycle is a new round; metrics depend on accurate round numbering.
