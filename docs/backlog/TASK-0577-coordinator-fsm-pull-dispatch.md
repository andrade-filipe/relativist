# TASK-0577: Coordinator FSM extension — pull-dispatch states + transitions (gated on `DispatchMode::Pull`)

**Spec:** SPEC-21 §3.6 R30, R32 (pull-based dispatch loop); §3.7 R37d (BSP barrier under pull dispatch; closes SC-019); §3.7 R37e (push-mode termination scoping; closes SC-013); §3.8 A5 (consumer of TASK-0514).
**Requirements:** R30 (coordinator MUST support pull-based dispatch mode); R32 (the 5-step pull protocol); R37d (BSP barrier = moment all workers acknowledge `NoMoreWork`); R37e (push mode UNCHANGED — pull-only states gated on `DispatchMode::Pull`).
**Priority:** P0 (T11 / T12 / T13 / T14 owner; pull-dispatch is the heterogeneous-worker fix per R37 SHOULD).
**Status:** TODO
**Depends on:** TASK-0514 (SPEC-13 amendment A5 landed — coordinator state list + transitions), TASK-0511 (SPEC-06 amendment), TASK-0575 (`RequestWork` / `NoMoreWork` variants in code), TASK-0576 (PROTOCOL_VERSION bump), TASK-0565 (`DispatchMode` field on GridConfig), TASK-0554 (`generate_and_partition_chunked` orchestrator — coordinator drives `make_net_stream::next` from `GeneratingNext` state).
**Blocked by:** TASK-0578 SHOULD land in coordinated review (FSMs are paired but independently shippable).
**Estimated complexity:** L (~250 LoC FSM state-machine + transition table + dispatcher loop; ~300 LoC integration tests). NOTE: this task is at the L bound — DEVELOPER should consider splitting `state-machine` from `dispatcher loop` if production exceeds 200 LoC.
**Bundle:** SPEC-21 Streaming Generation — Phase F (regression / polish / late-binding).

## Context

Per SPEC-21 §3.8 A5 (TASK-0514), the coordinator FSM gains 5 new states:

- `DispatchingFirst` (cold start)
- `AwaitingResults` (waiting for `PartitionResult` from any worker)
- `GeneratingNext` (calling `make_net_stream::next` then `strategy.allocate_batch`)
- `SendingNoMoreWork` (stream exhausted)
- `AwaitingFinalResults` (awaiting all post-`NoMoreWork` results, then merge)

Transitions per A5:
- `Init → DispatchingFirst`
- `DispatchingFirst → AwaitingResults`
- `AwaitingResults + RequestWork → GeneratingNext` (if stream not exhausted) **or** `SendingNoMoreWork` (if exhausted)
- `GeneratingNext + chunk-ready → AwaitingResults` (after sending `AssignPartition`)
- `SendingNoMoreWork + all-acks → AwaitingFinalResults`
- `AwaitingFinalResults + all-results → Merge` (BSP barrier per R37d)

Per R37d (closes SC-019), the BSP barrier under pull dispatch is the moment **all workers acknowledge `NoMoreWork`**. Workers MAY complete reductions and emit `PartitionResult` messages individually but MUST NOT begin merge until `NoMoreWork` arrives. This preserves G1 by reducing pull dispatch to a single logical BSP round.

Per R37e (closes SC-013), in **push mode** no `NoMoreWork` is sent. The new states are pull-only and gated on `DispatchMode::Pull` in `GridConfig`. Push-mode FSMs are UNCHANGED.

Per R32 step-by-step (verbatim §3.6):
1. Coordinator generates first chunk eagerly, dispatches to worker 0.
2. Worker 0 emits `PartitionResult(p0)` then `RequestWork`.
3. Coordinator on receiving `RequestWork`: either generate next chunk and `AssignPartition`, or send `NoMoreWork`.
4. Worker on receiving `NoMoreWork`: complete current reduction, send final `PartitionResult`, transition to Done.
5. Coordinator on receiving all final `PartitionResult`s: merge (BSP barrier per R37d).

## Acceptance Criteria

