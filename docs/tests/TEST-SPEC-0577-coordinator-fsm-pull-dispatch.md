# TEST-SPEC-0577: Coordinator FSM extension — pull-dispatch states + transitions (gated on `DispatchMode::Pull`)

**SPEC-21 §7 ID:** owner of T11 (pull-based dispatch protocol) and T12 (pull-vs-push equivalence) at the integration level. Mirrors TEST-SPEC-T11-pull-based-dispatch-protocol and TEST-SPEC-T12-pull-vs-push-equivalence (the spec-catalog T-tests delegate to this plumbing file for FSM transition coverage). Joint-owner of T13 (with TASK-0578) for short-stream edge case.
**Owning task:** TASK-0577.
**Parent spec:** SPEC-21 §3.6 R30, R32 (pull-based dispatch); §3.7 R37d (BSP barrier; closes SC-019); §3.7 R37e (push-mode termination scoping; closes SC-013); §3.8 A5 (consumer of TASK-0514). Cross-spec interaction with SPEC-19 R37f / A7 (delta+streaming via TEST-SPEC-0588).
**Type:** unit (FSM transition table) + integration (4-worker pull-dispatch end-to-end).
**Theory anchor:** ARG-001 G1 (BSP determinism preserved by single-logical-BSP-round reduction per R37d); DISC-008 v2 (sync/termination dimension).

---

## Inputs / Fixtures

- The post-TASK-0577 `CoordinatorState` enum with the 5 new pull-mode states (`DispatchingFirst`, `AwaitingResults`, `GeneratingNext`, `SendingNoMoreWork`, `AwaitingFinalResults`).
- The post-TASK-0577 `coordinator_pull_dispatch_loop(...)` entry point.
- `GridConfig` instances: `cfg_push` (`dispatch_mode = Push`), `cfg_pull` (`dispatch_mode = Pull`), `cfg_auto` (`dispatch_mode = Auto`).
- A mock `Transport` providing scripted `RequestWork` / `PartitionResult` messages with controlled ordering (no real TCP).
- A 12-chunk stream (`ep_annihilation_pure(48)` with `chunk_size = 4`) for T11 / T12.
- A 2-chunk stream (`ep_annihilation_pure(8)` with `chunk_size = 4`) for T13 short-stream coverage.
- The `nets_isomorphic` helper for T12 push-vs-pull equivalence.

## Unit Tests (FSM transition table)

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0577-01 | `transition_init_to_dispatching_first` | initial coordinator state `Init` with `cfg_pull` | trigger entry | `state == DispatchingFirst`. |
| UT-0577-02 | `transition_dispatching_first_to_awaiting_results` | state `DispatchingFirst`; first chunk dispatched | mock returns `Ok` from transport send | `state == AwaitingResults`. |
| UT-0577-03 | `transition_awaiting_results_request_work_stream_alive` | state `AwaitingResults`; `RequestWork` arrives; stream has more chunks | process message | `state == GeneratingNext`. |
| UT-0577-04 | `transition_awaiting_results_request_work_stream_exhausted` | state `AwaitingResults`; `RequestWork` arrives; stream exhausted | process | `state == SendingNoMoreWork`. |
| UT-0577-05 | `transition_generating_next_to_awaiting_results` | state `GeneratingNext`; chunk allocated by `strategy.allocate_batch` | dispatch `AssignPartition` | `state == AwaitingResults`. |
| UT-0577-06 | `transition_sending_no_more_work_all_acks_to_awaiting_final_results` | state `SendingNoMoreWork`; all 4 workers ack `NoMoreWork` (i.e., no further `RequestWork` arrivals) | check transition | `state == AwaitingFinalResults`. |
| UT-0577-07 | `transition_awaiting_final_results_to_merge` | state `AwaitingFinalResults`; all 4 final `PartitionResult` messages received | trigger merge | `state == MergingResults` (legacy state). (R37d BSP barrier.) |
| UT-0577-08 | `rejected_transition_dispatching_first_request_work_panics_in_debug` | state `DispatchingFirst`; `RequestWork` arrives (out-of-order) | process | `#[cfg(debug_assertions)]`: panics; `#[cfg(not(debug_assertions))]`: returns `CoordinatorError::UnexpectedMessage`. (TASK-0577 acceptance criterion line 45.) |
| UT-0577-09 | `partition_result_in_awaiting_results_buffered_not_merged` | state `AwaitingResults`; `PartitionResult` arrives | process | result is BUFFERED (added to internal `pending_results: Vec<PartitionResult>`); state UNCHANGED; merge does NOT happen. (R37d BSP barrier — line 46.) |
| UT-0577-10 | `merge_only_in_awaiting_final_results` | state `AwaitingFinalResults`; all results buffered (some from `AwaitingResults`, some final) | trigger merge | merge consumes ALL buffered results (mid-stream + final); single logical BSP round. (R37d single-round reduction.) |

