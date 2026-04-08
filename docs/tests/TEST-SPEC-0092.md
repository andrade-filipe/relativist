# TEST-SPEC-0092: Implement run_coordinator (distributed grid loop)

**Task:** TASK-0092
**Spec:** SPEC-06
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Complete distributed reduction with 2 workers

**Type:** Integration test (async)
**Input:**
```
// Build a simple net (e.g., CON-CON annihilation pair)
// Spawn 2 worker tasks that connect, receive partitions, reduce, and return results
let (final_net, metrics) = run_coordinator(net, &node_config, &grid_config, &strategy).await.unwrap();
```
**Expected:** `final_net` is in normal form (empty redex queue); result matches sequential `reduce_all` on the same net
**Verifies:** R16, R19, R20 -- distributed grid loop produces correct result

### T2: Net already in normal form produces 0 rounds

**Type:** Integration test (async)
**Input:** A net with no redexes (already in normal form)
**Expected:** `metrics.rounds == 0`; Shutdown sent to all workers immediately after accepting them
**Verifies:** Loop termination on normal form detection

### T3: Round limit reached exits with converged = false

**Type:** Integration test (async)
**Input:** A net that does not reach normal form within 1 round; `grid_config.max_rounds = Some(1)`
**Expected:** `metrics.rounds == 1`; `metrics.converged == false`
**Verifies:** R20 -- round limit terminates the loop

### T4: GridMetrics populated with network metrics

**Type:** Integration test (async)
**Input:** Run 2 rounds with 2 workers
**Expected:**
- `metrics.bytes_sent_per_round.len() == 2`
- `metrics.bytes_received_per_round.len() == 2`
- `metrics.network_send_time_per_round.len() == 2`
- `metrics.network_recv_time_per_round.len() == 2`
- All values are > 0
**Verifies:** R33, R34, R35 -- per-round network metrics are collected

### T5: Connection loss aborts the loop

**Type:** Integration test (async)
**Input:** Worker disconnects mid-round (after receiving partition but before sending result)
**Expected:** `Err(ProtocolError::ConnectionLost(_))` or `Err(ProtocolError::Timeout { .. })`
**Verifies:** R25 -- connection loss is fatal (no retry)

---

## Edge Cases

### E1: Single worker (degenerate distribution)

**Verify:** `run_coordinator` with 1 worker completes correctly. The single worker receives the entire net as one partition and returns it.
**Why:** Tests that distribution works with the minimum viable worker count.

### E2: Shutdown is always sent regardless of error

**Verify:** Even if the loop exits due to an error, `Shutdown` messages are sent to all connected workers (best-effort).
**Why:** Workers should not be left hanging after coordinator failure.
