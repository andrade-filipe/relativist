# TASK-0410: [SPEC-02 amendment A7] Implement `Net::union` structural-concatenation primitive

**Spec:** SPEC-20 §3.8 A7 (closes NF-001); consumed by SPEC-20 §4.2.2 v1-mode step 4 and delta-mode step 4.
**Requirements:** A7 (new primitive in SPEC-02 next revision).
**Priority:** P0 (blocker for SPEC-20 departure recovery tasks TASK-0440, TASK-0442, TASK-0443).
**Status:** TODO
**Depends on:** none (operates on existing `Net`/`AgentId`/`FreePort` types from SPEC-02).
**Blocked by:** none
**Estimated complexity:** S (~40-60 LoC production + ~60 LoC tests)
**Bundle:** SPEC-20 Elastic Grid — predecessor-spec amendment cluster.
**Tag:** `[SPEC-02 amendment]`

## Context

SPEC-02 currently defines no operation that combines two `Net` values into one. SPEC-20 §4.2.2 departure recovery (both v1 and delta modes) depends on such a primitive: reclaimed `retained_initial` / `retained_last_acked` partitions are renumbered into a disjoint `IdRange` (via `remap_partition_ids`, TASK-0411) and then unioned with surviving partitions before passing the result into `split()` for the re-partition cycle. A7 formalizes this as `Net::union(self, other: Net) -> Net` with a **disjoint-`AgentId` precondition** that the caller guarantees by construction.

`Net::union` is a *structural* concatenation, NOT a `merge()`. It does not consult `FreePort` cross-references; any border wiring between the two nets is resolved by the subsequent `split()` call via the standard SPEC-04 mechanism.

## Acceptance Criteria

- [ ] Add `Net::union(self, other: Net) -> Net` to `relativist-core/src/net/mod.rs` (or the struct's primary `impl Net` block).
- [ ] Precondition: caller guarantees disjoint `AgentId` ranges between `self` and `other`. Violation triggers a clear `debug_assert!` / `panic!` with message referencing SPEC-20 A7.
- [ ] Implementation concatenates agent arrays; preserves every `FreePort` entry from both sides in the resulting net's free-port list; does NOT detect or resolve cross-net `FreePort` matches.
- [ ] Operation is total when the disjointness precondition holds.
- [ ] Root-port propagation: if either net has a root/designated port concept, the resulting root follows `self`'s (document in Rustdoc).
- [ ] Invariants I1, I2 (per-agent slot validity) preserved by construction.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/net/mod.rs` | modify | Add `pub fn union(self, other: Net) -> Net` with Rustdoc citing SPEC-20 A7 and the disjointness precondition. |
| `relativist-core/src/net/union_tests.rs` *(optional)* | create | Inline unit tests under `#[cfg(test)] mod union_tests`. |

## Key Types / Signatures

```rust
impl Net {
    /// SPEC-20 §3.8 A7 — structural concatenation of two nets under the
    /// disjoint-`AgentId` precondition. The caller MUST ensure that the agent
    /// ID ranges of `self` and `other` do not overlap; SPEC-20 §4.2.2
    /// establishes this via `remap_partition_ids` (A4) + `compute_id_ranges`
    /// (R13) before calling union.
    ///
    /// Does NOT resolve cross-net `FreePort` matches — that is `merge()`'s
    /// responsibility (SPEC-05). Intended as input to a subsequent `split()`.
    pub fn union(self, other: Net) -> Net;
}
```

## Test Expectations (forward-ref for Stage 2 TEST-GENERATOR)

TEST-SPEC-0410 formalizes:

- `union_empty_right` — `a.union(Net::empty()) == a` up to structural equality.
- `union_empty_left` — `Net::empty().union(a) == a`.
- `union_disjoint_ids_preserves_agents` — agent counts sum; no collision.
- `union_preserves_freeports_from_both_sides` — free-port lists are concatenated.
- `union_panics_on_overlapping_ids` *(debug-only)* — precondition violation.

No new EG-* test IDs; `Net::union` is internal plumbing tested by the SPEC-20 departure integration tests (EG-I3, EG-I3-delta, EG-I5a).

## Invariants Touched

- I1, I2 (per-agent invariants) — preserved by construction.
- D4 (ID Uniqueness) — caller's precondition.
- D3 (Border Completeness) — deferred to the subsequent `split()`.

## Notes

- **Design contract**: `Net::union` is strictly cheaper than `merge()` because it does not inspect `FreePort` cross-references. It is a pure memory concatenation + free-port-list concat.
- **Amendment target**: this task is the SPEC-02 maintainer touch-point. Coordinate with ESPECIALISTA EM SPECS to land the formal SPEC-02 next revision that includes the A7 clause.
- **Bridge to SPEC-20**: TASK-0440 (v1 re-split on departure), TASK-0443 (delta re-split on departure) consume this primitive.

## DAG Links

- **Predecessors:** none.
- **Successors:** TASK-0411 (remap_partition_ids); TASK-0440, TASK-0442, TASK-0443 (departure recovery paths).
