# TEST-SPEC-0018: Verify PartialEq and Eq for Net

**Task:** TASK-0018
**Spec:** SPEC-02 R26, R26a
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Two empty nets are equal
`Net::new() == Net::new()` is `true`.

### T2: Identical nets with agents are equal
Create two nets with same agents, same connections. `net_a == net_b`.

### T3: Nets differing in next_id are not equal
Same agents/connections but different `next_id`. `net_a != net_b`.

### T4: Nets differing in agents are not equal
One net has an extra agent. `net_a != net_b`.

### T5: Nets differing in port connections are not equal
Same agents but different wiring. `net_a != net_b`.

### T6: Serialization round-trip preserves equality
`Net::from_bytes(&net.to_bytes()?) == Ok(net)`.

## Edge Cases

### E1: Net with root vs without root
`net_a.root = Some(...)`, `net_b.root = None`. `net_a != net_b`.

### E2: Nets differing only in redex queue order
Same redexes but different queue order. Structurally different → not equal.
