# TASK-0411: [SPEC-04 amendment A3+A4] Expose `allocate_border_ids` and `remap_partition_ids`

**Spec:** SPEC-20 §3.8 A3 (closes NF-006 for border_id allocator), §3.8 A4 (closes NF-006 for id remap). Consumed by R24d (border_id rebase on reclaim) and R30 (ID uniqueness after reclaim).
**Requirements:** A3 (`PartitionPlan::allocate_border_ids(count: u32) -> Result<Range<u32>, PartitionError>`), A4 (`remap_partition_ids(partition: Partition, new_range: IdRange) -> Result<Partition, PartitionError>`).
**Priority:** P0 (blocker for SPEC-20 departure recovery TASK-0440, TASK-0442, TASK-0443, and for the re-split path that enforces D3-elastic).
**Status:** TODO
**Depends on:** none (new API surface on existing SPEC-04 types).
**Blocked by:** none
**Estimated complexity:** M (~100-150 LoC production + ~80 LoC tests)
**Bundle:** SPEC-20 Elastic Grid — predecessor-spec amendment cluster.
**Tag:** `[SPEC-04 amendment]`

## Context

SPEC-20 R24d requires a fresh disjoint `border_id` range whenever a reclaimed partition re-enters the system; SPEC-20 R30 requires renumbering a reclaimed partition's agents into a fresh `IdRange` before it participates in `Net::union` (per A7, TASK-0410). SPEC-04's current surface does not expose either primitive: `compute_id_ranges(K_eff)` pre-allocates ranges at the start of a round, but there is no dynamic border_id allocator or a renumbering function.

A3 slots the new border_id allocator as R18a, following SPEC-04's existing R16-R18 cluster (**NOT** R15, which is about FreePort Lafont vs Boundary distinction — the v1 SPEC-20 draft misattributed the target; item 12 in the Round-3 NF closure fixes this). A4 adds R19a as a narrow exception to R19's "no remap" rule, invoked exclusively by SPEC-20 departure recovery.

## Acceptance Criteria

### A3 — `PartitionPlan::allocate_border_ids`

- [ ] Add `pub fn allocate_border_ids(&mut self, count: u32) -> Result<Range<u32>, PartitionError>` to `PartitionPlan` in `relativist-core/src/partition/`.
- [ ] Behavior: returns a fresh disjoint `border_id` range `[next_border_id, next_border_id + count)` and advances the internal cursor atomically.
- [ ] Error path: returns `PartitionError::BorderIdSpaceExhausted { requested: u32, available: u32 }` when `count > u32::MAX - next_border_id`.
- [ ] Add `BorderIdSpaceExhausted { requested: u32, available: u32 }` variant to `PartitionError`.

### A4 — `remap_partition_ids`

- [ ] Add `pub fn remap_partition_ids(partition: Partition, new_range: IdRange) -> Result<Partition, PartitionError>` to `relativist-core/src/partition/` (free function or associated fn; developer discretion).
- [ ] Behavior: renumbers every `AgentId` in `partition` into consecutive ids in `new_range`; updates every internal `PortRef::AgentPort(id, p)` edge; leaves `FreePort` entries unchanged (border_ids are rebased separately via A3).
- [ ] Error path: returns `PartitionError::NewRangeTooSmall { partition_size: u32, range_size: u32 }` when `partition.agent_count() > new_range.len()`.
- [ ] Add `NewRangeTooSmall { partition_size: u32, range_size: u32 }` variant to `PartitionError`.
- [ ] Only supported caller: SPEC-20 departure recovery (document in Rustdoc). v1 reduction continues to require zero remaps.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/partition/plan.rs` *(or wherever `PartitionPlan` lives)* | modify | Add `allocate_border_ids` method + `next_border_id` cursor field if absent. |
| `relativist-core/src/partition/mod.rs` | modify | Export `remap_partition_ids` free fn (or associated fn on `Partition`). |
| `relativist-core/src/partition/error.rs` *(or where `PartitionError` lives)* | modify | Add `BorderIdSpaceExhausted` + `NewRangeTooSmall` variants. |
| `relativist-core/src/partition/remap.rs` | create | New module holding `remap_partition_ids` implementation + unit tests. |

## Key Types / Signatures

```rust
impl PartitionPlan {
    /// SPEC-20 §3.8 A3 (owned by SPEC-04 next revision as R18a).
    pub fn allocate_border_ids(&mut self, count: u32)
        -> Result<Range<u32>, PartitionError>;
}

/// SPEC-20 §3.8 A4 (owned by SPEC-04 next revision as R19a).
/// Renumbers `partition`'s agents into `new_range`. Only supported caller is
/// SPEC-20 departure recovery (R30).
pub fn remap_partition_ids(
    partition: Partition,
    new_range: IdRange,
) -> Result<Partition, PartitionError>;

pub enum PartitionError {
    // ... existing variants ...
    BorderIdSpaceExhausted { requested: u32, available: u32 },  // A3
    NewRangeTooSmall { partition_size: u32, range_size: u32 },  // A4
}
```

## Test Expectations (forward-ref for Stage 2 TEST-GENERATOR)

TEST-SPEC-0411 formalizes:

- `allocate_border_ids_disjoint_successive_calls` — two sequential calls yield non-overlapping ranges.
- `allocate_border_ids_cursor_advances` — internal cursor advances by `count`.
- `allocate_border_ids_exhaustion_error` — near `u32::MAX`, returns `BorderIdSpaceExhausted`.
- `remap_preserves_agent_count_and_symbols` — agent count and symbol distribution preserved.
- `remap_rewrites_internal_edges` — every `PortRef::AgentPort` redirected consistently.
- `remap_leaves_freeports_unchanged` — `FreePort(bid)` untouched.
- `remap_too_small_range_error` — undersized `new_range` returns `NewRangeTooSmall`.

Exercised transitively by EG-U7a (border_id rebase) and EG-U12 (ID range disjointness).

## Invariants Touched

- D4 (ID Uniqueness) — the raison d'être of A4; rebuilt range guarantees disjointness.
- D3 (Border Completeness) — A3's fresh border_id range prevents collision.
- I1, I2 (per-agent invariants) — preserved by construction (edge rewrite is structure-preserving).

## Notes

- **Amendment-target correction**: SPEC-20 v1 draft said "SPEC-04 R15"; A3's correct slot is R18a following the R16-R18 border_id cluster (item 12 polish, Round-3 NF closure pass).
- **Coordinator side only**: `remap_partition_ids` is called by the coordinator in `AcceptingMembershipChanges`; workers never invoke it.
- **FreePort handling**: A4 deliberately leaves `FreePort(bid)` entries unchanged because border_ids are rebased via A3 on a separate axis. SPEC-20 §4.2.2 orchestrates the two calls in the correct order.

## DAG Links

- **Predecessors:** none.
- **Successors:** TASK-0440 (v1 departure re-split), TASK-0442 (delta departure reconstruct), TASK-0443 (departure reclaim orchestrator).
