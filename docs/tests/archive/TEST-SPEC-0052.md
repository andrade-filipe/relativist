# TEST-SPEC-0052: FreePort index construction per partition

**Task:** TASK-0052
**Spec:** SPEC-04
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Empty border entries produce empty HashMap
Call `build_free_port_index(&[])`. Expected: empty `HashMap`.

### T2: Two border entries produce two index entries
Input: `[(3, 0, 10), (7, 1, 11)]`. Expected: `HashMap { 10: AgentPort(3, 0), 11: AgentPort(7, 1) }`.

### T3: Values are AgentPort with correct agent_id and port_id
Input: `[(5, 2, 42)]`. Assert `index.get(&42) == Some(&AgentPort(5, 2))`.

### T4: Single border entry
Input: `[(0, 0, 0)]`. Expected: `HashMap { 0: AgentPort(0, 0) }`. Size is 1.

### T5: Index size matches input length
Input with 5 entries, all with unique border IDs. Assert `index.len() == 5`.

## Edge Cases

### E1: Border ID 0 is valid
Input: `[(1, 0, 0)]`. Expected: `HashMap { 0: AgentPort(1, 0) }`. Border ID 0 is not special.

### E2: Large border IDs
Input: `[(0, 0, u32::MAX - 1)]`. Expected: `HashMap { u32::MAX - 1: AgentPort(0, 0) }`. No overflow.