## Unit Tests (push-mode regression — R37e)

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0577-11 | `push_mode_no_more_work_never_emitted` | `cfg_push`; full pipeline run | inspect transport's emitted message log | ZERO `Message::NoMoreWork` instances. (R37e — line 47 acceptance.) |
| UT-0577-12 | `push_mode_pull_states_unreachable` | `cfg_push` | run pipeline | none of the 5 pull-mode states are entered (verified via state-trace assertion); legacy push-mode states are exercised. |
| UT-0577-13 | `push_mode_debug_assert_on_no_more_work_send_attempt` | `cfg_push`; force a code path that erroneously calls `transport.send(NoMoreWork)` | debug build | `debug_assert!` fires per TASK-0577 acceptance line 47; release build is a silent no-op (defensive). |

## Unit Tests (Auto resolution — TASK-0577 NOTE line 98)

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0577-14 | `auto_resolves_to_push_when_chunk_size_u32_max` | `cfg_auto` with `chunk_size = u32::MAX` | resolve dispatch mode at orchestrator entry | resolves to `Push` (R26 short-circuit; no chunked dispatch). |
| UT-0577-15 | `auto_resolves_to_pull_when_streaming_with_excess_capacity` | `cfg_auto` with `chunk_size = 16`, `len_estimate = 64`, `num_workers = 4` (len > workers) | resolve | resolves to `Pull`. |
| UT-0577-16 | `auto_resolves_to_push_when_chunks_le_workers` | `cfg_auto` with `chunk_size = 16`, `len_estimate = 16`, `num_workers = 4` (one chunk per worker — push is sufficient) | resolve | resolves to `Push` (degenerate streaming case; pull overhead unjustified). Documented per TASK-0577 NOTE line 98. |

## Integration tests (T11 — pull-based dispatch protocol)

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| IT-0577-T11-01 | `t11_4_workers_12_chunks_full_pull_protocol` | `cfg_pull`; `ep_annihilation_pure(48)` with `chunk_size = 4` (12 chunks); 4 workers | run end-to-end | (a) all 12 chunks dispatched via `AssignPartition`; (b) all 4 workers receive `NoMoreWork` exactly once; (c) all 4 final `PartitionResult`s received; (d) merged output isomorphic to single-worker baseline. |
| IT-0577-T11-02 | `t11_request_work_count_equals_chunks_plus_workers_minus_first` | same | count `RequestWork` messages received by coordinator | `== 12 + 4 - 1 = 15` (each worker except worker 0 sends after each chunk it processes; first chunk is eager; final `RequestWork` from each worker triggers `NoMoreWork`). NOTE: exact count depends on R32 step 2 timing — adjust if TASK-0578 implementation differs; document in test. |

## Integration tests (T12 — pull-vs-push equivalence)

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| IT-0577-T12-01 | `t12_pull_vs_push_merged_output_isomorphic` | same input under `cfg_pull` and `cfg_push` separately | run both; merge | `nets_isomorphic(merged_pull, merged_push) == true`. (G1 preserved across dispatch modes.) |
| IT-0577-T12-02 | `t12_dual_tree_pull_push_isomorphic` | `dual_tree(8)` under both modes | run + merge | `nets_isomorphic == true`. |

## Integration tests (T13 partial — joint with TASK-0578)

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| IT-0577-T13-01 | `t13_short_stream_2_chunks_4_workers` | `cfg_pull`; `ep_annihilation_pure(8)` with `chunk_size = 4` (2 chunks); 4 workers | run end-to-end | first 2 workers receive 1 chunk each, then `NoMoreWork`; last 2 workers receive `NoMoreWork` immediately (R35 closure); merged output correct. (Joint with TEST-SPEC-0578 IT-0578-T13-01 worker side.) |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Stream exhausted before any `RequestWork` arrives (size = 0) | coordinator transitions `DispatchingFirst → SendingNoMoreWork` directly; all 4 workers receive `NoMoreWork` as their first message. (R35 extreme corner.) |
| EC-2 | Worker disconnects mid-stream (transport returns `Err`) | coordinator MUST transition to `Failed` state (legacy error path); pull-mode states are NOT a new error path. |
| EC-3 | Worker sends `PartitionResult` with malformed payload | `CoordinatorError::DecodeError`; state transitions to `Failed`. |
| EC-4 | `delta_mode == true` + `dispatch_mode == Pull` | coordinator drives `BorderGraph::extend_with_chunk_borders` between successive `AssignPartition` messages per R37f / A7. (Cross-cuts TEST-SPEC-0588.) |

