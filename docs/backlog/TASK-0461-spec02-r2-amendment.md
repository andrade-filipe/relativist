# TASK-0461: [SPEC-02 amendment A2] Relax R2 — `AgentId` reuse via free-list with explicit clearing protocol

**Spec:** SPEC-22 §3.8 A2 (closes SC-002 part 1).
**Requirements:** A2 (formal SPEC-02 R2 amendment); enables SPEC-22 R1-R10c.
**Priority:** P0 (blocker for free-list core implementation TASK-0471/0472/0473).
**Status:** TODO
**Depends on:** TASK-0460 (I3 → I3' must land first; A2 cites I3').
**Blocked by:** none
**Estimated complexity:** S (~25 LoC SPEC-02 next-revision diff; no production code)
**Bundle:** SPEC-22 Arena Management — predecessor-spec amendment cluster (Phase A).
**Tag:** `[SPEC-02 amendment]`

## Context

SPEC-02 R2 (verbatim line 37) currently states: "The `AgentId` type MUST be `u32`, monotonically increasing, never reused within an execution (cf. SPEC-01, I3). **(MUST)**". SPEC-22's free-list mechanism reuses IDs after the slot is fully cleared. The amendment lifts the "never reused" clause and substitutes a uniqueness condition with an explicit clearing protocol (port slots `DISCONNECTED`, `agents[id] == None`, `freeport_redirects` entry purged) before reuse, plus a protected-tombstone exception (R10c).

## Acceptance Criteria

- [ ] ESPECIALISTA EM SPECS lands the SPEC-02 next-revision diff replacing R2 (line 37) with the SPEC-22 §3.8 A2 *New text* verbatim.
- [ ] R2's cross-reference updated from `SPEC-01, I3` to `SPEC-01, I3'`.
- [ ] R2 cites SPEC-22 R1-R10c as the reuse mechanism.
- [ ] R2 explicitly enumerates the clearing protocol triple: `(a)` ports `DISCONNECTED`, `agents[id] == None`, `freeport_redirects` entry purged; `(b)` not a protected tombstone.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `codigo/relativist/specs/SPEC-02-net-representation.md` | modify (by ESPECIALISTA EM SPECS only) | R2 text replacement per SPEC-22 §3.8 A2. |

## Test Expectations (forward-ref)

TEST-SPEC-0461 — no direct unit test; A2 enables the existing T1/T3/T4 test family in SPEC-22 §7.1 (covered by TASK-0473 implementation).

## Invariants Touched

- I3' (consumed; R2 now defers to I3' uniqueness).
- D1c (FreePort bijectivity — preserved by the `freeport_redirects` purge clause).

## Notes

- Pure spec-text amendment; no code in this task.
- R2's old "never reused" clause was a *sufficiency* condition for I3 monotonicity. Under I3', it becomes redundant — uniqueness is sufficient and the free-list provides it.

## DAG Links

- **Predecessors:** TASK-0460 (I3').
- **Successors:** TASK-0472 (create_agent with free-list pop), TASK-0473 (remove_agent with free-list push).
