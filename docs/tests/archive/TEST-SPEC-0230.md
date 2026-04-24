# TEST-SPEC-0230: Implement verify_no_redexes_full_scan (R41)

**Task:** TASK-0230
**Spec:** SPEC-05 (R41, SC-010)
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Net in Normal Form - no panic
Create a `Net` with 4 agents, none connected at principal ports to each other (no active pairs). Call `verify_no_redexes_full_scan(&net)`. Assert no panic.

### T2: Undiscovered redex triggers panic
Create a `Net` with 2 CON agents (id=0, id=1) connected at principal ports (`AgentPort(0, 0) <-> AgentPort(1, 0)`). Do NOT insert the redex into the queue (simulating a `connect()` bug). Call `verify_no_redexes_full_scan(&net)`. Assert panic with message containing "undiscovered redex". Use `#[should_panic(expected = "undiscovered redex")]`.

### T3: Empty net - no panic
Create an empty `Net` (no agents). Call `verify_no_redexes_full_scan(&net)`. Assert no panic.

### T4: Net with agents but only auxiliary connections
Create 4 agents. Wire `AgentPort(0, 1) <-> AgentPort(1, 1)` and `AgentPort(2, 2) <-> AgentPort(3, 2)`. No principal-principal connections. Call `verify_no_redexes_full_scan`. Assert no panic.

### T5: Multiple undiscovered redexes
Create 4 agents: `AgentPort(0, 0) <-> AgentPort(1, 0)` and `AgentPort(2, 0) <-> AgentPort(3, 0)`. Neither redex in the queue. Call `verify_no_redexes_full_scan`. Assert panic (on the first detected redex).

### T6: Only compiled in debug mode
Verify via code inspection that the function is annotated with `#[cfg(debug_assertions)]`. In release mode (`cargo test --release`), the function does not exist and is not called.

## Edge Cases

### E1: Agent with principal port pointing to FreePort
Create an agent whose principal port (`AgentPort(0, 0)`) points to `FreePort(5)`. Call `verify_no_redexes_full_scan`. Assert no panic. FreePort targets are not redexes.

### E2: Agent with principal port pointing to own principal port (self-loop)
Create an agent whose `AgentPort(0, 0)` points to `AgentPort(0, 0)`. This is structurally unusual. The function should detect this as an undiscovered redex only if `other_id != agent.id` check is absent. Verify the implementation's behavior matches the spec pseudocode (which checks `other_id != agent.id`, so self-loops would NOT trigger the panic).

### E3: Sparse agent array with holes
Create a `Net` with capacity 100 but only agents at ids 5, 50, 95. Agent 5 and 50 connected at principal ports (undiscovered redex). Call `verify_no_redexes_full_scan`. Assert panic. Verifies the scan correctly skips `None` slots.
