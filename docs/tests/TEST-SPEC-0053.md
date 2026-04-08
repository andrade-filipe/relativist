# TEST-SPEC-0053: Debug assertion for C1 (complete agent coverage)

**Task:** TASK-0053
**Spec:** SPEC-04
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Correct split passes assertion
Create a net with agents `[0, 1, 2, 3]`. Build 2 partitions: partition 0 has agents `{0, 1}`, partition 1 has agents `{2, 3}`. Call `assert_c1(&net, &partitions)`. Must not panic.

### T2: Missing agent triggers panic
Create a net with agents `[0, 1, 2]`. Build 2 partitions: partition 0 has `{0}`, partition 1 has `{1}` (agent 2 is missing). Call `assert_c1`. Expected: panic with message containing "C1".

### T3: Duplicated agent triggers panic
Create a net with agents `[0, 1]`. Build 2 partitions: partition 0 has `{0, 1}`, partition 1 has `{1}` (agent 1 duplicated). Call `assert_c1`. Expected: panic with message indicating disjointness violation.

### T4: Single partition with all agents passes
Net with agents `[0, 1, 2]`. One partition with all 3 agents. `assert_c1` must not panic.

### T5: Empty net with empty partitions passes
Net with 0 agents. Partitions list is empty or contains only empty partitions. `assert_c1` must not panic.

## Edge Cases

### E1: Assertion is compiled out in release mode
`assert_c1` is gated by `#[cfg(debug_assertions)]`. In release builds, calling the function should be a no-op (function does not exist).

### E2: Partition with extra agent not in original net
Net with agents `[0, 1]`. Partition has agents `{0, 1, 5}` where agent 5 does not exist in the original. `assert_c1` should panic (count mismatch: 3 != 2).
