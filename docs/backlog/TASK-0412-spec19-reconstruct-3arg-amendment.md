# TASK-0412: [SPEC-19 amendment A8] Extend `reconstruct` to accept optional `reclaimed_partitions`

**Spec:** SPEC-20 §3.8 A8 (closes NF-005). Consumed by SPEC-20 §4.2.2 delta-mode departure step 4.
**Requirements:** A8 (3-argument `reconstruct(border_graph, surviving_partitions, reclaimed_partitions)`).
**Priority:** P0 (blocker for SPEC-20 delta-mode departure recovery TASK-0443).
**Status:** TODO
**Depends on:** TASK-0410 (`Net::union` used internally), TASK-0411 (`remap_partition_ids` establishes the disjointness precondition before this call).
**Blocked by:** none (SPEC-19 `reconstruct` is shipped).
**Estimated complexity:** S (~30-50 LoC production + ~60 LoC tests)
**Bundle:** SPEC-20 Elastic Grid — predecessor-spec amendment cluster.
**Tag:** `[SPEC-19 amendment]`

## Context

SPEC-19 R38 currently defines `reconstruct(border_graph, worker_partitions)` (2-argument). SPEC-20 §4.2.2 delta-mode departure step 4 invokes a 3-argument variant where reclaimed partitions (materialized from `retained_last_acked` or `retained_initial`) are included alongside surviving partitions before the subsequent `split()`. The 2-argument signature is preserved as the default (empty `reclaimed_partitions`), so all existing SPEC-19 callers are unaffected.

Disjointness of the `surviving_partitions ∪ reclaimed_partitions` agent-id sets is guaranteed by SPEC-20 construction (`remap_partition_ids` per A4/TASK-0411 renumbers reclaimed partitions before this call).

## Acceptance Criteria

- [ ] Extend the existing `reconstruct` function signature in `relativist-core/src/merge/` (or wherever SPEC-19 R38 lives) with an optional third parameter `reclaimed_partitions: Vec<Partition>` defaulting to the empty vector (Rust lacks default parameters; use function overload via two named entry points OR a single entry with `Vec<Partition>` that callers pass `Vec::new()` to).
- [ ] **Preferred API shape**: single 3-arg function `reconstruct(border_graph, surviving, reclaimed: Vec<Partition>)`. Existing 2-arg callers migrate to pass `Vec::new()` for `reclaimed`.
- [ ] Semantics: treats `surviving_partitions ∪ reclaimed_partitions` as the input partition set (union via `Net::union` under disjointness precondition).
- [ ] When `reclaimed_partitions.is_empty()`, behavior is bit-identical to the existing 2-arg signature (regression guarantee).
- [ ] Downstream SPEC-19 callers (R29 final merge) migrate cleanly.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/merge/` (SPEC-19 R38 site) | modify | Extend `reconstruct` signature; add explicit doc reference to SPEC-20 §3.8 A8. |
| Any existing SPEC-19 call sites (e.g., `protocol/coordinator.rs` R29 final merge) | modify | Pass `Vec::new()` for `reclaimed`. |

## Key Types / Signatures

```rust
/// SPEC-19 R38 (amended by SPEC-20 §3.8 A8). When `reclaimed_partitions` is
/// empty, behavior is identical to the pre-amendment 2-argument version.
pub fn reconstruct(
    border_graph: &BorderGraph,
    surviving_partitions: Vec<Partition>,
    reclaimed_partitions: Vec<Partition>,  // NEW; empty Vec = legacy behavior
) -> Net;
```

## Test Expectations (forward-ref for Stage 2 TEST-GENERATOR)

TEST-SPEC-0412 formalizes:

- `reconstruct_empty_reclaimed_matches_legacy` — bit-exact equivalence for all existing SPEC-19 fixtures.
- `reconstruct_with_one_reclaimed_partition` — agent count = survivors + reclaimed.
- `reconstruct_with_multiple_reclaimed_partitions` — distinct reclaimed partitions unioned correctly.
- `reconstruct_panics_on_overlapping_reclaimed_agent_ids` *(debug-only)* — A7 disjointness precondition violation surfaces clearly.

Exercised transitively by EG-I3-delta and EG-U7c.

## Invariants Touched

- D3 (Border Completeness) — `reconstruct` already owns this; extension does not weaken it.
- D4 (ID Uniqueness) — preserved by the SPEC-20 caller's `remap_partition_ids` precondition.

## Notes

- **Migration**: all existing SPEC-19 2-arg callers need a 1-line edit (`Vec::new()`). Alternatively, a thin wrapper `reconstruct_legacy(bg, survivors)` can be kept for backwards compatibility — developer discretion.
- **Owned by**: SPEC-19 maintainer; this task is the v2-development touch-point for the amendment.

## DAG Links

- **Predecessors:** TASK-0410, TASK-0411.
- **Successors:** TASK-0443 (delta-mode departure reclaim + reconstruct).
