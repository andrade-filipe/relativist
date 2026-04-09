# TEST-SPEC-0185: Graph isomorphism (nets_isomorphic)

**Task:** TASK-0185
**Spec:** SPEC-09, SPEC-08
**Requirements verified:** R4, R36, R37

---

## Tests

### T1: Two empty nets are isomorphic
**Input:** `Net::new()`, `Net::new()`
**Expected:** `true`

### T2: Identical single-agent nets
**Input:** Net with one CON agent, and a clone of it.
**Expected:** `true`

### T3: Different symbols -> not isomorphic
**Input:** Net A with one CON, Net B with one DUP.
**Expected:** `false`

### T4: Same topology, different AgentIds
**Input:** Net A: CON(id=0) <-> ERA(id=1). Net B: CON(id=5) <-> ERA(id=3). Same wiring pattern.
**Expected:** `true`

### T5: Same agents, different connectivity -> not isomorphic
**Input:** Net A: CON.p0 <-> ERA.p0. Net B: CON.p1 <-> ERA.p0 (different port).
**Expected:** `false`

### T6: CON-CON pair with swapped IDs
**Input:** Net A: CON(0).p0 <-> CON(1).p0. Net B: CON(1).p0 <-> CON(0).p0.
**Expected:** `true`

### T7: EP-annihilation result isomorphism
**Input:** Build ep_annihilation(10), reduce_all, compare two copies.
**Expected:** `true` (both reduce to empty net)

### T8: Different FreePort indices -> not isomorphic
**Input:** Net A: agent.p0 <-> FreePort(0). Net B: agent.p0 <-> FreePort(1).
**Expected:** `false`

### T9: Different agent counts -> not isomorphic
**Input:** Net A: 3 agents. Net B: 2 agents.
**Expected:** `false`

### T10: Church numeral encode/decode isomorphism
**Input:** `encode_nat(5)` and another `encode_nat(5)`.
**Expected:** `true`

### T11: Different Church numerals -> not isomorphic
**Input:** `encode_nat(3)` and `encode_nat(4)`.
**Expected:** `false`

### T12: Performance — 50-agent net in < 1 second
**Input:** `encode_nat(20)` (41 agents). Compare with clone.
**Expected:** Returns `true` within 1 second.

## Edge Cases

1. **Self-loops:** Agent with p1 <-> p2 self-loop (Church 0's lam_x).
2. **Disconnected ports:** Agents with DISCONNECTED ports.
3. **Root comparison:** Nets with different root settings but same structure.
