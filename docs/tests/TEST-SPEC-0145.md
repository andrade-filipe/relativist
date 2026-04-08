# TEST-SPEC-0145: Add #[instrument] to partition split()

**Task:** TASK-0145
**Spec:** SPEC-11
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Existing partition tests still pass

**Input:** Run all existing partition module tests
**Expected:** All pass -- no behavioral change
**Verifies:** Annotation-only change

### T2: split() function has #[instrument] attribute

**Input:** Code review: `src/partition/*.rs` split() function
**Expected:** Has `#[tracing::instrument]` with `skip(net, strategy)` and fields `input_agents`, `k`, `strategy`
**Verifies:** R6 -- required instrumented function

### T3: Span fields are populated (manual verification)

**Input:** Run split with `RUST_LOG=relativist::partition=info`; inspect log output
**Expected:** Span entry shows `input_agents=<N>`, `k=<K>`, `strategy=<name>`
**Verifies:** R6, R8 -- structured span fields

---

## Edge Cases

### E1: Large Net argument is not serialized into span

**Verify:** The `#[instrument]` attribute uses `skip(net, strategy)` to exclude the `Net` and strategy objects.
**Why:** Serializing a large `Net` into a span would be expensive and noisy.

### E2: Span level is filterable

**Verify:** With default log filter (`relativist::partition=info`), the span is created. With `relativist::partition=error`, the span is NOT created.
**Why:** Hot-path spans must be controllable via filter level.
