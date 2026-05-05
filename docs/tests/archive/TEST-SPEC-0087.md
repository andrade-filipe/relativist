# TEST-SPEC-0087: Implement connect_with_retry (exponential backoff)

**Task:** TASK-0087
**Spec:** SPEC-06
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Successful connection on first attempt

**Type:** Integration test (async)
**Input:**
```
let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
let addr = listener.local_addr().unwrap();
let config = NodeConfig { bind: addr, ..Default::default() };
let stream = connect_with_retry(&config).await;
```
**Expected:** `stream.is_ok()` -- connection succeeds immediately
**Verifies:** R18 -- worker connects to coordinator

### T2: All retries exhausted returns ConnectionLost

**Type:** Integration test (async)
**Input:**
```
let config = NodeConfig { bind: "127.0.0.1:1".parse().unwrap(), ..Default::default() };
let result = connect_with_retry(&config).await;
```
(Port 1 is unlikely to have a listener)
**Expected:** `result.is_err()` and `matches!(result.unwrap_err(), ProtocolError::ConnectionLost(_))`
**Verifies:** R23 -- returns error after 10 failed attempts

### T3: Backoff delay sequence is correct

**Type:** Unit test
**Input:** Extract or test the delay calculation logic: `min(2^attempt, 16)` for attempt 0..9
**Expected:** Sequence: 1, 2, 4, 8, 16, 16, 16, 16, 16, 16 (seconds)
**Verifies:** Exponential backoff capped at 16s with 10 attempts

---

## Edge Cases

### E1: Connection succeeds after initial failures

**Verify:** Start a listener AFTER 2 seconds; `connect_with_retry` should succeed on attempt 2 or 3.
**How:** Spawn a delayed task that starts listening, then call `connect_with_retry`.
**Why:** Tests the retry loop's ability to connect once the server becomes available.

### E2: tracing::warn is emitted for each failed attempt

**Verify:** Each failed connection attempt logs at `warn` level with attempt number and delay.
**How:** Use `tracing-test` or check log output for patterns like "attempt 1/10" and "retry in 1s".
**Why:** Diagnostics for connection failures in production.
