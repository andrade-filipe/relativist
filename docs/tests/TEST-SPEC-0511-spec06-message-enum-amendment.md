# TEST-SPEC-0511: SPEC-06 Message enum amendment (RequestWork / NoMoreWork variants + PROTOCOL_VERSION bump)

**SPEC-21 §7 ID:** plumbing only (transitively gates T11/T12/T13/T14).
**Owning task:** TASK-0511.
**Parent spec:** SPEC-21 §3.6 R31; SPEC-21 §3.7 R37c; SPEC-21 §3.8 A2; SC-001 (part 1) closure.
**Type:** unit + integration (wire round-trip).
**Theory anchor:** None direct (wire-format hygiene). DISC-008 (sync/termination dimension).

---

## Inputs / Fixtures

- The post-bump live `PROTOCOL_VERSION` constant (whatever its bumped value at landing time).
- A test-only `PREVIOUS_LIVE_VERSION` constant captured at TASK-0511 implementation time as the value the constant held immediately before this bump.
- Constructed `Message::RequestWork { worker_id: WorkerId(0) }` and `Message::NoMoreWork {}` instances.
- A v_old emulator deserializer pinned to `PREVIOUS_LIVE_VERSION`.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0511-01 | `protocol_version_strictly_one_above_predecessor` | the live `PROTOCOL_VERSION` at test-build time | `assert_eq!(PROTOCOL_VERSION, PREVIOUS_LIVE_VERSION + 1);` | passes. **CRITICAL:** the assertion MUST NOT be `assert_eq!(PROTOCOL_VERSION, 4);` or any hard-coded integer — this is the third spec in the wave to bump the constant (after SPEC-22 and SPEC-20); landing order is not fixed. See Determinism notes. |
| UT-0511-02 | `request_work_variant_serde_round_trip` | a `Message::RequestWork { worker_id: WorkerId(7) }` | encode then decode with the live `PROTOCOL_VERSION` deserializer | `Ok(decoded)`; `decoded == original` (`PartialEq`). |
| UT-0511-03 | `no_more_work_variant_serde_round_trip` | a `Message::NoMoreWork {}` | encode then decode with the live deserializer | `Ok(decoded)`; structurally equal to the original. |
| UT-0511-04 | `pre_spec21_binary_rejects_request_work` | the encoded bytes of `Message::RequestWork { worker_id: WorkerId(0) }` (live version) | decode using a deserializer pinned to `PREVIOUS_LIVE_VERSION` | `Err(ProtocolError::UnsupportedVersion { sender_version: PROTOCOL_VERSION, receiver_version: PREVIOUS_LIVE_VERSION })`. |
| UT-0511-05 | `pre_spec21_binary_rejects_no_more_work` | encoded `Message::NoMoreWork {}` | same as UT-0511-04 | same `UnsupportedVersion` error variant. |
| UT-0511-06 | `error_variant_is_unsupported_version_not_silent_corruption` | UT-0511-04 / UT-0511-05 | match the error | the variant is `ProtocolError::UnsupportedVersion { .. }` AND NOT `LengthMismatch`, `Eof`, `IoError`, `UnknownVariantTag`, or any silent-corruption pathway. (R37c rejection clause.) |
| UT-0511-07 | `existing_message_variants_unaffected` | every other `Message::*` variant existing pre-SPEC-21 | encode + decode round-trip with the live constant | all succeed; bump did not break sibling variants. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | `RequestWork` payload with `WorkerId(u32::MAX)` | round-trip OK (no integer-bounds clamp; wire encoding preserves full range). |
| EC-2 | Concatenated stream `[RequestWork, NoMoreWork, RequestWork]` | each frame decodes independently; per-frame version check fires once per frame, not once per stream. |
| EC-3 | A `v_old` peer sends `Message::AssignPartition` (an existing variant) to a live receiver | succeeds (existing variants are encoded the same). The version bump rejects only forward-incompatible payloads; sibling variants from the previous version are tolerated when the encoded variant tag was already known. (CONDITIONAL on SPEC-21 §3.8 A2 chosen tolerance policy — UT-0511-08 enforces documentation match.) |
| EC-4 | Future v_new+1 peer sends `Message::RequestWork` to current code | `UnsupportedVersion` (forward-compat negative gate). |

## Invariants asserted

- R31 (Message enum gains `RequestWork`, `NoMoreWork`).
- R37c (PROTOCOL_VERSION sequencing + rejection clause; closes SC-005 / SC-009-style).
- §3.8 A2 (SPEC-06 amendment with explicit version-bump rationale).

## ARG/DISC/REF citation

- DISC-008 v2 (sync/termination dimension — wire-version handshake is the sync side of the dimension).

## Determinism notes

**LANDING-ORDER-AWARE CONTRACT (CRITICAL — same pattern as TEST-SPEC-0476):** SPEC-21 R31 is the third spec in the wave to bump `PROTOCOL_VERSION`:

- SPEC-22 R9a: 2 → 3.
- SPEC-20: 3 → 4.
- SPEC-21 R31: 4 → 5 (assuming both predecessors land first).

**Different landing orders produce different absolute integers.** The TEST-SPEC and the implementing test code MUST NOT pin the exact integer in any assertion. Concretely:

- **FORBIDDEN:** `assert_eq!(PROTOCOL_VERSION, 5);` — couples the test to landing order.
- **REQUIRED:** `assert_eq!(PROTOCOL_VERSION, PREVIOUS_LIVE_VERSION + 1);` where `PREVIOUS_LIVE_VERSION` is captured from the codebase at test-build time as a snapshot taken immediately before TASK-0511's bump lands. The capture mechanism is identical to TEST-SPEC-0476's pattern (test-only constant, build-script substitution, or Rustdoc-anchored grep gate).

UT-0511-04 / UT-0511-05 reference values for `sender_version` / `receiver_version` are **resolved at test-build time** from the live constant and its predecessor — NEVER hardcoded as integers.

This contract is non-negotiable; any deviation is a CI-blocking finding. Code review MUST verify no integer-equal assertions on `PROTOCOL_VERSION` in this test or any sibling test that touches the constant.

The wire-handshake variant exercises the SPEC-18 framing path; if framing is async (`tokio` codec), use `#[tokio::test(flavor = "current_thread")]` with a deterministic single-runtime executor; otherwise plain `#[test]` is sufficient.

## Cross-test dependencies

- TEST-SPEC-0476 (SPEC-22 PROTOCOL_VERSION bump) — sibling defensive-language gate. Share the `PREVIOUS_LIVE_VERSION` capture pattern.
- TEST-SPEC-EG-U15a / EG-U15b (SPEC-20 protocol-version-mismatch) — share the `UnsupportedVersion` error variant and the defensive-language pattern.
- T11/T12/T13/T14 (SPEC-21 §7.5 pull-dispatch tests) — consume `RequestWork` / `NoMoreWork` round-trip semantics established here.
- TEST-SPEC-0575 / TEST-SPEC-0576 (forward-referenced from TASK-0511 but NOT in scope for the current Stage 2 wave; TASKs 0575/0576 are not yet authored) — flagged as a Stage-2 wave-2 dependency; this TEST-SPEC supersedes them in scope for Phase A coverage.
