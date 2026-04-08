# TEST-SPEC-0065: Implement merge function - unite agents and internal connections

**Task:** TASK-0065
**Spec:** SPEC-05 (R1, R2, R3, R8, R9a, R38)
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Single partition with no borders returns identical net
Create a `PartitionPlan` with 1 partition containing 2 CON agents wired internally, and an empty border map. Call `merge(plan)`. Assert the result net has 2 live agents with the same symbols and internal wiring.

### T2: Two partitions with disjoint agents, no borders
Create partition A with agent ids [0, 1] (CON, DUP) and partition B with agent ids [2, 3] (ERA, CON). Empty border map. Call `merge(plan)`. Assert result has 4 live agents with correct symbols.

### T3: next_id is max of all partitions
Create partition A with `subnet.next_id = 10` and partition B with `subnet.next_id = 20`. Call `merge(plan)`. Assert `result.next_id == 20`.

### T4: Internal connections preserved
Create partition A with two CON agents (id=0, id=1) where `AgentPort(0, 1)` connects to `AgentPort(1, 1)`. Call `merge`. Assert `get_target(AgentPort(0, 1)) == AgentPort(1, 1)` and `get_target(AgentPort(1, 1)) == AgentPort(0, 1)` in the result net.

### T5: Boundary FreePort targets marked as DISCONNECTED
Create a partition with agent id=0 whose port 1 connects to `FreePort(100)` (in the border map). Call `merge`. Assert the boundary port is temporarily DISCONNECTED in the result (before Step 3 restores it).

### T6: Lafont FreePorts copied directly
Create a partition with agent id=0 whose port 2 connects to `FreePort(5)` (NOT in the border map, id < border_id_start). Call `merge`. Assert `get_target(AgentPort(0, 2)) == FreePort(5)` in the result net (SC-007).

### T7: Partition redex queues discarded
Create partition A whose `subnet.redex_queue` contains stale entries `[(0, 1)]`. Call `merge`. Assert the result net's redex queue does not contain the stale entries from the partition (R9).

## Edge Cases

### E1: Empty partitions
Create a `PartitionPlan` with 2 partitions, both containing 0 agents, and an empty border map. Call `merge`. Assert result net has 0 live agents and `next_id` is valid.

### E2: Three partitions
Create 3 partitions with agents [0,1], [2,3], [4,5] respectively. Empty borders. Call `merge`. Assert result has 6 live agents and all internal connections are preserved.

### E3: next_id tie between partitions
Create partition A with `next_id = 15` and partition B with `next_id = 15`. Assert `result.next_id == 15`.
