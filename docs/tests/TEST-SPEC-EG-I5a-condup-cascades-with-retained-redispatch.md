# TEST-SPEC EG-I5a: CON-DUP cascades with retained re-dispatch (R24c, SC-005, SC-015) — empirical signature for ARG-006

**SPEC-20 §7.2 ID:** EG-I5a — **empirical signature for ARG-006 P10/P12** per `theory-bridge.md`.
**Owning task(s):** TASK-0410, TASK-0440.
**Type:** integration.
**Test name:** `test_condup_cascades_with_retained_redispatch`.

---

## Inputs / Fixtures

- v1 mode + hybrid; `elastic_departure = true`; `retain_partitions = true`.
- A CON-DUP-heavy net constructed to maximise emergent border redexes (use SPEC-09's `ep_annihilation_con(N=24)` or a hand-crafted CON-DUP cascade fixture).
- Scripted departure: in the middle of a CON-DUP cascade (round 2), one of the workers carrying part of the cascade disconnects.

## Expected behaviour

R24c (D3-elastic) ensures the reclaimed partition is re-introduced via re-`split` at a CLEAN round boundary, NEVER mid-cascade. The cascade continues correctly across the K_eff change. Final result matches `reduce_all`.

## Assertions

| # | Assertion |
|---|-----------|
| A1 | `canonicalise(final) == canonicalise(reduce_all(net))`. |
| A2 | The reclaim happens between rounds (assert via captured FSM trace: no transition into `Merge` at the moment of reclaim). |
| A3 | After reclaim, the CON-DUP cascade resumes and reaches the same emergent normal form as the local run. |
| A4 | `total_interactions == reduce_all_metrics.total_interactions`. |
| A5 | No partition contains a "mid-cascade" agent that was reduced in two different rounds (assert via instrumentation that each agent is touched exactly the number of times the local trace touches it). |

## Edge / negative cases

- EC-1: 3 simultaneous departures during a cascade → coalesced re-split (cross-anchor EG-U8); cascade continues.
- EC-2: the departure causes the only worker carrying part of the cascade to disconnect; reclaim is the ONLY way to recover that part.

## Invariants asserted

- D3-elastic (R24c).
- G1 PRESERVED via ARG-006 P10 (idempotence) + P12 (mixed-trace recoverability).

## ARG/DISC/REF citation

**ARG-006** — empirical signature; per `theory-bridge.md` "Open Theoretical Debts" table, EG-I5a is the canonical CON-DUP-heavy empirical witness for the mixed-trace recoverability proof.

## Determinism notes

`#[tokio::test(flavor = "current_thread", start_paused = true)]`. Departure timing scripted to land at the precise moment a CON-DUP redex is in flight.

## Cross-test dependencies

- EG-I3 (general departure integration).
- EG-P5 (CON-DUP heavy property test).
