# TEST-SPEC-T8a: Wire-version rejection v3 → v2 (closes SC-007)

**SPEC-22 §7.1 ID:** T8a.
**Owning task:** TASK-0476 (PROTOCOL_VERSION bump and rejection clause).
**Parent spec:** SPEC-22 §3.1 R9a; §3.8 A9 (SPEC-18 PROTOCOL_VERSION amendment); SC-007 closure.
**Type:** integration (exercises the wire-handshake + serde rejection path).
**Theory anchor:** None direct (wire-format hygiene); aligns with SPEC-18 R31's `UnsupportedVersion` reject pattern (mirrored from SPEC-20 R37).

---

## Inputs / Fixtures

- A `Net` with non-empty `free_list` (force a `remove_agent` to populate before serializing).
- A "v3 serializer" — the post-bump live `PROTOCOL_VERSION` constant (whatever its bumped value at landing time; see Determinism notes).
- A "v2 deserializer" — a simulated counterpart with the **previous live** `PROTOCOL_VERSION` (i.e., the value the constant held immediately before the bump). The simulation MAY be implemented as a local clone of the deserialization path with the constant overridden by a test-only constant.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-T8a-01 | `v3_serialized_net_rejected_by_v2_deserializer` | net with non-empty `free_list`, serialized via the v3 path (live constant); test-side v2-emulator deserializer | invoke the v2-emulator deserializer on the v3 bytestring | returns `Err(ProtocolError::UnsupportedVersion { sender_version: <bumped_value>, receiver_version: <previous_live_value> })`. |
| UT-T8a-02 | `error_is_unsupported_version_not_length_mismatch` | same | same | the error variant is exactly `UnsupportedVersion`, NOT `LengthMismatch` / `Eof` / `IoError`. (Critical: silent corruption is the failure mode SC-007 was protecting against.) |
| UT-T8a-03 | `v3_to_v3_round_trip_succeeds` | same fixture; v3-serializer and v3-deserializer (both at live constant) | round-trip | succeeds; deserialized net `is_behaviorally_equal` to original (joint coverage with T8). |
| UT-T8a-04 | `wire_handshake_rejects_at_frame_decode_time` | a SPEC-18 frame containing the bumped-constant version byte; receiver expects the previous-live value | frame decode | rejects at frame-decode time, BEFORE serde decode begins. The error is propagated as `UnsupportedVersion` at the protocol layer (per SPEC-18 R31 reject pattern). |
| UT-T8a-05 | `v3_deserializer_v2_tolerance_policy` (CONDITIONAL — only if implementer chose the TOLERATE policy per SPEC-22 §6) | a v2-formatted net (no `free_list` in payload) deserialized by the v3 path | `Ok(net)` with `net.free_list.is_empty()`. (CONDITIONAL: if the implementer chose the REJECT policy per SPEC-22 R9a, this test is replaced by a symmetric rejection assertion.) The test asserts the policy chosen at TASK-0476 implementation time. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | A v1 (`PROTOCOL_VERSION = 1`) net (frozen baseline binary) deserialized by the v3 path | Behaviour per SPEC-22 R9a + SPEC-18 R31: `UnsupportedVersion`. v1 baseline binaries are frozen and not consumed by v2/v3 code paths (per R9a). |
| EC-2 | A net with `PROTOCOL_VERSION` value greater than the live bump (e.g., a future v4 net) deserialized by v3 | `UnsupportedVersion`. (Forward-compat negative — v3 deserializer rejects unknown future versions.) |
| EC-3 | Frame header version field corrupted (random byte) | `UnsupportedVersion` for any value not exactly equal to the live constant. |

## Invariants asserted

- R9a (PROTOCOL_VERSION bump and v2-vs-v3 rejection clause — closes SC-007).
- §3.8 A9 (SPEC-18 R28 amendment — bump direction normalized).

## ARG/DISC/REF citation

- None direct (wire-hygiene).

## Determinism notes

**SC-007 + SPEC-22 R9a sequencing caveat (verbatim from TASK-0476):** SPEC-20 bumps `PROTOCOL_VERSION` 3 → 4. SPEC-22 bumps 2 → 3. If both ship in the same v2-development cycle, the actual landing value depends on landing order:

- If SPEC-22 lands first: live constant becomes `3`. SPEC-20 then bumps to `4`.
- If SPEC-20 lands first (TASK-0417 already in flight): live constant becomes `4`, then SPEC-22 bumps to `5`.

**Test obligation (mandated by the prompt):** the TEST-SPEC and the implementing test code MUST NOT pin the exact integer in any assertion. Concretely:

- **FORBIDDEN:** `assert_eq!(PROTOCOL_VERSION, 3);`
- **REQUIRED:** `assert_eq!(PROTOCOL_VERSION, previous_live_version + 1);` where `previous_live_version` is captured from the codebase at test-build time via a snapshot or a test-only constant defined alongside the bump.

The test's UT-T8a-01 / UT-T8a-04 reference values for `sender_version` and `receiver_version` are **resolved at test-build time** from the live constant and its predecessor; they are NEVER hardcoded as integers.

This contract is documented in the test docstring and enforced by code review (no integer-equal assertions on `PROTOCOL_VERSION`).

Pure synchronous tests; no tokio scheduling for the serde path. The wire-handshake variant (UT-T8a-04) MAY exercise tokio if the SPEC-18 framing is async; in that case use `#[tokio::test]` with a deterministic single-runtime executor (no multi-thread scheduling) and document the async fences.

## Cross-test dependencies

- T8 covers the same-version success path; T8a covers the rejection path. Together they form the wire-compat boundary tests.
- TEST-SPEC-0476 plumbing test covers the constant-bump primitive; T8a is the integration-level closure.
- Coordinates with TEST-SPEC-EG-U15a/U15b (SPEC-20 protocol-version-mismatch tests) — both SPEC-20 and SPEC-22 use the same `UnsupportedVersion` error variant; tests must not collide on fixture names.
