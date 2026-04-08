# TEST-SPEC-0146: Add #[instrument] to reduction reduce_all()

**Task:** TASK-0146
**Spec:** SPEC-11
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Existing reduction tests still pass

**Input:** Run all existing reduction module tests
**Expected:** All pass -- no behavioral change
**Verifies:** Annotation-only change

### T2: reduce_all() function has #[instrument] attribute

**Input:** Code review: `src/reduction/*.rs` reduce_all() function
**Expected:** Has `#[tracing::instrument]` with `skip(net)` and fields `partition_index`, `initial_redexes`, `level = "info"`
**Verifies:** R6 -- required instrumented function

### T3: Span is not created at default WARN level

**Input:** Run reduce_all with default filter (`relativist::reduction=warn`)
**Expected:** No span entry in output (filtered out because span level is INFO)
**Verifies:** Hot-path optimization -- zero overhead at default level

---

## Edge Cases

### E1: Span created when filter lowered to info

**Verify:** With `RUST_LOG=relativist::reduction=info`, the span IS created with `partition_index` and `initial_redexes` fields.
**Why:** Operators need the ability to enable diagnostic spans.

### E2: Net argument excluded from span

**Verify:** `skip(net)` is present in the `#[instrument]` attribute.
**Why:** The `Net` struct is large and must not be serialized into tracing spans.
