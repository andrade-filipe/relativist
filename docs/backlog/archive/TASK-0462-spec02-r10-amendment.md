# TASK-0462: [SPEC-02 amendment A3] Restate R10 — `next_id` increment by `f = k - r` (fresh allocations only)

**Spec:** SPEC-22 §3.8 A3 (closes SC-002 part 2).
**Requirements:** A3 (formal SPEC-02 R10 amendment); enables SPEC-22 R3 / R24 (I3').
**Priority:** P0 (blocker for create_agent implementation TASK-0472).
**Status:** TODO
**Depends on:** TASK-0460 (I3').
**Blocked by:** none
**Estimated complexity:** S (~20 LoC SPEC-02 next-revision diff; no production code)
**Bundle:** SPEC-22 Arena Management — predecessor-spec amendment cluster (Phase A).
**Tag:** `[SPEC-02 amendment]`

## Context

SPEC-02 R10 (verbatim line 58) currently states: "The field `next_id` MUST be strictly greater than any `AgentId` in use in the net (cf. SPEC-01, I3). After creating `k` agents, `next_id` MUST be incremented by `k`." SPEC-22 R3 explicitly forbids incrementing `next_id` on the recycle path. The amendment redefines the increment as `f = k - r` where `r` is the count of recycled IDs and `f` is the count of fresh allocations. When the free-list is empty, `r = 0` and the original `k` increment is preserved.

## Acceptance Criteria

- [ ] ESPECIALISTA EM SPECS lands the SPEC-02 next-revision diff replacing R10 (line 58) with the SPEC-22 §3.8 A3 *New text* verbatim.
- [ ] R10's "strictly greater than any `AgentId` in use" clause updated to "strictly greater than any `AgentId` ever assigned (live, in the free-list, or previously freed and re-assigned)".
- [ ] R10's cross-reference updated from `SPEC-01, I3` to `SPEC-01, I3'`.
- [ ] R10 explicitly defines `r` (recycled-from-free-list count) and `f = k - r` (fresh allocations).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `codigo/relativist/specs/SPEC-02-net-representation.md` | modify (by ESPECIALISTA EM SPECS only) | R10 text replacement per SPEC-22 §3.8 A3. |

## Test Expectations (forward-ref)

TEST-SPEC-0462 — covered transitively by:
- T6 (commutation recycling: `next_id == 4` after CON-DUP with 2 recycled IDs — TASK-0472).
- T7a (CON-DUP under partial free-list — TASK-0497).

## Invariants Touched

- I3' upper-bound semantics (preserved with new accounting).

## Notes

- The fresh-vs-recycle decomposition is the natural accounting under I3'.
- Implementation surface is in TASK-0472 (`create_agent` increments `next_id` only on the fresh-allocation path).

## DAG Links

- **Predecessors:** TASK-0460.
- **Successors:** TASK-0472 (create_agent), TASK-0497 (SPEC-03 assertion audit).
