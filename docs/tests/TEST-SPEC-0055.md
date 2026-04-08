# TEST-SPEC-0055: FreePort index lazy reconstruction

**Task:** TASK-0055
**Spec:** SPEC-04
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Partition with no boundary FreePorts returns empty index
Subnet with 2 agents, no `FreePort(bid)` entries in the boundary range. `border_id_start=10, border_id_end=10`. Call `rebuild_free_port_index`. Expected: empty HashMap.

### T2: Boundary FreePort within range is included
Subnet with agent `0` port 1 connected to `FreePort(5)`. `border_id_start=5, border_id_end=10`. Call `rebuild_free_port_index`. Expected: `{5: AgentPort(0, 1)}`.

### T3: Lafont FreePort below range is excluded
Subnet with agent `0` port 1 connected to `FreePort(2)`. `border_id_start=5, border_id_end=10`. Call `rebuild_free_port_index`. Expected: empty HashMap (2 < 5, so it is a Lafont FreePort).

### T4: DISCONNECTED (u32::MAX) is excluded
Subnet with ERA agent `0` port 2 as `FreePort(u32::MAX)`. `border_id_start=0, border_id_end=100`. Call `rebuild_free_port_index`. Expected: `u32::MAX` is not in the index.

### T5: After simulated reconnection, rebuilt index reflects new agent
Subnet originally had `FreePort(5)` connected to agent `0`. After simulated reduction, `FreePort(5)` is now connected to agent `3`. `border_id_start=5, border_id_end=10`. Rebuild. Expected: `{5: AgentPort(3, p)}` where `p` is the port of agent 3.

### T6: After simulated erasure, rebuilt index points to new ERA agent
Agent `0` consumed by reduction, replaced by ERA agent `2` that inherits the `FreePort(7)` connection. `border_id_start=5, border_id_end=10`. Rebuild. Expected: `{7: AgentPort(2, p)}`.

### T7: Multiple boundary FreePorts all captured
Subnet with `FreePort(5)` on agent 0 port 0, `FreePort(6)` on agent 1 port 1. `border_id_start=5, border_id_end=10`. Expected: `{5: AgentPort(0, 0), 6: AgentPort(1, 1)}`.

## Edge Cases

### E1: border_id_start equals border_id_end (no borders)
`border_id_start=5, border_id_end=5`. The range `[5, 5)` is empty. Even if `FreePort(5)` exists in the port array, it should NOT be included (range is empty). Expected: empty HashMap.

### E2: FreePort ID at boundary of range
`border_id_start=10, border_id_end=11`. `FreePort(10)` should be included. `FreePort(11)` should NOT (exclusive end). `FreePort(9)` should NOT (below start).
