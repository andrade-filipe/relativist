# TEST-SPEC-0012: Implement disconnect

**Task:** TASK-0012
**Spec:** SPEC-02 R14
**Generated:** 2026-04-06

---

## Unit Tests

### T1: Disconnect breaks both sides
```rust
net.connect(AgentPort(a, 1), AgentPort(b, 2));
net.disconnect(AgentPort(a, 1));
assert_eq!(net.get_target(AgentPort(a, 1)), DISCONNECTED);
assert_eq!(net.get_target(AgentPort(b, 2)), DISCONNECTED);
```

### T2: Disconnect already-disconnected is no-op
### T3: Disconnect FreePort is no-op

## Edge Cases

### E1: Disconnect from target side also clears both
