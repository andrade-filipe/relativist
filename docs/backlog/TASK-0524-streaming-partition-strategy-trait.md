# TASK-0524: Define `StreamingPartitionStrategy` trait

**Spec:** SPEC-21 §3.1 R1, R2, R3, R7, R8, R9; SPEC-21 §4.2 trait definition.
**Requirements:** R1 (trait definition), R2 (`&mut self` stateful), R3 (allocate_batch + finalize methods), R7 (C1 guarantee), R8 (determinism), R9 (pure Core layer).
**Priority:** P0 (blocker for both strategy implementations and pipeline orchestrator).
**Status:** TODO
**Depends on:** TASK-0521 (AgentBatch), TASK-0522 (StreamingPartitionStats).
**Blocked by:** none
**Estimated complexity:** S (~30 LoC production + ~50 LoC tests including doc-tests).
**Bundle:** SPEC-21 Streaming Generation — Phase B (core types).

## Context

The streaming counterpart of `PartitionStrategy` (SPEC-04 R21). Unlike batch strategies that require the full net to compute the allocation function σ, streaming strategies assign agents to workers incrementally, one batch at a time, using only information available up to the current batch.

Per SPEC-21 §4.2:

```rust
pub trait StreamingPartitionStrategy {
    fn allocate_batch(
        &mut self,
        batch: &AgentBatch,
        num_workers: u32,
    ) -> Vec<(AgentId, WorkerId)>;

    fn finalize(&self) -> StreamingPartitionStats;
}
```

**Trait contract (per R7, R8, R9):**
- Every agent in the batch has exactly one assignment (R7 → C1).
- Every `WorkerId` is in range `[0, num_workers)`.
- No agent is assigned twice (within this batch or across batches).
- Determinism: same sequence of batches + same `num_workers` → identical assignment (R8).
- Pure Core layer: no async, no tokio, no I/O (R9).

Theory anchor (per §4.2 doc-comment): correctness of distributed IC reduction does not depend on partition quality (DISC-004 v2 §1.6, ARG-002 Passo 10) — any allocation function σ satisfying C1-C3 produces a correct result. Streaming strategies trade partition quality for bounded memory usage.

## Acceptance Criteria

- [ ] Define `pub trait StreamingPartitionStrategy` in `relativist-core/src/partition/streaming.rs`.
- [ ] Two methods present per §4.2: `allocate_batch(&mut self, batch: &AgentBatch, num_workers: u32) -> Vec<(AgentId, WorkerId)>` and `finalize(&self) -> StreamingPartitionStats`.
- [ ] Trait Rustdoc cross-references DISC-004 v2 §1.6 and ARG-002 Passo 10 per §4.2 doc-comment text.
- [ ] Trait Rustdoc documents the post-conditions for `allocate_batch` (R7 C1, R8 determinism, R9 pure-Core).
- [ ] No `unwrap()` / no `unsafe` in trait or implementations.
- [ ] Module is pure-Core: NO `tokio`, NO `async`, NO I/O imports — verified via `cargo build` in `partition` module without tokio dependency.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/partition/streaming.rs` | modify | Add `StreamingPartitionStrategy` trait. |
| `relativist-core/src/partition/mod.rs` | modify | Re-export `StreamingPartitionStrategy`. |

## Key Types / Signatures

```rust
pub trait StreamingPartitionStrategy {
    /// Assigns each agent in the batch to a worker.
    ///
    /// Post-conditions:
    /// - Every agent in the batch has exactly one assignment.
    /// - Every WorkerId is in range [0, num_workers).
    /// - No agent is assigned twice (within this batch or across batches).
    fn allocate_batch(
        &mut self,
        batch: &AgentBatch,
        num_workers: u32,
    ) -> Vec<(AgentId, WorkerId)>;

    /// Returns statistics about the partitioning so far.
    ///
    /// `chunks_processed` is pipeline-owned (SC-021); strategies SHOULD
    /// return `0` and let the pipeline stitch the actual count.
    fn finalize(&self) -> StreamingPartitionStats;
}
```

## Test Expectations (forward-ref)

TEST-SPEC-0524:
- Doc-test confirming the trait compiles standalone with both methods.
- Trait-bounds verification: `Box<dyn StreamingPartitionStrategy>` is constructible.
- T1 verification (round-robin determinism — TASK-0530 covers).
- T9 verification (strategy independence — covers via TASK-0531 if FENNEL implemented).

## Invariants Touched

- C1 (Complete Agent Coverage) — preserved by R7 contract.
- D1 (extended for streaming) — preserved by R7 + downstream pipeline.
- R9 pure-Core — preserved by no-async/no-tokio constraint.

## Notes

- The trait is the streaming counterpart of `PartitionStrategy` (SPEC-04 R21); DEVELOPER may add a doc-cross-reference.
- The `&mut self` in `allocate_batch` (R2) is the key API decision — strategies are stateful across batches (degree counters, assignment cache for FENNEL).
- The `WorkerId` newtype is from SPEC-04 (per §4.1 type-origins paragraph); SPEC-21 imports it via `use crate::partition::WorkerId;`.
- Consumed by TASK-0530 (RoundRobin impl), TASK-0531 (FENNEL impl), TASK-0554 (pipeline orchestrator takes `&mut dyn StreamingPartitionStrategy`).

## DAG Links

- **Predecessors:** TASK-0521, TASK-0522.
- **Successors:** TASK-0530, TASK-0531, TASK-0554.
