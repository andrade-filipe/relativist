# TEST-SPEC EG-P1: prop hybrid normal form invariant (G1)

**SPEC-20 §7.3 ID:** EG-P1
**Owning task(s):** TASK-0422 (property coverage forward-ref).
**Type:** property (proptest).
**Test name:** `prop_hybrid_normal_form_invariant`.

---

## Generators

- `arb_terminating_net()` — strategy producing random terminating nets within a bounded size envelope:
  - `agent_count ∈ [1, 64]`.
  - `symbol distribution`: 40% Con, 40% Dup, 20% Era.
  - Connectivity such that every agent has 3 wired ports; no self-loops; no duplicate wires.
  - **Termination guarantee:** generator constructs nets via combinations of small primitive shapes (DualTree, Annihilation, simple chains) that are known to terminate; rejects shapes that risk non-termination.
- `arb_k_remote()` — `0..=8` (so K_eff ∈ `[1, 9]`).

## Property statement

For all `(net, k_remote)` from the generators:
```
canonicalise(reduce_all(net.clone())) == canonicalise(run_grid(net, GridConfig{ hybrid_coordinator: true, ..defaults }, k_remote))
```

## Assertions

| # | Assertion |
|---|-----------|
| A1 | `prop_assert_eq!(canonicalise(local), canonicalise(distributed))` for each generated case. |
| A2 | `prop_assert_eq!(metrics_local.total_interactions, metrics_distributed.total_interactions)` (counted modulo any reduction-order non-determinism that canonicalisation absorbs). |

## Shrinking

If the property fails, proptest's shrinker should:
- Reduce `k_remote` toward 0 (smaller distribution; isolates whether the bug is hybrid-specific vs general).
- Reduce `agent_count` toward minimal failing case.
- Simplify the symbol distribution to the minimal CON/DUP/ERA mix that still triggers.

## Configuration

- `proptest::config::Config { cases: 256, .. }` for default CI runs; override via env `PROPTEST_CASES` to higher in nightly.
- Seeded RNG: respect `PROPTEST_RNG_SEED` env so failures are reproducible.

## Edge / negative cases

The proptest space already covers many edge cases; add explicit `#[test]` regression-anchor cases for any historically-discovered failing input (e.g., the smallest K=0 hybrid case from EG-U1).

## Invariants asserted

- T1-T7, D1, D3, D4, G1.

## ARG/DISC/REF citation

ARG-001 (G1).

## Determinism notes

**Property tests are inherently random; determinism is via seeding.**
- All proptest cases run with `PROPTEST_CASES` and `PROPTEST_RNG_SEED` honored.
- The distributed run inside each property case uses `#[tokio::test(flavor = "current_thread", start_paused = true)]` semantics — but proptest runs cannot use `#[tokio::test]` directly. Strategy: use `proptest! { #[test] fn ... }` with an explicit `tokio::runtime::Builder::new_current_thread().enable_time().start_paused(true).build()` inside the test body, then `runtime.block_on(...)`.
- Worker scripts within each generated case are deterministic given the seed.
- Failing cases recorded to `proptest-regressions/` for replay.

## Cross-test dependencies

- EG-I1 is the equivalent integration test on a fixed fixture set.
