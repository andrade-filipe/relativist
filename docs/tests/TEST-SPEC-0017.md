# TEST-SPEC-0017: Add serde + bincode serialization support

**Task:** TASK-0017
**Spec:** SPEC-02 R24, R25, R26, R27
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Serialize empty net
`Net::new().to_bytes()` returns `Ok(bytes)` where bytes is non-empty.

### T2: Deserialize empty net
`Net::from_bytes(&bytes)` returns `Ok(net)` matching the original.

### T3: Round-trip identity (R26)
Create net with agents, connections. `Net::from_bytes(&net.to_bytes()?) == Ok(net)`.

### T4: Round-trip preserves all fields
After round-trip: `agents`, `ports`, `redex_queue`, `next_id`, `root` all match.

### T5: Corrupt bytes return Err
`Net::from_bytes(&[0xFF, 0xFF])` returns `Err(RelError::Deserialize(...))`.

### T6: Truncated bytes return Err
Serialize net, truncate half the bytes, deserialize returns Err.

## Edge Cases

### E1: Net with redexes round-trips correctly
Create active pair (redex in queue). Round-trip preserves redex queue.

### E2: Net with root set round-trips correctly
Set `net.root = Some(AgentPort(0, 0))`. Round-trip preserves root.
