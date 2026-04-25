# TASK-0530: Implement `RoundRobinStreamingStrategy` (MVP, default)

**Spec:** SPEC-21 §3.1 R4, R7, R8; SPEC-21 §4.3 implementation.
**Requirements:** R4 (round-robin MVP), R7 (C1 union guarantee), R8 (determinism), §4.3 chunks_processed pipeline-owned note (SC-021 partial).
**Priority:** P0 (MVP default strategy; default for v2; matches `ContiguousIdStrategy` quality for sequential generators).
**Status:** TODO
**Depends on:** TASK-0524 (StreamingPartitionStrategy trait).
**Blocked by:** none
**Estimated complexity:** S (~50 LoC production + ~80 LoC tests).
**Bundle:** SPEC-21 Streaming Generation — Phase C (strategy implementations).

## Context

The simplest streaming strategy: assigns agents in round-robin order across workers. Per SPEC-21 R4 + §4.3:

```rust
pub struct RoundRobinStreamingStrategy {
    counter: u64,
    per_worker_counts: Vec<u64>,
}

impl StreamingPartitionStrategy for RoundRobinStreamingStrategy {
    fn allocate_batch(
        &mut self,
        batch: &AgentBatch,
        num_workers: u32,
    ) -> Vec<(AgentId, WorkerId)> {
        batch.agents.iter().map(|(id, _symbol)| {
            let worker = (self.counter % num_workers as u64) as WorkerId;
            self.counter += 1;
            self.per_worker_counts[worker as usize] += 1;
            (*id, worker)
        }).collect()
    }

    fn finalize(&self) -> StreamingPartitionStats {
        StreamingPartitionStats {
            total_agents: self.counter,
            per_worker_counts: self.per_worker_counts.clone(),
            border_wire_count: 0, // not tracked by round-robin
            chunks_processed: 0,  // PIPELINE-OWNED (SC-021); pipeline stitches.
        }
    }
}
```

**Properties (per §4.3):**
- O(1) per agent, O(B) per batch (B = batch size).
- Zero state beyond counter + per-worker counts.
- Deterministic by construction (R8 — counter is the only state).
- Same partition quality as `ContiguousIdStrategy` (SPEC-04 R22) for sequential generators.
- Ignores graph topology entirely (per Rationale §5.1: correctness independent of partition quality).

**Constructor:** `new(num_workers: u32)` — initializes counter=0 and per_worker_counts vec of length num_workers.

`WorkerId` is the `u32`-backed newtype from SPEC-04 (`pub struct WorkerId(pub u32)` per CLAUDE.md "Newtype pattern for IDs"); the `as WorkerId` cast in §4.3 pseudocode is shorthand for `WorkerId(...)` construction. DEVELOPER MUST adapt to the actual newtype-construction syntax (`WorkerId((self.counter % num_workers as u64) as u32)`).

## Acceptance Criteria

- [ ] Define `pub struct RoundRobinStreamingStrategy` in `relativist-core/src/partition/streaming.rs` with `counter: u64` and `per_worker_counts: Vec<u64>` fields (private if possible; public if API requires).
- [ ] Constructor `pub fn new(num_workers: u32) -> Self` initializes `counter=0` and `per_worker_counts` vec sized `num_workers`.
- [ ] Implement `StreamingPartitionStrategy` trait per §4.3 pseudocode. Adapt `as WorkerId` cast to actual newtype construction.
- [ ] `finalize()` returns `chunks_processed: 0` (pipeline-owned per SC-021); add Rustdoc note pointing to TASK-0554 / §4.6 Step 7.
- [ ] No `unwrap()`; no panic on `num_workers == 0` (return empty allocations or error — DEVELOPER decides; SPEC-21 silent on this edge case, document the chosen behavior).
- [ ] T1 verification: 100 agents in 5 batches of 20, num_workers=4 → agent 0 → worker 0, agent 1 → worker 1, …, agent n → worker n%4.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/partition/streaming.rs` | modify | Add `RoundRobinStreamingStrategy` struct + impl. |

## Key Types / Signatures

```rust
pub struct RoundRobinStreamingStrategy {
    counter: u64,
    per_worker_counts: Vec<u64>,
}

impl RoundRobinStreamingStrategy {
    pub fn new(num_workers: u32) -> Self;
}

impl StreamingPartitionStrategy for RoundRobinStreamingStrategy { ... }
```

## Test Expectations (forward-ref)

TEST-SPEC-0530:
- T1 (RoundRobinStreamingStrategy assignment correctness): 100 agents in 5 batches of 20, num_workers=4; verify each agent assigned exactly once; round-robin order; finalize() per-worker counts match.
- Determinism: invoke `allocate_batch` twice with the same input → identical output (R8).
- C1 cross-batch: union of all assignments covers every agent in every batch with no duplicates (R7).

## Invariants Touched

- C1 (Complete Agent Coverage) — preserved by counter-monotonic dispatch.
- R8 (determinism) — preserved by construction.
- R9 (pure Core) — preserved (no async, no tokio).

## Notes

- The strategy returns `chunks_processed: 0` deliberately; the pipeline (TASK-0554) stitches the real count per SC-021.
- `border_wire_count: 0` is correct — round-robin does not track topology, so border counts are pipeline-side (TASK-0554 increments via `install_connection`).
- This strategy is the **default** per R4; TASK-0565 (GridConfig field) sets `streaming_strategy: StreamingStrategyConfig::RoundRobin` as default.
- Consumed by TASK-0554 (pipeline uses via `&mut dyn StreamingPartitionStrategy`).

## DAG Links

- **Predecessors:** TASK-0524.
- **Successors:** TASK-0554 (pipeline consumes via trait object).
