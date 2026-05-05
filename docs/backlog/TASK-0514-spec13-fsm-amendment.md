# TASK-0514: [SPEC-13 amendment A5] Coordinator + Worker FSMs gain pull-mode states

**Spec:** SPEC-21 §3.8 A5 (closes SC-001 part 3, SC-015); SPEC-21 §3.6 R30, R32; SPEC-21 §3.7 R37d, R37e.
**Requirements:** A5 (formal SPEC-13 coordinator + worker FSM amendment).
**Priority:** P0 (blocker for TASK-0577 coordinator FSM production and TASK-0578 worker FSM production).
**Status:** TODO
**Depends on:** none (Phase A).
**Blocked by:** none
**Estimated complexity:** S (~50 LoC SPEC-13 next-revision diff; spec text only).
**Bundle:** SPEC-21 Streaming Generation — predecessor-spec amendment cluster (Phase A).
**Tag:** `[SPEC-13 amendment]`

## Context

SPEC-13 currently documents push-only FSMs (a single `AssignPartition` per worker, then `PartitionResult`, then merge). SPEC-21 R30-R32 introduce pull-based dispatch, requiring FSM extensions on both sides.

**Coordinator FSM new states (per SPEC-21 §3.8 A5 *New text*):**
- `DispatchingFirst` — cold start, sending the first chunk to each worker.
- `AwaitingResults` — waiting for `PartitionResult` from any worker.
- `GeneratingNext` — calling `make_net_stream::next` then `strategy.allocate_batch`.
- `SendingNoMoreWork` — generator stream exhausted, sending `NoMoreWork` to any worker that issues `RequestWork`.
- `AwaitingFinalResults` — awaiting all post-`NoMoreWork` results, then merge.

**Coordinator FSM transitions:**
- `Init → DispatchingFirst`
- `DispatchingFirst → AwaitingResults`
- `AwaitingResults + RequestWork → GeneratingNext` (if stream not exhausted) OR `SendingNoMoreWork` (if exhausted)
- `GeneratingNext + chunk-ready → AwaitingResults` (after sending `AssignPartition`)
- `SendingNoMoreWork + all-acks → AwaitingFinalResults`
- `AwaitingFinalResults + all-results → Merge` (BSP barrier per R37d)

**Worker FSM new states:**
- `AwaitingChunkAfterResult` — entered after sending `PartitionResult`, awaiting `AssignPartition` or `NoMoreWork`.
- `FinalReduction` — entered upon receiving `NoMoreWork`.

**Worker FSM transitions:**
- `ReducingChunk + chunk-done → AwaitingChunkAfterResult` (also emits `RequestWork`)
- `AwaitingChunkAfterResult + AssignPartition → ReducingChunk`
- `AwaitingChunkAfterResult + NoMoreWork → FinalReduction`
- `FinalReduction + reduction-done → SendFinalResult → Done`

**Push-mode FSMs UNCHANGED (per R37e).** Pull-only states gated on `DispatchMode::Pull` in `GridConfig` (the field added by TASK-0512). Workers MUST NOT add defensive `NoMoreWork` handling to the push-mode transition table; coordinators MUST NOT emit `NoMoreWork` in push mode.

The BSP-barrier semantics (R37d) reduce pull dispatch to a single "logical BSP round" regardless of wall-clock interleaving.

## Acceptance Criteria

- [ ] ESPECIALISTA EM SPECS lands the SPEC-13 next-revision diff amending the coordinator FSM section to add the 5 new pull-mode states with explicit transition tuples.
- [ ] ESPECIALISTA EM SPECS lands the SPEC-13 next-revision diff amending the worker FSM section to add the 2 new pull-mode states with explicit transition tuples.
- [ ] SPEC-13 explicitly notes that push-mode FSMs are UNCHANGED and pull-only states are gated on `DispatchMode::Pull`.
- [ ] SPEC-13 documents the BSP-barrier semantics under pull dispatch per R37d (workers MAY emit `PartitionResult` individually but MUST NOT begin merge until `NoMoreWork` arrives).
- [ ] Cross-references SPEC-21 R30, R32, R37d, R37e, §3.8 A5; cross-references SPEC-07 / SPEC-05 GridConfig `dispatch_mode` field (TASK-0512).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `codigo/relativist/specs/SPEC-13-system-architecture.md` | modify (by ESPECIALISTA EM SPECS only) | Coordinator FSM + worker FSM sections amended per SPEC-21 §3.8 A5; explicit push-mode-unchanged note added. |

## Test Expectations (forward-ref)

TEST-SPEC-0514 — covered by:
- TEST-SPEC-0577 coordinator FSM transitions (T11 pull-protocol).
- TEST-SPEC-0578 worker FSM transitions (T11).
- T12 (pull vs push equivalence — TASK-0579).
- T13 (short-stream — TASK-0581).
- T14 (heterogeneous workers — TASK-0584).

## Invariants Touched

- D5 (Exclusive Ownership) — preserved via chunk-to-worker assignment tracking (R33).
- D6 (Protocol termination) — preserved via R37d BSP barrier semantics under pull.
- G1 — preserved by R37d (pull dispatch reduces to single logical BSP round).

## Notes

- This is a spec-text-only task (no production code).
- The push-mode-unchanged guarantee is critical for backward compatibility — without it, every existing 1181-test scenario would need re-validation.
- Consumed by TASK-0577 (coordinator FSM impl), TASK-0578 (worker FSM impl), TASK-0579 (orchestration), TASK-0582 (push-mode termination scoping).

## DAG Links

- **Predecessors:** none.
- **Successors:** TASK-0577, TASK-0578, TASK-0579, TASK-0582, TASK-0583.
