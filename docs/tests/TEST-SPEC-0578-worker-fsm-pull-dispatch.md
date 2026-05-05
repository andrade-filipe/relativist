# TEST-SPEC-0578: Worker FSM extension — pull-dispatch states + heterogeneous-worker simulation harness

**SPEC-21 §7 ID:** primary owner of T13 (short-stream — joint with TASK-0577) and T14 (heterogeneous-worker simulation). Mirrors TEST-SPEC-T13-short-stream-fewer-chunks-than-workers and TEST-SPEC-T14-heterogeneous-worker-simulation (the spec-catalog T-tests delegate to this plumbing file).
**Owning task:** TASK-0578.
**Parent spec:** SPEC-21 §3.6 R32 (worker side of pull protocol), R35 (short-stream edge case); §3.7 R37d (BSP barrier — worker side); §3.7 R37e (push-mode termination — worker side); §3.8 A5 (consumer of TASK-0514).
**Type:** unit (FSM transition table) + integration (4-worker harness with heterogeneous timing).
**Theory anchor:** ARG-001 G1 (BSP determinism preserved — workers do NOT begin merge); ARG-004 (feasibility profiles — pull dispatch closes the heterogeneous-worker throughput gap per R37 SHOULD); AC-014 (Bench Methodology — wall-clock with warmup discard).

---

## Inputs / Fixtures

- The post-TASK-0578 `WorkerState` enum with the 2 new pull-mode states (`AwaitingChunkAfterResult`, `FinalReduction`).
- The post-TASK-0578 `worker_pull_loop(...)` entry point.
- `WorkerConfig` instances: `cfg_push` (`dispatch_mode = Push`), `cfg_pull` (`dispatch_mode = Pull`).
- A mock `Transport` providing scripted coordinator messages (`AssignPartition`, `NoMoreWork`).
- A 12-chunk pipeline: `ep_annihilation_pure(48)` with `chunk_size = 4`, 4 workers.
- A 2-chunk pipeline (short-stream): `ep_annihilation_pure(8)` with `chunk_size = 4`, 4 workers.
- A "slow worker" simulator: `SlowReductionWrapper` that adds `Duration::from_millis(N * delay_ms)` per chunk (parametric).

## Unit Tests (FSM transition table)

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0578-01 | `transition_reducing_chunk_done_to_awaiting_chunk_after_result` | state `ReducingChunk`; chunk reduction completes | check transition + outbound message | (a) `state == AwaitingChunkAfterResult`; (b) outbound `Message::PartitionResult(p)` sent; (c) outbound `Message::RequestWork { worker_id: WorkerId(self) }` sent immediately after. (R32 step 2.) |
| UT-0578-02 | `transition_awaiting_chunk_to_reducing_chunk_on_assign_partition` | state `AwaitingChunkAfterResult`; `AssignPartition(chunk_n)` arrives | process | `state == ReducingChunk`. |
| UT-0578-03 | `transition_awaiting_chunk_to_final_reduction_on_no_more_work` | state `AwaitingChunkAfterResult`; `NoMoreWork` arrives | process | `state == FinalReduction`. |
| UT-0578-04 | `transition_final_reduction_to_send_final_result` | state `FinalReduction`; reduction completes | check | (a) `state == SendFinalResult` (legacy state) → `Done`; (b) outbound final `PartitionResult` sent. |
| UT-0578-05 | `request_work_emitted_after_every_partition_result` | full pipeline run with N chunks for this worker | inspect outbound message log | exactly N `RequestWork` messages, each immediately following a `PartitionResult`. (R32 step 2 — TASK-0578 line 44.) |
| UT-0578-06 | `rejected_transition_done_assign_partition_returns_unexpected_message` | state `Done`; `AssignPartition` arrives (out-of-order from buggy coordinator) | process | `Err(WorkerError::UnexpectedMessage)`. (TASK-0578 acceptance line 43.) |
| UT-0578-07 | `worker_does_not_begin_merge` | full pipeline; worker reaches `Done` | inspect worker's local state | NO `merge` operation invoked locally; merge is COORDINATOR-side only. (R37d worker side — line 33.) |

