# TASK-0460: [SPEC-01 amendment A1] Relax I3 (Monotonicity) → I3' (Uniqueness of AgentIds)

**Spec:** SPEC-22 §3.8 A1 (consumed by SPEC-22 R24, R25, R27, R27a; informs Phase B/C/E tasks).
**Requirements:** A1 (formal SPEC-01 amendment); supports R24 (I3' statement) and R25 (D4 preservation under I3').
**Priority:** P0 (foundational; every free-list-aware task depends on the relaxed invariant).
**Status:** TODO
**Depends on:** none (operates on the SPEC-01 invariant statement only).
**Blocked by:** none
**Estimated complexity:** S (~30 LoC SPEC-01 next-revision diff; no production code)
**Bundle:** SPEC-22 Arena Management — predecessor-spec amendment cluster (Phase A).
**Tag:** `[SPEC-01 amendment]`

## Context

SPEC-01 I3 (Monotonicity of AgentIds) is the theoretical anchor for `next_id > max(live_id)`. SPEC-22's free-list mechanism reuses `AgentId` slots without renumbering; this is sufficient for uniqueness but breaks strict monotonicity of returned IDs. The amendment relaxes I3 → I3' (uniqueness, not monotonicity) while preserving the `next_id > any-ever-assigned-id` property as an upper bound. Border-referenced IDs are stabilized via protected tombstones (R10b/R10c).

## Acceptance Criteria

- [ ] ESPECIALISTA EM SPECS lands the SPEC-01 next-revision diff replacing I3 (lines 289-296) with the I3' text from SPEC-22 §3.8 A1 *New text*, verbatim.
- [ ] SPEC-01's invariant table (D-layer / I-layer cross-reference) updated to show I3 → I3' with a backward-pointer to SPEC-22 R24.
- [ ] Cross-reference to SPEC-22 R10b/R10c (protected tombstones) included in the I3' statement.
- [ ] No code change in this task — pure spec-text amendment.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `codigo/relativist/specs/SPEC-01-invariantes.md` | modify (by ESPECIALISTA EM SPECS only) | I3 → I3' text replacement; add SPEC-22 R24 backward-pointer. |

## Key Types / Signatures

(No code; spec-text amendment only.)

## Test Expectations (forward-ref for Stage 2 TEST-GENERATOR)

TEST-SPEC-0460 — no direct unit test; the I3' invariant is validated transitively by:
- T7 (post-recycling structural invariants — TASK-0495)
- T7a (CON-DUP under partial free-list — TASK-0497)
- T9 (per-partition ID range compliance — TASK-0480)

## Invariants Touched

- I3 → I3' (RELAXED — this is the amendment).
- D4 (preserved per R25 — partition-local free-list preserves cross-partition disjointness).

## Notes

- This task is the SPEC-01 maintainer touch-point. Coordinate with ESPECIALISTA EM SPECS to land the formal SPEC-01 next revision that includes A1.
- The amendment is recorded in SPEC-22 §3.8 A1 verbatim — copy-paste the *New text* clause directly into SPEC-01 and add the cross-reference.
- Stability of IDs during a `BorderGraph`-active round is preserved by protected tombstones (R10b/R10c), implemented in TASK-0482.

## DAG Links

- **Predecessors:** none.
- **Successors:** TASK-0495 (I3' debug assertions), TASK-0482 (BorderGraph protected tombstones), TASK-0497 (SPEC-03 assertion audit).
