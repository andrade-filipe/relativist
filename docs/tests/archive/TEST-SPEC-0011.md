# TEST-SPEC-0011: Implement connect

**Task:** TASK-0011
**Spec:** SPEC-02 R13, R9, R18, R18b
**Generated:** 2026-04-06

---

## Unit Tests

### T1: Bidirectional linkage
```rust
net.connect(AgentPort(a, 1), AgentPort(b, 2));
assert_eq!(net.get_target(AgentPort(a, 1)), AgentPort(b, 2));
assert_eq!(net.get_target(AgentPort(b, 2)), AgentPort(a, 1));
```

### T2: Principal-principal enqueues redex
```rust
net.connect(AgentPort(a, 0), AgentPort(b, 0));
assert_eq!(net.redex_queue.len(), 1);
assert_eq!(net.redex_queue[0], (a, b));
```

### T3: Principal-auxiliary does NOT enqueue
### T4: Auxiliary-auxiliary does NOT enqueue
### T5: AgentPort to FreePort (no redex, FreePort side is no-op)
### T6: Intra-agent connection is valid (R18b)
### T7: Same-port self-connection panics in debug (R18b)
```rust
#[should_panic(expected = "Same-port self-connection is invalid")]
```
