# TEST-SPEC-0149: Add FSM state transition logging

**Task:** TASK-0149
**Spec:** SPEC-11
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Existing coordinator and worker tests still pass

**Input:** Run all existing FSM tests
**Expected:** All pass -- no behavioral change (logging only)
**Verifies:** No logic changes

### T2: Coordinator FSM transitions emit INFO log events

**Input:** Run a coordinator through Init -> WaitingForWorkers -> Partitioning -> ... -> Done; capture log output
**Expected:** Each transition emits an INFO event with `from_state`, `to_state`, `event` fields
**Verifies:** R7 -- state transition logging

### T3: Worker FSM transitions emit INFO log events

**Input:** Run a worker through Init -> Idle -> Reducing -> Returning -> Done; capture log output
**Expected:** Each transition emits an INFO event with `from_state`, `to_state`, `event`, `worker_id` fields
**Verifies:** R7 -- state transition logging

### T4: Coordinator events include round field

**Input:** Inspect coordinator transition logs during a multi-round reduction
**Expected:** Transitions like WaitingForResults -> Merging include `round=N`
**Verifies:** R8 -- contextual fields

### T5: State names match SPEC-13 exactly

**Input:** Inspect the `from_state` and `to_state` values in log output
**Expected:** Names are PascalCase matching SPEC-13: `"Init"`, `"WaitingForWorkers"`, `"Partitioning"`, etc.
**Verifies:** R7 -- exact state name strings

---

## Edge Cases

### E1: Error state transition is logged

**Verify:** When the coordinator FSM transitions to `Error`, the log includes `to_state="Error"` and the triggering event.
**Why:** Error transitions are critical diagnostic events.

### E2: Event names use PascalCase

**Verify:** Event field values use PascalCase: `"AllWorkersConnected"`, `"PartitionReceived"`, etc.
**Why:** Consistent naming convention across the codebase.
