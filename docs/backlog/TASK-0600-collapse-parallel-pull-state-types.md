# TASK-0600 — Collapse parallel `Pull*` / `PullCoordinatorState` representations (QA-D010-013)

**Phase:** B-4b (D-011 hardening — second of three sub-tasks)
**Bundle:** D-011 — Tier 3 Hardening + Bench Enablement
**Status:** PENDING
**Priority:** P2 (MEDIUM — code-quality / maintenance debt)
**Spec:** SPEC-13 §x (FSM state catalog); SPEC-21 §3.8 A5 (pull-only FSM extensions).
**Origin:** QA-D010-013 — parallel state representations `Pull*` and `PullCoordinatorState` exist after D-010 landing; cause confusion and risk inconsistency.
**Estimated complexity:** M (~100 LoC production + ~50 LoC tests — refactor + test churn)
**Estimated stages duration:** Stages 2→3→4→5→6 over ~0.5 to 0.75 day.

---

## Context

D-010 introduced two co-existing pull-mode state types: a per-FSM `Pull*` enum and a coordinator-aggregate `PullCoordinatorState`. They overlap in semantics (both encode "pull dispatch is active") but are tracked independently — risk of drift is real, and reviewers (per QA-D010-013) flagged the confusion.

The fix is to **collapse** them into a single canonical representation. Strategy:
- Identify the single source of truth (likely the per-FSM enum, since it's authoritative per worker / coordinator instance).
- Replace usages of the redundant aggregate with derived computation (function or `From` impl).
- Maintain backward-compat at API boundaries (re-export the collapsed type with the old name as a `pub use` alias if downstream call sites are widespread).

## Dependencies

- None on D-011 amendments.
- Parallel-OK with B-4a/B-4c.

## Files in scope

| File | Change |
|------|--------|
| `relativist-core/src/protocol/coordinator.rs` (FSM area) | Identify and remove the redundant state representation. |
| `relativist-core/src/protocol/worker.rs` (FSM area, if mirrored) | Same. |
| `relativist-core/src/protocol/messages.rs` (if state types appear in messages) | Update if needed. |
| `relativist-core/tests/` — any FSM tests citing both types | Update to reference the canonical type. |

## Files explicitly OUT of scope

- The 5 coordinator + 2 worker pull-only states defined in SPEC-21 §3.8 A5 — those are the catalog and stay.
- Any change to the message protocol wire format.

## Acceptance criteria

1. Exactly **one** type encodes the per-FSM pull state; the other is removed (or kept as a `pub use` re-export alias for compat).
2. No place in `protocol/{coordinator,worker}.rs` reads BOTH types in the same control-flow path.
3. SPEC-13 / SPEC-21 §3.8 A5 state-set is preserved (no states added or removed — only the *type-level representation* is collapsed).
4. All existing FSM tests pass; if test files referenced the removed type, they're updated to the canonical one.
5. Lint clean.

## Test floor delta expected

**+0 to +2 tests** (mostly refactor; possibly +1 sanity test on the collapsed type's exhaustiveness).

## Notes

- Light QA scope — the structural risk here is "I removed the wrong one and now coordinator-aggregate state is opaque". Mitigation: explicit Stage 5 review of the chosen direction before refactor.
- Coordinate with anyone touching the FSM (none expected within D-011 itself).
