# TEST-SPEC-0067: Implement merge debug assertions

**Task:** TASK-0067
**Spec:** SPEC-05 (R10, R11)
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Valid merge passes assertions (debug mode)
Build a valid net with 4 agents. Split into 2 partitions (no local reduction). Merge back. Assert no panic. The debug assertions (`assert_all_invariants`) pass silently for a correctly merged net.

### T2: Corrupted net triggers assertion (debug mode)
Build a valid net. Split and merge. Then corrupt the result net by breaking bidirectionality (e.g., set `port_array[AgentPort(0, 1)]` to point to a nonexistent agent). Assert that `assert_all_invariants` panics (catches I2 reference validity violation). Use `#[should_panic]` attribute.

### T3: Assertions compiled out in release mode
Verify via documentation/code inspection that the assertion block uses `#[cfg(debug_assertions)]`. In release builds (`cargo test --release`), the assertion block is skipped entirely, so even a corrupted net would not panic at the assertion point.

## Edge Cases

### E1: Merge of empty partitions passes assertions
Create a `PartitionPlan` with empty partitions and empty borders. Merge. Assert no panic from debug assertions. An empty net is trivially valid.

### E2: Net with only ERA agents passes assertions
Build a net with 2 ERA agents connected at principal ports. Split, merge (boundary wire restored). Assert debug assertions pass (ERA agents have special slot rules per I6).
