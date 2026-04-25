# TEST-SPEC EG-I3-delta: elastic departure correctness delta (R18-R26, G1 CONDITIONAL)

**SPEC-20 §7.2 ID:** EG-I3-delta
**Owning task(s):** TASK-0410, TASK-0412, TASK-0438, TASK-0443.
**Type:** integration.
**Test name:** `test_elastic_departure_correctness_delta`.

---

## Inputs / Fixtures

Same as EG-I3 but with `delta_mode = true`. Two sub-cases:
- **Conservative path** (R24a-delta): reclaim via `reconstruct(&bg_snapshot, surviving, vec![reclaimed])` then re-split.
- **Optimized path** (R24b-delta): replay deltas onto live bg.

## Expected behaviour

Both paths produce a final result equal to `reduce_all`. The conservative path is unconditional (CLOSED via ARG-006 + P12). The optimized path is CONDITIONAL on ARG-005 (delta border completeness).

## Assertions

| # | Assertion |
|---|-----------|
| A1 | Conservative: for each fixture, `canonicalise(final_conservative) == canonicalise(reduce_all)`. |
| A2 | Optimized (when enabled per `theory-bridge.md` status): `canonicalise(final_optimized) == canonicalise(reduce_all)`; otherwise `#[ignore]` with citation. |
| A3 | `metrics.retained_last_acked_reclaims_per_round` records the reclaim. |
| A4 | If reclaim is `retained_initial` (no successful rounds), `metrics.retained_initial_reclaims_per_round` increments instead. |
| A5 | `BorderGraph` consistency post-reclaim: `bg.detect_border_redexes()` matches an exhaustive scan of the equivalent v1 state. |

## Edge / negative cases

- EC-1: a CON-DUP cascade straddles the boundary at the moment of departure → R24c (D3-elastic) ensures reclaim happens at clean boundary; correctness preserved (cross-anchor EG-I5a).
- EC-2: departure during the FinalStateRequest cycle of an unrelated join → composed scenario; assert both events are handled correctly.

## Invariants asserted

- D3 (Border Completeness via R39).
- G1 CLOSED for conservative (ARG-006); CONDITIONAL on ARG-005 for optimized.

## ARG/DISC/REF citation

ARG-005, ARG-006.

## Determinism notes

`#[tokio::test(flavor = "current_thread", start_paused = true)]`.

## Cross-test dependencies

EG-U7c, EG-U10b, TEST-SPEC-0412.
