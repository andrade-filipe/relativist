# TEST-SPEC-0014: Implement is_reduced and is_valid_redex

**Task:** TASK-0014
**Spec:** SPEC-02 R16, R17
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests — is_reduced

### T1: Empty net is reduced
`Net::new().is_reduced()` returns `true` (empty queue).

### T2: Net with redex is not reduced
Create CON(a) <-> CON(b) active pair. `net.is_reduced()` returns `false`.

### T3: Net after reduce_all is reduced
Create CON(a) <-> CON(b), run reduce_all. `net.is_reduced()` returns `true`.

## Unit Tests — is_valid_redex

### T4: Valid redex returns true
Create CON(a) <-> CON(b) with principal ports connected. `net.is_valid_redex(a, b)` returns `true`.

### T5: Stale redex (agent removed) returns false
Create pair, remove agent a. `net.is_valid_redex(a, b)` returns `false`.

### T6: Stale redex (connection changed) returns false
Create pair, disconnect a.p0, reconnect to a different agent. `net.is_valid_redex(a, b)` returns `false`.

### T7: Out-of-bounds agent ID returns false
`net.is_valid_redex(u32::MAX, 0)` returns `false`.

## Edge Cases

### E1: Both agents removed returns false
Remove both a and b. `net.is_valid_redex(a, b)` returns `false`.

### E2: Same agent ID for both args
`net.is_valid_redex(a, a)` — returns `false` (an agent's principal port cannot connect to itself per T1).
