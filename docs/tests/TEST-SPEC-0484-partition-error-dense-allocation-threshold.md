# TEST-SPEC-0484: `PartitionError::DenseAllocationExceedsThreshold` + sparse_build flag rejection (R30)

**SPEC-22 §7 ID:** none direct (M5 pathology guard); plus this plumbing file.
**Owning task:** TASK-0484.
**Parent spec:** SPEC-22 §3.4 R30.
**Type:** unit.

---

## Inputs / Fixtures

- A `PartitionConfig` with the new `sparse_build: bool` field.
- A partition fixture with controllable `id_range_size` and `live_count`.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0484-01 | `sparse_build_default_is_true` | `let cfg: PartitionConfig = Default::default();` | `cfg.sparse_build` | `== true`. |
| UT-0484-02 | `sparse_build_false_below_threshold_succeeds` | partition with `id_range_size == 4 * live_count` (boundary, not exceeded), `sparse_build == false` | `build_subnet(...)` | `Ok(_)`. (Dense path, R10a free-list population.) |
| UT-0484-03 | `sparse_build_false_above_threshold_rejects` | partition with `id_range_size == 5 * live_count` (exceeds), `sparse_build == false` | `build_subnet(...)` | `Err(PartitionError::DenseAllocationExceedsThreshold { partition_index, id_range_size, live_count })`. The error fields contain the actual sizes. |
| UT-0484-04 | `sparse_build_true_above_threshold_uses_sparse_path` | partition with `id_range_size > 4 * live_count`, `sparse_build == true` | `build_subnet(...)` | `Ok(_)`. (Sparse path; joint with TEST-SPEC-0492.) |
| UT-0484-05 | `error_field_id_range_size_correct` | UT-0484-03 setup; `id_range = 0..500`, `live_count = 100` | match the error | `id_range_size == 500`, `live_count == 100`. |
| UT-0484-06 | `error_variant_in_partition_error_enum` | the `PartitionError` enum surface | grep / inspect | the variant `DenseAllocationExceedsThreshold` is defined and `#[derive(Debug, Clone, PartialEq)]`-compatible. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | `id_range_size == 4 * live_count` exactly (boundary) | NOT exceeded. UT-0484-02 covers. |
| EC-2 | `id_range_size == 4 * live_count + 1` (just above boundary) | Exceeded. UT-0484-03's logic applies. |
| EC-3 | `live_count == 0` (empty partition) | `4 * 0 == 0`; any non-empty `id_range` exceeds. The error fires. (Documents the degenerate case; an empty partition with `sparse_build == false` is rejected.) |
| EC-4 | `id_range_size == 0` (empty range) | `0 > 4 * live_count` only if `live_count` is also 0; UT-0484 EC-3 covers that. Otherwise, NOT exceeded — succeeds. |

## Invariants asserted

- R30 (sparse_build flag MUST + threshold rejection).
- M5 pathology guard.

## ARG/DISC/REF citation

- AC-011 (HVM4 static heap partitioning — informs the threshold rationale).

## Determinism notes

Pure synchronous; deterministic threshold check at `build_subnet` entry.

## Cross-test dependencies

- TEST-SPEC-0492 covers the sparse path (R22).
- TEST-SPEC-0481 covers the dense path (R10a).
