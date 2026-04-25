# TASK-0551: `PartitionAccumulator::add_agent` + `connect` operations

**Spec:** SPEC-21 §4.9 (closes SC-006 partial); SPEC-21 §3.3 R23 (R23 reconciliation — sparse in-progress, dense at finalize).
**Requirements:** §4.9 add_agent (delegates to Sparse/Dense subnet), connect (updates free_port_index + delegates), per the §4.9 implementation skeleton.
**Priority:** P0 (operations consumed by every install_connection call in the pipeline).
**Status:** TODO
**Depends on:** TASK-0550 (PartitionAccumulator struct + constructor), TASK-0487 (SparseNet operations parity, SPEC-22).
**Blocked by:** none
**Estimated complexity:** S (~70 LoC production + ~100 LoC tests).
**Bundle:** SPEC-21 Streaming Generation — Phase E (pipeline orchestrator).

## Context

Per SPEC-21 §4.9:

```rust
fn add_agent(&mut self, id: AgentId, symbol: Symbol) {
    match &mut self.subnet {
        AccumulatorNet::Sparse(s) => s.create_agent_at(id, symbol),
        AccumulatorNet::Dense(n) => n.create_agent_at(id, symbol),
    }
    self.min_assigned_id = Some(self.min_assigned_id.map_or(id, |m| m.min(id)));
    self.max_assigned_id = Some(self.max_assigned_id.map_or(id, |m| m.max(id)));
    self.live_agent_count += 1;
}

fn connect(&mut self, a: PortRef, b: PortRef) {
    if let PortRef::FreePort(bid) = b { self.free_port_index.insert(bid, a); }
    if let PortRef::FreePort(bid) = a { self.free_port_index.insert(bid, b); }
    match &mut self.subnet {
        AccumulatorNet::Sparse(s) => s.connect(a, b),
        AccumulatorNet::Dense(n) => n.connect(a, b),
    }
}
```

**Invariants maintained:**
- `min_assigned_id` / `max_assigned_id` track the ID range for finalize (TASK-0552) — used to compute the SparseNet → dense `to_dense(Some(range))` conversion bounds.
- `live_agent_count` is the sparse-equivalent of `count_live_agents` (SPEC-22 R11) for this accumulator; used at finalize-time to enforce the SPEC-22 R10a/R22 4×-threshold.
- `free_port_index` is a reverse index of boundary FreePorts, populated when `connect` is called with at least one `FreePort(bid)` endpoint.

`SparseNet::create_agent_at` and `SparseNet::connect` are SPEC-22 operations from TASK-0487. `Net::create_agent_at` and `Net::connect` are SPEC-02 operations (preserved unchanged).

## Acceptance Criteria

- [ ] Implement `pub(crate) fn add_agent(&mut self, id: AgentId, symbol: Symbol)` per §4.9 verbatim. Delegates to `SparseNet::create_agent_at` for sparse variant and `Net::create_agent_at` for dense variant.
- [ ] Implement `pub(crate) fn connect(&mut self, a: PortRef, b: PortRef)` per §4.9 verbatim. Updates `free_port_index` BEFORE delegating to subnet's `connect`.
- [ ] `add_agent` updates `min_assigned_id`, `max_assigned_id`, `live_agent_count` fields.
- [ ] If a `FreePort(bid)` appears on either side of `connect`, it is inserted into `free_port_index` keyed by `bid`. If both sides are `FreePort`, both are inserted (which is technically degenerate — DEVELOPER MUST handle by adopting one or both as the canonical index entries; document the chosen policy in the doc-comment).
- [ ] No `unwrap()`; assertion-free in release builds.
- [ ] T1 invariant maintained in debug builds: every connect produces a bidirectional wire (delegated via SparseNet/Net `connect` which honors I1/I2 per SPEC-22 R26).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/partition/streaming.rs` | modify | Add `add_agent` and `connect` impl methods on `PartitionAccumulator`. |

## Key Types / Signatures

```rust
impl PartitionAccumulator {
    pub(crate) fn add_agent(&mut self, id: AgentId, symbol: Symbol);
    pub(crate) fn connect(&mut self, a: PortRef, b: PortRef);
}
```

## Test Expectations (forward-ref)

TEST-SPEC-0551:
- Add 100 agents to a sparse accumulator; verify `live_agent_count == 100`, `min_assigned_id == Some(0)`, `max_assigned_id == Some(99)` (under contiguous IDs).
- Add non-contiguous IDs (0 and 5_000_000); verify `min == 0`, `max == 5_000_000`, `live_count == 2`. Critically: verify the SparseNet is NOT internally allocating 5M+ ports (memory test against TASK-0466 / SPEC-22 R30 threshold).
- Connect with `FreePort(7)` on one side; verify `free_port_index[7]` contains the other endpoint.
- T1 / I1 / I2 preservation per SPEC-22 R26 (delegated test surface).

## Invariants Touched

- I3' uniqueness — preserved via SparseNet `create_agent_at` (per SPEC-22 R14/R15).
- T1 (port linearity) — preserved via SparseNet/Net `connect` (per SPEC-22 R26 / SPEC-02 R13).
- C2 (border wires registered) — partially established via free_port_index update.

## Notes

- The `create_agent_at(id, symbol)` operation is the SparseNet/Net method that creates an agent at a SPECIFIC id (not the next-available id from `next_id`). DEVELOPER MUST verify SparseNet has this method per TASK-0487; if it does not exist as named, adopt the equivalent from SPEC-22's actual operations API.
- The free_port_index policy when both sides are `FreePort` is unusual but theoretically possible (a wire connecting two boundary FreePorts). Recommendation: insert both — symmetric behavior simplifies the merge protocol's lookup discipline.
- Consumed by TASK-0553 (`install_connection` calls these), TASK-0554 (pipeline orchestrator), TASK-0555 (pending store resolution calls these).

## DAG Links

- **Predecessors:** TASK-0550, TASK-0487.
- **Successors:** TASK-0552, TASK-0553, TASK-0554, TASK-0555.
