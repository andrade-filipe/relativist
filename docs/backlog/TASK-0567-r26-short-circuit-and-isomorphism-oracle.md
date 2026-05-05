# TASK-0567: R26 short-circuit + T6/T8 isomorphism oracle (`chunk_size = u32::MAX` → `split()`)

**Spec:** SPEC-21 §3.4 R26 (closes SC-014); §3.8 A8 (split() unchanged; chunked pipeline additive).
**Requirements:** R26 (when `chunk_size == u32::MAX`, pipeline MUST short-circuit to SPEC-04 `split()` after collecting the full stream via R10 default-impl path; merge result MUST be **isomorphic** to v1 `split()`-produced result).
**Priority:** P0 (T6 / T8 isomorphism oracle blocker; v1 backward-compat fallback).
**Status:** TODO
**Depends on:** TASK-0540 (`make_net_stream` default impl), TASK-0554 (`generate_and_partition_chunked` orchestrator), TASK-0517 (SPEC-04 §4.5 additive amendment), TASK-0565 (GridConfig `chunk_size` field), TASK-0541 (ep_annihilation override), TASK-0542 (dual_tree override), TASK-0531 (Fennel strategy — for T9 axis).
**Blocked by:** none
**Estimated complexity:** M (~120 LoC short-circuit branch in orchestrator + isomorphism harness; ~250 LoC T6/T8 sweep tests).
**Bundle:** SPEC-21 Streaming Generation — Phase F (regression / polish / late-binding).

## Context

Per R26, when `GridConfig.chunk_size == u32::MAX` (sentinel value indicating "no chunking"), the pipeline MUST collect the entire stream into a single `Net` via the R10 default-impl materialization path and call SPEC-04 `split()` directly. The merge result MUST satisfy `nets_isomorphic` (SPEC-00 §6.12) with the v1 `split()`-produced result; **bit-identical layout is NOT guaranteed** because of SPEC-22 arena-management amendments (free-list, SparseNet, `freeport_redirects` propagation).

This task is the **T6 (streaming-vs-batch equivalence)** and **T8 (chunk-size independence)** isomorphism oracle. It exercises:

- T6 (§7.2): for every benchmark in the SPEC-09 catalog, assert `chunked_pipeline(net_stream, chunk_size=k).merge() ~ split(make_net()).merge()` (isomorphism, not byte-equality, per R26).
- T8 (§7.2): vary `chunk_size ∈ {2, 8, 64, 512, size}` across `ep_annihilation`, `dual_tree`, and one mixed benchmark; assert pairwise isomorphism of merged results.
- R26 short-circuit branch: with `chunk_size == u32::MAX`, the orchestrator MUST take the materialize-then-split path (not the streaming path).

Per §3.8 A8 (closes SC-001 part 4), SPEC-04 `split()` is UNCHANGED. The chunked pipeline is an ALTERNATIVE entry point selected by `GridConfig.chunk_size != u32::MAX`. `split()` is the fallback for the v1 backward-compat path.

## Acceptance Criteria

- [ ] `generate_and_partition_chunked` (TASK-0554) gains a top-of-function branch: if `chunk_size == u32::MAX`, materialize the stream into a `Net` (via `default_chunked_iter` collect) and delegate to `split(net, num_workers, strategy_for_split)`, returning a `ChunkedPartitionResult` constructed from the resulting `PartitionPlan` (R20-R21 conversion).
- [ ] T6 isomorphism harness: for each benchmark in {`ep_annihilation`, `dual_tree`, `mixed_net`}, run both batch path (`split()`) and streaming path (`generate_and_partition_chunked` with `chunk_size = N/4`); merge both; assert `nets_isomorphic(merged_batch, merged_streaming)` per SPEC-00 §6.12.
- [ ] T8 chunk-size independence: for `ep_annihilation_pure(1024)`, run with `chunk_size ∈ {2, 8, 64, 512, 1024}`; assert pairwise isomorphism of merged results.
- [ ] R26 short-circuit: assert that the materialize-then-split branch is taken when `chunk_size == u32::MAX` (instrumentation via test-only counter or trace event).
- [ ] No bit-equality assumed (test docstring MUST cite "isomorphism, not byte-equality" per R26 closure of SC-014).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/partition/streaming.rs` (or wherever TASK-0554 places the orchestrator) | modify | Add R26 short-circuit branch at function entry. |
| `relativist-core/tests/spec21_t6_streaming_vs_batch.rs` | create | T6 isomorphism harness across benchmarks. |
| `relativist-core/tests/spec21_t8_chunk_size_independence.rs` | create | T8 chunk-size sweep. |

## Key Types / Signatures

```rust
// In generate_and_partition_chunked(...) near top:
if chunk_size == u32::MAX {
    let mut net = Net::new();
    for batch in stream { /* materialize via default_chunked_iter inverse */ }
    let plan = split(net, num_workers, &mut DenseStrategyForSplit);
    return ChunkedPartitionResult::from_partition_plan(plan); // R20-R21
}
// ... else streaming path ...
```

## Test Expectations (forward-ref)

TEST-SPEC-T6-streaming-vs-batch-equivalence (full T6 lives here per INDEX line 258).
TEST-SPEC-T8-chunk-size-independence (full T8 lives here per INDEX line 260).
TEST-SPEC-0541, 0542, 0517, 0540 — partial T6/T8 already covered at the unit level; this task closes the integration-level gate.

## Invariants Touched

- D1 extended for streaming (R27 §3.5) — verified by isomorphism, not bit-equality.
- I3' (uniqueness) preserved in both paths.
- T1 (port linearity) preserved in both paths.

## Notes

- `nets_isomorphic` lives in SPEC-00 §6.12 helpers; reuse, do NOT duplicate.
- The short-circuit path is the v1 backward-compat fallback (R26). Combined with TASK-0588 (call-site discipline) and TASK-0589/0590 (recycle wiring), it ensures pre-SPEC-21 binaries that ship with `chunk_size = u32::MAX` default behave identically to v1.
- The "Strategy for split()" used inside the short-circuit branch SHOULD be a fixed `ContiguousIdStrategy` (SPEC-04 R22), regardless of `streaming_strategy: StreamingStrategyConfig`; document this explicitly so T9 (strategy independence) is not muddied. Cross-reference: T9 owner is TASK-0554 (axis varied) — T9 does NOT exercise the short-circuit path.
- Consumed by TASK-0500-style regression gate (cross-spec; if a SPEC-21 v1 backward-compat regression task is added later it MUST verify R26 short-circuit reproduces v1 metrics).

## DAG Links

- **Predecessors:** TASK-0540, TASK-0554, TASK-0517, TASK-0565, TASK-0541, TASK-0542, TASK-0531.
- **Successors:** SPEC-21 v1-backward-compat regression task (analog to TASK-0500); cross-spec gate.
