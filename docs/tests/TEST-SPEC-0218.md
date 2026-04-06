# TEST-SPEC-0218: Implement link helper function (safe port reconnection)

**Task:** TASK-0218
**Spec:** SPEC-03 R10, R25, R26
**Generated:** 2026-04-06

---

## Unit Tests

### T1: Link two live AgentPorts establishes bidirectional connection
Create two CON agents, call `link` with their auxiliary ports. Verify `get_target` returns the expected peer for both sides.

### T2: Link where one endpoint is a removed agent is a no-op
Create two CON agents, remove the first, then call `link`. Verify no port array mutation occurs (both ports remain DISCONNECTED).

### T3: Link where the other endpoint is a removed agent is a no-op
Same as T2 but remove the second agent instead of the first.

### T4: Link where both endpoints are removed agents is a no-op
Create two CON agents, remove both, then call `link`. Verify no port array mutation.

### T5: Link with FreePort endpoint always proceeds
Create one CON agent and call `link` between its auxiliary port and a `FreePort(0)`. Verify the AgentPort side has `FreePort(0)` written (one-sided write per R26).

### T6: Link between two principal ports detects new redex
Create two CON agents, call `link` on their principal ports (port 0). Verify the redex queue contains the new pair.

## Edge Cases

### E1: Link with FreePort on both sides delegates to connect (no-op in set_port for both)
Call `link` with `FreePort(0)` and `FreePort(1)`. Neither is "removed" so `connect` is called, but `set_port` is a no-op for both FreePort sides. Verify no panic.

### E2: Self-referencing annihilation pattern (integration-level)
Build a minimal CON-CON pair where `a.1 <-> b.2` and `a.2 <-> b.1`, remove both agents, then call `link` with the saved PortRef values. Both links should be no-ops. Verify the net has no live agents and no stale port entries for the removed agents.
