# TEST-SPEC-0009: Implement create_agent

**Task:** TASK-0009
**Spec:** SPEC-02 R11, R10
**Generated:** 2026-04-06

---

## Unit Tests

### T1: Create first agent
```rust
let mut net = Net::new();
let id = net.create_agent(Symbol::Con);
assert_eq!(id, 0);
assert_eq!(net.next_id, 1);
assert_eq!(net.agents[0], Some(Agent { symbol: Symbol::Con, id: 0 }));
```

### T2: Create 3 agents sequentially
```rust
let ids = [net.create_agent(Symbol::Con), net.create_agent(Symbol::Dup), net.create_agent(Symbol::Era)];
assert_eq!(ids, [0, 1, 2]);
assert_eq!(net.next_id, 3);
```

### T3: Port array expands (3 agents = 9 slots)
```rust
assert!(net.ports.len() >= 9);
```

### T4: New port slots are DISCONNECTED
```rust
for p in 0..3u8 {
    assert_eq!(net.ports[port_index(id, p)], DISCONNECTED);
}
```

## Edge Cases

### E1: ERA gets 3 slots (uniform layout)
```rust
let id = net.create_agent(Symbol::Era);
for p in 0..3u8 { assert_eq!(net.ports[port_index(id, p)], DISCONNECTED); }
```

### E2: Symbols stored correctly
```rust
assert_eq!(net.agents[0].unwrap().symbol, Symbol::Con);
assert_eq!(net.agents[1].unwrap().symbol, Symbol::Dup);
```