## Unit Tests (R35 short-stream tolerance)

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0578-08 | `r35_no_more_work_immediately_after_first_partition_result` | state `AwaitingChunkAfterResult` reached after first chunk; coordinator immediately sends `NoMoreWork` (no second `AssignPartition`) | process | `AwaitingChunkAfterResult → FinalReduction → SendFinalResult → Done`; no errors. |
| UT-0578-09 | `r35_no_more_work_as_first_message` | state `Init` (or `AwaitingAssign`); `NoMoreWork` arrives FIRST (extreme corner per TASK-0578 line 32) | process | clean transition to `FinalReduction → SendFinalResult → Done`; final `PartitionResult` is empty (no agents reduced). |
| UT-0578-10 | `r35_partition_result_empty_legal` | UT-0578-09 outcome | inspect emitted `PartitionResult` | the `Net` payload is empty (zero agents); `state == Done`. (R35 closure operationalized.) |

## Unit Tests (R37e push-mode regression)

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0578-11 | `push_mode_no_request_work_emitted` | `cfg_push`; full pipeline run | inspect outbound message log | ZERO `Message::RequestWork` instances. (R37e — TASK-0578 line 46 acceptance.) |
| UT-0578-12 | `push_mode_no_more_work_unexpected` | `cfg_push`; force coordinator to send `NoMoreWork` (buggy) | process | `Err(WorkerError::UnexpectedMessage)`; push-mode workers do NOT expect this variant. (R37e enforcement — line 35-36.) |
| UT-0578-13 | `push_mode_pull_states_unreachable` | `cfg_push` | run pipeline | none of `AwaitingChunkAfterResult`, `FinalReduction` are entered (state-trace assertion). |

## Integration tests (T13 — short-stream)

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| IT-0578-T13-01 | `t13_short_stream_2_chunks_4_workers_worker_side` | `cfg_pull`; 2-chunk pipeline; 4 workers | run; observe per-worker behavior | workers 0 and 1 receive 1 chunk + `NoMoreWork`; workers 2 and 3 receive `NoMoreWork` immediately (or as first message); all 4 emit final `PartitionResult` (some empty); merged result correct. (Joint with TEST-SPEC-0577 IT-0577-T13-01.) |
| IT-0578-T13-02 | `t13_extreme_size_zero_all_workers_no_more_work_first` | `cfg_pull`; size-0 pipeline | run | all 4 workers receive `NoMoreWork` as first message; all emit empty `PartitionResult`; coordinator merges to empty net. |

## Integration tests (T14 — heterogeneous-worker simulation harness)

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| IT-0578-T14-01 | `t14_4_workers_one_slow_pull_throughput_higher_than_push` | 4 workers, worker 0 wrapped in `SlowReductionWrapper(2x delay)`; same 12-chunk workload | run pipeline twice: once with `cfg_push`, once with `cfg_pull`; measure wall-clock time per AC-014 methodology (warmup runs discarded) | `pull_wall_clock_ms < push_wall_clock_ms * 0.90`; pull-dispatch achieves ≥10% higher throughput. (R37 SHOULD operationalized.) |
| IT-0578-T14-02 | `t14_pull_chunk_distribution_skews_to_fast_workers` | same setup | observe per-worker chunk count under pull mode | the slow worker (worker 0) processes FEWER chunks than the fast workers (workers 1-3); chunk distribution under push mode is uniform (round-robin). Documents the load-balancing benefit. |
| IT-0578-T14-03 | `t14_correctness_preserved_across_dispatch_modes` | same setup | merge results from both runs | `nets_isomorphic(merged_pull, merged_push) == true`. (G1 preserved across heterogeneous-worker axis.) |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Worker disconnects mid-`AwaitingChunkAfterResult` | `WorkerError::TransportError`; clean shutdown without sending stale `PartitionResult`. |
| EC-2 | Coordinator sends `AssignPartition` and `NoMoreWork` back-to-back without waiting for `RequestWork` (buggy coordinator) | worker ignores the out-of-order `NoMoreWork` if state is `ReducingChunk` (queues it) OR returns `WorkerError::UnexpectedMessage`. Document policy choice. |
| EC-3 | `dispatch_mode == Pull && delta_mode == true` | worker arena recycle behavior is gated by TEST-SPEC-0589 / TEST-SPEC-0590 / TEST-SPEC-0591; the FSM triggers the recycle hook at chunk completion but does NOT itself implement the gating policy. (TASK-0578 NOTE line 93.) |
| EC-4 | `streaming_active` flag cleared on `FinalReduction → SendFinalResult` transition | post-streaming reductions can recycle normally. (Cross-cuts TEST-SPEC-0589 NOTE line 69.) |

