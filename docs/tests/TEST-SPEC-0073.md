# TEST-SPEC-0073: Implement count_live_agents helper

**Task:** TASK-0073
**Spec:** SPEC-05 (R35), SPEC-02 (R16a)
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Empty net returns 0
Create an empty `Net` (agents vec is empty or all None). Call `count_live_agents`. Assert result is `0`.

### T2: Net with 3 agents returns 3
Create a `Net` with 3 live agents (CON, DUP, ERA) at ids 0, 1, 2. Call `count_live_agents`. Assert result is `3`.

### T3: Net with holes (sparse agents) returns correct count
Create a `Net` with capacity for 10 agents but only agents at ids 0, 3, 7 are `Some`. All others are `None`. Call `count_live_agents`. Assert result is `3`.

### T4: All slots None returns 0
Create a `Net` with `agents` vec of length 5, all `None`. Call `count_live_agents`. Assert result is `0`.

### T5: All slots occupied
Create a `Net` with 4 agents at consecutive ids 0-3, all `Some`. Call `count_live_agents`. Assert result is `4`.

## Edge Cases

### E1: After agent removal
Create a `Net` with 2 agents. Remove agent 0 (set `agents[0] = None`). Call `count_live_agents`. Assert result is `1`.

### E2: Single agent
Create a `Net` with exactly 1 agent at id 0. Call `count_live_agents`. Assert result is `1`.

### E3: Module compiles
`cargo check` passes with `count_live_agents` accessible (either as `Net::count_live_agents()` method or free function in `src/merge.rs`).
