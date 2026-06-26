# TEST-SPEC-0045: Helper function max_freeport_id

**Task:** TASK-0045
**Spec:** SPEC-04
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Net with no FreePorts returns None
Create a net where all port entries are `AgentPort` or `DISCONNECTED`. Call `max_freeport_id(&net)`. Expected: `None`.

### T2: Net with multiple FreePorts returns the maximum
Create a net with port entries including `FreePort(0)`, `FreePort(3)`, `FreePort(1)`. Call `max_freeport_id(&net)`. Expected: `Some(3)`.

### T3: Net with only DISCONNECTED sentinels returns None
Create a net where all FreePort entries are `FreePort(u32::MAX)` (DISCONNECTED). Call `max_freeport_id(&net)`. Expected: `None`.

### T4: Net with mixed FreePorts and DISCONNECTED
Create a net with `FreePort(5)`, `FreePort(u32::MAX)`, `FreePort(2)`. Expected: `Some(5)`. The DISCONNECTED entry (`u32::MAX`) must not be considered.

### T5: Single FreePort(0) returns Some(0)
Net with one `FreePort(0)` entry among AgentPort entries. Expected: `Some(0)`.

## Edge Cases

### E1: FreePort(u32::MAX - 1) is valid and returned
Net with `FreePort(u32::MAX - 1)`. Expected: `Some(u32::MAX - 1)`. Only exact `u32::MAX` is excluded.

### E2: Empty port array returns None
Net with an empty `ports` Vec. Expected: `None`.
