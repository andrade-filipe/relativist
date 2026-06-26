# TEST-SPEC EG-U18: `initial_wait_timeout` supersedes `worker_connect_timeout` (R6, SC-020)

**SPEC-20 §7.1 ID:** EG-U18
**Owning task(s):** TASK-0413 (SPEC-06 amendment), TASK-0425 (solo loop), TASK-0436 (FSM).
**Type:** unit.
**Test name:** `test_initial_wait_timeout_supersedes_worker_connect_timeout`.

---

## Inputs / Fixtures

- `GridConfig { hybrid_coordinator: true, initial_wait_timeout: Duration::from_secs(30), worker_connect_timeout: Duration::from_secs(120), .. }`.
- K_remote=0 (no workers connect).

## Expected behaviour

R6 (MUST): when `hybrid_coordinator = true`, `initial_wait_timeout` (30s) supersedes `worker_connect_timeout` (120s). At t=30s, the coordinator transitions to `SoloReducing` and starts solo reduction. The `worker_connect_timeout` (120s) is NOT used at all in hybrid mode — solo can begin earlier.

## Assertions

| # | Assertion |
|---|-----------|
| A1 | Using a paused tokio clock, advance to t=29s — FSM is still in `WaitingForWorkers`. |
| A2 | Advance to t=30s exactly — FSM transitions to `SoloReducing`. |
| A3 | `metrics.solo_reduce_started_at` records 30s, not 120s. |
| A4 | The 120s `worker_connect_timeout` timer is NOT armed in hybrid mode (no `TimerKind::WorkerConnect` ever fires; assert via captured timer events). |
| A5 | When `hybrid_coordinator = false` (control case), at t=30s the FSM is still `WaitingForWorkers`; at t=120s it transitions to `Error` per the v1 path. |

## Edge / negative cases

- EC-1: `initial_wait_timeout = 0s` — FSM transitions immediately.
- EC-2: a worker connects at t=15s — FSM transitions to grid mode normally; the timer is cancelled and not relevant.
- EC-3: `initial_wait_timeout > worker_connect_timeout` (e.g., 200s vs 120s) — R6 forces R6's value to win in hybrid mode regardless of ordering. Document and test.

## Invariants asserted

None directly.

## ARG/DISC/REF citation

None.

## Determinism notes

`#[tokio::test(flavor = "current_thread", start_paused = true)]`. All timer transitions driven by manual `tokio::time::advance` calls.

## Cross-test dependencies

EG-U1a (solo with join) builds on this transition.
