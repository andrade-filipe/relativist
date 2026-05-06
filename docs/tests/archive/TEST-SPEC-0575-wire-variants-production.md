# TEST-SPEC-0575: `RequestWork` / `NoMoreWork` wire variants production (serde + framing + cross-cite to TEST-SPEC-0576)

**SPEC-21 §7 ID:** plumbing only (production-side closure of TEST-SPEC-0511 amendment-level coverage; transitively gates T11/T12/T13/T14).
**Owning task:** TASK-0575.
**Parent spec:** SPEC-21 §3.6 R31 (the two new `Message` enum variants for pull dispatch); §3.8 A2 (consumer of TASK-0511); §3.7 R37c (PROTOCOL_VERSION sequencing — coordinated with TASK-0576).
**Type:** unit + integration (bincode round-trip + framed-read + cross-version rejection).
**Theory anchor:** None direct (wire-format hygiene). DISC-008 (sync/termination dimension).

---

## Inputs / Fixtures

- The post-TASK-0575 live `Message` enum with the two new variants appended at the END (per SPEC-06 R5 discriminant-stability).
- The post-TASK-0576 live `PROTOCOL_VERSION` constant (whatever its bumped value at landing time per landing-order-aware contract; see TEST-SPEC-0576 / TEST-SPEC-0511 / TEST-SPEC-0476).
- Constructed `Message::RequestWork { worker_id: WorkerId(7) }` and `Message::NoMoreWork` instances.
- The SPEC-18 framed reader/writer (length-prefixed, bincode-encoded).
- A pre-bump deserializer pinned to `PREVIOUS_LIVE_VERSION` (test-only constant).

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0575-01 | `request_work_bincode_round_trip` | `Message::RequestWork { worker_id: WorkerId(7) }` | `bincode::serialize(&msg)` then `bincode::deserialize::<Message>(&bytes)` (live version) | `Ok(decoded)`; `decoded == original` (`PartialEq`). |
| UT-0575-02 | `no_more_work_bincode_round_trip` | `Message::NoMoreWork` | same | `Ok(decoded)`; structurally equal. |
| UT-0575-03 | `framed_read_request_work_then_no_more_work` | a buffer containing length-prefixed encoded `RequestWork`, then `NoMoreWork`, then `RequestWork` | iterate `framed_reader.next_frame()` three times | each frame decodes independently; per-frame variant matches sequence. (Wire framing UNCHANGED — joint with SPEC-18 R-N.) |
| UT-0575-04 | `pre_bump_deserializer_rejects_request_work` | encoded bytes of `Message::RequestWork { worker_id: WorkerId(0) }` (live version) | decode using deserializer pinned to `PREVIOUS_LIVE_VERSION` | `Err(ProtocolError::UnsupportedVersion { sender_version: PROTOCOL_VERSION, receiver_version: PREVIOUS_LIVE_VERSION })`. (Cross-cuts TEST-SPEC-0576 coverage; landing-order-aware integers per defensive contract.) |
| UT-0575-05 | `pre_bump_deserializer_rejects_no_more_work` | encoded `Message::NoMoreWork` | same | same `UnsupportedVersion` error. |
| UT-0575-06 | `existing_message_variants_unaffected_by_append` | every pre-SPEC-21 `Message::*` variant (`AssignPartition`, `PartitionResult`, `Hello`, etc.) | encode + decode round-trip with the live constant | all succeed; the append-only amendment did not break sibling variants. |
| UT-0575-07 | `discriminant_stability_request_work_at_end` | the live `Message` enum definition source | grep variant ordering | `RequestWork` and `NoMoreWork` appear at the END of the enum definition (SPEC-06 R5; TASK-0575 acceptance criterion line 27). |
| UT-0575-08 | `request_work_derive_partial_eq` | two `Message::RequestWork { worker_id: WorkerId(7) }` instances | `==` comparison | `true`; `Debug, Clone, PartialEq, Eq` derives present (compile-time enforced; UT does the runtime sanity check). |
| UT-0575-09 | `no_more_work_serde_payload_size_minimal` | `Message::NoMoreWork` (unit variant) | encode | resulting buffer size is at most `4 bytes` (variant discriminant + length prefix; bincode-specific). Sanity gate against accidental field additions. |
| UT-0575-10 | `worker_id_u32_max_round_trip` | `Message::RequestWork { worker_id: WorkerId(u32::MAX) }` | round-trip | `Ok(decoded)`; `decoded.worker_id.0 == u32::MAX` (no integer-bounds clamp). |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | A `v_old` peer sends an existing variant (`AssignPartition`) to a live receiver | succeeds (existing variants are encoded the same; the version bump rejects only forward-incompatible payloads). Cross-cite TEST-SPEC-0511 EC-3 + SPEC-21 §3.8 A2 tolerance policy. |
| EC-2 | A future v_new+1 peer sends `Message::RequestWork` to current code | `UnsupportedVersion` (forward-compat negative gate; mirrors TEST-SPEC-0476 EC-2). |
| EC-3 | Concatenated stream `[RequestWork(0), NoMoreWork, RequestWork(1)]` decoded with live version | all three frames decode; per-frame independence (UT-0575-03 expanded). |
| EC-4 | Truncated buffer (length prefix says 100 bytes, only 50 present) | framed reader returns `Err(ProtocolError::Eof)` or equivalent — NOT silent corruption. Mirrors TEST-SPEC-0511 UT-0511-06. |
| EC-5 | Variant tag corruption (random byte in the discriminant slot) | decoder returns `Err(ProtocolError::UnknownVariantTag)` or equivalent; never silently misinterprets. |

