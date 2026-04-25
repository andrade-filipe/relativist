# TASK-0511: [SPEC-06 amendment A2] `Message` enum gains `RequestWork` / `NoMoreWork` + PROTOCOL_VERSION sequencing

**Spec:** SPEC-21 §3.8 A2 (closes SC-001 part 1); SPEC-21 §3.7 R37c (PROTOCOL_VERSION sequencing); SPEC-21 §3.1 R31.
**Requirements:** A2 (formal SPEC-06 amendment — Message enum + PROTOCOL_VERSION bump per defensive `PREVIOUS_LIVE_VERSION + 1` language).
**Priority:** P0 (wire-compat blocker; coordinates with SPEC-22 TASK-0476 and SPEC-20 TASK-0417).
**Status:** TODO
**Depends on:** none (Phase A predecessor amendment).
**Blocked by:** none
**Estimated complexity:** S (~30 LoC SPEC-06 next-revision diff; spec text only).
**Bundle:** SPEC-21 Streaming Generation — predecessor-spec amendment cluster (Phase A).
**Tag:** `[SPEC-06 amendment]`

## Context

SPEC-06 currently catalogs the `Message` enum variant set covering registration handshake, `AssignPartition`, `PartitionResult`, `Shutdown`, etc. (R1, R2, R5). SPEC-21 R31 introduces two new variants required by pull-based dispatch (R30-R32):

```rust
RequestWork { worker_id: WorkerId },
NoMoreWork,
```

`RequestWork` is sent by the worker to indicate readiness for a new chunk. `NoMoreWork` is sent by the coordinator when the generator stream is exhausted.

Both variants serialize through SPEC-18 wire-format-v2 serde without modification to the framing layer (length-prefixed, bincode-encoded). Per SPEC-21 R37c (closes SC-005), the `PROTOCOL_VERSION` constant MUST be bumped using **defensive `PREVIOUS_LIVE_VERSION + 1` language** (NOT a hardcoded absolute integer) so that merge-order reshuffling between SPEC-20 / SPEC-21 / SPEC-22 does not silently produce wrong absolute version numbers.

Pre-bump deserializers MUST reject post-bump payloads with `ProtocolError::UnsupportedVersion` (mirrors SPEC-22 R10b's rejection clause and the SPEC-20 R37 v3-vs-v4 pattern).

## Acceptance Criteria

- [ ] ESPECIALISTA EM SPECS lands the SPEC-06 next-revision diff amending the `Message` enum catalog with verbatim *Old text* and *New text* per SPEC-21 §3.8 A2.
- [ ] `Message` enum text includes `RequestWork { worker_id: WorkerId }` and `NoMoreWork` variants (appended at end per SPEC-06 R5 discriminant-stability rule).
- [ ] PROTOCOL_VERSION clause amended to mandate `PREVIOUS_LIVE_VERSION + 1` defensive language (not a hardcoded number).
- [ ] Pre-bump deserializers reject post-bump payloads with `UnsupportedVersion` — explicit clause added.
- [ ] Cross-references SPEC-21 R31, R37c, §3.8 A2 and SPEC-22 R9a / TASK-0476 (the SPEC-22 PROTOCOL_VERSION precedent that SPEC-21 R37c mirrors).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `codigo/relativist/specs/SPEC-06-wire-protocol.md` | modify (by ESPECIALISTA EM SPECS only) | `Message` enum catalog (R1/R2/R5 surface) amended per SPEC-21 §3.8 A2; PROTOCOL_VERSION clause amended per R37c. |

## Test Expectations (forward-ref)

TEST-SPEC-0511 — covered by:
- TEST-SPEC-0575 wire round-trip for new variants.
- TEST-SPEC-0576 PROTOCOL_VERSION mismatch rejection (defensive sequencing).
- T11 (pull-based dispatch protocol — TASK-0579).

## Invariants Touched

- Wire compatibility (controlled break, mirrors SPEC-22 R9a / SPEC-20 R37).

## Notes

- This is a spec-text-only task (no production code).
- The PROTOCOL_VERSION bump itself happens in TASK-0576 (production), which depends on TASK-0476 (SPEC-22 wire-version-bump precedent).
- The variants are mode-agnostic at the wire layer (per R37e) but mode-specific at the FSM layer (push mode MUST NOT emit them; per R37e); the enforcement of that scoping lives in TASK-0577 / TASK-0578 (FSM extensions) and TASK-0582 (push-mode termination scoping).
- Consumed by TASK-0575 (wire variants production), TASK-0576 (version-bump production), TASK-0579 (orchestration).

## DAG Links

- **Predecessors:** none.
- **Successors:** TASK-0575, TASK-0576, TASK-0579.
