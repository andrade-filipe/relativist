# TEST-SPEC-0010: Implement get_target and set_port helpers

**Task:** TASK-0010
**Spec:** SPEC-02 R15
**Generated:** 2026-04-06

---

## Unit Tests

### T1: set_port then get_target roundtrip
```rust
net.set_port(AgentPort(id, 0), AgentPort(99, 1));
assert_eq!(net.get_target(AgentPort(id, 0)), AgentPort(99, 1));
```

### T2: get_target out of bounds returns DISCONNECTED
```rust
assert_eq!(net.get_target(AgentPort(999, 0)), DISCONNECTED);
```

### T3: get_target(FreePort) returns DISCONNECTED
```rust
assert_eq!(net.get_target(FreePort(42)), DISCONNECTED);
```

### T4: set_port(FreePort) is a no-op
```rust
let before = net.clone();
net.set_port(FreePort(42), AgentPort(id, 0));
assert_eq!(net, before);
```

## Edge Cases

### E1: set_port on out-of-bounds is silent no-op
```rust
net.set_port(AgentPort(999, 0), FreePort(1)); // no panic
```

### E2: get_target on freshly created agent returns DISCONNECTED
```rust
for p in 0..3 { assert_eq!(net.get_target(AgentPort(id, p)), DISCONNECTED); }
```
