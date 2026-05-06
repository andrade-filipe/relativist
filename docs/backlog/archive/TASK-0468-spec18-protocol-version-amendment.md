# TASK-0468: [SPEC-18 amendment A9] Bump `PROTOCOL_VERSION` 2 → 3 for `Net.free_list` wire layout

**Spec:** SPEC-22 §3.8 A9 (closes SC-007).
**Requirements:** A9 (formal SPEC-18 R28 amendment); enables SPEC-22 R9, R9a.
**Priority:** P0 (blocker for TASK-0476 wire-coordination implementation).
**Status:** TODO
**Depends on:** none (operates on SPEC-18 R28 verbatim).
**Blocked by:** none
**Estimated complexity:** S (~20 LoC SPEC-18 next-revision diff; no production code)
**Bundle:** SPEC-22 Arena Management — predecessor-spec amendment cluster (Phase A).
**Tag:** `[SPEC-18 amendment]`

## Context

SPEC-18 R28 (line 163; live constant `PROTOCOL_VERSION = 2`) currently states: "The `PROTOCOL_VERSION` constant MUST be bumped from `1` to `2`. This is a one-time, intentional wire compatibility break." SPEC-22's free-list field addition to `Net` changes the bincode layout (one new `Vec<AgentId>` field at the end of the struct). The amendment mandates a 2 → 3 bump with v2-vs-v3 rejection symmetric to the SPEC-20 R37 v3-vs-v4 rejection clause. Persisted v1/v2 `.bin` files become unreadable by v3 binaries (acceptable because v1 baseline binaries are frozen and not consumed by v2/v3 code paths).

## Acceptance Criteria

- [ ] ESPECIALISTA EM SPECS lands the SPEC-18 next-revision diff amending R28 with the SPEC-22 §3.8 A9 *New text* verbatim.
- [ ] R28 explicitly mandates the 2 → 3 bump upon SPEC-22 landing.
- [ ] R28 specifies the v2 deserializer rejection of v3 nets via existing `UnsupportedVersion` error path (mirrors SPEC-18 R31).
- [ ] R28 documents v1/v2 `.bin` file unreadability as acceptable (frozen baseline binaries).
- [ ] R28 cross-references the migration path documented in SPEC-22 §6.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `codigo/relativist/specs/SPEC-18-wire-format-v2.md` | modify (by ESPECIALISTA EM SPECS only) | R28 amended per SPEC-22 §3.8 A9; constant table updated to show `PROTOCOL_VERSION = 3`. |

## Test Expectations (forward-ref)

TEST-SPEC-0468 — covered by T8a (SPEC-22 §7.1 — wire-version rejection test, TASK-0476).

## Invariants Touched

- R26 (round-trip identity across binary versions — preserved via clean rejection on mismatch).

## Notes

- The wire break is justified by the `free_list` field addition; v1/v2 binaries cannot deserialize v3 nets without producing length-mismatch errors.
- v3 deserializers MAY ALSO reject v2 nets, OR MAY tolerate them as nets with an empty `free_list` (deserializer-defined; documented in SPEC-22 §6 Migration).
- This amendment composes with TASK-0417 (SPEC-20 v3→v4 bump) — be careful: SPEC-20's bump is to v4, SPEC-22's is to v3. Sequencing matters: if SPEC-22 lands first, `PROTOCOL_VERSION = 3`; SPEC-20 then bumps to 4. Coordinate the constant value with the cicd agent / SPEC-20 author. Round 2 closure log §SC-007 confirms this path.

## DAG Links

- **Predecessors:** none.
- **Successors:** TASK-0476 (wire-coordination implementation, T8a test surface).
