# TEST-SPEC-0219: Stale boundary FreePort precondition assertion

**Task:** TASK-0219
**Spec:** SPEC-04
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Clean net with only Lafont FreePorts passes
Net with `FreePort(0)`, `FreePort(1)`, `FreePort(2)` and no boundary-range FreePorts. Call `assert_no_stale_boundary_freeports(&net)`. Must not panic.

### T2: Net with stale boundary FreePort triggers panic
Net with a `FreePort(1000000)` artificially inserted (simulating an unresolved boundary from a previous round, well above any plausible Lafont range). Call `assert_no_stale_boundary_freeports(&net)`. Expected: panic with descriptive message.

### T3: Net with no FreePorts at all passes
Net where all port entries are `AgentPort` or `DISCONNECTED`. Call `assert_no_stale_boundary_freeports(&net)`. Must not panic.

### T4: Net with only DISCONNECTED entries passes
Net with multiple `FreePort(u32::MAX)` (DISCONNECTED) entries. Call the assertion. Must not panic (DISCONNECTED is not a stale boundary).

### T5: Net with consecutive low-range FreePorts passes
Net with `FreePort(0)` through `FreePort(5)`. These are plausible Lafont FreePorts. Assertion passes.

## Edge Cases

### E1: Assertion compiled out in release mode
`assert_no_stale_boundary_freeports` is gated by `#[cfg(debug_assertions)]`. In release builds, calling the function is a no-op.

### E2: First-round split (fresh net) always passes
A freshly constructed net with only Lafont FreePorts (no prior split/merge cycle). The assertion must pass trivially since there are no stale boundaries.
