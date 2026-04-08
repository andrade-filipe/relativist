# TEST-SPEC-0091: Implement coordinator shutdown protocol

**Task:** TASK-0091
**Spec:** SPEC-06
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: All workers receive Shutdown message

**Type:** Integration test (async)
**Input:**
```
// 3 mock workers connected via duplex or TCP
shutdown_workers(&mut worker_streams).await;
// Each mock worker reads one frame
```
**Expected:** Each mock worker receives `Message::Shutdown`
**Verifies:** R2 -- Shutdown variant sent to all workers

### T2: Disconnected worker does not abort shutdown of others

**Type:** Integration test (async)
**Input:**
```
// 3 workers connected; drop worker 1's stream before shutdown
shutdown_workers(&mut worker_streams).await;
```
**Expected:** Workers 0 and 2 still receive `Message::Shutdown`; no panic; error for worker 1 is logged but not propagated
**Verifies:** Best-effort shutdown: individual failures are tolerated

### T3: Function returns void (no Result)

**Type:** Compilation test
**Input:** `shutdown_workers(&mut streams).await;` -- no `.unwrap()` or `?` needed
**Expected:** Compiles without Result handling
**Verifies:** Shutdown is fire-and-forget; errors are non-critical

### T4: Shutdown logged at INFO level

**Type:** Integration test (async)
**Input:** Call `shutdown_workers` with 2 connected workers
**Expected:** `tracing::info!` emitted with "shutdown" in the message
**Verifies:** Observability for shutdown events

---

## Edge Cases

### E1: Empty worker list

**Verify:** `shutdown_workers(&mut vec![]).await` completes without error or panic.
**Why:** Edge case: no workers to shut down (e.g., immediate termination before any workers connected).

### E2: All workers already disconnected

**Verify:** If all streams are closed before calling `shutdown_workers`, the function completes normally with logged warnings.
**Why:** Graceful handling when all connections have already been lost.
