# TEST-SPEC EG-P5: prop CON-DUP heavy churn (G1 CLOSED for v1/conservative via ARG-006; CONDITIONAL on ARG-005 for delta-optimized) — empirical signature for ARG-006

**SPEC-20 §7.3 ID:** EG-P5 — **empirical signature for ARG-006 P10/P12** per `theory-bridge.md`.
**Owning task(s):** TASK-0443 (delta departure consumer; the optimized variant gates ARG-005).
**Type:** property (proptest).
**Test name:** `prop_condup_heavy_churn`.

---

## Generators

- `arb_condup_heavy_net()` — strategy parameterised on the `ep_annihilation_con(N)` generator from SPEC-09, with `N ∈ [4, 24]`. Output is a CON-DUP-heavy net guaranteed to terminate.
- `arb_membership_schedule()` — interleaves joins (1-3) and departures (1-3) across rounds.
- `arb_mode()` — one of `{V1, DeltaConservative, DeltaOptimized}`.

## Property statement

For all `(condup_net, membership_schedule, mode)`:
```
canonicalise(reduce_all(condup_net)) == canonicalise(run_grid(
    condup_net, build_config(mode, elastic_join=true, elastic_departure=true, retain_partitions=true),
    membership_schedule
))
```

## Assertions

| # | Assertion |
|---|-----------|
| A1 | `prop_assert_eq!(canonicalise(local), canonicalise(distributed))` for v1 and DeltaConservative cases. |
| A2 | DeltaOptimized cases: same equivalence; if ARG-005 is pending per `theory-bridge.md`, mark `#[ignore]` with explicit citation. |
| A3 | The cascade structure (count of CON-DUP interactions) matches between local and distributed. |
| A4 | Total interactions match. |

## Shrinking

- Reduce `N` in the CON-DUP generator.
- Reduce membership schedule length.
- Switch to `V1` mode first (most thoroughly proven path).

## Configuration

- `cases: 128`.
- Honor `PROPTEST_RNG_SEED`.

## Edge / negative cases

- EC: a CON-DUP cascade triggers ≥ 50 emergent border redexes in a single round — the BorderGraph and merge correctly absorb them.
- EC: a worker carrying part of the cascade departs during the cascade — reclaim path correctly continues (cross-anchor EG-I5a).

## Invariants asserted

- D3 (Border Completeness).
- G1 CLOSED for v1 + DeltaConservative via ARG-006; CONDITIONAL on ARG-005 for DeltaOptimized.

## ARG/DISC/REF citation

**ARG-006** (mixed-trace recoverability — CLOSED for v1 + DeltaConservative). **ARG-005** (delta border completeness — gates the DeltaOptimized sub-property). Per `theory-bridge.md` "Open Theoretical Debts" → EG-P5 is the empirical proptest witness for ARG-006 under CON-DUP-heavy workloads.

## Determinism notes

Same as EG-P1/P2 (per-case tokio runtime current_thread + start_paused).

## Cross-test dependencies

EG-I5a (fixed-fixture cascade departure scenario), EG-P2 (general v1 departure proptest).
