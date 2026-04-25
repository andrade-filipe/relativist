# TASK-0576: PROTOCOL_VERSION bump production (defensive `PREVIOUS_LIVE_VERSION + 1` for SPEC-21 R31 variants)

**Spec:** SPEC-21 §3.7 R37c (PROTOCOL_VERSION sequencing for R31 wire variants; closes SC-005 / SC-009-style); §3.8 A2 (consumer of TASK-0511 for the SPEC-06 amendment text).
**Requirements:** R37c (defensive `PREVIOUS_LIVE_VERSION + 1` language; pre-bump deserializers MUST reject post-bump payloads with `UnsupportedVersion`).
**Priority:** P0 (wire-compat coordination across SPEC-20 / SPEC-21 / SPEC-22 — landing-order-aware).
**Status:** TODO
**Depends on:** TASK-0511 (SPEC-06 amendment landed), TASK-0476 (SPEC-22 PROTOCOL_VERSION 2→3 production landed — establishes `PREVIOUS_LIVE_VERSION` semantics), TASK-0575 (wire variants exist as code).
**Blocked by:** TASK-0476 MUST land first (SPEC-22 is the second-in-the-wave; SPEC-21 R31 is the third).
**Estimated complexity:** S (~20 LoC version constant bump + rejection clause + ~60 LoC version-mismatch tests).
**Bundle:** SPEC-21 Streaming Generation — Phase F (regression / polish / late-binding).

## Context

Per SPEC-21 R37c (verbatim line 263-267), R31 introduces two new variants on `Message`. Every `Message`-catalog addition is a wire-format change. SPEC-22 R9a / §3.8 A9 plans a PROTOCOL_VERSION bump (v2 → v3); SPEC-20 plans an independent bump (v3 → v4). **SPEC-21 R31 is the third spec in the wave to touch the constant.**

SPEC-21's disposition (verbatim from §3.7 R37c):

> The PROTOCOL_VERSION bump for SPEC-21 R31 MUST follow defensive `PREVIOUS_LIVE_VERSION + 1` language and NOT a hardcoded absolute integer. The current live version (post-SPEC-22 / post-SPEC-20 landing, whichever lands first) is the baseline; SPEC-21's bump is `+1`. The SPEC-22 R10b rejection clause (v_old deserializers MUST reject v_new payloads with `UnsupportedVersion`) MUST also apply to the SPEC-21 bump.

This task ships:
1. The actual `PROTOCOL_VERSION` constant increment in `relativist-protocol/src/version.rs` (or wherever SPEC-22 TASK-0476 placed it).
2. The defensive `assert!(PROTOCOL_VERSION == PREVIOUS_LIVE_VERSION + 1)` compile-time / startup-time check (mirrors TEST-SPEC-0476 PROTOCOL_VERSION defensive contract, pattern verbatim per INDEX line 273-275).
3. The pre-SPEC-21 binary rejection clause: a deserializer at the previous version receiving a payload with the new variant tag MUST return `ProtocolError::UnsupportedVersion`.

Landing-order awareness: if SPEC-22 ships first (TASK-0476 lands `v2 → v3`) and SPEC-21 ships second, this task lands `v3 → v4`. If SPEC-20 also ships in between, this task lands `v4 → v5`. The defensive `PREVIOUS_LIVE_VERSION + 1` language MAKES THAT REORDERING SAFE.

## Acceptance Criteria

- [ ] `PROTOCOL_VERSION` constant incremented by `1` from whatever the live value is at landing time (NOT a hardcoded absolute number).
- [ ] Code-level assertion or `const_assert` enforces `PROTOCOL_VERSION == PREVIOUS_LIVE_VERSION + 1` (where `PREVIOUS_LIVE_VERSION` is the constant from the prior shipped spec — SPEC-22 or SPEC-20, whichever was last).
- [ ] `decode_message_v_prev(buf_with_post_bump_payload)` returns `Err(ProtocolError::UnsupportedVersion)` — never silently misinterprets the variant tag.
- [ ] Cross-version test: a v_prev binary attempting to decode a buffer that was encoded by a v_new (post-SPEC-21) binary REJECTS with `UnsupportedVersion`, mirroring SPEC-22 TEST-SPEC-0476 verbatim pattern.
- [ ] Documentation comment on the constant cites SPEC-21 §3.7 R37c, SPEC-22 R9a, and the SPEC-20 R37 sequencing rule.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-protocol/src/version.rs` (or equivalent) | modify | Bump `PROTOCOL_VERSION` by `+1`; add defensive assertion. |
| `relativist-protocol/tests/wire_protocol_version_streaming_bump.rs` | create | v_prev-vs-v_new mismatch rejection test (mirrors `wire_protocol_version_freelist_bump.rs` from TEST-SPEC-0476). |

## Key Types / Signatures

```rust
// In version.rs (defensive, landing-order-aware):
pub const PREVIOUS_LIVE_VERSION: u16 = /* SPEC-22 or SPEC-20 latest */;
pub const PROTOCOL_VERSION: u16 = PREVIOUS_LIVE_VERSION + 1; // SPEC-21 R37c
const _: () = assert!(PROTOCOL_VERSION == PREVIOUS_LIVE_VERSION + 1);
```

## Test Expectations (forward-ref)

Reuse pattern from TEST-SPEC-0476 (cited by TEST-SPEC-0511 per INDEX line 274). Tests at this layer:
- UT-0576-01: `PROTOCOL_VERSION == PREVIOUS_LIVE_VERSION + 1` (compile-time const_assert).
- UT-0576-02: pre-bump deserializer rejects `Message::RequestWork` payload with `UnsupportedVersion`.
- UT-0576-03: pre-bump deserializer rejects `Message::NoMoreWork` payload with `UnsupportedVersion`.
- UT-0576-04: post-bump deserializer accepts both variants (positive control).

## Invariants Touched

- Wire compatibility (controlled break, mirrors SPEC-22 R9a / TASK-0476 precedent).

## Notes

- Landing-order coordination: the `PREVIOUS_LIVE_VERSION` constant changes meaning depending on whether SPEC-22 / SPEC-20 ship before or after this task. The defensive `+1` language MUST be preserved verbatim per R37c — DO NOT hardcode absolute integers.
- The CI guard checking that no test uses a hardcoded version literal MUST be extended to cover the SPEC-21 variants (covered by R37c closure — the assertion `PROTOCOL_VERSION == PREVIOUS_LIVE_VERSION + 1` is the ENFORCEMENT MECHANISM for the no-hardcoded-literal rule).
- Consumed by TASK-0577 (FSM checks version compatibility during handshake), TASK-0578 (worker version check at startup).

## DAG Links

- **Predecessors:** TASK-0511, TASK-0476, TASK-0575.
- **Successors:** TASK-0577, TASK-0578.
