# TEST-SPEC EG-U8: multiple-worker departure v1 (R26)

**SPEC-20 §7.1 ID:** EG-U8
**Owning task(s):** TASK-0440.
**Type:** unit.
**Test name:** `test_departure_multiple_workers_v1`.

---

## Inputs / Fixtures

- v1 mode + hybrid; K_remote=4 (K_eff=5).
- 2 of the 4 workers depart simultaneously between rounds (e.g., both timeout in the same `collect_timeout` window).

## Expected behaviour

R26: multiple simultaneous departures are coalesced into a SINGLE re-split cycle. K_eff_new = K_eff - 2 = 3 (1 self + 2 surviving remote). Both reclaimed partitions are remapped to disjoint ranges and unioned into the surviving net before re-split.

## Assertions

| # | Assertion |
|---|-----------|
| A1 | `metrics.workers_departed_per_round[k] == 2` (in the affected round). |
| A2 | `metrics.retained_initial_reclaims_per_round[k] + retained_last_acked_reclaims_per_round[k] == 2`. |
| A3 | Exactly ONE re-split call is invoked (not two sequential re-splits). |
| A4 | K_eff after re-split == 3. |
| A5 | All `border_id` ranges across the new K_eff=3 partitions are disjoint. |
| A6 | `canonicalise(final) == canonicalise(reduce_all(input))`. |

## Edge / negative cases

- EC-1: 2 departures arrive at slightly different times, both within the same `collect_timeout` window → still coalesced into one re-split.
- EC-2: 2 departures, second arrives AFTER re-split for the first has already begun — second triggers ANOTHER re-split (sequential), which is suboptimal but correct.
- EC-3: 2 departures with one being a `LeaveRequest{Urgent}` and one being a timeout — same path, same coalescing.

## Invariants asserted

- D3-elastic (R24c).
- D4-elastic (R30, R11a).
- G1 PRESERVED via ARG-006.

## ARG/DISC/REF citation

ARG-006.

## Determinism notes

`#[tokio::test(flavor = "current_thread", start_paused = true)]`. Both worker streams scripted; departures scheduled via manual clock advance to fire in the same window.

## Cross-test dependencies

EG-U9 (D == K_eff edge case), EG-U7 (single departure).
