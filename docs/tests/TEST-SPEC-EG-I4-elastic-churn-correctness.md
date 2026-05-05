# TEST-SPEC EG-I4: elastic churn correctness (R9-R30, G1 CONDITIONAL → CLOSED for v1/conservative via ARG-006)

**SPEC-20 §7.2 ID:** EG-I4
**Owning task(s):** TASK-0447.
**Type:** integration.
**Test name:** `test_elastic_churn_correctness`.

---

## Inputs / Fixtures

- v1 mode + hybrid; `elastic_join = true`; `elastic_departure = true`; `retain_partitions = true`.
- Initial K_remote = 2 (K_eff = 3).
- Scripted churn schedule across rounds:
  - Round 2: +3 joins (K_eff → 6).
  - Round 4: -2 departures (K_eff → 4).
  - Round 5: +1 join (K_eff → 5).
- One scripted run per benchmark net: `dual_tree(depth=5)`, `mixed_net_medium`.

## Expected behaviour

The full churn schedule executes; reduction proceeds; final result matches `reduce_all`.

## Assertions

| # | Assertion |
|---|-----------|
| A1 | `canonicalise(final) == canonicalise(reduce_all(net))` for each fixture. |
| A2 | `metrics.workers_joined_per_round.iter().sum::<u32>() == 4`. |
| A3 | `metrics.workers_departed_per_round.iter().sum::<u32>() == 2`. |
| A4 | The K_eff trajectory matches `[3, 3, 6, 6, 4, 5]` (round-by-round). |
| A5 | Total interactions == `reduce_all_metrics.total_interactions`. |

## Edge / negative cases

- EC-1: a join + departure in the same window (cross-link EG-U11).
- EC-2: a worker joins, then leaves, then rejoins (with a fresh WorkerId per R11) — same workload still terminates correctly.

## Invariants asserted

- D3-elastic, D4-elastic.
- G1 PRESERVED via ARG-006 (departure side); ARG-001 (join + reduce side).

## ARG/DISC/REF citation

ARG-006 (departure side; CLOSED for v1).

## Determinism notes

`#[tokio::test(flavor = "current_thread", start_paused = true)]`. All churn events scripted via manual clock advance.

## Cross-test dependencies

EG-U11, EG-I2, EG-I3, EG-P4 (proptest version).