## Invariants asserted

- R31 (Message enum gains `RequestWork`, `NoMoreWork`).
- §3.8 A2 (SPEC-06 amendment with append-only discriminant-stability).
- SPEC-06 R5 (discriminant stability — preserved by appending at end).
- SPEC-18 R-N (length-prefixed framing UNCHANGED).
- Wire compatibility (controlled break, paired with TASK-0576 PROTOCOL_VERSION bump; cross-cuts TEST-SPEC-0576 + TEST-SPEC-0511).

## ARG/DISC/REF citation

- DISC-008 v2 (sync/termination dimension — wire-version handshake is the sync side; pull-protocol message catalog is the termination side).

## Determinism notes

Bincode 1.x serde tests are deterministic by construction. Framed-reader tests (UT-0575-03) MAY exercise tokio if the SPEC-18 framing is async; in that case use `#[tokio::test(flavor = "current_thread")]` with deterministic single-runtime executor. Otherwise plain `#[test]` is sufficient.

UT-0575-04 / UT-0575-05 rely on the `PREVIOUS_LIVE_VERSION` test-only constant established by TEST-SPEC-0511 / TEST-SPEC-0476 / TEST-SPEC-0576. **NEVER hardcode integer version values.** The reference values for `sender_version` / `receiver_version` are resolved at test-build time from the live constant and its predecessor.

UT-0575-07 (discriminant stability grep gate) MUST anchor on the variant names `RequestWork` and `NoMoreWork` AND their position relative to the previous last variant (capture the last pre-SPEC-21 variant name as a snapshot at test-build time). Defends against accidental enum reordering across landings.

## Cross-test dependencies

- **TEST-SPEC-0511 (SPEC-06 amendment-level coverage)** — predecessor; this task is the production-side closure for the wire variants.
- **TEST-SPEC-0576 (PROTOCOL_VERSION bump production)** — sibling; UT-0575-04 / UT-0575-05 share the `PREVIOUS_LIVE_VERSION` capture pattern. The `UnsupportedVersion` error variant is the cross-cut.
- **TEST-SPEC-0476 (SPEC-22 PROTOCOL_VERSION precedent)** — defensive landing-order-aware contract source.
- **TEST-SPEC-T11-pull-based-dispatch-protocol** — consumer; pull-dispatch protocol exercises both variants end-to-end. This plumbing file is the round-trip primitive.
- **TEST-SPEC-T12-pull-vs-push-equivalence** — consumer; push mode emits ZERO `NoMoreWork` per R37e (verified at the FSM layer in TEST-SPEC-0577).
- **TEST-SPEC-0577 / TEST-SPEC-0578 (coordinator/worker FSMs)** — consumers; FSMs emit and consume these variants.
- **TEST-SPEC-0589 / TEST-SPEC-0590 (SPEC-22 R10b strategy wiring under streaming)** — consumers; the wire variants drive the chunked-dispatch state changes that gate free-list recycling.
