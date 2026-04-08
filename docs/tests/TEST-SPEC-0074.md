# TEST-SPEC-0074: Integration test - split/merge identity (D1 round-trip)

**Task:** TASK-0074
**Spec:** SPEC-05 (R10, D1)
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Simple 2-agent net with 1 internal wire
Create a net with 2 CON agents: `AgentPort(0, 1) <-> AgentPort(1, 1)`. Split into 2 partitions with `ContiguousIdStrategy`. Merge (no local reduction). Assert the result net has 2 agents with the same symbols, and `get_target(AgentPort(0, 1)) == AgentPort(1, 1)`.

### T2: 4-agent chain with mixed symbols
Create 4 agents: CON(0), DUP(1), ERA(2), CON(3). Wire them in a chain: `0.p1 <-> 1.p1`, `1.p2 <-> 2.p0`, `0.p2 <-> 3.p1`. Split into 2 workers. Merge. Assert all 4 agents present with correct symbols and all wires preserved.

### T3: Net with pre-existing Lafont FreePort
Create a net with 2 CON agents. Agent 0 port 2 -> `FreePort(3)` (Lafont interface port, id < border_id_start). Split into 2 partitions. Merge. Assert `get_target(AgentPort(0, 2)) == FreePort(3)` is preserved (SC-007).

### T4: 2 workers with 1 border wire
Create a net with 2 CON agents: `AgentPort(0, 0) <-> AgentPort(1, 0)` (active pair). Split such that agent 0 is in partition A, agent 1 in partition B (border wire created). Merge (no reduction). Assert `get_target(AgentPort(0, 0)) == AgentPort(1, 0)` restored.

### T5: 3 workers with multiple border wires
Create a net with 6 agents split across 3 partitions. Multiple cross-partition wires. Merge. Assert all wires restored correctly.

### T6: 1 worker - net returned unchanged
Create a net with 4 agents. Split into 1 partition. Merge. Assert net is identical (no border wires created for 1 partition).

### T7: Empty net round-trip
Create an empty net. Split into 2 partitions. Merge. Assert result is an empty net.

### T8: Isomorphism verification - same agent count
For all tests above, assert `count_live_agents(original) == count_live_agents(merged)`.

## Edge Cases

### E1: Net with only ERA agents
Create 2 ERA agents connected at principal ports. Split into 2 partitions (border at principal wire). Merge. Assert the active pair is restored.

### E2: 4 workers on a 2-agent net
Create a net with 2 agents. Split into 4 workers (2 workers will have empty partitions). Merge. Assert result has 2 agents with correct wiring.

### E3: All agents in one partition
Create a net with 4 agents. Use a partition strategy that assigns all agents to worker 0. Split into 2 workers (worker 1 gets empty partition). Merge. Assert result is isomorphic to original.