- [ ] Coordinator FSM type (`CoordinatorState` enum or struct-with-marker) gains 5 new states per A5; old push-mode states unchanged.
- [ ] FSM is gated on `GridConfig.dispatch_mode`: when `Push` (or `Auto` resolving to push), the new states are unreachable; when `Pull`, the legacy single-shot dispatch path is unreachable.
- [ ] Transition table verbatim per A5; rejected transitions (e.g., `DispatchingFirst + RequestWork`) panic in debug, return error in release.
- [ ] R37d BSP barrier: a `PartitionResult` arriving in `AwaitingResults` is **buffered**, not merged; merge happens ONLY in `AwaitingFinalResults` once all workers have ack'd `NoMoreWork`.
- [ ] Per R37e, push mode emits NO `NoMoreWork` — assertion in tests + `debug_assert!` in code path.
- [ ] Integration test (T11 partial): 4 workers, stream length 12 chunks, all workers receive `NoMoreWork` and contribute partitions; merged output isomorphic to single-worker baseline.
- [ ] Integration test (T13 partial — short stream, joint with TASK-0578): stream length 2 chunks, 4 workers; first 2 workers receive 1 chunk, then `NoMoreWork`; last 2 workers receive `NoMoreWork` immediately; merged output correct (R35 closure).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-net/src/coordinator/fsm.rs` (or equivalent) | modify | Add 5 pull-mode states + transition fn; gate on `dispatch_mode`. |
| `relativist-net/src/coordinator/pull_dispatch.rs` | create | Dispatcher loop driving `make_net_stream::next` from `GeneratingNext`. |
| `relativist-net/tests/coordinator_pull_fsm.rs` | create | Transition-table coverage + R37d BSP barrier verification. |

## Key Types / Signatures

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinatorState {
    // legacy push-mode states UNCHANGED:
    Init, AwaitingHello, /* ... */, MergingResults, Done,
    // new pull-mode states (R30-R32, A5):
    DispatchingFirst,
    AwaitingResults,
    GeneratingNext,
    SendingNoMoreWork,
    AwaitingFinalResults,
}

pub fn coordinator_pull_dispatch_loop(
    cfg: &GridConfig,
    stream: Box<dyn Iterator<Item = AgentBatch>>,
    strategy: &mut dyn StreamingPartitionStrategy,
    transport: &mut dyn Transport,
) -> Result<MergedResult, CoordinatorError>;
```

## Test Expectations (forward-ref)

Owner of T11 (pull-based dispatch protocol — TEST-SPEC-T11), T12 (pull-vs-push equivalence — TEST-SPEC-T12), partial T13 (short stream — joint with TASK-0578), partial T14 (heterogeneous worker simulation — joint with TASK-0578 simulator harness).
- TEST-SPEC-T11 (T11): 4 workers, 12-chunk stream, full pull protocol exercise.
- TEST-SPEC-T12 (T12): same input, push vs pull merge → isomorphic via `nets_isomorphic`.
- Push-mode regression: `dispatch_mode = Push` produces zero `NoMoreWork` messages on the wire (R37e enforcement).

## Invariants Touched

- G1 (BSP determinism) preserved by R37d single-logical-BSP-round reduction.
- R37e push-mode termination scoping enforced.

## Notes

- The `GeneratingNext` state pulls one batch from `make_net_stream::next` (R16 — synchronous iterator); no async coordination needed at this state per R16 close-SC-012 note.
- Cross-spec interaction with SPEC-19 delta: under `delta_mode && dispatch_mode == Pull`, the coordinator MUST also drive TASK-0588 `BorderGraph::extend_with_chunk_borders` between successive `AssignPartition` messages (R37f / A7). Wiring is owned by TASK-0588; this FSM task triggers the call from `GeneratingNext` post-`install_connection`.
- `DispatchMode::Auto` resolves to `Push` when `chunk_size == u32::MAX` (R26 short-circuit) and to `Pull` when `chunk_size < u32::MAX && len_estimate > num_workers`. Document the resolution rule in code comments.
- Consumed by T13/T14 simulator harness (TASK-0578-side); regression gate via cross-spec test.

## DAG Links

- **Predecessors:** TASK-0514, TASK-0511, TASK-0575, TASK-0576, TASK-0565, TASK-0554.
- **Successors:** TASK-0578 (paired worker FSM), TASK-0588 (BorderGraph extension call-site).
