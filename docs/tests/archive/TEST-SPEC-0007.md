# TEST-SPEC-0007: PORTS_PER_SLOT, port_index, DISCONNECTED

**Task:** TASK-0007
**Spec:** SPEC-02 R8 (partial)
**Generated:** 2026-04-06

---

## Unit Tests

### T1: PORTS_PER_SLOT value
```rust
assert_eq!(PORTS_PER_SLOT, 3);
```

### T2: port_index correctness
```rust
assert_eq!(port_index(0, 0), 0);
assert_eq!(port_index(0, 1), 1);
assert_eq!(port_index(0, 2), 2);
assert_eq!(port_index(1, 0), 3);
assert_eq!(port_index(1, 1), 4);
assert_eq!(port_index(5, 1), 16);
```

### T3: DISCONNECTED is FreePort(u32::MAX)
```rust
assert_eq!(DISCONNECTED, PortRef::FreePort(u32::MAX));
```

### T4: DISCONNECTED is distinguishable from valid AgentPort
```rust
assert_ne!(DISCONNECTED, PortRef::AgentPort(0, 0));
assert_ne!(DISCONNECTED, PortRef::AgentPort(u32::MAX, 0));
```

## Edge Cases

### E1: port_index with agent_id 0 and max port
```rust
assert_eq!(port_index(0, 2), 2);
```

### E2: port_index large agent_id
```rust
assert_eq!(port_index(1000, 0), 3000);
```
