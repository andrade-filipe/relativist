# TEST-SPEC-0135: Implement idle connection timeout

**Task:** TASK-0135
**Spec:** SPEC-10
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Future that completes immediately returns result

**Input:** `with_idle_timeout(Duration::from_secs(5), async { Ok::<_, SecurityError>(42) }).await`
**Expected:** `Ok(42)`
**Verifies:** T6 -- no false timeouts on fast operations

### T2: Future that exceeds timeout returns ConnectionTimeout

**Input:** `with_idle_timeout(Duration::from_millis(10), tokio::time::sleep(Duration::from_secs(60))).await` (use `tokio::time::pause()` in tests)
**Expected:** `Err(SecurityError::ConnectionTimeout)`
**Verifies:** T6 -- timeout fires on slow operations

### T3: Zero duration timeout fires immediately

**Input:** `with_idle_timeout(Duration::ZERO, async { tokio::task::yield_now().await; Ok::<_, SecurityError>(()) }).await`
**Expected:** `Err(SecurityError::ConnectionTimeout)` (timeout fires before yield returns)
**Verifies:** Edge case for zero-duration timeout

---

## Edge Cases

### E1: ConnectionTimeout error variant exists

**Verify:** `SecurityError::ConnectionTimeout` variant exists and displays `"connection idle timeout"`.
**Why:** The variant must be added to SecurityError for this task.

### E2: Timeout resets per message

**Verify:** When `with_idle_timeout` is called per-recv in a loop, each call independently measures from the moment it starts, not from the previous call.
**Why:** The idle timeout applies per-message-wait, not to the connection lifetime.
