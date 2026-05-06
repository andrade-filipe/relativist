# TASK-0552: `PartitionAccumulator::finalize` to_dense conversion + threshold rejection

**Spec:** SPEC-21 §4.9 finalize implementation; SPEC-21 §3.3 R23 (dense-finalized form size); SPEC-22 R10a / R22 / R30 (4×-threshold + DenseAllocationExceedsThreshold rejection).
**Requirements:** §4.9 finalize signature `(self, id_range, border_id_start, border_id_end) -> Partition`; consumes SPEC-22 `SparseNet::to_dense(Some(id_range))` per R20 closure of SC-006.
**Priority:** P0 (terminal accumulator step; produces SPEC-04 `Partition` consumed by merge).
**Status:** TODO
**Depends on:** TASK-0551 (add_agent + connect), TASK-0490 (SparseNet::to_dense, SPEC-22), TASK-0484 (PartitionError::DenseAllocationExceedsThreshold, SPEC-22).
**Blocked by:** none
**Estimated complexity:** S (~50 LoC production + ~80 LoC tests).
**Bundle:** SPEC-21 Streaming Generation — Phase E (pipeline orchestrator).

## Context

Per SPEC-21 §4.9 finalize implementation:

```rust
fn finalize(self, id_range: IdRange, border_id_start: u32, border_id_end: u32) -> Partition {
    // SPEC-22 §4.6 to_dense(id_range) is the canonical conversion;
    // it preserves freeport_redirects (SPEC-22 R13).
    let dense = match self.subnet {
        AccumulatorNet::Sparse(s) => s.to_dense(Some(id_range.as_range())),
        AccumulatorNet::Dense(n) => n,
    };
    Partition {
        subnet: dense,
        worker_id: self.worker_id,
        free_port_index: self.free_port_index,
        id_range,
        border_id_start,
        border_id_end,
    }
}
```

**Threshold contract (per SPEC-22 R10a / R22 / R30; closes SC-006).** When `id_range > 4 × live_agent_count` at finalize-time, SparseNet is mandatory through the conversion; the dense path SHALL be rejected with `PartitionError::DenseAllocationExceedsThreshold` (SPEC-22 R30). T10 (§7.4) MUST exercise this path.

**R23 reconciliation (per §4.9 closing).** R23's "MUST be sized to `max_agent_id_in_this_worker + 1`" applies to this dense-finalized form, NOT the in-progress SparseNet. `to_dense(Some(id_range))` produces a `Net` whose `agents.len() == id_range.end - id_range.start` and whose port array is sized to `(id_range.end - id_range.start) × PORTS_PER_SLOT` — sized to the partition's owning ID range, not the global `max_agent_id`.

**Result: Return type fallibility.** Because SPEC-22 R30 rejects the dense conversion at threshold, `finalize` MUST return `Result<Partition, PartitionError>` rather than `Partition` (the §4.9 pseudocode uses `Partition` for clarity; production code must use Result).

## Acceptance Criteria

- [ ] Implement `pub(crate) fn finalize(self, id_range: IdRange, border_id_start: u32, border_id_end: u32) -> Result<Partition, PartitionError>` on `PartitionAccumulator`.
- [ ] Match on `subnet`: `Sparse(s)` calls `s.to_dense(Some(id_range.as_range()))` (TASK-0490); `Dense(n)` returns `n` directly.
- [ ] BEFORE the conversion, evaluate the SPEC-22 R10a / R22 / R30 threshold: if `(id_range.end - id_range.start) > 4 * self.live_agent_count`, return `Err(PartitionError::DenseAllocationExceedsThreshold)` per SPEC-22 R30 (TASK-0484).
- [ ] On success, construct `Partition { subnet: dense, worker_id, free_port_index, id_range, border_id_start, border_id_end }` and return `Ok(partition)`.
- [ ] Verify `freeport_redirects` is preserved across the SparseNet→Net conversion (SPEC-22 R13 — verified by TASK-0491 round-trip closure).
- [ ] T10-relevant: in debug builds, log peak SparseNet memory before conversion vs Net memory after (instrumentation hook for the §7.4 test).
- [ ] No `unwrap()`; no panic.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/partition/streaming.rs` | modify | Add `finalize` impl method on `PartitionAccumulator`. |

## Key Types / Signatures

```rust
impl PartitionAccumulator {
    pub(crate) fn finalize(
        self,
        id_range: IdRange,
        border_id_start: u32,
        border_id_end: u32,
    ) -> Result<Partition, PartitionError>;
}
```

## Test Expectations (forward-ref)

TEST-SPEC-0552:
- Finalize on a small contiguous accumulator → `Ok(Partition)` with `subnet.agents.len() == id_range.end - id_range.start`.
- Finalize with `id_range > 4 × live_agent_count` (e.g., 10_000 range, 100 live agents) → `Err(DenseAllocationExceedsThreshold)`.
- T14a (partition-scoped to_dense): finalize correctness verified via SPEC-22 TASK-0490's existing test surface.
- freeport_redirects preserved: round-trip Sparse → finalize → Dense Net contains all original freeport_redirects entries.
- T10 (peak memory): instrument finalize to record peak; T10 in TASK-0584 / regression suite.

## Invariants Touched

- D4 (ID Uniqueness After Distributed Reduction) — preserved via `id_range` scoping.
- R23 (dense-finalized sizing) — enforced at finalize-time.
- M5 memory bound — the threshold rejection prevents the M5 dense-arena pathology.

## Notes

- The decision to return `Result` (rather than `Partition` per §4.9 pseudocode) is a production-code refinement; the spec pseudocode is informative.
- DEVELOPER MUST coordinate with TASK-0556 (`ChunkedPartitionResult` assembly) to handle the error case — the pipeline either propagates `Err` to its caller or aborts with a clear diagnostic.
- The `IdRange::as_range()` conversion is presumed to exist (SPEC-04 / SPEC-22 IdRange type); if not, DEVELOPER adds it as part of this task.
- Consumed by TASK-0556 (final result assembly), TASK-0554 (pipeline orchestrator), TASK-0570 (C1-C3 assertions exercise this path), TASK-0584 (T10 peak-memory measurement).

## DAG Links

- **Predecessors:** TASK-0551, TASK-0490, TASK-0484.
- **Successors:** TASK-0556, TASK-0570, TASK-0584.
