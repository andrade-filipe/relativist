# TASK-0510: [SPEC-04 amendment A1] §4.5 / R12 border-id allocation policy for streaming pipeline

**Spec:** SPEC-21 §3.8 A1 (closes SC-018); SPEC-21 §3.5 R29b; SPEC-21 §4.8.
**Requirements:** A1 (formal SPEC-04 R12 amendment). Enables SPEC-21 R29b (numbered requirement) and §4.8 step-3 partition `border_id_start`/`border_id_end` propagation.
**Priority:** P0 (blocker for TASK-0560 first-batch FreePort scan and TASK-0561 partition border-id-range propagation).
**Status:** TODO
**Depends on:** none
**Blocked by:** none
**Estimated complexity:** S (~25 LoC SPEC-04 next-revision diff; spec text only — no production code).
**Bundle:** SPEC-21 Streaming Generation — predecessor-spec amendment cluster (Phase A).
**Tag:** `[SPEC-04 amendment]`

## Context

SPEC-04 R12 currently mandates: *"Border IDs MUST be globally unique and MUST NOT collide with pre-existing FreePort IDs in the net. New border IDs MUST start from `max_existing_freeport_id + 1` (AC-002: `borderStart = maxFreePortId(netWires) + 1`)"* — this assumes a single global scan of the *full* net before partitioning, which the streaming pipeline cannot perform (the full net does not exist until the stream is exhausted).

SPEC-21 §3.8 A1 amends SPEC-04 R12 to differentiate the two paths:
- **Batch path** (`split()` when `chunk_size = u32::MAX`): unchanged — SPEC-04 R12 as stated.
- **Streaming path** (`generate_and_partition_chunked`): border IDs start at 0 and increment monotonically when no Lafont FreePorts are present in any batch, OR at `max_lafont_freeport_id_in_first_batch + 1` when the first batch carries Lafont FreePorts.

Generators that emit Lafont FreePorts SHOULD emit ALL of them in the first batch (allowing a single first-batch scan to fix the offset). The two paths produce non-overlapping but distinct border-id ranges; tests that exercise both `split()` and `generate_and_partition_chunked` MUST account for this.

## Acceptance Criteria

- [ ] ESPECIALISTA EM SPECS lands the SPEC-04 next-revision diff amending §4.5 / R12 with the SPEC-21 §3.8 A1 *New text* verbatim.
- [ ] R12 explicitly states the dual-path policy (batch vs streaming) and cross-references SPEC-21 R29b and §4.8.
- [ ] §4.5 documents that `Partition.border_id_start` / `border_id_end` (SPEC-04 R15a) MUST be set to the global range `[0, border_id_counter)` (or shifted by `max_lafont_freeport_id + 1` when Lafont FreePorts exist).
- [ ] §4.5 carries a one-sentence note that tests exercising both `split()` and `generate_and_partition_chunked` MUST account for the distinct border-id ranges (different absolute integers, same internal C3-bijectivity guarantee).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `codigo/relativist/specs/SPEC-04-partition.md` | modify (by ESPECIALISTA EM SPECS only) | §4.5 / R12 amended per SPEC-21 §3.8 A1; adds streaming-path clause and cross-refs to SPEC-21 R29b / §4.8. |

## Test Expectations (forward-ref)

TEST-SPEC-0510 — covered by:
- T5 (streaming pipeline C3 — TASK-0570).
- T6 (streaming vs batch isomorphism — TASK-0567 / streaming↔split() short-circuit).
- New cross-path test: assert `split()`-produced border-id range disjoint from streaming-produced range when both run on the same generator (covered jointly by TASK-0560 and TASK-0567).

## Invariants Touched

- C3 (FreePort Bijectivity) — preserved across both paths; the amendment only changes the absolute integer, not the bijection contract.
- T1 (Port Linearity) — unaffected.

## Notes

- This is a spec-text-only task (no production code).
- The amendment does NOT modify the batch-path semantics; existing v1 tests that depend on `max_existing_freeport_id + 1` continue to pass.
- Consumed by TASK-0560 (first-batch FreePort scan) and TASK-0561 (partition border-id-range propagation).

## DAG Links

- **Predecessors:** none.
- **Successors:** TASK-0560 (first-batch scan), TASK-0561 (partition propagation).
