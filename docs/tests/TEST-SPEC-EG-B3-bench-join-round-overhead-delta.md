# TEST-SPEC EG-B3: bench join-round overhead delta (R12a, R12-delta)

**SPEC-20 §7.4 ID:** EG-B3
**Owning task(s):** TASK-0446, TASK-0450.
**Type:** benchmark.
**Bench name:** `bench_join_round_overhead_delta`.

---

## Inputs / Fixtures

- A terminating net at medium size (e.g. `dual_tree(depth=5)`).
- `delta_mode = true`; hybrid + `elastic_join = true`.
- K_remote_initial = 2.
- Schedule: 1 join after round 2 completes.
- Two configurations:
  - **Delta-rejoin (R12-delta)**: 1 join triggers `FinalStateRequest` cycle.
  - **No-join control**: same net, same K_remote, no joiner; runs to completion uninterrupted.

## Metrics measured

- Wall-clock for the affected round (the round including the FinalStateRequest cycle vs the equivalent round without the cycle).
- `metrics.join_round_overhead_ms_per_round` for the affected round.
- Total wall-clock for the full run (control vs delta-rejoin).

## Pass / fail criteria

Comparative; reports the overhead `c_o_join` of the FinalStateRequest cycle. ROADMAP §2.40 break-even analysis consumes this number.

The bench:
- **Asserts correctness:** both configs produce `canonicalise(final) == canonicalise(reduce_all(net))`.
- **Reports** `c_o_join_ms = wall_clock(delta_rejoin_round) - wall_clock(equivalent_control_round)`.
- **Reports** `c_o_join_per_K_eff_byte` = c_o_join / (sum of partition payload bytes transferred during FinalStateRequest cycle).
- **Asserts** `c_o_join_ms > 0` (cycle is observably non-free).
- **Asserts** `metrics.join_round_overhead_ms_per_round[k]` matches the externally-measured wall-clock to within ±10% (sanity check on the metric).

## Assertions

| # | Assertion |
|---|-----------|
| A1 | Both configs produce correct final result. |
| A2 | `c_o_join_ms > 0`. |
| A3 | `metrics.join_round_overhead_ms_per_round[k] ≈ measured_c_o_join_ms (±10%)`. |
| A4 | Total interactions across both runs match. |

## Edge / negative cases

- EC: 2 simultaneous joins → still ONE FinalStateRequest cycle; report `c_o_join` for that single cycle.
- EC: a join triggers FinalStateRequest cycle in a round that already has > N redexes — the cycle cost dominates; report c_o_join in absolute and relative terms.

## Invariants asserted

None directly.

## ARG/DISC/REF citation

ARG-004 (break-even).

## Determinism notes

Wall-clock not deterministic; medians reported over 10 repetitions.

## Cross-test dependencies

- TASK-0446 (delta-rejoin cycle).
- TASK-0450 (`join_round_overhead_ms_per_round` metric).
- EG-I2-delta (correctness).
