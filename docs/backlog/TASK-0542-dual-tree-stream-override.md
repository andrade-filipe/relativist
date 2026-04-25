# TASK-0542: Implement `dual_tree_stream` override (forward-reference exercise)

**Spec:** SPEC-21 §3.2 R12 (SHOULD), R14 (forward references via `PendingConnection`).
**Requirements:** R12 SHOULD — generators with cross-batch dependencies; R14 (AgentBatch supports forward references).
**Priority:** P1 (SHOULD per R12; primary exercise of the forward-ref pipeline path).
**Status:** TODO
**Depends on:** TASK-0540 (Benchmark trait amendment), TASK-0555 (pending connection store production — for the consumer-side test).
**Blocked by:** none
**Estimated complexity:** M (~120 LoC production + ~150 LoC tests).
**Bundle:** SPEC-21 Streaming Generation — Phase D (generators).

## Context

Per SPEC-21 R14: for generators with cross-batch dependencies (e.g., `dual_tree`), the AgentBatch MUST support forward references via `PendingConnection` entries. The batch carries both resolved connections (both endpoints in the current or previous batches) and pending connections (target agent will appear in a future batch).

`dual_tree` generates a balanced binary tree: leaves first, then internal nodes layer-by-layer up to the root. Cross-batch dependency: leaf-to-parent wires emitted in the leaf's batch are forward references resolved when the parent's batch arrives.

**Memory bound (per §4.7).** The pending store holds at most O(`width_of_current_layer`) entries — bounded by tree width, not tree size. For `dual_tree(size = 2^k)`, max pending ≈ `2^(k-1)` at the leaf-to-internal transition.

**R37g `MAX_PENDING_LIFETIME` interaction.** The default `MAX_PENDING_LIFETIME = 16` is sufficient for trees up to ~2^16 leaves. Larger trees MAY require either (a) generator refactor to emit forward-referenced agents earlier, or (b) increasing `GridConfig.max_pending_lifetime` (per TASK-0566). The streaming variant for `dual_tree(size)` MUST document its forward-ref lifetime characteristic; if measured to exceed `MAX_PENDING_LIFETIME` at default size, streaming mode SHOULD be flagged off and the default-impl path used (per R37g).

## Acceptance Criteria

- [ ] Define `pub fn dual_tree_stream(size: u32, chunk_size: usize) -> Box<dyn Iterator<Item = AgentBatch>>` in `relativist-core/src/io/generators.rs` (or chosen location matching TASK-0541).
- [ ] Generation order: leaves first (batch 1+), internal nodes ascending in tree levels, root last.
- [ ] Each leaf batch emits `Pending { source: (leaf_id, parent_port), target_agent_id: parent_id, target_port: child_port }` for the leaf-to-parent wire (parent does not yet exist).
- [ ] When the parent's batch is emitted, no `Pending` directive is needed from the parent's side — the consumer (TASK-0555) resolves the leaf's pending entry by indexing on `parent_id`.
- [ ] R15 monotonicity preserved: AgentIds increase across batches.
- [ ] R37g lifetime bound: forward references resolve within `MAX_PENDING_LIFETIME = 16` chunks for `size ≤ 2^16` (~65k leaves). For larger sizes, document the bound-violation behavior in Rustdoc (drop to default-impl path).
- [ ] Override `Benchmark::make_net_stream` for the `DualTree` impl analogously to TASK-0541.
- [ ] Pure-Core: no async, no tokio, no I/O.
- [ ] Test: `generate_and_partition_chunked(dual_tree_stream(8, 4), 2, RoundRobin)` produces a complete tree with 7 internal wires (3 leaf-to-internal + 3 internal-to-internal + 1 root-port), zero pending refs at end.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/io/generators.rs` *(or chosen location)* | modify | Add `dual_tree_stream` free function. |
| `relativist-core/src/bench/...` *(DualTree impl)* | modify | Override `Benchmark::make_net_stream`. |

## Key Types / Signatures

```rust
pub fn dual_tree_stream(
    size: u32,
    chunk_size: usize,
) -> Box<dyn Iterator<Item = AgentBatch>>;

impl Benchmark for DualTree {
    fn make_net_stream(
        &self,
        size: u32,
        chunk_size: usize,
    ) -> Box<dyn Iterator<Item = AgentBatch>> {
        dual_tree_stream(size, chunk_size)
    }
}
```

## Test Expectations (forward-ref)

TEST-SPEC-0542:
- T3 (forward reference resolution): generate batch with `Pending` to agent 50; later batch contains agent 50; verify pending resolves and wire installs correctly.
- T7 (end-to-end reduction equivalence): `reduce_all(make_net(dual_tree(8)))` ≡ `run_grid` with streaming pipeline; verify isomorphic results AND identical interaction counts (SPEC-01 T7).
- Pending lifetime for `dual_tree(64)`: max pending entries < `MAX_PENDING_LIFETIME` × max forward refs per chunk.
- R15 monotonicity preserved across batches.

## Invariants Touched

- C2 (Complete Wire Coverage) — preserved via forward-ref resolution path.
- C1 (Complete Agent Coverage) — preserved.
- R15 (monotonicity, generator-phase) — preserved.

## Notes

- The forward-reference resolution discipline is the consumer's concern (TASK-0555 pending store). This task is the producer side.
- The R37g lifetime bound interaction means: if `dual_tree(size)` for some chosen `size` produces forward refs whose lifetimes exceed `MAX_PENDING_LIFETIME`, the stream emits a debug-assert (TASK-0595) and the user MUST either bump `max_pending_lifetime` in GridConfig OR fall back to the default-impl path via TASK-0540.
- This task primarily exercises the forward-ref pipeline; alternative SHOULD-overrides (`mixed_net`, `church_*`, M5 family per TASK-0513 migration table) are deferred — most use the default-impl path.
- Consumed by TASK-0554 (pipeline), TASK-0555 (pending store consumer), TASK-0567 (T8 chunk-size independence on dual_tree), TASK-0600 (regression).

## DAG Links

- **Predecessors:** TASK-0540, TASK-0555 (pending store production — needed for the consumer-side test).
- **Successors:** TASK-0567, TASK-0600.
