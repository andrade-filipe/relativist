# TASK-0469: [SPEC-19 amendment A10] Extend §3.2 `BorderGraph` contract — recycle-protection under delta mode

**Spec:** SPEC-22 §3.8 A10 (closes SC-005).
**Requirements:** A10 (formal SPEC-19 §3.2 R8-R12 amendment); enables SPEC-22 R10b, R10c.
**Priority:** P0 (blocker for TASK-0482 BorderGraph slot-id stability).
**Status:** TODO
**Depends on:** TASK-0460 (I3').
**Blocked by:** none
**Estimated complexity:** S (~40 LoC SPEC-19 next-revision diff; no production code)
**Bundle:** SPEC-22 Arena Management — predecessor-spec amendment cluster (Phase A).
**Tag:** `[SPEC-19 amendment]`

## Context

SPEC-19 §3.2 R8-R12 (BorderGraph contract) currently stores `BorderState` entries indexed by `border_id`, each carrying `side_a: PortRef`, `side_b: PortRef`, `worker_a: WorkerId`, `worker_b: WorkerId`, and a derived `is_redex: bool`. No interaction with worker-side ID recycling is specified. The amendment adds the constraint: under `GridConfig.delta_mode == true`, free-list recycling must preserve BorderGraph slot-id stability via two compliant strategies (R10b):

- **Strategy A (`RecyclePolicy::DisableUnderDelta`, default):** workers MUST NOT pop from the free-list during a delta-mode round; `create_agent` falls back to `next_id` allocation; the free-list still accumulates pushes from `remove_agent` and is drained at the next clean partition boundary.
- **Strategy B (`RecyclePolicy::BorderClean`):** workers MAY pop from the free-list only for IDs not present in `border_entries` (partition-local HashSet shadow per SPEC-04 R20-R22). If popped ID is border-referenced, re-push it (or stash in side-list) and allocate fresh.

Border-referenced IDs become protected tombstones (R10c) and are NOT recycled until the next `reconstruct` clean boundary.

## Acceptance Criteria

- [ ] ESPECIALISTA EM SPECS lands the SPEC-19 next-revision diff amending §3.2 R8-R12 (or adding a new R12a) with the SPEC-22 §3.8 A10 *New text* verbatim.
- [ ] §3.2 explicitly enumerates Strategy A (`DisableUnderDelta`) and Strategy B (`BorderClean`).
- [ ] §3.2 specifies that `RecyclePolicy::DisableUnderDelta` is the conservative default.
- [ ] §3.2 specifies the protected-tombstone semantics (R10c) — slot stays `None`, ports `DISCONNECTED`, ID NOT in `free_list`, until next `reconstruct`.
- [ ] §3.2 cross-references SPEC-22 R10b, R10c and the `GridConfig.recycle_under_delta: RecyclePolicy` field.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `codigo/relativist/specs/SPEC-19-delta-protocol.md` | modify (by ESPECIALISTA EM SPECS only) | §3.2 R8-R12 amended per SPEC-22 §3.8 A10. |

## Test Expectations (forward-ref)

TEST-SPEC-0469 — covered by T9a (Strategy A protected tombstone) and T9b (Strategy B border-clean), both in SPEC-22 §7.1 (TASK-0482).

## Invariants Touched

- D2 (Border completeness) — protected by the recycle restriction.
- D3 (Cross-round border discovery) — same.
- G1 (under delta mode) — protected by R10b, conditional on ARG-005 for delta-optimized strategies.
- I3' stability subclause (border-referenced IDs preserved across rounds via protected tombstones).

## Notes

- The threat model that R10b prevents (verbatim from SPEC-22 R10b closing paragraph): round N produces border `B = (border_id, AgentPort(47, 0), AgentPort(123, 0))`; in round N+1 worker recycles ID 47 to a different `Symbol`; coordinator dispatches a `CommutationBatch` indexing `AgentPort(47, 0)`; worker's local `agents[47]` now resolves to a different rule than the BorderGraph computed → G1 violation. R10b prevents the recycle in step 2.
- The `GridConfig.recycle_under_delta` field is added to `GridConfig` (which lives in SPEC-05 with extensions per SPEC-19/SPEC-20/SPEC-22). This is a small SPEC-05 / SPEC-19 surface extension — coordinate with TASK-0415 (SPEC-20 GridConfig owner).

## DAG Links

- **Predecessors:** TASK-0460.
- **Successors:** TASK-0482 (RecyclePolicy + is_border_protected wiring).
