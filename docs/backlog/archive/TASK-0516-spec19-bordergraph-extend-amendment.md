# TASK-0516: [SPEC-19 amendment A7] `BorderGraph::extend_with_chunk_borders` method addition

**Spec:** SPEC-21 §3.8 A7 (closes SC-017); SPEC-21 §3.7 R37f.
**Requirements:** A7 (formal SPEC-19 §3.2 amendment — new method on `BorderGraph`).
**Priority:** P0 (blocker for TASK-0588 call-site discipline production under delta+streaming).
**Status:** TODO
**Depends on:** none (Phase A).
**Blocked by:** none
**Estimated complexity:** S (~20 LoC SPEC-19 next-revision diff; spec text only).
**Bundle:** SPEC-21 Streaming Generation — predecessor-spec amendment cluster (Phase A).
**Tag:** `[SPEC-19 amendment]`

## Context

SPEC-19 §3.2 currently constructs `BorderGraph` once from the initial `PartitionPlan`'s `borders` map and provides no incremental-extension API. Under the conjunction `delta_mode && streaming_active` (per SPEC-21 R37f), the coordinator's `BorderGraph` becomes stale after chunk 1 because each subsequent chunk discovers new border wires that cross-link previously-dispatched chunks.

SPEC-21 §3.8 A7 adds a new method to `BorderGraph`:

```rust
pub fn extend_with_chunk_borders(
    &mut self,
    new_borders: &HashMap<u32, (PortRef, PortRef)>,
)
```

**Method semantics (per SPEC-21 §3.8 A7 *New text*):**
- Merges new border entries into the existing `BorderGraph`.
- MUST be called by the coordinator after each `install_connection` invocation that yields a border wire under `delta_mode && streaming_active`, BEFORE chunk N+1's `AssignPartition` is dispatched.
- Idempotent on previously-seen border IDs.
- No-op if `new_borders.is_empty()`.

**Ownership split (per SC-017 closure).** SPEC-19 owns the implementation; SPEC-21 owns the call-site discipline (production task TASK-0588).

## Acceptance Criteria

- [ ] ESPECIALISTA EM SPECS lands the SPEC-19 next-revision diff amending §3.2 with the new method signature and semantics per SPEC-21 §3.8 A7 *New text*.
- [ ] §3.2 text documents idempotency on previously-seen border IDs.
- [ ] §3.2 text documents no-op semantics on empty input.
- [ ] §3.2 text adds a "Call-site discipline" note pointing to SPEC-21 R37f for the *when-to-call* contract.
- [ ] Cross-references SPEC-21 R37f, §3.8 A7.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `codigo/relativist/specs/SPEC-19-delta-protocol.md` | modify (by ESPECIALISTA EM SPECS only) | §3.2 amended — adds `extend_with_chunk_borders` method signature and semantics. |

## Test Expectations (forward-ref)

TEST-SPEC-0516 — covered by:
- TEST-SPEC-0588 call-site discipline (R37f under delta+streaming).
- T6 / T7 (streaming + delta combined isomorphism).
- M5 milestone gate (`ep_con 100M coordinator-side`) — out of scope for SPEC-21 but enabled by this amendment.

## Invariants Touched

- D2 (Border completeness) — preserved across chunks via incremental BorderGraph extension.
- D3 (Cross-round border discovery) — extended to cross-chunk discovery.
- G1 (under delta + streaming) — preserved via call-site discipline (SC-017 closure).

## Notes

- This is a spec-text-only task (no production code).
- The actual `extend_with_chunk_borders` impl in `relativist-core/src/protocol/...` (or wherever BorderGraph lives) is NOT this task — it's owned by SPEC-19 and is a separate SPEC-19 follow-up. SPEC-21 only commits to the spec-text amendment and the call-site discipline.
- The call-site discipline production is TASK-0588.
- Consumed by TASK-0588 (call-site discipline production).

## DAG Links

- **Predecessors:** none.
- **Successors:** TASK-0588.
