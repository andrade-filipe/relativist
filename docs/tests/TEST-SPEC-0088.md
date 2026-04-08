# TEST-SPEC-0088: Implement coordinator worker-accept phase

**Task:** TASK-0088
**Spec:** SPEC-06
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Accept exactly N workers

**Type:** Integration test (async)
**Input:**
```
let config = NodeConfig { bind: "127.0.0.1:0", num_workers: 3, worker_connect_timeout: Duration::from_secs(5), ..Default::default() };
// Spawn 3 tokio tasks that connect to the listener
let (listener, streams) = accept_workers(&config).await.unwrap();
```
**Expected:** `streams.len() == 3`; each stream has a valid remote address
**Verifies:** R17 -- coordinator accepts exactly `num_workers` connections

### T2: Timeout with fewer workers returns error

**Type:** Integration test (async)
**Input:**
```
let config = NodeConfig { bind: "127.0.0.1:0", num_workers: 3, worker_connect_timeout: Duration::from_secs(1), ..Default::default() };
// Only connect 1 worker
let result = accept_workers(&config).await;
```
**Expected:** `result.is_err()` -- either `ProtocolError::Timeout` or a coordinator-level error
**Verifies:** R24 -- timeout after `worker_connect_timeout`

### T3: Worker connection logging

**Type:** Integration test (async)
**Input:** Connect 2 workers and capture tracing output
**Expected:** `tracing::info!` emitted for each connection with worker index and remote address
**Verifies:** Acceptance criteria: logs each worker connection

### T4: Listener returned alongside streams

**Type:** Integration test (async)
**Input:** Call `accept_workers` with 1 worker
**Expected:** Return type is `Ok((TcpListener, Vec<TcpStream>))` -- both are valid
**Verifies:** Listener lifetime matches the grid loop

---

## Edge Cases

### E1: Zero workers configured

**Verify:** `NodeConfig { num_workers: 0, .. }` either returns immediately with an empty Vec or returns an error.
**Why:** Edge case: no workers means no TCP connections needed, but may not be meaningful.

### E2: Workers connect in arbitrary order

**Verify:** Connect 3 workers with deliberate delays between them (e.g., 0ms, 50ms, 100ms). All 3 are accepted regardless of order.
**Why:** Workers may start at different times; the accept loop must not assume ordering.
