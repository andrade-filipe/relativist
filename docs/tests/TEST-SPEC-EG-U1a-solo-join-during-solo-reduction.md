# TEST-SPEC EG-U1a: solo-join during solo reduction

**SPEC-20 §7.1 ID:** EG-U1a
**Owning task(s):** TASK-0425 (solo loop), TASK-0436 (FSM).
**Type:** unit (FSM transition exercised under simulated time).
**Test name:** `test_solo_join_during_solo_reduction`.

**Requirement:** R5/R5a (solo budget loop), R15 (`SoloReducing × WorkerJoined → CheckTermination`), SC-009 (closes solo-mode preemption).

---

## Inputs / Fixtures

- `GridConfig { hybrid_coordinator: true, elastic_join: true, solo_budget: 1000, .. }`.
- A net large enough that solo reduction needs ≥ 3 batches.
- A scripted "fake worker" that connects and sends `JoinRequest{ version: 4 }` after exactly 1 batch has completed (or "after exactly k batches" parameterised).

## Expected behaviour

1. Coordinator enters `SoloReducing` (no remote workers connected within `initial_wait_timeout`).
2. After batch 1 emits `SoloReduceBatchComplete`, the fake worker is mid-flight; the FSM observes `WorkerJoined` while between batches.
3. R15: `SoloReducing × WorkerJoined → CheckTermination`. The current batch is allowed to complete (per R5a), then the FSM transitions, NOT mid-batch.
4. After CheckTermination, the FSM transitions to `AcceptingMembershipChanges` then `Partitioning` for K_eff=2 (self + new worker).
5. Subsequent rounds run as hybrid grid until the net reaches normal form.

## Assertions

| # | Assertion |
|---|-----------|
| A1 | The FSM trace shows: `WaitingForWorkers → SoloReducing → CheckTermination → AcceptingMembershipChanges → Partitioning → WaitingForResults → ...`. |
| A2 | `WorkerJoined` arrives WHILE the FSM is in `SoloReducing` (assert via captured event log). |
| A3 | The current batch DOES complete after `WorkerJoined` (assert `metrics.solo_reduce_batches_completed >= 2` even though the worker arrived between batch 1 and 2). |
| A4 | Final result `canonicalise(out) == canonicalise(reduce_all(net))`. |
| A5 | `metrics.workers_joined_per_round.iter().sum::<u32>() == 1`. |

## Edge / negative cases

- EC-1: `WorkerJoined` arrives DURING a batch — the batch finishes, then the transition fires (no mid-batch preemption per R5a).
- EC-2: `WorkerJoined` arrives AFTER local normal form is reached — the FSM has already exited `SoloReducing` via `SoloReductionComplete`; this case is a different path (not covered by EG-U1a; document the cross-reference).
- EC-3: `solo_budget = 1` (extremely small) — the FSM should still complete a single redex per batch and observe the join between two such tiny batches.

## Invariants asserted

- D6 (Protocol Termination): the protocol completes for both solo and grid phases.
- G1: end-to-end correctness preserved through phase transition.

## ARG/DISC/REF citation

None.

## Determinism notes

**Critical: this test depends on tokio scheduling.** Strategy:
- `#[tokio::test(flavor = "current_thread", start_paused = true)]`.
- Use `tokio::time::advance` (manual clock) to deterministically position the join request "between batches".
- The fake worker is a `tokio::io::duplex` end fed messages via a script that respects the controlled clock.
- All `WorkerJoined` event delivery is via an async channel whose queue is drained between explicit `tokio::yield_now().await`s in the test driver.
- Result canonicalisation is deterministic.

## Cross-test dependencies

- Shares the simulated-clock pattern with EG-U6a, EG-U18, EG-U13.
- TASK-0425 (solo loop) is the prerequisite; TASK-0436 (FSM) provides the transition table.
