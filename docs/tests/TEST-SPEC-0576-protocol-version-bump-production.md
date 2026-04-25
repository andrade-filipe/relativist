# TEST-SPEC-0576: PROTOCOL_VERSION bump production (defensive `PREVIOUS_LIVE_VERSION + 1` for SPEC-21 R31 — landing-order-aware)

**SPEC-21 §7 ID:** plumbing only (production-side closure of TEST-SPEC-0511 PROTOCOL_VERSION defensive coverage; mirrors TEST-SPEC-0476 verbatim pattern).
**Owning task:** TASK-0576.
**Parent spec:** SPEC-21 §3.7 R37c (PROTOCOL_VERSION sequencing for R31 wire variants; closes SC-005 / SC-009-style); §3.8 A2 (consumer of TASK-0511).
**Type:** unit + integration (compile-time const_assert + runtime cross-version mismatch test).
**Theory anchor:** None direct (wire-format hygiene).

---

## CRITICAL — DEFENSIVE PROTOCOL_VERSION CONTRACT

**This TEST-SPEC ENFORCES the defensive `PREVIOUS_LIVE_VERSION + 1` contract for SPEC-21 R31.** SPEC-21 R31 is the **third spec in the wave** to bump `PROTOCOL_VERSION`:

- SPEC-22 R9a: 2 → 3 (TASK-0476 / TEST-SPEC-0476).
- SPEC-20 R37: 3 → 4 (independent landing).
- **SPEC-21 R31: 4 → 5 (assuming both predecessors land first; LANDING-ORDER-DEPENDENT).**

**Different landing orders produce different absolute integers.** Concretely:
- **FORBIDDEN:** `assert_eq!(PROTOCOL_VERSION, 5);` — couples the test to landing order.
- **REQUIRED:** `assert_eq!(PROTOCOL_VERSION, PREVIOUS_LIVE_VERSION + 1);` where `PREVIOUS_LIVE_VERSION` is captured at test-build time as a snapshot taken immediately before TASK-0576's bump lands.

The capture mechanism is identical to TEST-SPEC-0476 / TEST-SPEC-0511 (test-only constant, build-script substitution, or Rustdoc-anchored grep gate). This contract is **non-negotiable**; any deviation is a CI-blocking finding.

---

## Inputs / Fixtures

- The post-bump live `PROTOCOL_VERSION` constant (whatever its bumped value at landing time).
- A test-only `PREVIOUS_LIVE_VERSION` constant captured at TASK-0576 implementation time as the value the constant held immediately before this bump (i.e., the result of the previous spec to land, whichever was last among SPEC-22 / SPEC-20).
- A `Message::RequestWork { worker_id: WorkerId(0) }` and `Message::NoMoreWork` instance (from TASK-0575).
- A pre-bump deserializer pinned to `PREVIOUS_LIVE_VERSION`.
- A post-bump deserializer using the live constant.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0576-01 | `protocol_version_strictly_one_above_predecessor` | the live `PROTOCOL_VERSION` at test-build time | `assert_eq!(PROTOCOL_VERSION, PREVIOUS_LIVE_VERSION + 1);` | passes. **CRITICAL:** the assertion MUST NOT be `assert_eq!(PROTOCOL_VERSION, 5);` or any hard-coded integer per the defensive contract. |
| UT-0576-02 | `protocol_version_const_assert_compiles` | the source-level `const _: () = assert!(PROTOCOL_VERSION == PREVIOUS_LIVE_VERSION + 1);` | `cargo build` | succeeds; the const_assert (or equivalent compile-time check) is present at the constant declaration site (TASK-0576 acceptance criterion). |
| UT-0576-03 | `pre_bump_deserializer_rejects_request_work` | encoded bytes of `Message::RequestWork { worker_id: WorkerId(0) }` (live version) | decode using deserializer pinned to `PREVIOUS_LIVE_VERSION` | `Err(ProtocolError::UnsupportedVersion { sender_version: PROTOCOL_VERSION, receiver_version: PREVIOUS_LIVE_VERSION })`. (R37c rejection clause; mirrors TEST-SPEC-0476 UT-0476-02.) |
| UT-0576-04 | `pre_bump_deserializer_rejects_no_more_work` | encoded `Message::NoMoreWork` | same | same `UnsupportedVersion` error variant. |
| UT-0576-05 | `post_bump_deserializer_accepts_both_variants` | encoded `RequestWork` and `NoMoreWork` | decode with live deserializer | both `Ok(decoded)`; `decoded == original`. (Positive control.) |
| UT-0576-06 | `error_variant_is_unsupported_version_not_silent_corruption` | UT-0576-03 / UT-0576-04 | match the error | the variant is `ProtocolError::UnsupportedVersion { .. }` AND NOT `LengthMismatch`, `Eof`, `IoError`, `UnknownVariantTag`, or any silent-corruption pathway. (R37c rejection clause; mirrors TEST-SPEC-0511 UT-0511-06 + TEST-SPEC-0476 UT-0476-04.) |
| UT-0576-07 | `protocol_version_documented_in_rustdoc` | grep the constant's Rustdoc | `cargo doc --no-deps` | the docstring contains "SPEC-21 §3.7 R37c", "SPEC-22 R9a", and a citation to the SPEC-20 R37 sequencing rule per TASK-0576 acceptance criterion. |
| UT-0576-08 | `previous_live_version_constant_documented` | the test-only `PREVIOUS_LIVE_VERSION` constant | inspect | the constant has a doc-comment explaining the landing-order-aware semantics and citing TEST-SPEC-0476 + TEST-SPEC-0511 as the precedent pattern. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | A v_prev binary attempting to handshake with a v_new (post-SPEC-21) binary | rejected with `UnsupportedVersion` at handshake, before any payload is exchanged. |
| EC-2 | A future v_new+1 peer (e.g., post-SPEC-23) sends `Message::RequestWork` to current v_new code | `UnsupportedVersion` (forward-compat negative; mirrors TEST-SPEC-0476 EC-2). |
| EC-3 | Frame header version field corrupted to a random byte not equal to live | `UnsupportedVersion`. (Mirrors TEST-SPEC-0476 EC-1.) |
| EC-4 | The CI guard checking that no test uses a hardcoded version literal | the lint MUST be extended to cover SPEC-21 variants per TASK-0576 NOTE line 66. The const_assert is the ENFORCEMENT MECHANISM; this gate ensures the assertion's body uses `PREVIOUS_LIVE_VERSION + 1`, not an integer literal. |
| EC-5 | Landing order shuffles (SPEC-21 lands before SPEC-22 or SPEC-20 in some test scenario) | `PREVIOUS_LIVE_VERSION` value differs but the `+1` invariant holds; tests pass regardless. (Defensive contract is the WHOLE POINT.) |

