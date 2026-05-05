# TASK-0522: Define `StreamingPartitionStats` struct

**Spec:** SPEC-21 §4.1 type definitions; SPEC-21 §3.1 R3 (`finalize` returns this).
**Requirements:** StreamingPartitionStats struct from §4.1.
**Priority:** P0 (foundational type for trait + ChunkedPartitionResult).
**Status:** TODO
**Depends on:** none.
**Blocked by:** none
**Estimated complexity:** S (~20 LoC production + ~30 LoC tests).
**Bundle:** SPEC-21 Streaming Generation — Phase B (core types).

## Context

Statistics about a streaming partitioning run. Returned by `StreamingPartitionStrategy::finalize()` (R3) and embedded in `ChunkedPartitionResult.stats` (R20).

Per SPEC-21 §4.1:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StreamingPartitionStats {
    pub total_agents: u64,
    pub per_worker_counts: Vec<u64>,
    pub border_wire_count: u64,
    pub chunks_processed: u64,
}
```

**Critical ownership note (closes SC-021).** The `chunks_processed` field is **pipeline-owned**, NOT strategy-owned. The strategy returns `chunks_processed: 0` as a placeholder; the pipeline maintains a local `chunks_seen: u64` counter incremented per iteration and assigns `result.stats.chunks_processed = chunks_seen` after `strategy.finalize()` (per §4.6 pipeline pseudocode Step 7). T1 (§7.1) MUST verify the pipeline-stitched count, not the strategy-returned value.

This ownership convention is implemented by TASK-0530 (RoundRobinStreamingStrategy) returning 0 and TASK-0554 (pipeline orchestrator) stitching the actual count.

## Acceptance Criteria

- [ ] Define `pub struct StreamingPartitionStats` in the same module as `AgentBatch` / `ConnectionDirective` (TASK-0520 / TASK-0521).
- [ ] Four fields present with exact types: `total_agents: u64`, `per_worker_counts: Vec<u64>`, `border_wire_count: u64`, `chunks_processed: u64`.
- [ ] Derive `Debug`, `Clone`, `serde::Serialize`, `serde::Deserialize`.
- [ ] Rustdoc on `chunks_processed` MUST document the pipeline-owned convention per SC-021 closure (`See SPEC-21 §4.3 note + §4.6 Step 7 stitch step`).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/partition/streaming.rs` | modify | Add `StreamingPartitionStats` struct. |

## Key Types / Signatures

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StreamingPartitionStats {
    /// Total agents assigned across all batches.
    pub total_agents: u64,
    /// Number of agents assigned to each worker.
    pub per_worker_counts: Vec<u64>,
    /// Number of border wires created.
    pub border_wire_count: u64,
    /// Number of chunks processed.
    /// PIPELINE-OWNED: strategies return 0 as a placeholder; the
    /// pipeline stitches the actual count via `chunks_seen` (closes SC-021).
    pub chunks_processed: u64,
}
```

## Test Expectations (forward-ref)

TEST-SPEC-0522:
- Construct stats; verify Debug + Clone + serde round-trip.
- T1 partial (per-worker counts verification — TASK-0530 strategy correctness).

## Invariants Touched

- None at type level.

## Notes

- The pipeline-owned convention for `chunks_processed` MUST be documented via Rustdoc — without this, future readers may incorrectly trust the strategy-returned value, which is always 0 by design.
- Consumed by TASK-0524 (trait), TASK-0530 (round-robin returns), TASK-0531 (FENNEL returns), TASK-0554 (pipeline stitch), TASK-0556 (ChunkedPartitionResult assembly).

## DAG Links

- **Predecessors:** none.
- **Successors:** TASK-0524, TASK-0530, TASK-0531, TASK-0554, TASK-0556.
