# TASK-0517: [SPEC-04 amendment A8] §4.5 `split()` UNCHANGED — chunked pipeline is additive

**Spec:** SPEC-21 §3.8 A8 (closes SC-001 part 4); SPEC-21 §6 Migration Path.
**Requirements:** A8 (formal SPEC-04 §4.5 clarification — split() unchanged; chunked pipeline ADDITIVE).
**Priority:** P1 (clarifying amendment; no consuming production task — purely documentation).
**Status:** TODO
**Depends on:** none (Phase A).
**Blocked by:** none
**Estimated complexity:** S (~10 LoC SPEC-04 next-revision diff; spec text only).
**Bundle:** SPEC-21 Streaming Generation — predecessor-spec amendment cluster (Phase A).
**Tag:** `[SPEC-04 amendment]`

## Context

SPEC-04 §4.5 documents `split()` as the canonical partition entry point. SPEC-21 introduces `generate_and_partition_chunked()` (§3.3 R17) which produces `ChunkedPartitionResult` — structurally compatible with `PartitionPlan` per R20-R21.

Without an explicit additive-amendment note, downstream readers of SPEC-04 may be surprised by SPEC-21's parallel pipeline and assume `split()` has been deprecated or modified. SPEC-21 §3.8 A8 documents the additive nature explicitly:

- `split()` is UNCHANGED — same semantics, same R-numbers (R6/R12/R16-R18/R28).
- Chunked pipeline is ALTERNATIVE entry point selected by `GridConfig.chunk_size != u32::MAX`.
- The two paths produce **structurally compatible** output (`PartitionPlan` from `split()`, `ChunkedPartitionResult.partitions + .borders` from streaming, with `ChunkedPartitionResult` convertible to `PartitionPlan` per R20-R21).
- `split()` is the fallback for the v1 backward-compat path (R26 short-circuit when `chunk_size = u32::MAX`).

This amendment clarifies the relationship and prevents a future spec-critic round from flagging "is split() obsolete?" as an open question.

## Acceptance Criteria

- [ ] ESPECIALISTA EM SPECS lands the SPEC-04 next-revision diff amending §4.5 with a one-paragraph note per SPEC-21 §3.8 A8.
- [ ] §4.5 explicitly states that `split()` is UNCHANGED.
- [ ] §4.5 cross-references SPEC-21 §3.3 R17 (`generate_and_partition_chunked`), R20-R21 (structural compatibility), R26 (v1 backward-compat short-circuit).
- [ ] §4.5 documents that the two paths produce non-overlapping but coexistent entry points.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `codigo/relativist/specs/SPEC-04-partition.md` | modify (by ESPECIALISTA EM SPECS only) | §4.5 amended — adds the additive-pipeline clarification per SPEC-21 §3.8 A8. |

## Test Expectations (forward-ref)

TEST-SPEC-0517 — covered by:
- TEST-SPEC-0567 R26 short-circuit (`chunk_size = u32::MAX`).
- T6 (streaming vs batch isomorphism).
- All existing SPEC-04 split() tests UNCHANGED — regression gate via TASK-0600.

## Invariants Touched

- None directly (clarifying amendment only).

## Notes

- This is a spec-text-only task (no production code).
- The amendment is small (single paragraph) but important for downstream readability.
- No-op for downstream production tasks; no consuming task depends on this beyond the regression gate (TASK-0600) verifying split() semantics are preserved.
- Consumed by: nothing (terminal documentation amendment).

## DAG Links

- **Predecessors:** none.
- **Successors:** TASK-0600 (verifies split() semantics preserved).
