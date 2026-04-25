# TASK-0575: `RequestWork` / `NoMoreWork` wire variants production (serialize, deserialize, framing)

**Spec:** SPEC-21 §3.6 R31 (the two new `Message` enum variants for pull dispatch); §3.8 A2 (consumer of TASK-0511); §3.7 R37c (PROTOCOL_VERSION sequencing — coordinated with TASK-0576).
**Requirements:** R31 (production wiring of the two new variants through SPEC-18 wire-format-v2 serde without modification to the framing layer).
**Priority:** P0 (blocker for TASK-0577 coordinator FSM, TASK-0578 worker FSM, TASK-0579 orchestration).
**Status:** TODO
**Depends on:** TASK-0511 (SPEC-06 amendment A2 landed), TASK-0476 (SPEC-22 PROTOCOL_VERSION precedent for the bump pattern).
**Blocked by:** TASK-0576 (PROTOCOL_VERSION bump production — landing-order dependency: variants land first as code, version bump lands as a coordinated patch).
**Estimated complexity:** S (~50 LoC enum variants on `Message` + bincode round-trip tests; ~80 LoC serde tests).
**Bundle:** SPEC-21 Streaming Generation — Phase F (regression / polish / late-binding).

## Context

Per SPEC-21 R31 (closes SC-001 part 1 jointly with TASK-0511), the `Message` enum (in `relativist-protocol/src/message.rs`) gains:

```rust
RequestWork { worker_id: WorkerId },
NoMoreWork,
```

`RequestWork` is sent by the worker to indicate readiness for a new chunk. `NoMoreWork` is sent by the coordinator when the generator stream is exhausted. Both variants serialize through SPEC-18 wire-format-v2 serde without modification to the framing layer (length-prefixed, bincode-encoded; SPEC-06 R5 discriminant-stability rule means they are appended at the end of the enum).

Per R37e (closes SC-013), in **push mode** no `NoMoreWork` is sent; the worker receives a single `AssignPartition` per SPEC-05 merge protocol. `NoMoreWork` is meaningful only in pull mode. Worker FSM (TASK-0578) and coordinator FSM (TASK-0577) MUST NOT cross-pollute the variant. **This task ships the wire layer; the FSM scoping is enforced in TASK-0577 / TASK-0578.**

## Acceptance Criteria

- [ ] `Message::RequestWork { worker_id: WorkerId }` and `Message::NoMoreWork` variants added at the END of the enum (SPEC-06 R5 discriminant-stability).
- [ ] Both variants derive `Debug, Clone, PartialEq, Eq, Serialize, Deserialize`.
- [ ] Bincode round-trip succeeds for each variant: `bincode::serialize_then_deserialize(Message::RequestWork { worker_id: WorkerId(7) }) == orig`.
- [ ] Wire framing (length-prefix per SPEC-18 R-N) UNCHANGED — verified by encoding both variants and re-decoding with the existing framed reader.
- [ ] `match Message` exhaustiveness checks across all consumers updated (or guarded by `#[non_exhaustive]` if already so).
- [ ] No FSM logic emitted from this task — pure wire layer.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-protocol/src/message.rs` | modify | Append `RequestWork` and `NoMoreWork` variants per SPEC-06 R5. |
| `relativist-protocol/tests/wire_v3_streaming_variants.rs` | create | Bincode round-trip + framing-compat tests for both variants. |

## Key Types / Signatures

```rust
// In Message enum, AT THE END (SPEC-06 R5 discriminant-stability):
RequestWork { worker_id: WorkerId },
NoMoreWork,
```

## Test Expectations (forward-ref)

TEST-SPEC-0511 cites this task as the production-side coverage. Tests at this layer:
- UT-0575-01: bincode round-trip `RequestWork`.
- UT-0575-02: bincode round-trip `NoMoreWork`.
- UT-0575-03: framed-read of a buffer containing both variants in sequence.
- UT-0575-04: pre-bump deserializer rejection (cross-cuts with TASK-0576).

## Invariants Touched

- Wire compatibility (controlled break, paired with TASK-0576 PROTOCOL_VERSION bump).
- SPEC-06 R5 discriminant stability — preserved by appending at end.

## Notes

- This task does NOT bump PROTOCOL_VERSION; that's TASK-0576. The variants land as code first; the PROTOCOL_VERSION patch is a separate atomic landing per the SPEC-22 R9a / TASK-0476 precedent.
- The `WorkerId` type is the existing newtype from `relativist-net` (no new types needed).
- Consumed by TASK-0577 (coordinator FSM emits `NoMoreWork`), TASK-0578 (worker FSM emits `RequestWork`), TASK-0589 / TASK-0590 (cross-spec wiring under SPEC-22 R10b strategies).

## DAG Links

- **Predecessors:** TASK-0511, TASK-0476 (precedent).
- **Successors:** TASK-0576, TASK-0577, TASK-0578.
