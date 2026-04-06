# TEST-SPEC-0013: Implement remove_agent

**Task:** TASK-0013
**Spec:** SPEC-02 R12
**Generated:** 2026-04-06

---

## Unit Tests

### T1: Remove CON disconnects all 3 ports, slot becomes None
### T2: Remove ERA (only 1 port to disconnect)
### T3: Remove already-removed is no-op
### T4: next_id unchanged after removal

## Edge Cases

### E1: Remove out-of-bounds id is no-op (no panic)
