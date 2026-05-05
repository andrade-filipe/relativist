# TEST-SPEC EG-I2-delta: elastic join correctness delta (R9-R14, R12-delta, G1 CONDITIONAL)

**SPEC-20 §7.2 ID:** EG-I2-delta
**Owning task(s):** TASK-0446.
**Type:** integration.
**Test name:** `test_elastic_join_correctness_delta`.

---

## Inputs / Fixtures

Same as EG-I2 but with `delta_mode = true`. The mid-run joins exercise the FinalStateRequest cycle (R12-delta).

## Expected behaviour

After each join: coordinator broadcasts `FinalStateRequest`, collects final partitions from all active workers, reconstructs the full net, re-splits, dispatches `InitialPartition` to all (incl. joiner). Reduction continues; final result matches `reduce_all`.

## Assertions

| # | Assertion |
|---|-----------|
| A1 | Final result matches `reduce_all` for every fixture. |
| A2 | Each join triggers exactly one FinalStateRequest cycle (verify via captured FSM trace). |
| A3 | `metrics.join_round_overhead_ms_per_round` accumulates positive entries on rounds following each join. |
| A4 | The joining worker receives `InitialPartition`, NOT a delta. |
| A5 | After each cycle, `BorderGraph` is fully consistent (cross-check via `bg.detect_border_redexes()` returning the same set as the post-reconstruct exhaustive scan). |

## Edge / negative cases

- EC-1: a worker times out during `FinalStateRequest` collection → treated as a departure; integration with departure path; assert correctness preserved.
- EC-2: 2 joins arrive simultaneously → still one FinalStateRequest cycle; K_eff_new = K_eff_old + 2.

## Invariants asserted

- D3 (Border Completeness via SPEC-19 R39).
- G1 CONDITIONAL on ARG-005 for the optimized R24b-delta path; CLOSED for conservative.

## ARG/DISC/REF citation

ARG-005.

## Determinism notes

`#[tokio::test(flavor = "current_thread", start_paused = true)]`.

## Cross-test dependencies

EG-U5-delta, EG-B3 (cost benchmark for FinalStateRequest).
