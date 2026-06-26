# TASK-0425: `SoloReducing` state + `reduce_n(solo_budget)` batch loop

**Spec:** SPEC-20 §3.1 R5 (solo mode, closes SC-009), R5a (termination conditions), R6 (initial_wait_timeout supersedes worker_connect_timeout), R15 (solo → grid transition on join). §4.1.1 `SoloReducing` state; §4.1.4 transitions.
**Requirements:** R5, R5a, R6, R15.
**Priority:** P0 (hybrid correctness + join responsiveness).
**Status:** TODO
**Depends on:** TASK-0414 (`SoloReducing` state enum), TASK-0415 (`solo_budget`, `initial_wait_timeout`), TASK-0422 (event loop), TASK-0436 (FSM wiring).
**Blocked by:** TASK-0414, TASK-0422.
**Estimated complexity:** M (~130-180 LoC production + ~80 LoC tests)
**Bundle:** SPEC-20 Elastic Grid — Phase 2.1 Hybrid Coordinator.

## Context

When K == 0 at round start AND `hybrid_coordinator = true`, the coordinator enters `SoloReducing`: reduces the entire net via `reduce_n(net, solo_budget)` in a loop, polling the async event loop between batches for `WorkerJoined` events. This trades minor per-batch overhead for join responsiveness. Terminates on (i) empty redex queue (→ `Done`) or (ii) `WorkerJoined` (→ `CheckTermination` → grid mode). R5a requires the in-flight batch to complete before processing a join. R6 makes `initial_wait_timeout` (default 30s) supersede `worker_connect_timeout` when hybrid.

## Acceptance Criteria

- [ ] Implement solo-mode logic in the coordinator: loop `reduce_n(net, cfg.solo_budget)` emitting `SoloReduceBatchComplete` events between batches (TASK-0422 select picks them up).
- [ ] FSM transitions (implemented in TASK-0436, verified here):
  - `WaitingForWorkers × InitialWaitTimeout [K=0 && hybrid] → SoloReducing` (R6).
  - `WaitingForWorkers × InitialWaitTimeout [K=0 && !hybrid] → Error` (preserves v1 fatal).
  - `SoloReducing × SoloReduceBatchComplete [redexes_remain] → SoloReducing` (loop).
  - `SoloReducing × SoloReductionComplete → Done` (R5a (i)).
  - `SoloReducing × WorkerJoined(id) → CheckTermination` (R5a (ii), transition deferred until end of in-flight batch per R5a).
- [ ] `solo_budget = u32::MAX` degenerates to single-batch `reduce_all` (no join responsiveness; benchmark-only).
- [ ] `initial_wait_timeout` (default 30s) cancels `worker_connect_timeout` (SPEC-06 R24, default 120s) in hybrid mode (R6 MUST).
- [ ] Emit metrics: `SoloReducing` entry timestamp, batch count, total solo interactions.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/protocol/coordinator.rs` | modify | Solo-reducing state handling + timer management. |
| `relativist-core/src/protocol/timers.rs` *(if exists)* | modify | `InitialWait` timer supersedes `WorkerConnect` in hybrid mode. |

## Test Expectations (forward-ref)

- EG-U1 `test_hybrid_coordinator_single_machine` (R1, R5).
- EG-U1a `test_solo_join_during_solo_reduction` (R5/R5a, R15, SC-009).
- EG-U18 `test_initial_wait_timeout_supersedes_worker_connect_timeout` (R6, SC-020).

## Invariants Touched

- D2 (Local Reduction Equivalence) — preserved: solo reduction is the same `reduce_n` primitive.
- D6 (Protocol Termination) — preserved via solo termination (finite T7 interactions).

## Notes

- **Batch size tradeoff**: default `solo_budget = 10_000` is a ~1% overhead trade for acceptable join responsiveness. Documented in R33a.

## DAG Links

- **Predecessors:** TASK-0414, TASK-0422.
- **Successors:** TASK-0436 (FSM wiring), TASK-0426 (initial wait timeout), TASK-0432 (join handler).
