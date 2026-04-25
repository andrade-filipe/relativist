# TASK-0464: [SPEC-02 amendment A5] Extend R12 — `remove_agent` pushes free-list, purges `freeport_redirects`

**Spec:** SPEC-22 §3.8 A5 (closes SC-001 second surface and SC-002 R12 amendment scope).
**Requirements:** A5 (formal SPEC-02 R12 amendment); enables SPEC-22 R2, R6, R10c.
**Priority:** P0 (blocker for TASK-0473 `remove_agent` implementation).
**Status:** TODO
**Depends on:** TASK-0460 (I3'), TASK-0461 (R2 reuse).
**Blocked by:** none
**Estimated complexity:** S (~25 LoC SPEC-02 next-revision diff; no production code)
**Bundle:** SPEC-22 Arena Management — predecessor-spec amendment cluster (Phase A).
**Tag:** `[SPEC-02 amendment]`

## Context

SPEC-02 R12 currently states: "MUST mark the agent's slot as `None`, disconnect all its ports from the port array, and NOT reuse the ID." The amendment lifts the no-reuse clause and threads two additional steps: (a) purge any `freeport_redirects` entry keyed by the agent's ID (closes SC-001 second surface — stale redirect would reference a different agent after recycle); (b) push the ID onto `free_list` UNLESS the ID is a protected tombstone per R10c (border-referenced under delta mode).

## Acceptance Criteria

- [ ] ESPECIALISTA EM SPECS lands the SPEC-02 next-revision diff replacing R12 with the SPEC-22 §3.8 A5 *New text* verbatim.
- [ ] R12 explicitly threads the `freeport_redirects` purge step (closes SC-001 second surface).
- [ ] R12 includes the protected-tombstone exception clause (defers to SPEC-22 R10c).
- [ ] R12 cross-references SPEC-22 R2 (push), R6 (no duplicates), R10c (protected tombstones).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `codigo/relativist/specs/SPEC-02-net-representation.md` | modify (by ESPECIALISTA EM SPECS only) | R12 text replacement per SPEC-22 §3.8 A5. |

## Test Expectations (forward-ref)

TEST-SPEC-0464 — covered transitively by T1, T3, T4 (port slot reinitialization), T9a (BorderGraph protected tombstone).

## Invariants Touched

- D1c (FreePort bijectivity — protected by the `freeport_redirects` purge clause).
- I3' (uniqueness preserved via R6 no-duplicates).

## Notes

- Originally declared in SPEC-22 frontmatter (Round 1); now formalized with verbatim target-spec text per Round 2 §3.8 A5.
- The `is_border_protected(id)` predicate is wired in TASK-0482; until that wiring lands, the predicate is a no-op (always `false`) — the recycle path always pushes in non-distributed contexts.

## DAG Links

- **Predecessors:** TASK-0460, TASK-0461.
- **Successors:** TASK-0473 (remove_agent).
