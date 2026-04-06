# TEST-SPEC-0023: Implement interact_void (ERA-ERA rule)

**Task:** TASK-0023
**Spec:** SPEC-03 Section 4.1.3 (ERA-ERA Void)
**Generated:** 2026-04-06

---

## Unit Tests

### T1: Two ERA agents connected at principal ports -- both removed after interact_void
Create two ERA agents, connect their principal ports (forming an active pair),
call `interact_void`. Verify `net.get_agent(a)` and `net.get_agent(b)` are both `None`.

### T2: Net agent count decreases by exactly 2
Create a net with 2 ERA agents (and optionally other agents for context),
call `interact_void` on the ERA pair. Verify `count_live_agents()` dropped by 2.

### T3: Ports of removed agents are DISCONNECTED
After `interact_void`, verify that the port array slots for both agents' principal
ports contain `DISCONNECTED`. Since ERA has arity 0, only port 0 matters, but
the 3-slot layout means slots 1 and 2 should also be `DISCONNECTED`.

### T4: Stale redex left in queue after removal
Connect two ERA agents (which pushes a redex to the queue), then call
`interact_void`. Verify the redex is still in the queue (interact_void does
NOT drain the queue). Verify `is_valid_redex(a, b)` returns `false` (stale).

## Edge Cases

### E1: Other agents in the net are unaffected
Create a net with 2 ERA agents and 1 CON agent. Call `interact_void` on the
ERA pair. Verify the CON agent is still live and its ports are unchanged.
