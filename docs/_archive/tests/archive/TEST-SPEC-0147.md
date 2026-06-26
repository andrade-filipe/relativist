# TEST-SPEC-0147: Add #[instrument] to merge merge()

**Task:** TASK-0147
**Spec:** SPEC-11
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Existing merge tests still pass

**Input:** Run all existing merge module tests
**Expected:** All pass -- no behavioral change
**Verifies:** Annotation-only change

### T2: merge() function has #[instrument] attribute

**Input:** Code review: `src/merge/*.rs` merge() function
**Expected:** Has `#[tracing::instrument]` with `skip(partitions)` and fields `partition_count`, `border_redexes`
**Verifies:** R6 -- required instrumented function

### T3: border_redexes uses deferred recording

**Input:** Code review: the `border_redexes` field uses `tracing::field::Empty` and is later recorded via `tracing::Span::current().record("border_redexes", value)`
**Expected:** Field is filled after merge logic computes the border redex count
**Verifies:** R6 -- deferred field recording pattern

---

## Edge Cases

### E1: Partition data excluded from span

**Verify:** `skip(partitions)` is present in the `#[instrument]` attribute.
**Why:** Partition data is large and must not be serialized.

### E2: Span level is info

**Verify:** The `#[instrument]` has `level = "info"`, so the span is only created when the merge module target is at INFO or more verbose.
**Why:** Merge is not as hot as reduction but should still be filterable.
