# TEST-SPEC-0049: split() general case orchestrator (Steps 2-7)

**Task:** TASK-0049
**Spec:** SPEC-04
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: 4-agent net, 2 workers -- correct agent distribution
Net with agents `[0, 1, 2, 3]` with internal wiring. `split(&net, 2, &ContiguousIdStrategy)`. Verify partition 0 has agents `{0, 1}` and partition 1 has agents `{2, 3}`.

### T2: 2 agents, 4 workers -- excess workers get empty partitions
Net with agents `[0, 1]`. `split(&net, 4, &ContiguousIdStrategy)`. Verify 4 partitions total: 2 non-empty (with 1 agent each), 2 empty (no live agents).

### T3: Border map has correct entries for cut wires
Net with agents `0` and `1` connected via principal ports. `split(&net, 2, &ContiguousIdStrategy)`. Verify `plan.borders` has exactly 1 entry mapping to `(AgentPort(0, 0), AgentPort(1, 0))`.

### T4: subnet.next_id correctly initialized per ID range
Net with agents `[0, 1]`, `split(&net, 2, &strategy)`. Verify `partitions[0].subnet.next_id >= partitions[0].id_range.start` and `partitions[1].subnet.next_id >= partitions[1].id_range.start`.

### T5: Union of all partition agents equals original agents
Net with agents `[0, 1, 2, 3, 4]`. `split(&net, 3, &strategy)`. Collect all live agent IDs across all 3 partitions. Assert the union equals `{0, 1, 2, 3, 4}`.

### T6: Each partition has correct worker_id
`split(&net, 3, &strategy)`. Assert `partitions[0].worker_id == 0`, `partitions[1].worker_id == 1`, `partitions[2].worker_id == 2`.

### T7: border_id_start and border_id_end are set on all partitions
Net with 2 border wires. Verify all partitions share the same `border_id_start` and `border_id_end` values, and `border_id_end - border_id_start == 2`.

### T8: Deterministic output
Call `split(&net, 2, &strategy)` twice on the same net. Assert both `PartitionPlan` results are identical (same partitions, same borders).

## Edge Cases

### E1: Net with no internal wires, all borders
Net with 4 agents, each pair connected across partitions with `num_workers=4`. Verify every wire becomes a border and each partition has 0 internal connections.

### E2: Net with pre-existing Lafont FreePorts
Net with agent `0` connected to `FreePort(0)`. `split(&net, 2, &strategy)`. Verify `FreePort(0)` appears in the subnet unchanged and is NOT in the border map.
