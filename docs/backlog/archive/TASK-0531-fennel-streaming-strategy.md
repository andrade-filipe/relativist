# TASK-0531: Implement `FennelStreamingStrategy` (advanced, opt-in)

**Spec:** SPEC-21 §3.1 R5, R6; SPEC-21 §4.4 implementation; SPEC-21 §8 Q3 (alpha disposition).
**Requirements:** R5 (FENNEL/LDG-style streaming strategy with `alpha` configurable), R6 (assignment-cache memory bound: O(total_agents) at ~8 bytes/agent vs ~64 bytes/agent for full net = 8× reduction).
**Priority:** P1 (advanced strategy; SHOULD per R5; FENNEL drops to FUTURE if calibration shows fixed-default `alpha = 1.0` is materially worse than batch FENNEL on representative benchmarks per Q3).
**Status:** TODO
**Depends on:** TASK-0524 (StreamingPartitionStrategy trait).
**Blocked by:** none
**Estimated complexity:** M (~150 LoC production + ~120 LoC tests).
**Bundle:** SPEC-21 Streaming Generation — Phase C (strategy implementations).

## Context

FENNEL/LDG-style streaming strategy. Assigns each agent to the worker where it has the most already-assigned neighbors, with a capacity penalty:

```
score(w) = neighbors_w(A) - alpha * degree(w)
```

where `neighbors_w(A)` = number of A's ports connected to agents already assigned to worker w; `degree(w)` = total agents assigned to w so far; `alpha` = configurable balance parameter (default 1.0 per Q3 disposition).

Per SPEC-21 §4.4:

```rust
pub struct FennelStreamingStrategy {
    assignment_cache: HashMap<AgentId, WorkerId>,  // R6 cache: ~8 bytes/agent
    per_worker_counts: Vec<u64>,
    alpha: f64,
}
```

**Memory bound (R6):** `assignment_cache` grows to at most O(total_agents) entries, storing only the (AgentId, WorkerId) mapping (~8 bytes per agent). 8× reduction compared to holding the full net (~64 bytes per agent).

**Q3 disposition (per SC-009 closure / Q3):** SPEC-21 adopts a fixed default `alpha = 1.0`. Per-benchmark calibration via AC-014 methodology is a separate task (NOT a Stage-1 blocker). Adaptive alpha adjusting as batches arrive is documented as future work. **If calibration shows fixed `alpha = 1.0` is materially worse than batch FENNEL on representative benchmarks, FENNEL drops to FUTURE scope and only RoundRobin remains in v2.**

**REF-TBD annotation (per SC-020 / TCC-root cleanup).** The Tsourakakis 2014 (FENNEL) and Stanton & Kliot 2012 (LDG) citations in the doc-comment MUST carry the `REF-TBD (TCC-root cleanup; not yet registered in docs/theory-bridge.md)` annotation per SPEC-21 §11 Change Log SC-020. The bibliography registration is BIBLIOTECARIO scope, NOT this task's scope.

## Acceptance Criteria

- [ ] Define `pub struct FennelStreamingStrategy` in `relativist-core/src/partition/streaming.rs` with `assignment_cache: HashMap<AgentId, WorkerId>`, `per_worker_counts: Vec<u64>`, `alpha: f64`.
- [ ] Constructor `pub fn new(num_workers: u32, alpha: f64) -> Self` initializes empty cache, zero counts, given alpha. Default alpha helper `pub fn with_default_alpha(num_workers: u32) -> Self` sets `alpha = 1.0` per Q3.
- [ ] Implement `StreamingPartitionStrategy::allocate_batch`:
  - For each agent A in `batch.agents`:
    - Compute `neighbors_w(A)` for each worker w by scanning `batch.connections` for `Resolved` directives where one endpoint is A and the other endpoint's agent is in `assignment_cache`.
    - Compute `score(w) = neighbors_w - alpha * degree_w` for each w; pick `argmax`.
    - On ties, deterministic tiebreak: lowest worker id wins (per R8).
    - Insert `(A.id, chosen_w)` into cache; increment `per_worker_counts[chosen_w]`.
  - Return the assignment vec.
- [ ] Implement `finalize()`: return `StreamingPartitionStats` with cache size = total_agents, per_worker_counts cloned, `border_wire_count: 0` (pipeline-owned), `chunks_processed: 0` (pipeline-owned per SC-021).
- [ ] Cache memory bound asserted in tests: cache.len() == total_agents after final batch (R6 / C1 cross-check).
- [ ] No `unwrap()`; no `panic` on float NaN (use `f64::partial_cmp` with explicit fallback).
- [ ] Doc-comment carries `REF-TBD (TCC-root cleanup)` annotation for FENNEL / LDG citations.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/partition/streaming.rs` | modify | Add `FennelStreamingStrategy` struct + impl. |

## Key Types / Signatures

```rust
pub struct FennelStreamingStrategy {
    assignment_cache: HashMap<AgentId, WorkerId>,
    per_worker_counts: Vec<u64>,
    alpha: f64,
}

impl FennelStreamingStrategy {
    pub fn new(num_workers: u32, alpha: f64) -> Self;
    pub fn with_default_alpha(num_workers: u32) -> Self; // alpha = 1.0
}

impl StreamingPartitionStrategy for FennelStreamingStrategy { ... }
```

## Test Expectations (forward-ref)

TEST-SPEC-0531:
- T9 (strategy independence): for fixed (benchmark, size, chunk_size), test with both Round-Robin and FENNEL; merged results MUST be isomorphic to each other and to the sequential baseline. Verifies partition quality affects performance but never correctness.
- R6 cache memory bound: `assignment_cache.len() == total_agents` after final batch.
- R8 determinism: invoke twice with same input → identical output (incl. tiebreak).
- Fixed-alpha calibration sanity: with `alpha = 1.0` on `dual_tree(64)` num_workers=4, verify load imbalance is not catastrophic (per-worker count within 2× of mean).

## Invariants Touched

- C1 (Complete Agent Coverage) — preserved per R7.
- R8 (determinism) — preserved via deterministic tiebreak.
- R9 (pure Core) — preserved.

## Notes

- This is SHOULD per R5; if calibration in Q3 shows fixed-alpha `alpha = 1.0` materially worse than batch FENNEL, FENNEL drops to FUTURE per Q3 disposition. Document this caveat in the struct's Rustdoc.
- The `argmax` over `neighbors_w - alpha * degree_w` is computed in O(num_workers) per agent; total per-batch cost O(B × W) where B = batch size, W = num_workers. For large W this becomes a hot path — DEVELOPER may pre-compute `degree_w` between agents to reduce constant factors.
- Connection-scanning to count neighbors is O(C) per agent where C = batch.connections.len(). DEVELOPER may build a per-batch reverse-index to amortize.
- Consumed by TASK-0554 (pipeline uses via `&mut dyn StreamingPartitionStrategy`); TASK-0565 wires the `StreamingStrategyConfig::Fennel` variant.

## DAG Links

- **Predecessors:** TASK-0524.
- **Successors:** TASK-0554 (pipeline integration), TASK-0565 (config surface).
