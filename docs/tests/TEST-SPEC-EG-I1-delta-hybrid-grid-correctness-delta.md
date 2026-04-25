# TEST-SPEC EG-I1-delta: hybrid grid correctness delta (R1, R4-delta, G1)

**SPEC-20 §7.2 ID:** EG-I1-delta
**Owning task(s):** TASK-0437.
**Type:** integration.
**Test name:** `test_hybrid_grid_correctness_delta`.

---

## Inputs / Fixtures

Same fixtures as EG-I1 but with `delta_mode = true`. Two subsets:
- **Conservative path** (R24a; CLOSED unconditionally via ARG-006 mixed-trace recoverability for delta-conservative).
- **Optimized path** (R24b; CONDITIONAL on ARG-005 — gated by SPEC-19 implementation).

## Expected behaviour

Distributed hybrid `run_grid_delta` matches local `reduce_all` for every fixture.

## Assertions

| # | Assertion |
|---|-----------|
| A1 | Conservative path: for each (net, K_remote), `canonicalise(run_grid_delta_conservative(net, hybrid, K_remote)) == canonicalise(reduce_all(net))`. |
| A2 | Optimized path: same equivalence for the optimized variant. (If ARG-005 is provisionally CLOSED at run time per `theory-bridge.md`, run; else mark `#[ignore]` with a doc-comment citing the gate.) |
| A3 | `total_interactions` matches v1 hybrid. |
| A4 | The self-worker delta payload structure matches the remote (cross-anchor with EG-U4-delta-wire-symmetry). |

## Edge / negative cases

- EC-1: net with a CON-DUP cascade across the self/remote boundary — delta border resolution exercised end-to-end; result still matches local reduce_all.
- EC-2: net at normal form — delta result equals input; no rounds executed.

## Invariants asserted

- D3 (Border Completeness via R39 / ARG-005).
- G1 — PRESERVED via R39-G1-v1 + R39-G1-delta. CLOSED for conservative; CONDITIONAL on ARG-005 for optimized.

## ARG/DISC/REF citation

ARG-005 (delta border completeness — gates the optimized path); ARG-006 (mixed-trace recoverability — gates the conservative path).

## Determinism notes

`#[tokio::test(flavor = "current_thread", start_paused = true)]`.

## Cross-test dependencies

- EG-U4-delta, EG-U4-delta-wire-symmetry.
- EG-I1 (v1 baseline counterpart).
- EG-P6 (delta-elastic property test).
