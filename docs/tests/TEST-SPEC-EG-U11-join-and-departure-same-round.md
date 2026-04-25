# TEST-SPEC EG-U11: join + departure same round (§4.2.3, R26)

**SPEC-20 §7.1 ID:** EG-U11
**Owning task(s):** TASK-0435, TASK-0447, TASK-0436, TASK-0451.
**Type:** unit.
**Test name:** `test_join_and_departure_same_round`.

---

## Inputs / Fixtures

- Hybrid; K_remote=2 (K_eff=3); v1 mode.
- 1 worker joins AND 1 worker departs in the same `AcceptingMembershipChanges` window.

## Expected behaviour

§4.2.3 commits to a single re-split cycle per window. Per R26, the changes are coalesced:
- Net K_eff change: `K_eff_new = K_eff_old + 1 (join) - 1 (depart) = 3`.
- Reclaim the departed worker's partition.
- Allocate WorkerId to the joiner.
- Re-split into K_eff_new partitions.
- Dispatch.

## Assertions

| # | Assertion |
|---|-----------|
| A1 | `metrics.workers_joined_per_round[k] == 1`. |
| A2 | `metrics.workers_departed_per_round[k] == 1`. |
| A3 | K_eff_new == K_eff_old (in this case 3). |
| A4 | The single round-2 dispatch contains 3 partitions, one of which is destined for the new joiner. |
| A5 | The departed worker's `retained_*` is released after reclaim. |
| A6 | `canonicalise(final) == canonicalise(reduce_all(input))`. |

## Edge / negative cases

- EC-1: 2 joins + 1 departure → K_eff_new = K_eff + 1.
- EC-2: 1 join + 2 departures → K_eff_new = K_eff - 1.
- EC-3: join arrives ε before departure → both fall in the same window per R10b; same coalescing.

## Invariants asserted

- D3-elastic (R24c).
- D4-elastic (R11a, R30).
- G1 PRESERVED via ARG-006 (departure side); ARG-001 P2 (join side).

## ARG/DISC/REF citation

ARG-006 (departure path).

## Determinism notes

`#[tokio::test(flavor = "current_thread", start_paused = true)]`. Both events scripted to fire within the same window via manual clock advance.

## Cross-test dependencies

- EG-I4 (churn integration test).
- EG-U6/U6a/U7/U8.
