# TEST-SPEC EG-P4: prop full-matrix correctness (G1 CONDITIONAL, SC-015)

**SPEC-20 §7.3 ID:** EG-P4
**Owning task(s):** TASK-0455 (transitive — regression gate).
**Type:** property (proptest, full matrix).
**Test name:** `prop_full_matrix_correctness`.

---

## Generators

- `arb_terminating_net()`.
- `arb_hybrid()` — bool.
- `arb_strict_bsp()` — bool.
- `arb_delta_mode()` — bool.
- `arb_join_schedule(k_initial)` — vector of `(round, count)` pairs, may be empty.
- `arb_leave_schedule(k_initial)` — vector of `(round, worker_idx, kind)` triples, may be empty.
- `arb_k_initial()` — `1..=6`.

The full Cartesian-product space for one generated case: `(net, hybrid, strict_bsp, delta_mode, k_initial, join_schedule, leave_schedule)`.

## Property statement

For all generated tuples (within terminating-nets scope):
```
canonicalise(reduce_all(net)) == canonicalise(run_grid_full_matrix(
    net, build_config(hybrid, strict_bsp, delta_mode, ...),
    k_initial, join_schedule, leave_schedule
))
```

## Assertions

| # | Assertion |
|---|-----------|
| A1 | `prop_assert_eq!(canonicalise(local), canonicalise(distributed))` for every case. |
| A2 | `prop_assert!(metrics.workers_joined_per_round.iter().sum::<u32>() as usize == join_schedule.iter().map(|(_,n)| *n as usize).sum::<usize>())`. |
| A3 | `prop_assert!(metrics.workers_departed_per_round.iter().sum::<u32>() as usize == leave_schedule.len())`. |

## Shrinking

- Disable elastic schedules first (faster failure isolation).
- Reduce K_initial.
- Reduce net size.
- Disable strict_bsp / delta_mode one at a time.

## Configuration

- `cases: 64` (each case is expensive — large matrix; full BSP run with scripted membership changes).
- Honor `PROPTEST_RNG_SEED`.

## Edge / negative cases

- EC: `delta_mode=true, optimized R24b path` — gated on ARG-005 status; `#[ignore]` if pending per `theory-bridge.md`.

## Invariants asserted

- G1 CONDITIONAL (per matrix mode).
- D3-elastic, D4-elastic.

## ARG/DISC/REF citation

ARG-001, ARG-005 (gates the delta-optimized path), ARG-006 (gates departure paths).

## Determinism notes

Per-case tokio runtime: `current_thread + start_paused`. Scripts deterministic given seed.

## Cross-test dependencies

EG-P1, EG-P2, EG-P5, EG-P6 — each is a focused subset of this matrix.
