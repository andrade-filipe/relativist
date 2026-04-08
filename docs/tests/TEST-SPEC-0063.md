# TEST-SPEC-0063: Implement rebuild_free_port_index function

**Task:** TASK-0063
**Spec:** SPEC-05 (R20, R21, R22, R23; SPEC-04 R15a)
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Empty subnet returns empty index
Create an empty `Net` (no agents). Call `rebuild_free_port_index(&net, 100, 200)`. Assert the returned `HashMap` is empty.

### T2: Single agent connected to boundary FreePort
Create a `Net` with one CON agent (id=0). Wire port 1 of agent 0 to `FreePort(150)`. Set `border_id_start=100`, `border_id_end=200`. Call `rebuild_free_port_index`. Assert result contains `{150: AgentPort(0, 1)}` and length is 1.

### T3: Multiple boundary FreePort connections
Create a `Net` with two CON agents (id=0, id=1). Wire agent 0 port 1 to `FreePort(100)`, agent 1 port 2 to `FreePort(101)`. Set `border_id_start=100`, `border_id_end=200`. Assert result is `{100: AgentPort(0, 1), 101: AgentPort(1, 2)}`.

### T4: Lafont FreePort excluded
Create a `Net` with one CON agent (id=0). Wire port 1 to `FreePort(5)` (Lafont, below border_id_start). Set `border_id_start=100`, `border_id_end=200`. Assert result is empty (Lafont FreePorts are excluded).

### T5: DISCONNECTED sentinel excluded
Create a `Net` with one CON agent (id=0). Wire port 1 to `FreePort(u32::MAX)` (DISCONNECTED). Set `border_id_start=100`, `border_id_end=200`. Assert result is empty.

### T6: Internal connections ignored
Create a `Net` with two CON agents (id=0, id=1) wired together (agent 0 port 1 -> agent 1 port 1). No FreePort connections. Assert result is empty.

### T7: After erasure - FreePort transferred to ERA replacement
Simulate the erasure scenario: create an ERA agent (id=2) that inherited a boundary FreePort connection at `FreePort(120)`. Set `border_id_start=100`, `border_id_end=200`. Call `rebuild_free_port_index`. Assert `{120: AgentPort(2, 0)}` is in the result.

### T8: FreePort at border_id_end boundary (exclusive)
Create a `Net` with one CON agent wired to `FreePort(200)`. Set `border_id_start=100`, `border_id_end=200`. Assert result is empty (`border_id_end` is exclusive).

## Edge Cases

### E1: border_id_start == border_id_end (empty range)
Call with `border_id_start=100`, `border_id_end=100`. Even if agents have FreePort connections with id=100, the range is empty so result must be empty.

### E2: Agent with all ports connected to boundary FreePorts
Create a CON agent (id=0) with port 0 -> `FreePort(100)`, port 1 -> `FreePort(101)`, port 2 -> `FreePort(102)`. Set `border_id_start=100`, `border_id_end=200`. Assert result contains all three entries.

### E3: Mixed boundary and Lafont FreePorts on same agent
Create a CON agent (id=0) with port 0 -> `FreePort(5)` (Lafont), port 1 -> `FreePort(150)` (boundary), port 2 -> `AgentPort(1, 0)` (internal). Assert result contains only `{150: AgentPort(0, 1)}`.
