# TEST-SPEC EG-I3: elastic departure correctness v1 (R18-R26, G1 CLOSED via ARG-006)

**SPEC-20 §7.2 ID:** EG-I3 — **empirical signature for ARG-006 P10/P12** per `theory-bridge.md` "Open Theoretical Debts" table.
**Owning task(s):** TASK-0410, TASK-0438, TASK-0440.
**Type:** integration.
**Test name:** `test_elastic_departure_correctness_v1`.

---

## Inputs / Fixtures

- v1 mode + hybrid; `elastic_departure = true`; `retain_partitions = true`.
- Initial cohort: K_remote = 4 (K_eff = 5).
- Mid-run schedule: at the start of round 2, 1 remote worker disconnects abruptly (TCP RST or timeout).
- SPEC-09 benchmark nets parameterised: `dual_tree(depth=4)`, `ep_annihilation_con(N=16)`, `mixed_net_small`.

## Expected behaviour

R18-R26 departure cycle:
1. Detection (timeout or ConnLost) at round 2.
2. Reclaim from `retained_initial` (or `retained_last_acked` if w1 had completed at least 1 round).
3. Remap reclaimed partition to a fresh disjoint id range; rebase border_ids.
4. Union with surviving net; re-split with K_eff_new = 4 (1 self + 3 surviving remotes).
5. Continue rounds.
6. Final result matches `reduce_all`.

## Assertions

| # | Assertion |
|---|-----------|
| A1 | For each fixture, `canonicalise(final) == canonicalise(reduce_all(net))`. |
| A2 | `metrics.workers_departed_per_round.iter().sum::<u32>() == 1`. |
| A3 | Reclaim is recorded in either `retained_initial_reclaims_per_round` or `retained_last_acked_reclaims_per_round`. |
| A4 | After re-split: K_eff = 4. |
| A5 | All id ranges remain disjoint (cross-anchor TEST-SPEC-EG-U12). |
| A6 | Total interactions == `reduce_all_metrics.total_interactions`. |

## Edge / negative cases

- EC-1: departure occurs in round 1 (worker's first round) → reclaim from `retained_initial`.
- EC-2: departure occurs in round 5 (worker had 4 successful rounds) → reclaim from `retained_last_acked`.
- EC-3: departure of a worker that has an in-flight `PartitionResult` arriving moments after disconnect → late result is dropped per R31.

## Invariants asserted

- D3-elastic (R24c).
- D4-elastic (R30, R11a).
- G1 PRESERVED via ARG-006 P10/P11/P12.

## ARG/DISC/REF citation

**ARG-006 (mixed-trace recoverability) — empirical signature.** Per `theory-bridge.md` ("Open Theoretical Debts" → "Empirical signature of ARG-006 (SPEC-20 EG-I3, EG-I5a, EG-P2, EG-P5)"), this test provides the empirical evidence that the proof's P10 (idempotence) + P12 (mixed-trace recoverability) translate into observable run correctness for v1 mode.

## Determinism notes

`#[tokio::test(flavor = "current_thread", start_paused = true)]`. Worker disconnects scripted via duplex stream close at deterministic clock points.

## Cross-test dependencies

- EG-U7, EG-U7a, EG-U7b — unit-level reclaim path coverage.
- EG-I3-delta (delta-mode counterpart).
- EG-P2 (proptest version).
- EG-I5a (CON-DUP cascade departure scenario).
