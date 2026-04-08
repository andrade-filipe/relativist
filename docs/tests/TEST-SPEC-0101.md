# TEST-SPEC-0101: Initialize tracing subscriber in main

**Task:** TASK-0101
**Spec:** SPEC-07, SPEC-13
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: init_tracing function exists and is public

**Type:** Compilation test
**Input:** `use relativist::observability::init_tracing; init_tracing();`
**Expected:** Compiles successfully
**Verifies:** Function is public and callable

### T2: Default log level is info when RUST_LOG is unset

**Type:** Manual/integration test
**Input:** Run the binary without RUST_LOG set; emit `tracing::debug!("test")` and `tracing::info!("test")`
**Expected:** Info-level messages appear in output; debug-level messages do NOT appear
**Verifies:** R35 -- default filter is `info`

### T3: RUST_LOG overrides default level

**Type:** Manual/integration test
**Input:** Run with `RUST_LOG=debug`; emit `tracing::debug!("test")`
**Expected:** Debug-level messages appear in output
**Verifies:** R35 -- EnvFilter reads RUST_LOG

### T4: Invalid RUST_LOG value does not panic

**Type:** Integration test
**Input:** Set `RUST_LOG="!@#$%"` and call `init_tracing()`
**Expected:** Function does not panic; falls back to `info` level
**Verifies:** Graceful fallback on invalid filter values

---

## Edge Cases

### E1: init_tracing called before Cli::parse

**Verify:** In `main.rs`, `init_tracing()` appears before `Cli::parse()`.
**How:** Source code inspection of `main.rs`.
**Why:** Ensures all CLI parsing errors are logged through the tracing infrastructure.

### E2: Log output includes timestamps and level

**Verify:** Default tracing output contains timestamp, log level (INFO/DEBUG/etc.), and message text.
**How:** Manual inspection of output format.
**Why:** R36 -- structured logging with timestamps for diagnostics.
