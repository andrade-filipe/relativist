# TEST-SPEC-0476: PROTOCOL_VERSION bump and wire-version rejection (defensive landing-order-aware)

**SPEC-22 §7 ID:** T8a (spec-catalog) plus this plumbing file.
**Owning task:** TASK-0476.
**Parent spec:** SPEC-22 §3.1 R9a; §3.8 A9 (SPEC-18 PROTOCOL_VERSION amendment); SC-007 closure.
**Type:** unit + integration.
**Theory anchor:** None direct (wire-format hygiene).

---

## Inputs / Fixtures

- The post-bump live `PROTOCOL_VERSION` constant (whatever its bumped value at landing time).
- A test-only `PREVIOUS_LIVE_VERSION` constant captured at TASK-0476 implementation time as the value the constant held immediately before this bump.
- A `Net` with non-empty `free_list` (force a `remove_agent` first).

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0476-01 | `protocol_version_strictly_greater_than_predecessor` | the live `PROTOCOL_VERSION` at test-build time | `assert!(PROTOCOL_VERSION > PREVIOUS_LIVE_VERSION);` | passes. **CRITICAL:** the assertion MUST NOT be `assert_eq!(PROTOCOL_VERSION, 3);` — see Determinism notes for the SC-007 + SPEC-20 sequencing rationale. The assertion is `PROTOCOL_VERSION == PREVIOUS_LIVE_VERSION + 1` (strict +1 increment). |
| UT-0476-02 | `wire_handshake_v3_to_v2_rejected` | net serialized with the live constant; v2-emulator deserializer using `PREVIOUS_LIVE_VERSION` | `decode(bytes)` | returns `Err(ProtocolError::UnsupportedVersion { sender_version: PROTOCOL_VERSION, receiver_version: PREVIOUS_LIVE_VERSION })`. |
| UT-0476-03 | `wire_handshake_v3_to_v3_accepted` | net serialized with the live constant; deserializer with the live constant | round-trip | `Ok(net2)`; `net.is_behaviorally_equal(&net2) == true`. (Sanity smoke.) |
| UT-0476-04 | `error_variant_is_unsupported_version_not_other` | from UT-0476-02 | match the error | the variant is `ProtocolError::UnsupportedVersion { .. }` AND NOT `LengthMismatch`, `Eof`, `IoError`, or any silent-corruption variant. |
| UT-0476-05 | `protocol_version_documented_in_rustdoc` | grep the constant's Rustdoc at test time | `cargo doc --no-deps` | the docstring contains "SPEC-22 R9a" and a citation to either "REJECT-v2" or "TOLERATE-v2-as-empty-free-list" policy choice (whichever the implementer chose at TASK-0476 implementation time). |
| UT-0476-06 | `v3_deserializer_v2_tolerance_or_rejection` (CONDITIONAL on policy choice) | a v2-formatted net serialized with `PREVIOUS_LIVE_VERSION`; deserializer with the live constant | depending on the implementer's chosen policy: either `Ok(net)` with `net.free_list.is_empty()` (TOLERATE) OR `Err(UnsupportedVersion)` (REJECT) | the test asserts the OBSERVABLE matches the documented choice. The test code MUST cite SPEC-22 §6 Migration Path and the chosen path documented in the constant's Rustdoc. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Frame header version field corrupted to a random byte not equal to live | `UnsupportedVersion`. |
| EC-2 | Future v4 net (constant > live) deserialized by current code | `UnsupportedVersion`. (Forward-compat negative.) |
| EC-3 | v1 baseline binaries (`results/locked/v1_local_baseline/`) — frozen, NOT consumed by v2/v3 paths | The test does NOT load these binaries; documented as out of scope per R9a. (Asserted by absence of the load path in any v2-development code.) |

## Invariants asserted

- R9a (PROTOCOL_VERSION bump and v2-vs-v3 rejection clause — closes SC-007).
- §3.8 A9 (SPEC-18 R28 amendment).

## ARG/DISC/REF citation

- None direct.

## Determinism notes

**LANDING-ORDER-AWARE CONTRACT (CRITICAL):** SPEC-20 bumps `PROTOCOL_VERSION` 3 → 4. SPEC-22 bumps 2 → 3. If both ship in the same v2-development cycle, the actual landing value depends on landing order:

- If SPEC-22 lands first: the live constant becomes `3`. SPEC-20 then bumps it to `4`.
- If SPEC-20 lands first (TASK-0417 already in flight): live constant becomes `4`, then SPEC-22 bumps to `5`.

**The TEST-SPEC and the implementing test code MUST NOT pin the exact integer in any assertion.** Concretely:

- **FORBIDDEN:** `assert_eq!(PROTOCOL_VERSION, 3);` — couples the test to landing order.
- **REQUIRED:** `assert_eq!(PROTOCOL_VERSION, PREVIOUS_LIVE_VERSION + 1);` where `PREVIOUS_LIVE_VERSION` is captured from the codebase at test-build time via a snapshot, a test-only constant defined alongside the bump, or a build-script substitution.

The acceptance criteria in TASK-0476 already use this defensive language ("verify at code time"). This TEST-SPEC mirrors that language verbatim.

UT-0476-02 / UT-0476-04 reference values for `sender_version` / `receiver_version` are **resolved at test-build time** from the live constant and its predecessor — NEVER hardcoded as integers.

This contract is non-negotiable; any deviation is a CI-blocking finding. Code review MUST verify no integer-equal assertions on `PROTOCOL_VERSION`.

The wire-handshake variant (UT-0476-02) MAY exercise tokio if the SPEC-18 framing is async; in that case use `#[tokio::test(flavor = "current_thread")]` with deterministic single-runtime executor; document the async fences. Otherwise plain `#[test]` is sufficient.

## Cross-test dependencies

- T8a (spec-catalog) is the integration mirror; this plumbing test covers the constant-bump primitive.
- TEST-SPEC-EG-U15a / TEST-SPEC-EG-U15b (SPEC-20 protocol-version-mismatch) share the `UnsupportedVersion` error variant; ensure no fixture name collision.
- TEST-SPEC-0475 covers the same-version round-trip; this test covers the version-mismatch path.
- The chosen v3 deserializer policy (REJECT-v2 vs TOLERATE-v2-as-empty-free-list) is documented in SPEC-22 §6 Migration Path AND in the constant's Rustdoc; UT-0476-05 enforces the documentation requirement.
