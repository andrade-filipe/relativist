# TEST-SPEC-T16: Sparse `build_subnet` integration — full split → reduce → merge → G1 isomorphism

**SPEC-22 §7.2 ID:** T16.
**Owning task:** TASK-0492 (sparse-then-dense build_subnet integration); joint with TASK-0483 (merge free-list reconciliation), TASK-0489 (to_sparse), TASK-0490 (to_dense).
**Parent spec:** SPEC-22 §3.2 R22; §3.8 A7 (SPEC-04 build_subnet amendment), A8 (SPEC-05 merge amendment); SC-009 closure.
**Type:** integration.
**Theory anchor:** ARG-002 (Partitioning Preserves Structure — split/reduce/merge correctness); REF-002 Proposition 1 (strong confluence).

---

## Inputs / Fixtures

- A non-trivial net of 1000 agents (mix of CON/DUP/ERA per a deterministic generator from SPEC-14).
  - Use a Church-arithmetic-like topology that produces ~50 redexes after split.
  - Set agents and wires deterministically (seeded RNG with fixed seed `42`).
- Partition into 4 workers via the SPEC-04 contiguous-id strategy:
  - Partition 0: id_range `0..250`.
  - Partition 1: id_range `250..500`.
  - Partition 2: id_range `500..750`.
  - Partition 3: id_range `750..1000`.
- `PartitionConfig.sparse_build = true` (default).
- **Force the sparse path:** parameterize the test with a reduced 4× threshold (e.g., set `live_count` per partition deliberately small relative to range) so that R22 fires.
- Two computation modes for comparison:
  - **Mode A (sequential baseline):** clone the net, run `reduce_all` to normal form on the whole net.
  - **Mode B (sparse-grid):** run `split` → `build_subnet` (sparse path) → reduce each partition → `merge`.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-T16-01 | `sparse_build_path_is_taken_above_threshold` | net + partitions; threshold-exceeded test fixture | call `build_subnet` per partition; instrument the sparse vs dense branch | the sparse path executes for every partition (verified via test-only counter or tracing span). |
| UT-T16-02 | `g1_isomorphism_sequential_vs_sparse_grid` | Mode A result `nf_seq`, Mode B result `nf_grid` | `nf_seq.is_behaviorally_equal(&nf_grid)` | `true`. (G1 — closes SC-009 at the integration level.) |
| UT-T16-03 | `each_partition_reduces_independently` | Mode B; mid-execution snapshot per partition | each partition's reduction produces a normal form local to its agents and border ports | confirmed; no mid-reduction cross-partition message dependency. |
| UT-T16-04 | `merged_free_list_satisfies_no_duplicates` | Mode B post-merge | sort `nf_grid.free_list.clone()`; assert no consecutive equal IDs | confirmed (R6 + D4 disjointness automatic guarantee per §3.8 A8). |
| UT-T16-05 | `merged_free_list_only_contains_none_slots` | Mode B post-merge | `nf_grid.validate_free_list()` (helper from TASK-0475) | `Ok(())`. (R12 / A8 algorithm post-condition.) |
| UT-T16-06 | `freeport_redirects_preserved_through_sparse_path` | Mode B; pre-split net has 5 entries in `freeport_redirects` | post-merge: `nf_grid.freeport_redirects.contains_key(...)` for each pre-split key | preserved. (Closes SC-001 second surface end-to-end.) |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Threshold NOT exceeded (`id_range_size <= 4 * live_count`) — dense path forced | Both Mode A and Mode B produce the same nf via the dense build_subnet. T16 still asserts G1 via UT-T16-02. (Joint with TASK-0481 dense path.) |
| EC-2 | Single-partition (1 worker, full id_range) | `merge` is a no-op on the single partition; UT-T16-02 still passes (Mode A and Mode B agree trivially). |
| EC-3 | All agents removed in Mode A (degenerate empty net) | Both modes produce empty net; G1 trivially passes. |
| EC-4 | Net with no FreePort connections | UT-T16-06 trivially passes (both empty). |

## Invariants asserted

- G1 (Distributed result equals local result) — load-bearing for SPEC-22's grid-correctness story.
- R22 (sparse build_subnet at threshold).
- §3.8 A7 (SPEC-04 build_subnet amendment).
- §3.8 A8 (SPEC-05 merge amendment).
- D4 (Partition disjointness — preserved across split + sparse-build + merge).
- M5 feasibility (closes the 800 MB pathology — verified at the build_subnet branch level).

## ARG/DISC/REF citation

- ARG-002 (Partitioning Preserves Structure) — full P1-P6 cycle exercised here.
- REF-002 Proposition 1 (strong confluence — guarantees Mode A and Mode B converge to the same normal form regardless of reduction order).

## Determinism notes

The random-seed `42` ensures the initial net is reproducible across test runs. The reduction strategy within each partition may permute the visit order of redexes (per `reduce_all`'s implementation), but G1 / `is_behaviorally_equal` is confluence-safe (per R21 + REF-002 Prop 1). Mode A and Mode B may produce different intermediate states; only the final normal form is asserted.

`HashMap` iteration in `to_sparse`/`to_dense` is non-deterministic; the assertions use `is_behaviorally_equal` and set-equality, NOT byte-equality. Pure synchronous (single-threaded) test; no tokio. The test is **fully deterministic** at the assertion level; the only non-determinism is in iteration order, which is absorbed by the helper.

## Cross-test dependencies

- TEST-SPEC-0492 covers the sparse-build_subnet primitive.
- TEST-SPEC-0483 covers `merge` free-list reconciliation primitive.
- T9 covers per-partition free-list correctness (a slice of T16).
- T14 / T14a cover the conversion primitives.
- This test is the **load-bearing G1 closure for SPEC-22**; if it fails, the sparse-grid path is broken.
