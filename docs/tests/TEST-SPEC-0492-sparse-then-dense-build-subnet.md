# TEST-SPEC-0492: Sparse-then-dense `build_subnet` integration under 4× threshold (R22)

**SPEC-22 §7 ID:** T16 (spec-catalog) plus this plumbing file.
**Owning task:** TASK-0492.
**Parent spec:** SPEC-22 §3.2 R22; §3.8 A7; SC-009 closure.
**Type:** integration.

---

## Inputs / Fixtures

- A net of 1000 agents with deterministic structure (seed `42`).
- 4 partitions; some partitions exceed the threshold (`id_range_size > 4 * live_count`) and some don't.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0492-01 | `sparse_path_taken_above_threshold` | partition with `id_range_size = 5 * live_count`, `sparse_build = true` | `build_subnet(...)` (instrumented to track which branch fired) | sparse branch executed. |
| UT-0492-02 | `dense_path_taken_below_threshold` | partition with `id_range_size = 3 * live_count`, `sparse_build = true` | same | dense branch executed (sparse build is permitted but dense is taken for performance). |
| UT-0492-03 | `sparse_path_freeport_redirects_preserved` | partition that triggers sparse path; pre-split net has 5 entries in `freeport_redirects` | post `build_subnet` | the resulting `Net.freeport_redirects` contains the relevant subset for the partition. (Closes SC-001 second surface in the build_subnet path.) |
| UT-0492-04 | `sparse_path_id_range_set_on_returned_net` | sparse path | `result.id_range == Some(partition.id_range.clone())`. |
| UT-0492-05 | `sparse_path_free_list_only_in_range` | sparse path | every ID in `result.free_list` is in `partition.id_range`. (R10a via `to_dense(Some(range))`.) |
| UT-0492-06 | `sparse_path_g1_round_trip` (T16 spec-catalog) | full split → sparse build_subnet → reduce → merge → final net | `final_net.is_behaviorally_equal(&sequential_baseline_net) == true`. |
| UT-0492-07 | `m5_pathology_avoided_under_sparse` | partition with `id_range = 0..100_000_000`, `live_count = 10_000_000` (sparse path required) | `build_subnet` runs without OOM | runs in bounded memory (< 200 MB instead of 800 MB). (Stress test; may be skipped in CI by default and run on demand.) |
| UT-0492-08 | `signature_returns_result` | the function signature | `build_subnet(...)` returns `Result<Net, PartitionError>` | confirmed; the test handles both `Ok` and `Err` cases. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | `id_range_size == 4 * live_count` (boundary, NOT exceeded) | Dense path taken; UT-0492-02 covers. |
| EC-2 | `live_count == 0` (empty partition) | Threshold exceeded trivially (`id_range_size > 0`); sparse path taken; result is an empty `Net` with full-range free-list. |
| EC-3 | Single-partition (whole net) above threshold | Sparse path taken; resulting net is the entire converted net; G1 trivially passes. |

## Invariants asserted

- R22 (sparse path at threshold).
- §3.8 A7 (SPEC-04 build_subnet amendment).
- D4, G1, M5 feasibility.

## ARG/DISC/REF citation

- ARG-002 (Partitioning Preserves Structure).
- AC-011 (HVM4 static heap partitioning).

## Determinism notes

Seeded RNG ensures reproducibility. `HashMap` iteration in sparse path is non-deterministic but the post-conversion `Net` is order-invariant in observable content. `is_behaviorally_equal` absorbs the redex-queue order ambiguity. Pure synchronous; no tokio at the build_subnet level (any async is at SPEC-04 split orchestration, out of scope here).

## Cross-test dependencies

- T16 spec-catalog mirror.
- TEST-SPEC-0481 (dense path), TEST-SPEC-0484 (threshold rejection), TEST-SPEC-0489 (`to_sparse`), TEST-SPEC-0490 (`to_dense(Some(range))`).
- The signature change (`build_subnet` returning `Result<Net, PartitionError>` and taking `&PartitionConfig`) ripples through `split()` — coordinate with the `partition` module owner.
