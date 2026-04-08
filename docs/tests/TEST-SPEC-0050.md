# TEST-SPEC-0050: Build sub-net for one partition

**Task:** TASK-0050
**Spec:** SPEC-04
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Two agents internally connected, same partition -- identical wiring
Net with agents `0` and `1` connected via `AgentPort(0,0) <-> AgentPort(1,0)`. Both assigned to worker 0. Call `build_subnet`. Verify subnet has both agents with `ports[port_index(0,0)] == AgentPort(1,0)` and `ports[port_index(1,0)] == AgentPort(0,0)`.

### T2: Two agents in different partitions -- FreePort(bid) replacement
Net with agents `0` and `1` connected via principal ports. Agent `0` in worker 0, agent `1` in worker 1. Border entry for worker 0: `(0, 0, 42)`. Verify subnet for worker 0 has `ports[port_index(0,0)] == FreePort(42)`.

### T3: Pre-existing FreePort preserved
Net with agent `0` port 1 connected to `FreePort(5)`. Build subnet for worker 0. Verify `ports[port_index(0,1)] == FreePort(5)`.

### T4: ERA agent -- all 3 port slots copied
Net with ERA agent `0` (arity 0). Ports 1 and 2 are `DISCONNECTED`. Build subnet. Verify all 3 port slots are present: port 0 has the actual connection, ports 1 and 2 are `DISCONNECTED`.

### T5: Port linearity maintained
Net with 2 agents in same partition, 1 border wire (bid=10). Build subnet. Scan all port entries: `FreePort(10)` appears exactly once.

### T6: Sparse sizing -- agents Vec sized to max_agent_id + 1
Net with agents at IDs `[0, 5]` assigned to worker 0. Build subnet. Verify `subnet.agents.len() >= 6` (at least `5 + 1`). Slots `1, 2, 3, 4` are `None`.

### T7: Ports Vec sized correctly
Net with agents at IDs `[0, 5]`. Build subnet. Verify `subnet.ports.len() >= 6 * PORTS_PER_SLOT` (i.e., 18). Port slots for absent agents are `DISCONNECTED`.

### T8: Agents not in partition have None slots
Net with agents `[0, 1, 2]`. Worker 0 gets only agent `0`. Verify `subnet.agents[1] == None` and `subnet.agents[2] == None`.

## Edge Cases

### E1: Empty partition (no agents assigned)
Worker gets 0 agents. Build subnet. Verify `subnet.agents` is empty (or minimally sized), `subnet.ports` is empty, no panic.

### E2: Single agent with all ports as DISCONNECTED
ERA agent `0` with all 3 ports as `DISCONNECTED`. Build subnet. All 3 port slots are `DISCONNECTED`, agent is present.
