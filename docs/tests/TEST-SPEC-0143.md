# TEST-SPEC-0143: Implement default log filter string

**Task:** TASK-0143
**Spec:** SPEC-11
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: DEFAULT_LOG_FILTER contains all 8 per-target directives

**Input:** Check `DEFAULT_LOG_FILTER` string
**Expected:** Contains `"relativist::coordinator=info"`, `"relativist::worker=info"`, `"relativist::reduction=warn"`, `"relativist::protocol=warn"`, `"relativist::partition=info"`, `"relativist::net=warn"`, `"relativist::observability=info"`, `"relativist::security=info"`, and trailing `"warn"`
**Verifies:** R5 -- all per-target levels specified

### T2: build_env_filter returns EnvFilter when RUST_LOG is not set

**Input:** Ensure `RUST_LOG` is unset; call `build_env_filter()`
**Expected:** Returns an `EnvFilter` without panicking
**Verifies:** R5 -- default filter fallback

### T3: build_env_filter respects RUST_LOG when set

**Input:** Set `RUST_LOG="trace"` in env; call `build_env_filter()`
**Expected:** Returns an `EnvFilter` that enables trace-level events
**Verifies:** R4 -- RUST_LOG override

---

## Edge Cases

### E1: Invalid RUST_LOG falls back to default

**Input:** Set `RUST_LOG="[invalid-syntax"` in env; call `build_env_filter()`
**Expected:** Returns the default filter (does not panic)
**Why:** Graceful handling of invalid user input.

### E2: DEFAULT_LOG_FILTER ends with global default

**Verify:** The filter string ends with `"warn"` (or contains a global default level).
**Why:** Ensures any crate/module not explicitly listed defaults to WARN.
