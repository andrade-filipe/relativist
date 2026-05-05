# TEST-SPEC EG-P3: prop id-ranges disjoint after re-partition (D4-elastic, R30)

**SPEC-20 §7.3 ID:** EG-P3
**Owning task(s):** TASK-0452 (debug-assertions task; consumes this proptest).
**Type:** property (proptest).
**Test name:** `prop_id_ranges_disjoint_after_repartition`.

---

## Generators

- `arb_k_eff_trajectory()` — vector of K_eff values: `Vec<u32>` with each value `∈ [1, 12]` and length `∈ [2, 10]`. The trajectory simulates a series of re-partitions.
- For each transition (k_old → k_new), choose a transition reason via:
  - `arb_transition_reason()` — `{Join, Departure, Mixed}`. The reason determines whether existing partitions retain their AgentId range or are reclaimed and remapped.

## Property statement

For all generated K_eff trajectories, after applying every transition:
- The K_eff partitions' `IdRange`s are pairwise disjoint.
- `partition_index` is dense `[0, K_eff)`.
- The set of WorkerIds present is a subset of `[0, u32::MAX]` (never reused).

## Assertions

| # | Assertion |
|---|-----------|
| A1 | `prop_assert!(all_ranges_disjoint(&partitions_at_each_transition))`. |
| A2 | `prop_assert_eq!(partition_indices.iter().copied().collect::<BTreeSet<_>>(), (0..k_eff).collect::<BTreeSet<_>>())` at each transition. |
| A3 | `prop_assert!(no_worker_id_reused(&worker_id_history))`. |
| A4 | `prop_assert!(border_id_ranges_disjoint(&partitions_at_each_transition))`. |

## Shrinking

- Reduce trajectory length to minimum failing.
- Reduce K_eff values toward 1.

## Configuration

- `cases: 256`.
- Honor `PROPTEST_RNG_SEED`.

## Edge / negative cases

- EC: K_eff = 1 (single partition) — single range covering full id space; trivially disjoint.
- EC: K_eff oscillating (e.g., `[3, 5, 3, 5, 3]`) — every recomputation must produce disjoint ranges.

## Invariants asserted

- D4-elastic (R11a, R13, R30).
- D3 (border_id disjointness via SPEC-04 A3).

## ARG/DISC/REF citation

None.

## Determinism notes

Pure synchronous (no tokio). Proptest seeded; deterministic given seed.

## Cross-test dependencies

EG-U12, EG-U12a (fixed-fixture unit tests).
