# TEST-SPEC-0106: Implement print_summary function

**Task:** TASK-0106
**Spec:** SPEC-07
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: count_agents_by_symbol returns correct counts

**Type:** Unit test
**Input:**
```
let mut net = Net::new();
// Add 3 CON agents, 2 DUP agents, 1 ERA agent
```
**Expected:** `count_agents_by_symbol(&net, Symbol::Con) == 3`, `count_agents_by_symbol(&net, Symbol::Dup) == 2`, `count_agents_by_symbol(&net, Symbol::Era) == 1`
**Verifies:** Agent counting helper works correctly

### T2: print_summary does not panic for empty net

**Type:** Unit test
**Input:**
```
let net = Net::new();
let metrics = GridMetrics::default();
print_summary(&net, &metrics);
```
**Expected:** Function completes without panic; outputs "Converged: false", "Rounds: 0", "Final agents: 0"
**Verifies:** Graceful handling of empty/default inputs

### T3: Summary includes convergence and round count

**Type:** Integration test (capture stdout)
**Input:**
```
let metrics = GridMetrics { converged: true, rounds: 5, total_interactions: 42, total_time: Duration::from_millis(1234), .. };
print_summary(&net, &metrics);
```
**Expected:** Output contains "Converged: true" (or "yes"), "Rounds: 5", "Total interactions: 42", "Total time: 1.234s"
**Verifies:** R15 -- human-readable summary with key fields

### T4: Network overhead shown only for distributed mode

**Type:** Integration test (capture stdout)
**Input:** Metrics with non-empty `bytes_sent_per_round = vec![1024]`
**Expected:** Output contains "Bytes sent:", "Bytes received:", "Network overhead:"
**Verifies:** Conditional sections for distributed mode

---

## Edge Cases

### E1: Net with all agents deleted (tombstones)

**Verify:** `count_agents_by_symbol` on a net where all agents are `None` returns 0 for all symbols.
**Why:** After full reduction, all agents may be consumed.

### E2: print_summary uses println!, not tracing

**Verify:** Output goes to stdout via `println!`, not through the tracing subscriber.
**How:** Code review; output appears even when `RUST_LOG=off`.
**Why:** This is user-facing output, not diagnostic logging.
