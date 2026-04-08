# TEST-SPEC-0064: Implement is_principal_pair helper

**Task:** TASK-0064
**Spec:** SPEC-05 (R12, R14)
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Both principal ports -> true
`is_principal_pair(AgentPort(1, 0), AgentPort(2, 0))` returns `true`.

### T2: First principal, second auxiliary -> false
`is_principal_pair(AgentPort(1, 0), AgentPort(2, 1))` returns `false`.

### T3: First auxiliary, second principal -> false
`is_principal_pair(AgentPort(1, 1), AgentPort(2, 0))` returns `false`.

### T4: Both auxiliary -> false
`is_principal_pair(AgentPort(1, 1), AgentPort(2, 2))` returns `false`.

### T5: FreePort and AgentPort -> false
`is_principal_pair(FreePort(5), AgentPort(2, 0))` returns `false`.

### T6: AgentPort and FreePort -> false
`is_principal_pair(AgentPort(1, 0), FreePort(10))` returns `false`.

### T7: Both FreePort -> false
`is_principal_pair(FreePort(5), FreePort(6))` returns `false`.

## Edge Cases

### E1: Same agent, both principal ports
`is_principal_pair(AgentPort(7, 0), AgentPort(7, 0))` returns `true`. (Self-loops on principal ports are structurally valid for detection purposes, even if semantically unusual.)

### E2: Port index 2 (second auxiliary)
`is_principal_pair(AgentPort(1, 0), AgentPort(2, 2))` returns `false`. Verifies port index 2 is not mistaken for port 0.

### E3: Module compiles
`cargo check` passes with `is_principal_pair` in `src/merge.rs`.
