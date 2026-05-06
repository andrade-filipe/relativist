# TASK-0463: [SPEC-02 amendment A4] Clarify R11 — "next available ID" subsumes free-list pop

**Spec:** SPEC-22 §3.8 A4 (closes SC-009 / R3-vs-R11 ambiguity flagged in Round 1 cross-spec audit).
**Requirements:** A4 (formal SPEC-02 R11 clarification); enables SPEC-22 R3, R4.
**Priority:** P0 (clarification — blocks TASK-0472 from carrying R11 contradiction).
**Status:** TODO
**Depends on:** TASK-0460 (I3'), TASK-0461 (R2 reuse), TASK-0462 (R10 increment).
**Blocked by:** none
**Estimated complexity:** S (~20 LoC SPEC-02 next-revision diff; no production code)
**Bundle:** SPEC-22 Arena Management — predecessor-spec amendment cluster (Phase A).
**Tag:** `[SPEC-02 amendment]`

## Context

SPEC-02 R11 currently defines `create_agent` as "MUST create a new agent with the next available ID, insert it into the agent arena (expanding if necessary), and return the assigned `AgentId`." Round 1's cross-spec audit flagged this as a PARTIAL CONTRADICTION with SPEC-22 R3 (which makes "next available" mean `free_list.pop()` first, then `next_id`). The amendment is a clarification (not a relaxation): it pins down "next available ID" as `free_list.pop()` if the free-list is non-empty AND the popped ID is not a protected tombstone (R10b/R10c), otherwise `next_id` (with `next_id` incremented). Arena expansion happens only on the fresh-allocation path.

## Acceptance Criteria

- [ ] ESPECIALISTA EM SPECS lands the SPEC-02 next-revision diff replacing R11 with the SPEC-22 §3.8 A4 *New text* verbatim.
- [ ] R11 explicitly subsumes both the free-list path (with R10b/R10c protected-tombstone check) and the `next_id` path under "next available ID".
- [ ] R11 specifies arena expansion happens *only* on the fresh-allocation path (free-list pop reuses an existing slot).
- [ ] Complexity statement preserved: O(1) amortized.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `codigo/relativist/specs/SPEC-02-net-representation.md` | modify (by ESPECIALISTA EM SPECS only) | R11 text replacement per SPEC-22 §3.8 A4. |

## Test Expectations (forward-ref)

TEST-SPEC-0463 — covered transitively by T1 (basic recycling), T3 (free-list exhaustion), and T6 (commutation recycling) — all owned by TASK-0472.

## Invariants Touched

- None (clarification of an existing operation; semantics already implied by R3+R10 amendments).

## Notes

- This amendment closes the R3-vs-R11 ambiguity flagged in the Round 1 cross-spec audit table.
- The `is_border_protected(id)` predicate referenced in the amendment is wired in TASK-0482.

## DAG Links

- **Predecessors:** TASK-0460, TASK-0461, TASK-0462.
- **Successors:** TASK-0472 (create_agent).
