# TEST-SPEC-0134: Implement connection limits

**Task:** TASK-0134
**Spec:** SPEC-10
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Limiter with max=2, acquire 2 succeeds

**Input:** `let l = ConnectionLimiter::new(2); l.try_acquire(); l.try_acquire();`
**Expected:** Both calls return `true`
**Verifies:** Under-limit acquisitions succeed

### T2: Limiter with max=2, third acquire fails

**Input:** `let l = ConnectionLimiter::new(2); l.try_acquire(); l.try_acquire(); l.try_acquire();`
**Expected:** First two return `true`, third returns `false`
**Verifies:** R31 -- limit enforcement

### T3: Release allows new acquisition

**Input:** `let l = ConnectionLimiter::new(2); l.try_acquire(); l.try_acquire(); l.release(); l.try_acquire();`
**Expected:** The last `try_acquire()` returns `true`
**Verifies:** Release frees a slot

### T4: Initial active count is 0

**Input:** `ConnectionLimiter::new(10).active()`
**Expected:** `0`
**Verifies:** Clean initial state

### T5: Concurrent acquire from multiple threads respects limit

**Input:** Spawn 100 threads, each calling `try_acquire()` on a limiter with `max=50`
**Expected:** Exactly 50 return `true`, 50 return `false`
**Verifies:** Thread-safe CAS loop correctness

---

## Edge Cases

### E1: Limiter with max=0

**Input:** `ConnectionLimiter::new(0).try_acquire()`
**Expected:** Returns `false` -- zero capacity means no connections allowed
**Why:** Edge case for misconfiguration; should not panic.

### E2: Release without prior acquire

**Input:** `let l = ConnectionLimiter::new(10); l.release();`
**Expected:** `active()` wraps to `usize::MAX` (underflow) or is handled gracefully
**Why:** Defensive programming -- release should be paired with acquire.
