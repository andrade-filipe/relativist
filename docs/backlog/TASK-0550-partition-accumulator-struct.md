# TASK-0550: Define `PartitionAccumulator` (SparseNet-backed) struct + constructor

**Spec:** SPEC-21 §4.9 (closes SC-006); SPEC-21 §3.3 R23.
**Requirements:** SparseNet-backed accumulator per §4.9 — defaults to `SparseNet`, finalizes to `Net` only at pipeline end.
**Priority:** P0 (foundational accumulator type for the pipeline orchestrator).
**Status:** TODO
**Depends on:** TASK-0486 (SparseNet struct exists, SPEC-22), TASK-0489 (Net::to_sparse, SPEC-22), TASK-0520..0524 (Phase B types).
**Blocked by:** none
**Estimated complexity:** S (~80 LoC production + ~80 LoC tests).
**Bundle:** SPEC-21 Streaming Generation — Phase E (pipeline orchestrator).

## Context

Per SPEC-21 §4.9 (post-SC-006 redesign), each `PartitionAccumulator` wraps EITHER a `SparseNet` (SPEC-22 R22 / §4.4) OR a dense `Net` (SPEC-02). Defaults to `SparseNet` because the streaming pipeline does not know up-front whether the assignment strategy will produce contiguous or non-contiguous IDs.

The dense-arena pathology under FENNEL non-contiguous assignment (`id_range × PORTS_PER_SLOT` PortRef entries to hold ~3000 live ports when assigning agent 0 and agent 5_000_000 to the same worker) is **eliminated** by SparseNet adoption. T10 (peak-memory) measures this.

```rust
enum AccumulatorNet {
    Sparse(SparseNet),
    Dense(Net),
}

struct PartitionAccumulator {
    subnet: AccumulatorNet,
    free_port_index: HashMap<u32, PortRef>,
    worker_id: WorkerId,
    min_assigned_id: Option<AgentId>,
    max_assigned_id: Option<AgentId>,
    live_agent_count: u64,
}

impl PartitionAccumulator {
    fn new(worker_id: WorkerId) -> Self {
        Self {
            subnet: AccumulatorNet::Sparse(SparseNet::new()),
            free_port_index: HashMap::new(),
            worker_id,
            min_assigned_id: None,
            max_assigned_id: None,
            live_agent_count: 0,
        }
    }
}
```

**Frame-reuse pattern (per AC-010 cross-reference, §4.9 intro):** one persistent SparseNet per worker, mutated chunk-by-chunk; NOT reallocated per chunk. This mirrors HVM4 WNF goto-state-machine frame reuse.

**R23 reconciliation (per §4.9 closing).** R23's "MUST be sized to `max_agent_id_in_this_worker + 1`" applies to the **dense-finalized form** (TASK-0552 `finalize()`), NOT the in-progress SparseNet accumulator. Implementers MUST NOT pre-size a dense Vec at construction time hoping to "amortize" the resize — that resurrects SC-006.

## Acceptance Criteria

- [ ] Define `pub(crate) enum AccumulatorNet { Sparse(SparseNet), Dense(Net) }` in `relativist-core/src/partition/streaming.rs`.
- [ ] Define `pub(crate) struct PartitionAccumulator` with the 6 fields per §4.9 verbatim.
- [ ] Constructor `pub fn new(worker_id: WorkerId) -> Self` defaults to `AccumulatorNet::Sparse(SparseNet::new())`. Empty free_port_index, None min/max, 0 live count.
- [ ] Visibility: `pub(crate)` (internal pipeline type, not part of public API).
- [ ] Doc-comment includes the §4.9 frame-reuse rationale (AC-010 cross-reference) and the R23 reconciliation note.
- [ ] R23 forbidden pattern (pre-sizing dense Vec at construction) is NOT used — verified by code review and integration test.
- [ ] No `unwrap()`; no `unsafe`.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/partition/streaming.rs` | modify | Add `AccumulatorNet` enum + `PartitionAccumulator` struct + `new` constructor. |

## Key Types / Signatures

```rust
pub(crate) enum AccumulatorNet {
    Sparse(SparseNet),
    Dense(Net),
}

pub(crate) struct PartitionAccumulator {
    pub(crate) subnet: AccumulatorNet,
    pub(crate) free_port_index: HashMap<u32, PortRef>,
    pub(crate) worker_id: WorkerId,
    pub(crate) min_assigned_id: Option<AgentId>,
    pub(crate) max_assigned_id: Option<AgentId>,
    pub(crate) live_agent_count: u64,
}

impl PartitionAccumulator {
    pub(crate) fn new(worker_id: WorkerId) -> Self;
}
```

## Test Expectations (forward-ref)

TEST-SPEC-0550:
- Construct fresh accumulator; verify defaults (SparseNet variant, empty fields).
- Verify SparseNet variant is the construction default (NOT Dense — fail-fast if a future refactor flips this).
- Multi-worker construction: instantiate 4 accumulators with `WorkerId(0..4)`, verify worker_id field correctness.

## Invariants Touched

- T1 / I1 / I2 (preserved — SparseNet honors these per SPEC-22 R26).
- D4 (preserved post-finalize — TASK-0552 enforces).
- I3' (preserved — SparseNet `agents: HashMap<AgentId, Agent>` per SPEC-22 R13/R29).

## Notes

- This task only defines the type and constructor. `add_agent` / `connect` are TASK-0551; `finalize` is TASK-0552.
- The dense path is reachable only via `finalize()` post-conversion (per §4.9 closing); the in-progress representation is ALWAYS sparse.
- Consumed by TASK-0551 (add_agent + connect operations), TASK-0552 (finalize), TASK-0554 (pipeline allocates one per worker).

## DAG Links

- **Predecessors:** TASK-0486 (SparseNet exists), TASK-0489 (Net::to_sparse — though not used at construction), TASK-0524 (StreamingPartitionStrategy types).
- **Successors:** TASK-0551, TASK-0552.
