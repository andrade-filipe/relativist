# TEST-SPEC-T14: Heterogeneous worker simulation

**SPEC-21 §7.5 ID:** T14.
**Owning task:** TASK-0514 + future TASK-0584 (out of scope wave 1).
**Parent spec:** SPEC-21 §3.6 R30-R35; §7.5 T14; ARG-004 V1 (efficiency-shaped, correctness-unconditional).
**Type:** integration.
**Theory anchor:** ARG-001 G1; ARG-004 (feasibility profiles).

---

## Inputs / Fixtures

- 4 workers; worker 0 reduces 2× faster than the others (achieved by either smaller chunks for worker 0, or injected `tokio::time::sleep` delays in workers 1-3).
- `DispatchMode::Pull`.
- `ep_annihilation_con(200)` or equivalent fixture.

## Unit Tests

| ID | Test | Then |
|----|------|------|
| UT-T14-01 | Run with simulated heterogeneity | worker 0 processes more chunks than worker 3 (worker 0 sends more RequestWork messages). |
| UT-T14-02 | Result correctness | `is_behaviorally_equal(heterogeneous_result, sequential_baseline) == true`. |
| UT-T14-03 | All 4 workers contribute to the result | every chunk is processed by exactly one worker; no chunk is lost. |
| UT-T14-04 | Total chunk count = ceil(total_agents / chunk_size) | counted chunks match the math. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Worker 0 100× faster than others | extreme imbalance; worker 0 processes ~all chunks; correctness still holds. |
| EC-2 | All workers identical speed | uniform distribution; correctness holds (degenerate to T11/T12). |

## Invariants asserted

- G1 (correctness independent of speed distribution).
- ARG-004 V1 (efficiency-shaped, correctness-unconditional).

## Determinism notes

**This test is NOT a wall-clock test.** Heterogeneity simulation MUST use deterministic mechanisms (e.g., `tokio::time::pause()` + `tokio::time::advance(...)` for controlled time progression, OR per-worker chunk-count-multipliers). Wall-clock-based heterogeneity (real `tokio::time::sleep`) introduces flake risk; the test MUST avoid it.

UT-T14-01 chunk-count assertion (worker 0 > worker 3) is order-of-magnitude, not exact: `worker_0_chunks > worker_3_chunks`. The exact ratio is workload-dependent.

## Cross-test dependencies

- **TEST-SPEC-T11** (pull baseline).
- **TEST-SPEC-T13** (short-stream extreme).
- **TASK-0584** (out of scope wave 1) — full T14 with profiling.
