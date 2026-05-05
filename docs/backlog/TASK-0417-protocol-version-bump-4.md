# TASK-0417: Bump `PROTOCOL_VERSION` 3 → 4

**Spec:** SPEC-20 §3.5 R37 (closes SC-004); R0d (full-rejoin under mode-mismatch).
**Requirements:** R37; coordinates with R0d handshake rejection.
**Priority:** P0 (wire-breaking; must land atomically with TASK-0418 + TASK-0419).
**Status:** TODO
**Depends on:** none (bump-only).
**Blocked by:** TASK-0418, TASK-0419 (should land in the same PR to avoid partial wire state).
**Estimated complexity:** S (~5 LoC production + ~20 LoC tests)
**Bundle:** SPEC-20 Elastic Grid — wire protocol foundations.

## Context

SPEC-20 introduces 5 new `Message` variants (discriminants 12-16) and new payload types (`LeaveKind`, `WorkerCapabilities`, `JoinNackReason`). A v3 coordinator cannot bincode-decode a `JoinRequest` from a v4 worker. R37 mandates the bump from 3 to 4. The existing handshake-rejection path (SPEC-06 R2a; SPEC-19 R37) handles version mismatch via `RegisterNack` / `JoinNack` at connection time (R0d, R35a).

## Acceptance Criteria

- [ ] Change `const PROTOCOL_VERSION: u32 = 3;` to `const PROTOCOL_VERSION: u32 = 4;` in `relativist-core/src/protocol/coordinator.rs`.
- [ ] Add a comment block citing SPEC-20 R37, R0d, and the 5-variant extension rationale.
- [ ] No migration code (R37 explicit prohibition).
- [ ] Handshake rejection for version-mismatched connections remains consistent with R0d.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/protocol/coordinator.rs` | modify | `PROTOCOL_VERSION` const bump + 2-line comment. |

## Test Expectations (forward-ref)

- `test_protocol_version_bumped_to_4` (sentinel).
- EG-U15a / EG-U15b (initial `Register` vs mid-session `JoinRequest` version-mismatch rejection paths).

## Invariants Touched

- None (wire-version sentinel only).

## Notes

- This is an atomic wire break — MUST ship in the same PR as TASK-0418 (`Message` extension) and TASK-0419 (handshake branch).

## DAG Links

- **Predecessors:** none.
- **Successors:** TASK-0418, TASK-0419, all worker wire consumers.
