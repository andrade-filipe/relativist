# TASK-0515: [SPEC-22 amendment A6] R10b/R10c protected-tombstone discipline broadens to streaming

**Spec:** SPEC-21 §3.8 A6 (closes SC-007); SPEC-21 §3.7 R37b.
**Requirements:** A6 (formal SPEC-22 R10b/R10c amendment — trigger-condition broadening from `delta_mode` to `(delta_mode || streaming_active) && id ∈ border_referenced_set`).
**Priority:** P0 (blocker for TASK-0589 Strategy A wiring and TASK-0590 Strategy B wiring under streaming).
**Status:** TODO
**Depends on:** TASK-0469 (SPEC-19 §3.2 amendment landed in SPEC-22 wave), TASK-0482 (SPEC-22 RecyclePolicy enum production landed).
**Blocked by:** none
**Estimated complexity:** S (~25 LoC SPEC-22 next-revision diff; spec text only).
**Bundle:** SPEC-21 Streaming Generation — predecessor-spec amendment cluster (Phase A).
**Tag:** `[SPEC-22 amendment]`

## Context

SPEC-22 R10b currently scopes the protected-tombstone discipline to `delta_mode == true`. SPEC-21 streaming pipeline maintains a coordinator-side `border_map: HashMap<u32, (PortRef, PortRef)>` and a pending-connection store that both reference live `AgentId`s — these are subject to the same recycle-vs-border-identity hazard that R10b was designed for.

**Threat model under streaming alone (no delta).** If a worker's free-list pops a slot ID that is referenced by an active border in the coordinator's `border_map` (or by a pending connection), and the slot is reassigned to a new agent before chunk N+1's `AssignPartition`, the border-target identity becomes ambiguous and `merge` (SPEC-05) can wire two distinct logical agents together. This is the same G1 violation pattern as R10b's delta-mode threat, just under a different active-tracking surface.

SPEC-21 §3.8 A6 amends SPEC-22 R10b to broaden the trigger condition:

> Protected-tombstone discipline applies when `(delta_mode || streaming_active) && id ∈ border_referenced_set`.

The two normative strategies (`RecyclePolicy::DisableUnderDelta` / `RecyclePolicy::BorderClean`) are renamed conceptually to `DisableUnderBorderTracking` / `BorderClean` in implementation **but the wire-level enum name `RecyclePolicy::DisableUnderDelta` is preserved for backward compatibility** (the field name is misleading post-SPEC-21 but stable; same precedent as SPEC-22 SC-013 / DISC-012 stale-tag handling).

**Alternative one-liner closure (per R37b).** An implementation MAY use the cargo feature gate `streaming-no-recycle` that disables the worker free-list outright during streaming. This satisfies R37b trivially without requiring SPEC-22 amendments. SPEC-22 §3.8 A6 documents this as the "valid one-liner closure" path.

## Acceptance Criteria

- [ ] ESPECIALISTA EM SPECS lands the SPEC-22 next-revision diff amending §3.1 R10b *Old text* / *New text* per SPEC-21 §3.8 A6.
- [ ] R10b trigger condition broadened from `delta_mode == true` to `(delta_mode || streaming_active) && id ∈ border_referenced_set`.
- [ ] R10b explicitly preserves the wire-level enum name `RecyclePolicy::DisableUnderDelta` (backward-compat, even though the meaning broadens).
- [ ] R10b documents the alternative `streaming-no-recycle` cargo feature gate as a valid one-liner closure.
- [ ] R10c protected-tombstone semantics extend identically to streaming-active context.
- [ ] Cross-references SPEC-21 R37b, §3.8 A6.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `codigo/relativist/specs/SPEC-22-arena-management.md` | modify (by ESPECIALISTA EM SPECS only) | §3.1 R10b / R10c amended per SPEC-21 §3.8 A6. |

## Test Expectations (forward-ref)

TEST-SPEC-0515 — covered by:
- T9a / T9b SPEC-22 (Strategy A / Strategy B under delta) — PRECEDENT, unaffected.
- TEST-SPEC-0589 streaming-active Strategy A.
- TEST-SPEC-0590 streaming-active Strategy B.
- TEST-SPEC-0591 streaming-no-recycle cargo gate equivalence.

## Invariants Touched

- G1 — preserved by broadened recycle protection (same mechanism as R10b under delta).
- D2 / D3 (border completeness, cross-round border discovery) — preserved.

## Notes

- This is a spec-text-only task (no production code).
- The strategy-name ambiguity (`DisableUnderDelta` field name now triggers also under streaming) is documented inline; renaming would break wire-format and is rejected.
- The streaming-active flag itself is set by the worker dispatch loop entering the chunked-dispatch phase; production wiring is in TASK-0589 / TASK-0590.
- Consumed by TASK-0589 (Strategy A wiring), TASK-0590 (Strategy B wiring), TASK-0591 (streaming-no-recycle alternative).

## DAG Links

- **Predecessors:** TASK-0469 (SPEC-19 BorderGraph contract baseline), TASK-0482 (RecyclePolicy enum exists).
- **Successors:** TASK-0589, TASK-0590, TASK-0591.
