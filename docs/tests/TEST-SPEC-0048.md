# TEST-SPEC-0048: split() trivial case (num_workers=1)

**Task:** TASK-0048
**Spec:** SPEC-04
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: split with num_workers=1 returns 1 partition
Create a net with 3 agents. Call `split(&net, 1, &strategy)`. Assert `plan.partitions.len() == 1`.

### T2: Single partition contains cloned net
Call `split(&net, 1, &strategy)` on a net with agents `[0, 1, 2]`. Verify `plan.partitions[0].subnet` has the same agents and port connections as the original net.

### T3: Border map is empty in trivial case
Call `split(&net, 1, &strategy)`. Assert `plan.borders.is_empty()`.

### T4: id_range covers full u32 space
Call `split(&net, 1, &strategy)`. Assert `plan.partitions[0].id_range == IdRange { start: 0, end: u32::MAX }`.

### T5: worker_id is 0
Call `split(&net, 1, &strategy)`. Assert `plan.partitions[0].worker_id == 0`.

### T6: free_port_index is empty
Call `split(&net, 1, &strategy)`. Assert `plan.partitions[0].free_port_index.is_empty()`.

### T7: border_id_start equals border_id_end
Call `split(&net, 1, &strategy)`. Assert `plan.partitions[0].border_id_start == plan.partitions[0].border_id_end`.

### T8: Strategy allocate() is NOT called
Use a mock strategy that panics if `allocate()` is called. Call `split(&net, 1, &mock_strategy)`. Must not panic.

## Edge Cases

### E1: split with num_workers=0 returns trivial case
Call `split(&net, 0, &strategy)`. Assert `plan.partitions.len() == 1` and `plan.borders.is_empty()`. Defensive handling, no panic.

### E2: split with num_workers=1 on empty net
Call `split(&empty_net, 1, &strategy)` where the net has 0 agents. Assert 1 partition with empty subnet, empty borders.
