# TEST-SPEC-0008: Define Net struct and constructors

**Task:** TASK-0008
**Spec:** SPEC-02 R6, R6a, R7, R8, R9, R10, R22, R26a
**Generated:** 2026-04-06

---

## Unit Tests

### T1: Net::new() returns empty net
```rust
let net = Net::new();
assert!(net.agents.is_empty());
assert!(net.ports.is_empty());
assert!(net.redex_queue.is_empty());
assert_eq!(net.next_id, 0);
assert_eq!(net.root, None);
```

### T2: Net::with_capacity pre-allocates
```rust
let net = Net::with_capacity(100);
assert!(net.agents.capacity() >= 100);
assert!(net.ports.capacity() >= 300); // 100 * PORTS_PER_SLOT
assert!(net.agents.is_empty());
assert!(net.ports.is_empty());
assert!(net.redex_queue.is_empty());
assert_eq!(net.next_id, 0);
assert_eq!(net.root, None);
```

### T3: Net implements Clone
```rust
let net = Net::new();
let net2 = net.clone();
assert_eq!(net, net2);
```

### T4: Net implements PartialEq and Eq (R26a)
```rust
let a = Net::new();
let b = Net::new();
assert_eq!(a, b);
```

### T5: Net implements Debug
```rust
let net = Net::new();
let debug_str = format!("{:?}", net);
assert!(debug_str.contains("Net"));
```

### T6: Net serde round-trip
```rust
let net = Net::new();
let bytes = bincode::serialize(&net).unwrap();
let des: Net = bincode::deserialize(&bytes).unwrap();
assert_eq!(net, des);
```

### T7: Net::with_capacity(0) works like new()
```rust
let net = Net::with_capacity(0);
assert!(net.agents.is_empty());
assert!(net.ports.is_empty());
assert_eq!(net.next_id, 0);
assert_eq!(net.root, None);
```

## Edge Cases

### E1: Net::with_capacity large value
```rust
let net = Net::with_capacity(10_000);
assert!(net.agents.capacity() >= 10_000);
assert!(net.ports.capacity() >= 30_000);
```

### E2: Two independently created nets are equal
```rust
let a = Net::new();
let b = Net::new();
assert_eq!(a, b);
```

### E3: Cloned net is independent (modify original doesn't affect clone)
```rust
let mut net = Net::new();
let clone = net.clone();
net.next_id = 42;
assert_ne!(net, clone);
```
