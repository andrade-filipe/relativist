# TEST-SPEC-0514: SPEC-13 FSM amendment (coordinator/worker pull-mode states)

**SPEC-21 §7 ID:** plumbing only (gates T11, T12, T13, T14).
**Owning task:** TASK-0514.
**Parent spec:** SPEC-21 §3.6 R30, R32; §3.7 R37d, R37e; §3.8 A5; SC-001 (part 3) closure; SC-015 closure.
**Type:** unit (FSM transition table verification, spec-text-only at this stage).
**Theory anchor:** ARG-001 P3 (border completeness across rounds); ARG-003 R7 (multi-round cycle).

---

## Inputs / Fixtures

- The amended `CoordinatorState`, `CoordinatorEvent`, `CoordinatorAction` enums in SPEC-13 §4.
- The amended `WorkerState`, `WorkerEvent`, `WorkerAction` enums.
- A doc-test rendering of the transition tables for `DispatchMode::Pull`.
- A `GridConfig { dispatch_mode: DispatchMode::Push, .. }` baseline fixture.
- A `GridConfig { dispatch_mode: DispatchMode::Pull, .. }` test fixture.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0514-01 | `coordinator_pull_states_present` | enum `CoordinatorState` after amendment | grep for the new pull-only state(s) (e.g., `AwaitingRequestWork`, `DispatchingChunk`, or whatever names §3.8 A5 introduces) | all states named in §3.8 A5 are present in the enum. |
| UT-0514-02 | `worker_pull_states_present` | enum `WorkerState` after amendment | grep for the new pull-only state(s) (e.g., `RequestingWork`, `AwaitingChunk`) | all named states present. |
| UT-0514-03 | `coordinator_event_request_work_present` | enum `CoordinatorEvent` | grep | `RequestWorkReceived { worker_id }` variant present. |
| UT-0514-04 | `coordinator_action_send_chunk_or_no_more_work_present` | enum `CoordinatorAction` | grep | `SendChunk { worker_id, chunk }` and `SendNoMoreWork { worker_id }` variants present. |
| UT-0514-05 | `push_mode_fsm_unchanged` | the existing v1 push-mode transitions | diff against pre-SPEC-21 baseline | byte-for-byte unchanged (R37e). The amendment is additive only. |
| UT-0514-06 | `pull_mode_states_gated_on_dispatch_mode` | the FSM evaluator with `DispatchMode::Push` | attempt a transition into a pull-only state | the evaluator MUST refuse / panic / return error (gate enforcement; pull-only states are unreachable under Push). |
| UT-0514-07 | `bsp_barrier_under_pull_documented` | SPEC-13 §4 prose | grep for "BSP barrier" near the pull-mode transition table; cross-ref to SPEC-21 R37d | substring present; R37d explicitly named. |
| UT-0514-08 | `push_mode_termination_unchanged` | SPEC-13 prose for push-mode termination | grep for "no `NoMoreWork`" or equivalent disclaimer | substring present (R37e: push-mode termination uses the existing `Result` collection path, not `NoMoreWork`). |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | A worker in `AwaitingChunk` receives `Result` (out-of-order message) | the FSM MUST treat this as a protocol error or buffer per §4 specification; behavior MUST be deterministic (no race). |
| EC-2 | The coordinator transitions `DispatchingChunk → AwaitingRequestWork` then receives `RequestWork` from a different worker | the request MUST be queued; the dispatch sequence remains FIFO per §4. |
| EC-3 | All workers send `RequestWork` simultaneously when only 1 chunk remains | the coordinator dispatches the chunk to ONE worker (deterministic ordering: lowest `WorkerId` first) and sends `NoMoreWork` to the remaining workers (R35). |

## Invariants asserted

- R30, R32 (pull dispatch protocol).
- R37d (BSP barrier under pull dispatch — closes SC-019).
- R37e (push-mode termination — closes SC-013).
- D5 (Exclusive Ownership) — preserved via chunk-to-worker assignment tracking.
- D6 (Protocol termination) — preserved via R37d BSP barrier semantics.

## ARG/DISC/REF citation

- ARG-001 P3 (border completeness across rounds).
- ARG-003 R7 (multi-round cycle).

## Determinism notes

Pure spec-text gate at this stage. The FSM transition behavioral tests are TEST-SPEC-0577 / TEST-SPEC-0578 (forward-referenced from TASK-0514, NOT in scope for the current Stage 2 wave). UT-0514-06 / EC-3 require deterministic ordering (lowest-WorkerId-first); the implementing test code MUST use a controlled tokio runtime (`#[tokio::test(flavor = "current_thread")]`) when exercising this in a behavioral test, never `flavor = "multi_thread"` (which would make ordering racy).

## Cross-test dependencies

- TEST-SPEC-0577, TEST-SPEC-0578 (FSM behavioral) — out of scope for Stage 2 wave 1; flagged for wave 2.
- TEST-SPEC-T11, T12, T13, T14 (pull-protocol behavioral) — depend on this FSM amendment.
- TEST-SPEC-EG-U18 (initial-wait FSM in SPEC-20) — share the FSM evaluator pattern; ensure no enum-variant collision when SPEC-20 + SPEC-21 land together.
