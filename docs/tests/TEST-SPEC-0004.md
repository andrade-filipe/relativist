# TEST-SPEC-0004: Define PortRef enum

**Task:** TASK-0004
**Spec:** SPEC-02 R4
**Generated:** 2026-04-06

---

## Unit Tests

### T1: Variant discrimination
AgentPort and FreePort with same inner values are NOT equal.
```rust
assert_ne!(PortRef::AgentPort(0, 0), PortRef::FreePort(0));
```

### T2: AgentPort equality
```rust
assert_eq!(PortRef::AgentPort(5, 1), PortRef::AgentPort(5, 1));
assert_ne!(PortRef::AgentPort(5, 1), PortRef::AgentPort(5, 2));
assert_ne!(PortRef::AgentPort(5, 1), PortRef::AgentPort(6, 1));
```

### T3: FreePort equality
```rust
assert_eq!(PortRef::FreePort(42), PortRef::FreePort(42));
assert_ne!(PortRef::FreePort(42), PortRef::FreePort(43));
```

### T4: Copy semantics
```rust
let p = PortRef::AgentPort(1, 0);
let p2 = p; // Copy
assert_eq!(p, p2); // original still usable
```

### T5: Pattern matching extracts fields
```rust
match PortRef::AgentPort(7, 2) {
    PortRef::AgentPort(id, port) => { assert_eq!(id, 7); assert_eq!(port, 2); }
    _ => panic!("wrong variant"),
}
match PortRef::FreePort(99) {
    PortRef::FreePort(bid) => assert_eq!(bid, 99),
    _ => panic!("wrong variant"),
}
```

### T6: Serde round-trip
```rust
for pr in [PortRef::AgentPort(100, 0), PortRef::FreePort(55)] {
    let bytes = bincode::serialize(&pr).unwrap();
    let des: PortRef = bincode::deserialize(&bytes).unwrap();
    assert_eq!(pr, des);
}
```

### T7: Hash (usable as HashMap key)
```rust
let mut set = HashSet::new();
set.insert(PortRef::AgentPort(1, 0));
set.insert(PortRef::FreePort(1));
assert_eq!(set.len(), 2);
```

### T8: Debug formatting
```rust
assert!(format!("{:?}", PortRef::AgentPort(3, 1)).contains("AgentPort"));
assert!(format!("{:?}", PortRef::FreePort(7)).contains("FreePort"));
```

## Edge Cases

### E1: FreePort(u32::MAX) is a valid value
Reserved for DISCONNECTED sentinel (TASK-0007), but PortRef itself allows it.

### E2: AgentPort with PortId > 2
Structurally valid at the type level. Validation is at call sites (arity check).
