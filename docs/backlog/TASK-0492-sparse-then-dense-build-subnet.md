# TASK-0492: Sparse-then-dense `build_subnet` integration under 4× threshold (R22 — consumer of A7)

**Spec:** SPEC-22 §3.2 R22; §3.8 A7 (consumes SPEC-04 amendment).
**Requirements:** R22 (when `partition.id_range.end - partition.id_range.start > 4 * partition.live_agent_count`, `build_subnet` MUST use `SparseNet` internally and call `to_dense(Some(partition.id_range.clone()))` before the reduction loop. Otherwise, `build_subnet` MAY use the existing dense path.).
**Priority:** P0 (M5 pathology fix; closes SC-009).
**Status:** TODO
**Depends on:** TASK-0466 (SPEC-04 amendment), TASK-0481 (dense-path build_subnet), TASK-0484 (sparse_build threshold rejection), TASK-0489 (to_sparse), TASK-0490 (to_dense with id_range).
**Blocked by:** none
**Estimated complexity:** M (~120 LoC production + ~150 LoC tests)
**Bundle:** SPEC-22 Arena Management — Phase D (SparseNet).

## Context

`build_subnet` produces a partition subnet for worker `i`. Under the dense path (TASK-0481), the function allocates `vec![None; arena_len]` where `arena_len ~ id_range.end`. At M5 scale (100M agents, partition with `id_range = 0..100M` holding 10M live agents), this is 800 MB per partition — pathological.

R22 mandates the sparse fallback when the threshold fires (`id_range.end - id_range.start > 4 * partition.live_agent_count`):
1. Construct a `SparseNet` (memory proportional to live agents only — ~10 MB at M5 vs 800 MB dense).
2. Place live agents and port entries in the sparse representation.
3. Call `to_dense(Some(partition.id_range.clone()))` to materialize a dense `Net` for the reduction loop, with the free-list correctly scoped to `[range.start, range.end)` per R10a.

The exposed signature of `build_subnet` remains unchanged (still returns `Net`). The sparse path is purely internal.

## Acceptance Criteria

- [ ] Modify `build_subnet` in `relativist-core/src/partition/helpers.rs` to compute the threshold check at entry: `id_range_size = id_range.end - id_range.start`, `live_count = worker_agents.len()`, `threshold_exceeded = id_range_size > 4 * live_count`.
- [ ] If `threshold_exceeded && config.sparse_build`: take the sparse path. Construct a `SparseNet`, place live agents (per existing logic but in sparse), copy port entries, set `freeport_redirects` from the source net, then `let dense = sparse.to_dense(Some(id_range.clone()));` and return `dense`.
- [ ] If `threshold_exceeded && !config.sparse_build`: rejected by TASK-0484 with `PartitionError::DenseAllocationExceedsThreshold` (this branch should never reach `build_subnet` body — `split` orchestrator handles).
- [ ] If `!threshold_exceeded`: dense path (existing TASK-0481 logic).
- [ ] The dense `Net` returned from `to_dense(Some(range))` already has the partition free-list populated by R10a (TASK-0490 builds this in).
- [ ] Set the dense `Net.id_range = Some(range.clone())` (TASK-0490 does this).
- [ ] Performance budget: at M5 (100M ID space, 10M live), sparse path allocates ~10 × HashMap-overhead-per-agent (~100 MB total) instead of 800 MB dense.
- [ ] Test T16 (SPEC-22 §7.2): build a 1000-agent net, partition for 4 workers using `SparseNet`-based construction; reduce all partitions; merge; assert the result is **isomorphic** (G1) to sequential reduction of the original net.
- [ ] Test: artificially small threshold (parameterize for test speed) — verify sparse path is taken when `id_range.end - id_range.start > 4 * live_count`.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/partition/helpers.rs` | modify | Top-of-function threshold check + sparse path branch. |

## Key Types / Signatures

```rust
pub fn build_subnet(
    net: &Net,
    worker_agents: &HashSet<AgentId>,
    sigma: &SubstitutionTable,
    border_entries: &[BorderEntry],
    id_range: core::ops::Range<AgentId>,
    config: &PartitionConfig,
) -> Result<Net, PartitionError> {
    let id_range_size = (id_range.end - id_range.start) as u64;
    let live_count = worker_agents.len() as u64;
    let threshold_exceeded = id_range_size > 4 * live_count;

    if threshold_exceeded && !config.sparse_build {
        return Err(PartitionError::DenseAllocationExceedsThreshold { /* ... */ });
    }

    if threshold_exceeded {
        // Sparse path
        let mut sparse = SparseNet::with_capacity(live_count as usize);
        // ... place live agents and port entries ...
        sparse.freeport_redirects = /* copy from net */;
        let dense = sparse.to_dense(Some(id_range.clone()));
        Ok(dense)
    } else {
        // Dense path (TASK-0481 logic)
        // ...
    }
}
```

## Test Expectations (forward-ref)

TEST-SPEC-0492:
- T16: G1 round-trip via sparse build → reduce → merge. Uses `is_behaviorally_equal` from TASK-0491.
- `sparse_path_taken_above_threshold`.
- `dense_path_taken_below_threshold`.
- `sparse_path_freeport_redirects_preserved`.
- `sparse_path_id_range_set_on_returned_net`.

## Invariants Touched

- D4 (preserved — partition-scoped free-list).
- M5 feasibility (closes the 800 MB pathology).
- G1 (preserved across sparse path).

## Notes

- The signature change of `build_subnet` (adding `&PartitionConfig` and returning `Result`) may ripple through `split()` — coordinate with the `partition` module owner.
- The threshold constant (`4`) is hardcoded; do not parameterize without spec amendment.

## DAG Links

- **Predecessors:** TASK-0466, TASK-0481, TASK-0484, TASK-0489, TASK-0490.
- **Successors:** TASK-0500 (regression gate).
