# TEST-SPEC-0019: Implement get_agent and get_agent_mut accessors

**Task:** TASK-0019
**Spec:** SPEC-02 R15a, R15b
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests — get_agent

### T1: Returns Some for live agent
Create agent, `get_agent(id)` returns `Some(&agent)` with correct symbol and id.

### T2: Returns None for out-of-range ID
`net.get_agent(u32::MAX)` returns `None`.

### T3: Returns None for removed agent
Create agent, remove it, `get_agent(id)` returns `None`.

### T4: Returns None on empty net
`Net::new().get_agent(0)` returns `None`.

### T5: Correct symbol in returned agent
Create CON agent, verify `get_agent(id).unwrap().symbol == Symbol::Con`.

## Unit Tests — get_agent_mut

### T6: Returns Some and allows mutation
Create agent, `get_agent_mut(id)` returns `Some`, can read symbol.

### T7: Returns None for out-of-range ID
`net.get_agent_mut(999)` returns `None`.

### T8: Returns None for removed agent
Create and remove agent, `get_agent_mut(id)` returns `None`.

## Edge Cases

### E1: Sequential agent creation returns correct agents
Create 3 agents (CON, DUP, ERA). `get_agent` for each returns correct symbol.

### E2: get_agent after connect still works
Create agents, connect them, `get_agent` still returns correct references.
