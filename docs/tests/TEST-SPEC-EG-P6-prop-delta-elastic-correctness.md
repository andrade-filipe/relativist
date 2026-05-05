# TEST-SPEC EG-P6: prop delta-elastic correctness (R0a, G1 CONDITIONAL, SC-001)

**SPEC-20 §7.3 ID:** EG-P6
**Owning task(s):** TASK-0443.
**Type:** property (proptest).
**Test name:** `prop_delta_elastic_correctness`.

---

## Generators

- `arb_terminating_net()`.
- `arb_membership_schedule()` (joins + departures).
- Fixed mode: `delta_mode = true`, `elastic_join = true`, `elastic_departure = true`.

## Property statement

For all `(net, schedule)` in pure delta + elastic mode:
```
canonicalise(reduce_all(net)) == canonicalise(run_grid_delta(
    net, build_config(delta_mode=true, elastic_*=true),
    schedule
))
```

## Assertions

| # | Assertion |
|---|-----------|
| A1 | `prop_assert_eq!(canonicalise(local), canonicalise(distributed))`. |
| A2 | The conservative path is exercised unconditionally (R24a-delta + R23b-delta). |
| A3 | Optimized path: gated on ARG-005 status per `theory-bridge.md`. |

## Shrinking

- Disable elastic_join first.
- Disable elastic_departure.
- Reduce net size.

## Configuration

- `cases: 96`.
- Honor `PROPTEST_RNG_SEED`.

## Edge / negative cases

- EC: a join + departure in the same window in delta mode — both paths exercised together.

## Invariants asserted

- D3, D4-elastic.
- G1 CLOSED for conservative; CONDITIONAL on ARG-005 for optimized.

## ARG/DISC/REF citation

ARG-005, ARG-006.

## Determinism notes

Per-case tokio runtime current_thread + start_paused. Seeded.

## Cross-test dependencies

EG-I1-delta, EG-I2-delta, EG-I3-delta.
