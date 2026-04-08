# TEST-SPEC-0015: Debug assertions for invariants I1, I2, I3, I6, I7

**Task:** TASK-0015
**Spec:** SPEC-02 R18, R18a, R18b, R19, R20, R21
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests — assert_adjacency_consistent (I1/T1)

### T1: Valid net passes without panic
Create net with several connected agents. `assert_adjacency_consistent()` does not panic.

### T2: DISCONNECTED at non-root port panics
Manually set a non-root port to DISCONNECTED. Expect panic with "T1 violated".

### T3: Root agent principal port exemption
Set `net.root = Some(AgentPort(id, 0))`. The root's principal port is DISCONNECTED in port array — no panic.

### T4: Bidirectionality violation panics
Set `ports[a] = b` but `ports[b] != a`. Expect panic with "I1 violated".

### T5: Cross-port self-loop accepted
Create agent with p1 <-> p2 (Church(0) pattern). `assert_adjacency_consistent()` does not panic.

## Unit Tests — assert_refs_valid (I2)

### T6: Valid references pass
Net with agents whose port refs all point to existing agents. No panic.

### T7: Reference to nonexistent agent panics
Port array contains `AgentPort(999, 0)` where agent 999 doesn't exist. Expect panic.

## Unit Tests — assert_next_id_valid (I3)

### T8: Valid next_id passes
Net with agents 0,1,2 and `next_id = 3`. No panic.

### T9: next_id too low panics
Manually set `next_id = 0` with agents present. Expect panic.

## Unit Tests — assert_era_unused_ports_clean (I6)

### T10: Clean ERA passes
ERA agent with DISCONNECTED at slots 1 and 2. No panic.

### T11: Dirty ERA port panics
Write a PortRef into ERA's auxiliary slot. Expect panic with "I6 violated".

## Unit Tests — assert_root_consistent (I7)

### T12: Valid root passes
`net.root = Some(AgentPort(live_id, 0))` with DISCONNECTED in port array. No panic.

### T13: FreePort root panics
`net.root = Some(FreePort(0))`. Expect panic with "R6a violated".

### T14: Non-zero port root panics
`net.root = Some(AgentPort(id, 1))`. Expect panic with "R6a violated".

### T15: Dead agent root panics
Root references removed agent. Expect panic with "R6a violated".

### T16: Root port not DISCONNECTED panics
Root port internally connected. Expect panic with "R18a violated".

## Unit Tests — count_stale_redexes

### T17: Fresh net has 0 stale redexes
No agents removed, queue has valid entries. Returns 0.

### T18: Stale count after agent removal
Create pair, remove one agent. `count_stale_redexes() > 0`.

## Unit Tests — assert_all_invariants

### T19: Valid net passes all checks
Create well-formed net. `assert_all_invariants()` does not panic.

## Edge Cases

### E1: Empty net passes all invariants
`Net::new().assert_all_invariants()` does not panic.

### E2: Net with only ERA agents passes
ERA agents with no connections except principal ports. All invariants hold.
