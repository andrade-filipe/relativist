# TASK-0497: SPEC-03 reduction-engine assertion audit — reformulate as I3'-compatible (R27a — consumer of A6)

**Spec:** SPEC-22 §3.3 R27a; §3.8 A6 (consumes SPEC-03 §4.3 amendment).
**Requirements:** R27a (rule implementations MUST NOT assume monotonicity of returned IDs across multiple `create_agent` calls in a single rule fire. Allowed: `debug_assert!(self.agents[new_id as usize].is_some())` (uniqueness), `debug_assert!(self.next_id > new_id)` (upper-bound). Forbidden: `assert!(new_id > old_max_id)`, `assert!(new_id == self.next_id - 1)`, monotonicity claims. `assert_next_id_valid` (SPEC-02) is preserved — its check is consistent with I3').
**Priority:** P0 (audit blocker; required for free-list × CON-DUP correctness).
**Status:** TODO
**Depends on:** TASK-0465 (SPEC-03 amendment), TASK-0472 (create_agent recycle path).
**Blocked by:** none
**Estimated complexity:** M (~80 LoC audit + ~120 LoC tests including T7a)
**Bundle:** SPEC-22 Arena Management — Phase E (invariant amendments).

## Context

The amendment (TASK-0465) authors the assertion allowlist/denylist in SPEC-03 §4.3. This task is the IMPLEMENTATION-SIDE AUDIT: scan `relativist-core/src/reduction/` for any `assert!(new_id > old_max_id)`-style monotonicity claims and replace with the allowed patterns. CON-DUP commutation is the load-bearing case (creates 4 agents per fire; under partial free-list, 2 may be recycled IDs smaller than `next_id` and 2 may be fresh IDs larger).

## Acceptance Criteria

- [ ] Audit every file in `relativist-core/src/reduction/**/*.rs` for assertion patterns matching the forbidden list:
  - `assert!(new_id > old_max_id)` and variants.
  - `assert!(new_id == self.next_id - 1)`.
  - `debug_assert!(new_id > self.next_id - 1)`.
  - Any other monotonicity claim across multiple `create_agent` calls.
- [ ] For each forbidden pattern found, replace with one of the allowed patterns:
  - `debug_assert!(self.agents[new_id as usize].is_some())` (uniqueness post-create).
  - `debug_assert!(self.next_id > new_id)` (upper-bound check).
- [ ] Preserve `assert_next_id_valid` (SPEC-02 §4.5) — its `(i as u32) < self.next_id` for `slot.is_some()` is I3'-compatible (free-list IDs are in `None` slots, so they don't trip the assertion).
- [ ] Special attention to `interact_comm` (CON-DUP, ~TASK-0026) which calls `create_agent` 4 times per fire — verify no inter-call monotonicity claim.
- [ ] Test T7a (SPEC-22 §7.1): pre-populate a net with a CON-DUP redex AND 2 IDs in the free-list. Reduce the redex once. Assert the 4 new agents satisfy I3' (uniqueness — `agents[id].is_some()` for all 4 returned IDs and no duplicates), but DO NOT assert monotonicity. Run with `cargo test --release` AND `cargo test` (debug); both must pass.
- [ ] Document the audit results in a comment at the top of each modified file: "// SPEC-22 R27a audit: <count> forbidden patterns replaced; allowed patterns retained."

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/reduction/anni.rs` | audit + modify if needed | Replace forbidden assertion patterns. |
| `relativist-core/src/reduction/comm.rs` | audit + modify if needed | Same. **Load-bearing for CON-DUP.** |
| `relativist-core/src/reduction/eras.rs` | audit + modify if needed | Same. |
| `relativist-core/src/reduction/void.rs` | audit + modify if needed | Same. |
| `relativist-core/src/reduction/step.rs` | audit + modify if needed | Same. |

## Key Types / Signatures

(No type/signature changes; pure assertion-pattern audit and replacement.)

## Test Expectations (forward-ref)

TEST-SPEC-0497:
- T7a (SPEC-22 §7.1): CON-DUP under partial free-list — release + debug both pass.
- `audit_no_forbidden_assertion_patterns_in_reduction` — meta-test: grep `src/reduction/` at test time and assert no forbidden patterns. (Optional.)

## Invariants Touched

- I3' (consumed; assertion language now consistent).
- T5 (CON-DUP topology — preserved; assertions only change, not the rule).

## Notes

- This is a Stage 3 DEVELOPER-style audit task, but task-splitter records it as an atomic task because SPEC-22 §3.8 A6 explicitly delegates the audit responsibility.
- If no forbidden patterns are found in the audit (i.e., SPEC-03 implementation never used monotonicity claims to begin with), the task still ships with the T7a test as evidence of audit completion.

## DAG Links

- **Predecessors:** TASK-0465, TASK-0472.
- **Successors:** TASK-0500 (regression).