## Invariants asserted

- R30 (coordinator MUST support pull-based dispatch).
- R32 (5-step pull protocol verbatim).
- R37d (BSP barrier = all workers ack `NoMoreWork`; single logical BSP round).
- R37e (push mode UNCHANGED; pull-only states gated on `DispatchMode::Pull`).
- §3.8 A5 (SPEC-13 amendment).
- G1 (BSP determinism preserved across pull/push axis — UT-0577-12 + IT-0577-T12-01).
- T11 (pull-based dispatch protocol; full integration-level closure).
- T12 (pull-vs-push equivalence; full integration-level closure).
- T13 partial (short-stream coordinator side; full closure jointly with TASK-0578).

## ARG/DISC/REF citation

- ARG-001 G1 (BSP determinism — preserved by R37d single-logical-BSP-round reduction).
- DISC-008 v2 (sync/termination dimension — pull dispatch is the termination side).

## Determinism notes

**TOKIO SCHEDULING:** All FSM tests use `#[tokio::test(flavor = "current_thread")]` to ensure deterministic message ordering. Multi-thread runtimes would introduce `tokio::select!` race conditions on `RequestWork` / `PartitionResult` arrivals. The mock `Transport` MUST use a single-producer-single-consumer channel with FIFO ordering; do NOT use `tokio::time::sleep` to enforce ordering.

**FSM TRANSITION ATOMICITY:** State transitions are observable test-only via a `state_trace: Vec<CoordinatorState>` instrumentation appended on every `next_state()` call (gated on `#[cfg(test)]`). UT-0577-12 (push-mode pull-states-unreachable) asserts the trace contains zero pull-mode entries.

**R37e ENFORCEMENT:** UT-0577-11 / UT-0577-13 enforce the push-mode `NoMoreWork` zero-emission contract via transport-log assertion. The `debug_assert!` in UT-0577-13 fires only in debug builds; release builds rely on the FSM gating to make the path unreachable.

**R37d BSP BARRIER (G1 PRESERVATION):** UT-0577-09 / UT-0577-10 assert that mid-stream `PartitionResult` arrivals are buffered, not merged. The merge-once invariant is the operational closure of G1 under pull dispatch.

**Auto resolution rule (UT-0577-14..16):** `Auto → Push` when `chunk_size == u32::MAX` (R26 short-circuit) OR `len_estimate <= num_workers` (degenerate); `Auto → Pull` otherwise. Document the rule in code comments per TASK-0577 NOTE line 98.

## Cross-test dependencies

- **TEST-SPEC-0514 (SPEC-13 FSM amendment-level coverage)** — predecessor.
- **TEST-SPEC-0511 (SPEC-06 amendment / RequestWork-NoMoreWork wire variants)** — predecessor.
- **TEST-SPEC-0575 (wire variants production)** — predecessor; FSM emits and consumes these variants.
- **TEST-SPEC-0576 (PROTOCOL_VERSION bump production)** — predecessor; FSM handshake depends on the version constant.
- **TEST-SPEC-0565 (GridConfig dispatch_mode field)** — predecessor; FSM gates on `cfg.dispatch_mode`.
- **TEST-SPEC-0554 (orchestrator)** — predecessor; coordinator drives `make_net_stream::next` from `GeneratingNext` state.
- **TEST-SPEC-0578 (worker FSM pull-dispatch)** — sibling; pairs in review. T13 / T14 are jointly owned.
- **TEST-SPEC-T11-pull-based-dispatch-protocol** — spec-catalog mirror; this file owns integration-level closure.
- **TEST-SPEC-T12-pull-vs-push-equivalence** — spec-catalog mirror; same delegation.
- **TEST-SPEC-T13-short-stream-fewer-chunks-than-workers** — spec-catalog mirror; jointly owned with TASK-0578.
- **TEST-SPEC-T14-heterogeneous-worker-simulation** — owned primarily by TASK-0578 simulator harness; this FSM provides the coordinator side.
- **TEST-SPEC-0588 (BorderGraph extend call-site)** — consumer of the `GeneratingNext` state hook (delta+streaming combined path; EC-4).