## Invariants asserted

- R37c (PROTOCOL_VERSION sequencing + rejection clause; closes SC-005 / SC-009-style for SPEC-21).
- §3.8 A2 (SPEC-06 amendment with explicit version-bump rationale).
- Wire compatibility (controlled break; mirrors SPEC-22 R9a / TASK-0476 precedent).
- Defensive landing-order-aware contract (NO hardcoded integer assertions).

## ARG/DISC/REF citation

- DISC-008 v2 (sync/termination dimension — wire-version handshake).
- TEST-SPEC-0476 (SPEC-22 PROTOCOL_VERSION 2→3) — precedent pattern.
- TEST-SPEC-0511 (SPEC-21 amendment-level PROTOCOL_VERSION coverage) — precedent pattern.

## Determinism notes

**LANDING-ORDER-AWARE CONTRACT (CRITICAL — SAME PATTERN AS TEST-SPEC-0476 + TEST-SPEC-0511):**

SPEC-21 R31 is the **third** spec in the wave to bump `PROTOCOL_VERSION`. Different landing orders produce different absolute integers:

- SPEC-22 R9a: 2 → 3.
- SPEC-20 R37: 3 → 4.
- SPEC-21 R31: 4 → 5 (assuming both predecessors land first).

If SPEC-21 lands BEFORE SPEC-22 or SPEC-20 (atypical but possible), the absolute integers shift but the `+1` invariant holds. The TEST-SPEC and the implementing test code MUST NOT pin the exact integer in any assertion. The capture mechanism is:

1. **Build-script substitution:** A `build.rs` that reads the live `PROTOCOL_VERSION` from the source and emits `PREVIOUS_LIVE_VERSION = PROTOCOL_VERSION - 1` as a generated constant in `OUT_DIR`. (Preferred per TASK-0576 NOTE line 66.)
2. **Test-only constant declared alongside the bump:** `pub const PREVIOUS_LIVE_VERSION: u16 = /* explicit snapshot */;` with a `const _: () = assert!(PROTOCOL_VERSION == PREVIOUS_LIVE_VERSION + 1);` guard at the declaration site.
3. **Rustdoc-anchored grep gate:** A CI lint that grep's the constant's docstring for the predecessor version label and refuses to merge if the assertion uses a different integer.

UT-0576-03 / UT-0576-04 reference values for `sender_version` / `receiver_version` are **resolved at test-build time** from the live constant and its predecessor — NEVER hardcoded as integers.

The wire-handshake variant (UT-0576-03 / UT-0576-04) MAY exercise the SPEC-18 framing path; if framing is async (`tokio` codec), use `#[tokio::test(flavor = "current_thread")]` with a deterministic single-runtime executor; otherwise plain `#[test]` is sufficient.

This contract is non-negotiable; any deviation is a CI-blocking finding. Code review MUST verify no integer-equal assertions on `PROTOCOL_VERSION` in this test or any sibling test that touches the constant.

## Cross-test dependencies

- **TEST-SPEC-0476 (SPEC-22 PROTOCOL_VERSION 2→3 precedent)** — defensive contract source. UT-0576-01 mirrors UT-0476-01 verbatim except for the spec citation.
- **TEST-SPEC-0511 (SPEC-21 amendment-level coverage)** — sibling defensive coverage. The `PREVIOUS_LIVE_VERSION` capture pattern is shared.
- **TEST-SPEC-0575 (R31 wire variants production)** — predecessor; the variants must exist as code before the version bump can ship. UT-0576-03 / UT-0576-04 / UT-0576-05 consume the variants from TASK-0575.
- **TEST-SPEC-EG-U15a / EG-U15b (SPEC-20 protocol-version-mismatch)** — share the `UnsupportedVersion` error variant and the defensive-language pattern.
- **TEST-SPEC-T8a (SPEC-22 wire-version rejection)** — share the integration-level mirror pattern.
- **TEST-SPEC-T11 / T12 / T13 / T14 (SPEC-21 pull-dispatch tests)** — consumers; pull-protocol exercises post-bump wire format.
- **TEST-SPEC-0577 / TEST-SPEC-0578 (coordinator/worker FSMs)** — consumers; FSM handshake depends on this version constant.
