# TEST-SPEC-0068: Implement drain_stale_redexes function

**Task:** TASK-0068
**Spec:** SPEC-05 (R27)
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Empty queue remains empty
Create a `Net` with an empty `redex_queue`. Call `drain_stale_redexes(&mut net)`. Assert `redex_queue.is_empty()`.

### T2: All valid redexes retained
Create a `Net` with 2 CON agents (id=0, id=1) connected at principal ports (`AgentPort(0, 0) <-> AgentPort(1, 0)`). Push `(0, 1)` to the redex queue. Call `drain_stale_redexes`. Assert queue still contains `(0, 1)` and length is 1.

### T3: All stale redexes removed (agents consumed)
Create a `Net`. Push `(5, 6)` to the redex queue where agents 5 and 6 do not exist (`agents[5]` and `agents[6]` are `None`). Call `drain_stale_redexes`. Assert queue is empty.

### T4: Mixed valid and stale, order preserved
Create a `Net` with agents 0, 1, 2, 3. Wire `AgentPort(0, 0) <-> AgentPort(1, 0)` and `AgentPort(2, 0) <-> AgentPort(3, 0)`. Push `(0, 1)`, `(99, 100)` (stale - nonexistent agents), `(2, 3)` to queue in that order. Call `drain_stale_redexes`. Assert queue is `[(0, 1), (2, 3)]` in that order.

### T5: Stale due to rewired principal port
Create agents 0 and 1 with `AgentPort(0, 0) <-> AgentPort(1, 0)`. Push `(0, 1)`. Then rewire `AgentPort(0, 0)` to point to `AgentPort(2, 0)` instead. Call `drain_stale_redexes`. Assert the entry `(0, 1)` is removed because `get_target(AgentPort(0, 0)) != AgentPort(1, 0)`.

## Edge Cases

### E1: Duplicate entries in queue
Push `(0, 1)` twice into the queue where both agents exist and are connected. Call `drain_stale_redexes`. Assert both entries are retained (drain does not deduplicate; it only checks validity).

### E2: Single-agent stale (one agent exists, other does not)
Create agent 0 but not agent 7. Push `(0, 7)` to queue. Call `drain_stale_redexes`. Assert queue is empty.

### E3: Large queue with all stale entries
Push 1000 entries `(i, i+1000)` where none of the referenced agents exist. Call `drain_stale_redexes`. Assert queue is empty. Verifies O(Q) complexity does not cause issues.
