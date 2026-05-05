# TEST-SPEC-0155: Implement /health and /ready endpoints

**Task:** TASK-0155
**Spec:** SPEC-11 R21, R22, R22a
**Generated:** 2026-04-08 (retroactive)

---

## Unit Tests

### T1: GET /health returns HTTP 200

**Type:** Integration (HTTP handler)
**Input:** Send `GET /health` to the axum router
**Expected:** Response status is `200 OK`
**Verifies:** R21 -- liveness probe always returns 200 if process is alive

### T2: GET /health returns body "ok"

**Type:** Integration (HTTP handler)
**Input:** Send `GET /health` to the axum router, read response body as UTF-8
**Expected:** Body text is `"ok"`
**Verifies:** R21 -- body content is the literal string "ok"

### T3: GET /health Content-Type is text/plain

**Type:** Integration (HTTP handler)
**Input:** Send `GET /health`, inspect `Content-Type` header
**Expected:** Header value starts with `text/plain`
**Verifies:** R21 -- Content-Type for health endpoint is text/plain

### T4: GET /ready returns HTTP 503 when is_ready is false

**Type:** Integration (HTTP handler)
**Input:** Construct `AppState` with `is_ready = Arc::new(AtomicBool::new(false))`, send `GET /ready`
**Expected:** Response status is `503 Service Unavailable`
**Verifies:** R22, R22a -- Init state maps to not-ready

### T5: GET /ready returns HTTP 200 when is_ready is true

**Type:** Integration (HTTP handler)
**Input:** Construct `AppState` with `is_ready = Arc::new(AtomicBool::new(true))`, send `GET /ready`
**Expected:** Response status is `200 OK`
**Verifies:** R22, R22a -- WaitingForWorkers or later maps to ready

### T6: GET /ready transitions from 503 to 200 on flag change

**Type:** Integration (HTTP handler)
**Input:**
1. Create router with `is_ready = false`
2. Send `GET /ready` -- expect 503
3. Set `is_ready.store(true, Ordering::Relaxed)`
4. Rebuild router or use same state, send `GET /ready` again
**Expected:** First response is 503, second is 200
**Verifies:** R22a -- readiness tracks the AtomicBool flag in real time

### T7: GET /ready transitions from 200 back to 503 on Error state

**Type:** Integration (HTTP handler)
**Input:**
1. Create router with `is_ready = true`
2. Send `GET /ready` -- expect 200
3. Set `is_ready.store(false, Ordering::Relaxed)`
4. Send `GET /ready` again
**Expected:** First response is 200, second is 503
**Verifies:** R22, R22a -- Error state sets is_ready back to false, producing 503

---

## Edge Cases

### E1: health_handler is independent of AppState

**Verify:** `health_handler()` takes no `State` parameter and returns 200 regardless of registry or readiness flag state.
**Why:** The liveness probe must not depend on any application state -- if the process can serve HTTP, it is alive.

### E2: ready_handler uses Ordering::Relaxed

**Verify:** The `is_ready.load()` call uses `Ordering::Relaxed` (not `Acquire` or `SeqCst`).
**Why:** Per TASK-0155 notes, exact consistency is not required for a readiness probe. A brief delay is acceptable.

### E3: Concurrent readiness flag mutations do not cause panics

**Verify:** Spawn multiple tasks that rapidly toggle `is_ready` between true and false while simultaneously sending `GET /ready` requests. No panics or undefined behavior occurs.
**Why:** The `AtomicBool` must be safe under concurrent access, which Rust guarantees, but the test documents the expectation.
