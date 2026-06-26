# TASK-0465: [SPEC-03 amendment A6] Reformulate §4.3 debug-assertion language as I3'-compatible

**Spec:** SPEC-22 §3.8 A6 (closes SC-010); informs SPEC-22 R27a.
**Requirements:** A6 (formal SPEC-03 §4.3 amendment); enables SPEC-22 R27a (I3'-compatible assertions in 6 reduction rules).
**Priority:** P0 (blocker for TASK-0497 SPEC-03 assertion audit).
**Status:** TODO
**Depends on:** TASK-0460 (I3').
**Blocked by:** none
**Estimated complexity:** S (~30 LoC SPEC-03 next-revision diff; no production code)
**Bundle:** SPEC-22 Arena Management — predecessor-spec amendment cluster (Phase A).
**Tag:** `[SPEC-03 amendment]`

## Context

SPEC-03 §4.3 (rule implementations) MAY contain `debug_assert!(new_id > old_max_id)` or equivalent monotonicity assertions on `create_agent` return values. Under I3', monotonicity does not hold: CON-DUP commutation creates 4 agents per fire and may receive any mix of recycled (smaller) and fresh (larger) IDs. The amendment authors an allowlist/denylist of assertion patterns:

- **Allowed:** `debug_assert!(self.agents[new_id as usize].is_some())` (uniqueness check post-create), `debug_assert!(self.next_id > new_id)` (next_id upper-bound check).
- **Forbidden:** `assert!(new_id > old_max_id)`, `assert!(new_id == self.next_id - 1)`, or any other monotonicity claim.

SPEC-02 §4.5 / `assert_next_id_valid` is preserved — its `(i as u32) < self.next_id` check for `slot.is_some()` is consistent with I3' (free-list IDs are in `None` slots, so they don't trip the assertion).

## Acceptance Criteria

- [ ] ESPECIALISTA EM SPECS lands the SPEC-03 next-revision diff inserting (or amending) §4.3 with the SPEC-22 §3.8 A6 *New text* allowlist/denylist verbatim.
- [ ] §4.3 cross-references SPEC-22 R27a (the in-rule reformulation requirement).
- [ ] §4.3 explicitly preserves `assert_next_id_valid` semantics (no edit needed at that surface).
- [ ] CON-DUP commutation flagged as the load-bearing case (creates 4 agents; recycling makes the 4 IDs non-monotonic).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `codigo/relativist/specs/SPEC-03-reduction-engine.md` | modify (by ESPECIALISTA EM SPECS only) | §4.3 amended with assertion allowlist/denylist per SPEC-22 §3.8 A6. |

## Test Expectations (forward-ref)

TEST-SPEC-0465 — implementation audit in TASK-0497 validates that no forbidden assertion remains in `src/reduction/`. T7a (CON-DUP under partial free-list) covers the load-bearing case.

## Invariants Touched

- I3' (consumed; assertions now check uniqueness, not monotonicity).
- T5 (CON-DUP topology — preserved; the amendment is purely an assertion-language change).

## Notes

- Pure spec-text amendment; the implementation audit is in TASK-0497 (Stage 3 DEVELOPER scans `src/reduction/` for forbidden patterns).
- The amendment explicitly delegates the audit responsibility to Stage 3 DEVELOPER — this task is the spec-side handshake.

## DAG Links

- **Predecessors:** TASK-0460.
- **Successors:** TASK-0497 (SPEC-03 assertion audit in `src/reduction/`).
