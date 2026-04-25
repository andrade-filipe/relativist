# TASK-0476: Bump `PROTOCOL_VERSION` 2 → 3 + v2-vs-v3 rejection clause

**Spec:** SPEC-22 §3.1 R9a (closes SC-007); SPEC-22 §6 Migration Path; §7.1 T8a.
**Requirements:** R9a (PROTOCOL_VERSION 2→3 mandate; v2 deserializers MUST reject v3 nets with `UnsupportedVersion`; v3 deserializers MAY tolerate v2 as empty free-list OR reject).
**Priority:** P0 (wire-compat blocker; coordinates with TASK-0468 SPEC-18 amendment).
**Status:** TODO
**Depends on:** TASK-0468 (SPEC-18 R28 amendment), TASK-0475 (serde participation in place).
**Blocked by:** none
**Estimated complexity:** S (~20 LoC production + ~60 LoC tests)
**Bundle:** SPEC-22 Arena Management — Phase B (free-list core implementation).

## Context

The introduction of `free_list` in the `Net` serialized layout MUST be coordinated with SPEC-18's `PROTOCOL_VERSION` constant. SPEC-18 currently sets `PROTOCOL_VERSION = 2` (verified at SPEC-18 line 536). SPEC-22 R9a mandates the bump to `3`; v2 deserializers reject v3 nets via the existing `ProtocolError::UnsupportedVersion` path (mirrors SPEC-20 R37 v3-vs-v4 rejection clause). v3 deserializers MAY tolerate v2 nets as empty `free_list` (deserializer-defined; documented in SPEC-22 §6).

**Sequencing caveat (Round 2 closure §SC-007):** SPEC-20's bump is from 3 to 4. If SPEC-22 lands first, `PROTOCOL_VERSION = 3`; SPEC-20 then bumps to 4. If SPEC-20 lands first (as it currently is in TASK-0417), then SPEC-22's bump is 4 → 5. **The DEVELOPER MUST verify the live constant at implementation time and adjust accordingly.** Document the chosen value at the top of the implementation comment.

## Acceptance Criteria

- [ ] Update the `PROTOCOL_VERSION` constant in the SPEC-18 wire-format module (`relativist-core/src/protocol/` or wherever the constant lives) to the next value above the SPEC-20 baseline (3 or 5 depending on landing order — verify at code time).
- [ ] Confirm the existing version-mismatch path (`ProtocolError::UnsupportedVersion`) rejects nets with `PROTOCOL_VERSION` mismatch.
- [ ] Document the chosen v3 deserializer policy (REJECT-v2 vs TOLERATE-v2-as-empty-free-list) in SPEC-22 §6 Migration Path **and** in the constant's Rustdoc.
- [ ] Test T8a: serialize a v3-format net (with non-empty free-list); attempt deserialization with a `PROTOCOL_VERSION = 2` deserializer (simulated via SPEC-18 version-mismatch handshake); assert `UnsupportedVersion` error returned (NOT length-mismatch, NOT silent-drop).
- [ ] Add a SPEC-18 wire-handshake test that confirms v2-vs-v3 rejection at frame-decode time (not just serde-decode).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/protocol/...` *(constant location TBD at code time)* | modify | Bump `PROTOCOL_VERSION` constant; update Rustdoc. |
| `codigo/relativist/specs/SPEC-22-arena-management.md` *(via ESPECIALISTA EM SPECS only — out of this task's scope)* | — | §6 Migration Path may need a one-line clarification of the chosen deserializer policy. (NOT this task's edit; flag for ESPECIALISTA EM SPECS if needed.) |

## Test Expectations (forward-ref)

TEST-SPEC-0476:
- T8a (SPEC-22 §7.1): version-mismatch rejection.
- `wire_handshake_v3_to_v2_rejected` — frame-level rejection.
- `wire_handshake_v3_to_v3_accepted` — sanity.

## Invariants Touched

- R9a (wire compatibility break — controlled).

## Notes

- v1 baseline binaries (`results/locked/v1_local_baseline/`) are frozen and not consumed by v2/v3 code paths — acceptable per SPEC-22 R9a.
- This task is the bridge between SPEC-22 (data layout) and SPEC-18 (wire). It does NOT modify the SPEC-18 spec text — that's TASK-0468's job (Phase A).

## DAG Links

- **Predecessors:** TASK-0468, TASK-0475.
- **Successors:** TASK-0500 (v1 backward-compat regression gate — full wire round-trip parity).
