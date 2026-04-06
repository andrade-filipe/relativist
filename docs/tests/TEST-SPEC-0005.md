# TEST-SPEC-0005: Define Agent struct

**Task:** TASK-0005
**Spec:** SPEC-02 R5
**Generated:** 2026-04-06

---

## Unit Tests

### T1: Agent construction and field access
```rust
let a = Agent { symbol: Symbol::Con, id: 42 };
assert_eq!(a.symbol, Symbol::Con);
assert_eq!(a.id, 42);
```

### T2: Agent equality (same symbol + id)
```rust
let a1 = Agent { symbol: Symbol::Dup, id: 7 };
let a2 = Agent { symbol: Symbol::Dup, id: 7 };
assert_eq!(a1, a2);
```

### T3: Agent inequality (different id)
```rust
let a1 = Agent { symbol: Symbol::Con, id: 1 };
let a2 = Agent { symbol: Symbol::Con, id: 2 };
assert_ne!(a1, a2);
```

### T4: Agent inequality (different symbol)
```rust
let a1 = Agent { symbol: Symbol::Con, id: 1 };
let a2 = Agent { symbol: Symbol::Dup, id: 1 };
assert_ne!(a1, a2);
```

### T5: Agent is Copy
```rust
let a = Agent { symbol: Symbol::Era, id: 0 };
let b = a; // Copy
assert_eq!(a, b); // original still usable
```

### T6: Agent serde round-trip
```rust
let a = Agent { symbol: Symbol::Con, id: 100 };
let bytes = bincode::serialize(&a).unwrap();
let des: Agent = bincode::deserialize(&bytes).unwrap();
assert_eq!(a, des);
```

### T7: Agent size is compact
```rust
assert_eq!(std::mem::size_of::<Agent>(), 8); // 1 byte symbol + 3 padding + 4 byte id
// Or possibly 5 with repr(C). Verify actual.
```

## Edge Cases

### E1: Agent with id 0
```rust
let a = Agent { symbol: Symbol::Era, id: 0 };
assert_eq!(a.id, 0);
```

### E2: Agent with max id
```rust
let a = Agent { symbol: Symbol::Con, id: u32::MAX };
assert_eq!(a.id, u32::MAX);
```
