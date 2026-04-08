# TEST-SPEC-0154: Add axum dependency and scaffold metrics_router

**Task:** TASK-0154
**Spec:** SPEC-11
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: metrics_router returns a Router without panic

**Input:** `metrics_router(Arc::new(Registry::default()), Arc::new(AtomicBool::new(false)))` (with `--features metrics`)
**Expected:** Returns an `axum::Router` instance
**Verifies:** Router construction works

### T2: AppState holds registry and readiness flag

**Input:** `AppState { registry: Arc::new(Registry::default()), is_ready: Arc::new(AtomicBool::new(false)) }`
**Expected:** Compiles and both fields are accessible
**Verifies:** R22a -- AtomicBool readiness flag

### T3: OPENMETRICS_CONTENT_TYPE constant value

**Input:** `OPENMETRICS_CONTENT_TYPE`
**Expected:** `"application/openmetrics-text; version=1.0.0; charset=utf-8"`
**Verifies:** R21 -- correct Content-Type

### T4: Build with metrics feature compiles

**Input:** `cargo check --features metrics`
**Expected:** Compilation succeeds
**Verifies:** axum and router code compile

### T5: Build without metrics feature excludes HTTP code

**Input:** `cargo check` without metrics feature
**Expected:** No HTTP/axum code compiled
**Verifies:** T9 partial -- feature gate

---

## Edge Cases

### E1: Router defines exactly 3 routes

**Verify:** Router has routes for `/metrics`, `/health`, `/ready`.
**Why:** R19, R20, R21, R22 -- three required endpoints.

### E2: Readiness flag uses AtomicBool, not AtomicU8

**Verify:** `AppState.is_ready` is `Arc<AtomicBool>`, not `Arc<AtomicU8>`.
**Why:** R22a -- AtomicBool is robust against enum reordering.
