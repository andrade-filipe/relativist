# TEST-SPEC-0220: Root port propagation during split (R28)

**Task:** TASK-0220
**Spec:** SPEC-04
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: AgentPort root inherited by correct partition
Net with `root = Some(AgentPort(3, 0))`. Partition containing agent 3 (agent_ids includes 3). Call `propagate_root`. Expected: `Some(AgentPort(3, 0))`.

### T2: AgentPort root not inherited by wrong partition
Net with `root = Some(AgentPort(3, 0))`. Partition NOT containing agent 3 (agent_ids is `[0, 1, 2]`). Call `propagate_root`. Expected: `None`.

### T3: None root yields None for all partitions
Net with `root = None`. Call `propagate_root` for any partition. Expected: `None`.

### T4: FreePort root inherited by partition with connected agent
Net with `root = Some(FreePort(5))`. Agent `2` is connected to `FreePort(5)` in the original net. Partition containing agent 2 (agent_ids includes 2). Call `propagate_root`. Expected: `Some(FreePort(5))`.

### T5: FreePort root not inherited by partition without connected agent
Net with `root = Some(FreePort(5))`. Agent `2` is connected to `FreePort(5)`. Partition NOT containing agent 2 (agent_ids is `[0, 1]`). Call `propagate_root`. Expected: `None`.

### T6: Split with 2 workers -- exactly one partition gets non-None root
Net with `root = Some(AgentPort(1, 0))`. `split(&net, 2, &strategy)`. Count how many partitions have `subnet.root != None`. Expected: exactly 1.

### T7: DISCONNECTED root treated as None
Net with `root = Some(FreePort(u32::MAX))` (DISCONNECTED sentinel). Call `propagate_root`. Expected: `None` (DISCONNECTED is not a real root).

## Edge Cases

### E1: Root agent is agent 0 in first partition
Net with `root = Some(AgentPort(0, 0))`. Agent 0 in partition 0. Verify partition 0 gets the root, all others get `None`.

### E2: FreePort root with no connected agent
Net with `root = Some(FreePort(99))` but no agent is connected to `FreePort(99)` (orphaned). Call `propagate_root`. Expected: `None` for all partitions (no agent owns this FreePort).
