# TEST-SPEC EG-B1: bench hybrid vs non-hybrid (R1, SC-010)

**SPEC-20 §7.4 ID:** EG-B1
**Owning task(s):** TASK-0450 (provides metrics surface).
**Type:** benchmark.
**Bench name:** `bench_hybrid_vs_nonhybrid`.

---

## Inputs / Fixtures

- A representative subset of SPEC-09 benchmark nets at 3 sizes each (small, medium, large): `ep_annihilation_con(N=8 / 32 / 128)`, `dual_tree(depth=3 / 5 / 7)`.
- K_remote ∈ `{1, 2, 4}`.
- Two configurations per (net, K_remote):
  - `--hybrid` (K_eff = K_remote + 1).
  - `--no-hybrid` (K_eff = K_remote; coordinator does not participate as worker).

## Metrics measured

- Wall-clock duration of `run_grid` from `WaitingForWorkers` exit through final result emission.
- Total interactions (sanity baseline; should be identical mod permutation).
- `effective_slots_per_round` (verified to be K_remote+1 vs K_remote respectively).

## Pass / fail criteria

This is a **comparative benchmark**, not an absolute-threshold benchmark. The framing per master plan Gate 1 ("apples-to-apples vs apples-to-oranges"):
- Both configurations run the SAME (net, K_remote) pair.
- Output reports both wall-clocks.
- The benchmark **passes structurally** (no panic; final result canonicalised matches `reduce_all`).
- A **non-regression gate** vs `results/locked/v1_local_baseline/` is checked: with `--no-hybrid` and v1 flags, the wall-clock must be within ±10% of the locked baseline (or whatever tolerance the existing v1 bench infrastructure uses).

## Assertions

| # | Assertion |
|---|-----------|
| A1 | Both runs produce `canonicalise(result) == canonicalise(reduce_all(net))`. |
| A2 | Both runs report the same `total_interactions` (modulo reduction-order permutation; canonicalisation absorbs differences). |
| A3 | The reported wall-clock is monotonically positive; no panics; no errors. |
| A4 | `--no-hybrid` runs at K_remote=2 reproduce the v1 baseline metrics within tolerance. |
| A5 | `--hybrid` reports `metrics.effective_slots_per_round.last() == Some(&(K_remote + 1))`; `--no-hybrid` reports `Some(&K_remote)`. |

## Edge / negative cases

- EC: K_remote = 0 + `--no-hybrid` → no reducer; coordinator should error per v1; the bench skips this combination.
- EC: very small net + large K_remote → bench measures the constant-time overhead floor; the comparison still produces a valid signal.

## Invariants asserted

- G1 (correctness across both modes; bench is also a correctness check).

## ARG/DISC/REF citation

ARG-004 (ROADMAP §2.40 break-even).

## Determinism notes

**Benchmarks are NOT deterministic in wall-clock; they are deterministic in correctness.** The wall-clock variation is reported as median + IQR over `N` repetitions per configuration (default 10).

## Cross-test dependencies

- Frozen `results/locked/v1_local_baseline/` is the regression anchor.
- TASK-0450 (`GridMetrics` elastic fields) provides the measurement surface.