## Invariants asserted

- R32 (worker-side 5-step pull protocol).
- R35 (short-stream / fewer-chunks-than-workers edge case; closes that SC).
- R37d worker side (workers MUST NOT begin merge).
- R37e worker side (push-mode workers do NOT emit `RequestWork` or expect `NoMoreWork`).
- §3.8 A5 (SPEC-13 amendment, worker side).
- G1 (BSP determinism — workers idle after final result; coordinator owns the BSP barrier).
- T13 (short-stream; full integration-level closure jointly with TASK-0577).
- T14 (heterogeneous-worker simulation; primary integration-level closure).
- ARG-004 (feasibility profile — pull dispatch closes the heterogeneous-worker throughput gap).

## ARG/DISC/REF citation

- ARG-001 G1 (BSP determinism — worker side).
- ARG-004 (feasibility profile — pull dispatch motivation).
- AC-014 (Bench Methodology — wall-clock with warmup discard for IT-0578-T14-01).

## Determinism notes

**TOKIO SCHEDULING (UT TESTS):** All FSM unit tests use `#[tokio::test(flavor = "current_thread")]` for deterministic message ordering. The mock `Transport` is a single-producer-single-consumer channel with FIFO ordering. NO `tokio::time::sleep` for ordering enforcement.

**FSM TRANSITION TRACE:** State transitions instrumented via `state_trace: Vec<WorkerState>` appended on every `next_state()` call (gated on `#[cfg(test)]`). UT-0578-13 asserts zero pull-mode entries under push mode.

**T14 WALL-CLOCK MEASUREMENT (IT-0578-T14-01):** AC-014 methodology MUST be followed:
1. Warmup runs (3 iterations) discarded.
2. Measurement runs (10 iterations) collected with `std::time::Instant::now()`.
3. Median (not mean) reported to dampen scheduler noise.
4. Test asserts `median(pull) < median(push) * 0.90`.
5. The `SlowReductionWrapper` injects deterministic delay via `std::thread::sleep` (NOT `tokio::time::sleep`) inside the reduction loop to simulate CPU-bound slowness; document this choice explicitly.
6. NOTE: this test is wall-clock-sensitive; on heavily-loaded CI runners the assertion MAY be flaky. Document the flakiness trade-off; a `#[ignore]` flag is acceptable for CI with a `--include-ignored` opt-in for local verification per TASK-0578 NOTE line 91-92.

**HETEROGENEOUS SIMULATION HARNESS:** Use single-threaded tokio runtime to avoid OS-thread spawning overhead. The 4 "workers" are 4 tokio tasks on the same runtime; the slow-wrapper is a per-task delay injection.

**R35 EXTREME CORNER (UT-0578-09):** the worker FSM MUST handle `NoMoreWork` from `Init` or `AwaitingAssign` states cleanly. If TASK-0578 chooses `AwaitingAssign` as the FIRST state (post-`Hello` handshake), the transition `AwaitingAssign + NoMoreWork → FinalReduction` MUST be in the table.

## Cross-test dependencies

- **TEST-SPEC-0514 (SPEC-13 FSM amendment-level coverage)** — predecessor.
- **TEST-SPEC-0511 (SPEC-06 amendment)** — predecessor.
- **TEST-SPEC-0575 (wire variants production)** — predecessor; worker emits / consumes these variants.
- **TEST-SPEC-0576 (PROTOCOL_VERSION bump production)** — predecessor; worker handshake depends on the version constant.
- **TEST-SPEC-0577 (coordinator FSM pull-dispatch)** — sibling; pairs in review. T11 / T12 / T13 jointly owned.
- **TEST-SPEC-T11-pull-based-dispatch-protocol** — spec-catalog mirror; coordinator side primary; this file is the worker-side closure.
- **TEST-SPEC-T12-pull-vs-push-equivalence** — spec-catalog mirror; jointly owned with TASK-0577.
- **TEST-SPEC-T13-short-stream-fewer-chunks-than-workers** — spec-catalog mirror; full closure here jointly with TASK-0577.
- **TEST-SPEC-T14-heterogeneous-worker-simulation** — spec-catalog mirror; primary owner is this file (simulator harness lives here).
- **TEST-SPEC-0589 / TEST-SPEC-0590 / TEST-SPEC-0591 (R10b strategy wiring under streaming)** — consumers; FSM triggers recycle hook at chunk completion (EC-3).
