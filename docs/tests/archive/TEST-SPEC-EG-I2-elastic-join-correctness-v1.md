# TEST-SPEC EG-I2: elastic join correctness v1 (R9-R14, R12-v1, G1)

**SPEC-20 §7.2 ID:** EG-I2
**Owning task(s):** TASK-0433.
**Type:** integration.
**Test name:** `test_elastic_join_correctness_v1`.

---

## Inputs / Fixtures

- v1 mode + hybrid + `elastic_join = true`.
- Initial K_remote = 1 (so K_eff = 2).
- After round 1 completes, 2 additional workers join sequentially.
- Several SPEC-09 benchmark nets (parameterised).

## Expected behaviour

After two joins, K_eff_new = 4 (1 self + 3 remote). The reduction continues across rounds and produces a final result equal to `reduce_all`.

## Assertions

| # | Assertion |
|---|-----------|
| A1 | For each net, `canonicalise(final) == canonicalise(reduce_all(net))`. |
| A2 | Round 1: `metrics.effective_slots_per_round[0] == 2`. |
| A3 | Round 2 (after 2 joins): `metrics.effective_slots_per_round[1] == 4`. |
| A4 | `metrics.workers_joined_per_round.iter().sum::<u32>() == 2`. |
| A5 | Total interactions across the run == `reduce_all_metrics.total_interactions`. |

## Edge / negative cases

- EC-1: 2 joins arrive together → K_eff jumps from 2 to 4 in one cycle. Verify single re-split.
- EC-2: a join arrives mid-round-1 → buffered; drained at the round-1/round-2 boundary (cross-anchor EG-U6).
- EC-3: net is small enough that round 1 reaches normal form before any join arrives → joins are silently absorbed; the run terminates.

## Invariants asserted

- D4-elastic (R11a, R30).
- G1 PRESERVED.

## ARG/DISC/REF citation

None.

## Determinism notes

`#[tokio::test(flavor = "current_thread", start_paused = true)]`. Joiner connect timing scripted via manual clock advance.

## Cross-test dependencies

EG-U5 (single-join unit test); EG-I2-delta (delta counterpart); EG-I4 (full churn).
