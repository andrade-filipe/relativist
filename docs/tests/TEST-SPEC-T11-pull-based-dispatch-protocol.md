# TEST-SPEC-T11: Pull-based dispatch protocol

**SPEC-21 §7.5 ID:** T11.
**Owning task:** TASK-0511 + TASK-0514 + future TASK-0579 (out of scope wave 1).
**Parent spec:** SPEC-21 §3.6 R30, R31, R32; §7.5 T11.
**Type:** integration.
**Theory anchor:** ARG-001 G1.

---

## Inputs / Fixtures

- `ep_annihilation_con(100)`, K=2 workers; `DispatchMode::Pull`.
- A controlled tokio current_thread runtime for deterministic message ordering.

## Unit Tests

| ID | Test | Then |
|----|------|------|
| UT-T11-01 | Run `run_grid` with `DispatchMode::Pull`, ep_annihilation_con(100), K=2 | completes successfully. |
| UT-T11-02 | Verify workers send `RequestWork` messages | trace logs / mock-based observation: each worker emits at least one `RequestWork`. |
| UT-T11-03 | Verify coordinator dispatches chunks via `AssignPartition` (or whatever R32 step-3 mandates) in response | per RequestWork, the coordinator responds with a chunk OR `NoMoreWork`. |
| UT-T11-04 | G1: result matches sequential baseline | `is_behaviorally_equal(pull_result, reduce_all(make_net(ep_annihilation_con(100))))`. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | A worker disconnects mid-pull | covered by SPEC-20 elastic-departure tests; out of scope here. |
| EC-2 | Both workers send RequestWork simultaneously | coordinator dispatches deterministically (lowest WorkerId first per FSM amendment EC-3). |

## Invariants asserted

- R30, R31, R32 (pull dispatch protocol).
- G1 (result matches sequential).

## Determinism notes

`#[tokio::test(flavor = "current_thread")]`. Multi-thread runtime would add scheduling non-determinism. The test MUST NOT use `tokio::time::sleep` for synchronization; use explicit `await`-pointed sequencing.

## Cross-test dependencies

- **TEST-SPEC-0511** (Message enum amendment) — RequestWork / NoMoreWork variants.
- **TEST-SPEC-0514** (FSM amendment) — pull-mode states.
- TEST-SPEC-0577 / TEST-SPEC-0578 (FSM behavioral tests) — out of scope wave 1.
