# TEST-SPEC EG-P2: prop departure normal form invariant v1 (G1 CLOSED via ARG-006) — empirical signature for ARG-006

**SPEC-20 §7.3 ID:** EG-P2 — **empirical signature for ARG-006 P10/P12** per `theory-bridge.md`.
**Owning task(s):** TASK-0440 (transitive consumer; departure orchestrator).
**Type:** property (proptest).
**Test name:** `prop_departure_normal_form_invariant_v1`.

---

## Generators

- `arb_terminating_net()` — same as EG-P1.
- `arb_k_initial()` — `2..=6`.
- `arb_departure_schedule(k_initial)` — vector of `(round_number, departing_worker_index)` pairs:
  - At least 1 entry; up to 3.
  - `round_number ∈ [1, 5]`.
  - `departing_worker_index < k_initial - departures_so_far` (so we never depart more workers than exist).
- `arb_departure_kind()` — one of `{Timeout, ConnLost, LeaveUrgent}` (LeaveAfter would simply remove the worker without reclaim — covered by EG-P4).

## Property statement

For all `(net, k_initial, departure_schedule, kinds)`:
```
canonicalise(reduce_all(net)) == canonicalise(run_grid_with_scripted_departures(
    net, GridConfig{ hybrid_coordinator: true, elastic_departure: true, retain_partitions: true, ..defaults },
    k_initial, departure_schedule, kinds
))
```

## Assertions

| # | Assertion |
|---|-----------|
| A1 | `prop_assert_eq!(canonicalise(local), canonicalise(distributed_with_departures))`. |
| A2 | `prop_assert_eq!(local_total_interactions, distributed_total_interactions)`. |
| A3 | `prop_assert_eq!(metrics.workers_departed_per_round.iter().sum::<u32>(), departure_schedule.len() as u32)`. |

## Shrinking

- Reduce departure count to minimum failing.
- Reduce K_initial.
- Reduce net size.
- Reduce departure round number.

## Configuration

- `cases: 128` (smaller than EG-P1 because each case is more expensive — full BSP run + scripted departures).
- Honor `PROPTEST_RNG_SEED`.
- `proptest-regressions/` for failing-case replay.

## Edge / negative cases

- EC: `D == K_eff` (full departure) — generator may produce this; assert solo fallback or Error per EG-U9 branches.

## Invariants asserted

- D3-elastic, D4-elastic.
- G1 PRESERVED via ARG-006.

## ARG/DISC/REF citation

**ARG-006** — empirical signature. Per `theory-bridge.md` "Open Theoretical Debts" → "Empirical signature of ARG-006 (SPEC-20 EG-I3, EG-I5a, EG-P2, EG-P5)", EG-P2 is the canonical proptest witness for the mixed-trace recoverability proof under v1 mode.

## Determinism notes

Same as EG-P1 (proptest seeded; tokio runtime current_thread + start_paused inside each case). Worker scripts deterministic given seed. Departure timing scripted via manual clock advance keyed off the seed-derived schedule.

## Cross-test dependencies

- EG-I3 (fixed-fixture integration counterpart; shares the ARG-006 anchor).
- EG-P5 (CON-DUP heavy proptest).
