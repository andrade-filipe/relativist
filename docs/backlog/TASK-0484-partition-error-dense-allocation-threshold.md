# TASK-0484: `PartitionError::DenseAllocationExceedsThreshold` + `sparse_build` flag rejection at threshold (R30)

**Spec:** SPEC-22 §3.4 R30 (closes SC-009 partial — at-threshold rejection path).
**Requirements:** R30 (`sparse_build: bool` flag in `PartitionConfig` with default `true`; `false` MUST be rejected with `PartitionError::DenseAllocationExceedsThreshold` when `id_range > 4 × live_agent_count`).
**Priority:** P0 (M5-pathology guard).
**Status:** TODO
**Depends on:** TASK-0466 (SPEC-04 §4.5 amendment).
**Blocked by:** none
**Estimated complexity:** S (~30 LoC production + ~50 LoC tests)
**Bundle:** SPEC-22 Arena Management — Phase C (distributed integration).

## Context

R30 strengthens the `sparse_build` flag from SHOULD to MUST under the threshold rule. Default: `sparse_build: true` (use sparse construction). When the user opts out via `sparse_build = false` AND the threshold is exceeded (`id_range.end - id_range.start > 4 × partition.live_agent_count`), `build_subnet` MUST reject at the top with `PartitionError::DenseAllocationExceedsThreshold`. The flag's `false` setting is therefore only honored when the threshold is NOT exceeded — preserves backward compat for small workloads, prevents M5 pathology.

## Acceptance Criteria

- [ ] Add `pub sparse_build: bool` field to `PartitionConfig` (or current canonical location). Default: `true`.
- [ ] Add `PartitionError::DenseAllocationExceedsThreshold { partition_index: usize, id_range_size: u64, live_count: u64 }` variant.
- [ ] In `build_subnet` (or in `split` orchestrator before `build_subnet` is called), at the top of the function: compute `id_range_size = id_range.end - id_range.start` and `live_count`. If `sparse_build == false && id_range_size > 4 * live_count`: return `Err(PartitionError::DenseAllocationExceedsThreshold { ... })`.
- [ ] When `sparse_build == false && threshold NOT exceeded`: proceed with the dense path (existing behavior + R10a free-list population from TASK-0481).
- [ ] When `sparse_build == true && threshold exceeded`: forward to TASK-0492 sparse-then-dense path.
- [ ] When `sparse_build == true && threshold NOT exceeded`: implementation may use either path (default to dense for performance; sparse is also acceptable).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/partition/types.rs` *(or wherever PartitionConfig lives)* | modify | Add `sparse_build: bool` field with `Default::default() == true`. |
| `relativist-core/src/error.rs` *(or wherever PartitionError lives)* | modify | Add `DenseAllocationExceedsThreshold` variant. |
| `relativist-core/src/partition/helpers.rs` | modify | Top-of-function threshold check in `build_subnet`. |

## Key Types / Signatures

```rust
pub struct PartitionConfig {
    // ... existing fields ...
    pub sparse_build: bool,  // SPEC-22 R30; default true
}

pub enum PartitionError {
    // ... existing variants ...
    DenseAllocationExceedsThreshold {
        partition_index: usize,
        id_range_size: u64,
        live_count: u64,
    },
}
```

## Test Expectations (forward-ref)

TEST-SPEC-0484:
- `sparse_build_default_is_true`.
- `sparse_build_false_below_threshold_succeeds` (id_range == 4× live).
- `sparse_build_false_above_threshold_rejects` — id_range = 5× live, sparse_build = false, expect `DenseAllocationExceedsThreshold`.
- `sparse_build_true_above_threshold_uses_sparse_path` — joint with TASK-0492.

## Invariants Touched

- M5 feasibility (closes the 800 MB pathology when explicitly opted out).
- R30 (consumed).

## Notes

- The 4× threshold is documented as a "clean safety margin" in R22; do not parameterize this constant unless the spec is amended.
- The error variant is used both at `build_subnet` entry and in the SPEC-04 `split` orchestrator where partition planning happens.

## DAG Links

- **Predecessors:** TASK-0466.
- **Successors:** TASK-0492 (sparse-then-dense build_subnet path).
