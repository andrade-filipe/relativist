# TEST-SPEC-0231: Implement count_live_agents and live_agents on Net

**Task:** TASK-0231
**Spec:** SPEC-02 R16a, R16b
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests — count_live_agents

### T1: Empty net returns 0
`Net::new().count_live_agents()` returns `0`.

### T2: Net with 3 agents returns 3
Create 3 agents (CON, DUP, ERA). `count_live_agents()` returns `3`.

### T3: After removal, count reflects live agents
Create 5 agents, remove 2. `count_live_agents()` returns `3`.

## Unit Tests — live_agents

### T4: Empty net yields 0 elements
`Net::new().live_agents().count()` returns `0`.

### T5: Yields agents in arena order, skipping None
Create 3 agents, remove middle one. `live_agents()` yields first and third only.

### T6: Consistency with count_live_agents
`net.live_agents().count() == net.count_live_agents()` for any net state.

### T7: Correct symbols in iterator
Create CON, DUP, ERA. `live_agents()` yields agents with symbols [Con, Dup, Era] in order.

## Edge Cases

### E1: All agents removed
Create 3 agents, remove all. `count_live_agents() == 0`, `live_agents().count() == 0`.

### E2: Single agent net
Create 1 agent. `count_live_agents() == 1`, `live_agents()` yields exactly that agent.
