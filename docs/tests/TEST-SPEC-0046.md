# TEST-SPEC-0046: Wire classification logic

**Task:** TASK-0046
**Spec:** SPEC-04
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Two agents in same partition, internal wire -- no borders
Net with agents `0` and `1`, connected via principal ports. Sigma: `{0: 0, 1: 0}`, `num_workers=1`. Expected: `borders` is empty, `border_entries[0]` is empty.

### T2: Two agents in different partitions -- one border wire
Net with agents `0` and `1`, connected via principal ports (`AgentPort(0,0)` <-> `AgentPort(1,0)`). Sigma: `{0: 0, 1: 1}`, `num_workers=2`. Expected: `borders` has 1 entry mapping `bid -> (AgentPort(0,0), AgentPort(1,0))`. `border_entries[0]` contains `(0, 0, bid)` and `border_entries[1]` contains `(1, 0, bid)`.

### T3: Pre-existing FreePort classified as interface, not border
Net with agent `0` port 1 connected to `FreePort(0)`. Sigma: `{0: 0}`, `num_workers=2`. Expected: `borders` is empty (FreePort(0) is Lafont, not border).

### T4: Border IDs start from max_freeport_id + 1
Net with existing `FreePort(5)` and two agents in different partitions. Expected: first border ID is `6` (i.e., `5 + 1`).

### T5: DISCONNECTED entries produce no classification
Net with agent `0` having port 2 as `DISCONNECTED` (`FreePort(u32::MAX)`). Expected: DISCONNECTED is skipped, no border entry for that port.

### T6: Multiple border wires get unique IDs
Net with agents `0` and `1` in different partitions, connected on ports 0 and 1 (two separate border wires). Expected: `borders` has 2 entries with different border IDs. Border IDs are sequential.

### T7: border_id_start and next_border_id are set correctly
Net with no existing FreePorts and 2 border wires created. Expected: `border_id_start == 0`, `next_border_id == 2`.

## Edge Cases

### E1: Net with no live agents
Empty net, `num_workers=2`. Expected: `borders` is empty, all `border_entries` are empty, `border_id_start == 0`, `next_border_id == 0`.

### E2: All agents in one partition, no borders
Net with 4 agents all assigned to worker 0 via sigma, `num_workers=2`. Expected: `borders` is empty, `border_entries[0]` is empty, `border_entries[1]` is empty.

### E3: Border between ERA agent (arity 0) and CON agent
ERA agent `0` at worker 0, CON agent `1` at worker 1, connected via principal ports. Only port 0 is a real connection; ports 1 and 2 of ERA are DISCONNECTED. Expected: exactly 1 border entry for the principal port connection, DISCONNECTED ports are skipped.
