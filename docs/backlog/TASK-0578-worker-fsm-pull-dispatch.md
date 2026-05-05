# TASK-0578: Worker FSM extension — pull-dispatch states + heterogeneous-worker simulation harness

**Spec:** SPEC-21 §3.6 R32 (worker side of the pull protocol), R35 (short-stream edge case); §3.7 R37d (BSP barrier — worker side); §3.7 R37e (push-mode termination — worker side); §3.8 A5 (consumer of TASK-0514).
**Requirements:** R32 worker steps; R35 (short-stream / fewer-chunks-than-workers edge case); R37d (worker MUST NOT begin merge until `NoMoreWork`); R37e (push-mode workers do NOT expect `NoMoreWork`).
**Priority:** P0 (T13 / T14 owner; pairs with TASK-0577 to complete pull-dispatch protocol).
**Status:** TODO
**Depends on:** TASK-0514 (SPEC-13 amendment A5 landed — worker state list + transitions), TASK-0511 (SPEC-06 amendment), TASK-0575 (`RequestWork` / `NoMoreWork` variants in code), TASK-0576 (PROTOCOL_VERSION bump), TASK-0577 (paired coordinator FSM).
**Blocked by:** none (paired with TASK-0577 but independently shippable).
**Estimated complexity:** L (~200 LoC worker FSM + heterogeneous-worker simulation harness; ~250 LoC tests). NOTE: at the L bound — DEVELOPER MAY split simulation harness into a separate sibling task if production exceeds 200 LoC.
**Bundle:** SPEC-21 Streaming Generation — Phase F (regression / polish / late-binding).

## Context

Per SPEC-21 §3.8 A5, the worker FSM gains 2 new states:

- `AwaitingChunkAfterResult` (entered after sending `PartitionResult`, awaiting `AssignPartition` or `NoMoreWork`)
- `FinalReduction` (entered upon receiving `NoMoreWork`)

Transitions per A5:
- `ReducingChunk + chunk-done → AwaitingChunkAfterResult` (also emits `RequestWork`)
- `AwaitingChunkAfterResult + AssignPartition → ReducingChunk`
- `AwaitingChunkAfterResult + NoMoreWork → FinalReduction`
- `FinalReduction + reduction-done → SendFinalResult → Done`

Per R32 worker steps:
1. Worker receives initial `AssignPartition` (chunk 0).
2. Worker reduces, sends `PartitionResult(p0)`, then immediately sends `RequestWork`.
3. On `AssignPartition(chunk_n)`: continue reducing.
4. On `NoMoreWork`: complete current reduction, send final `PartitionResult`, transition to `Done`.

Per R35 (closes SC short-stream edge case): when stream length < num_workers, some workers MUST handle receiving `NoMoreWork` immediately after their first chunk OR even before any `AssignPartition` (if they registered late). This task verifies the FSM tolerates that path.

Per R37d worker side: workers MUST NOT begin merge themselves; merge happens at the coordinator. Workers transition `FinalReduction → SendFinalResult → Done` and idle.

Per R37e worker side: push-mode workers do NOT expect `NoMoreWork`. The new states are pull-only and gated on `dispatch_mode == Pull` in the worker config (received via `Hello` handshake or `GridConfig` push).

This task ALSO ships the **heterogeneous-worker simulation harness** (T14 owner). The harness simulates 4 workers where one is 2x slower (artificial delay in the reduce loop) and verifies that pull dispatch achieves higher throughput than push dispatch with the same workload (per R37 SHOULD).

## Acceptance Criteria

- [ ] Worker FSM type (`WorkerState` enum) gains 2 new states per A5; old push-mode states unchanged.
- [ ] FSM gated on `dispatch_mode` (received from coordinator).
- [ ] Transition table verbatim per A5; rejected transitions (e.g., `Done + AssignPartition`) return `WorkerError::UnexpectedMessage`.
- [ ] Worker emits `RequestWork` immediately after every `PartitionResult` send (R32 step 2).
- [ ] Short-stream tolerance (R35): worker that receives `NoMoreWork` IMMEDIATELY after its first `PartitionResult` (or even as the FIRST message in extreme corner case) MUST transition cleanly to `FinalReduction → SendFinalResult → Done`.
- [ ] Push-mode regression: `dispatch_mode = Push` worker NEVER emits `RequestWork` and NEVER expects `NoMoreWork` (R37e enforcement).
- [ ] Heterogeneous-worker simulation harness: 4 workers, one with 2x artificial delay; pull-dispatch run achieves measurably higher chunk throughput than push-dispatch run on the same workload (T14 SHOULD per R37).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-net/src/worker/fsm.rs` (or equivalent) | modify | Add 2 pull-mode states + transition fn; gate on `dispatch_mode`. |
| `relativist-net/src/worker/pull_loop.rs` | create | Worker-side reduce-and-request loop. |
| `relativist-net/tests/worker_pull_fsm.rs` | create | Transition-table coverage + R35 short-stream + R37e push-mode regression. |
| `relativist-net/tests/heterogeneous_worker_simulation.rs` | create | T14 simulation harness (4 workers, 1 slow). |

## Key Types / Signatures

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerState {
    // legacy push-mode states UNCHANGED:
    Init, AwaitingAssign, ReducingChunk, /* ... */, Done,
    // new pull-mode states (R32, A5):
    AwaitingChunkAfterResult,
    FinalReduction,
}

pub fn worker_pull_loop(
    cfg: &WorkerConfig,
    transport: &mut dyn Transport,
) -> Result<(), WorkerError>;
```

## Test Expectations (forward-ref)

Owner of T13 (short-stream — joint with TASK-0577), T14 (heterogeneous-worker simulation — primary owner).
- TEST-SPEC-T11 / T12 (paired with TASK-0577).
- TEST-SPEC-T13: 2-chunk stream, 4 workers; verify workers 2 and 3 receive `NoMoreWork` immediately after registration (R35 closure).
- TEST-SPEC-T14: 12-chunk stream, 4 workers (1 slow, 2x delay); pull-dispatch throughput > push-dispatch throughput by ≥10%; methodology AC-014 wall-clock per R37 closure.

## Invariants Touched

- G1 (BSP determinism) — workers do NOT begin merge; coordinator owns the BSP barrier.
- R37e push-mode termination scoping — enforced.
- R35 short-stream edge case — covered.

## Notes

- The simulation harness uses an in-process mock `Transport` (no real TCP); avoid spawning real OS threads where possible — use `tokio::test` with a single-threaded runtime to keep the test deterministic.
- T14 throughput measurement uses AC-014 methodology (wall-clock with `std::time::Instant`, warmup runs discarded). Document explicitly in the test docstring.
- Cross-spec with SPEC-22 R10b: under `dispatch_mode == Pull && delta_mode == true`, the worker arena recycle behavior is gated by TASK-0589 / TASK-0590 / TASK-0591 — this FSM task triggers the recycle hook at chunk completion but does NOT itself implement the gating policy.
- Pairs with TASK-0577 in review.

## DAG Links

- **Predecessors:** TASK-0514, TASK-0511, TASK-0575, TASK-0576.
- **Successors:** TASK-0589 / TASK-0590 / TASK-0591 (recycle gating consumed at chunk-completion hook).
