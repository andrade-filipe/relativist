# TEST-SPEC EG-I1: hybrid grid correctness v1 (R1, R4-v1, G1)

**SPEC-20 §7.2 ID:** EG-I1
**Owning task(s):** TASK-0430.
**Type:** integration.
**Test name:** `test_hybrid_grid_correctness_v1`.

---

## Inputs / Fixtures

A parameterised test running `run_grid` in v1 + hybrid mode for several SPEC-09 benchmark nets:
- `ep_annihilation_con(N=8)`
- `dual_tree(depth=4)`
- `mixed_net_small`
- (Optionally) `church_add_3_4` if available.

For each net, vary K_remote ∈ {0, 1, 2, 4} so K_eff ∈ {1, 2, 3, 5}.

## Expected behaviour

Distributed hybrid `run_grid` produces a result canonically equal to local `reduce_all`.

## Assertions

| # | Assertion |
|---|-----------|
| A1 | For each (net, K_remote), `canonicalise(run_grid(net, hybrid, K_remote)) == canonicalise(reduce_all(net))`. |
| A2 | `metrics.total_interactions == reduce_all_metrics.total_interactions` for every (net, K_remote). |
| A3 | The number of rounds executed is finite and `<= some_documented_upper_bound`. |
| A4 | For every K_remote, `metrics.effective_slots_per_round.iter().all(|&s| s == K_remote + 1)`. |
| A5 | At K_remote = 0, the test exercises the hybrid-K-zero (solo-equivalent via in-process self) code path; result still matches. |

## Edge / negative cases

- EC-1: net is already at normal form (0 redexes) — result equals input; no rounds executed (or one trivial check round).
- EC-2: net with a single redex — at most one round.
- EC-3: net with > 1000 agents — verify no quadratic blowup; document upper bound.

## Invariants asserted

- T1-T7 (per-agent invariants preserved).
- D1 (Split/Merge Identity).
- D3 (Border Completeness via R39).
- D4 (ID Uniqueness).
- G1 (Fundamental Property) — PRESERVED via R39-G1-v1.

## ARG/DISC/REF citation

ARG-001 (G1).

## Determinism notes

`#[tokio::test(flavor = "current_thread", start_paused = true)]`. K remote workers spawn as tokio tasks connected via `tokio::io::duplex`. All scheduling deterministic. Result canonicalisation absorbs any reduction-order non-determinism.

## Cross-test dependencies

- EG-U1, EG-U2, EG-U3, EG-U4 each cover individual pieces.
- EG-I1-delta is the delta counterpart.
- EG-P1 is the proptest version.
