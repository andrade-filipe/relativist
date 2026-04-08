# TEST-SPEC-0148: Add #[instrument] to coordinator dispatch() and protocol handle_message()

**Task:** TASK-0148
**Spec:** SPEC-11
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Existing protocol tests still pass

**Input:** Run all existing protocol module tests
**Expected:** All pass -- no behavioral change
**Verifies:** Annotation-only change

### T2: dispatch() has #[instrument] attribute

**Input:** Code review: dispatch() function
**Expected:** Has `#[tracing::instrument]` with `skip(partition_data, stream)` and fields `worker_id`, `partition_index`, `size_bytes`
**Verifies:** R6 -- required instrumented function

### T3: handle_message() has #[instrument] attribute

**Input:** Code review: handle_message() function
**Expected:** Has `#[tracing::instrument]` with `skip(payload)` and fields `message_type`, `peer`
**Verifies:** R6 -- required instrumented function

### T4: Both spans use debug level

**Input:** With default filter (`relativist::protocol=warn`), neither span appears
**Expected:** No spans at default level
**Verifies:** Hot-path spans are filtered out in production

---

## Edge Cases

### E1: Spans appear at debug level

**Verify:** With `RUST_LOG=relativist::protocol=debug`, both `dispatch` and `handle_message` spans appear with their structured fields.
**Why:** Operators need protocol-level diagnostics when debugging.

### E2: Large payloads excluded from spans

**Verify:** `skip(partition_data)` and `skip(payload)` exclude raw bytes from span data.
**Why:** Wire protocol payloads can be megabytes; serializing them into spans would be catastrophic.
