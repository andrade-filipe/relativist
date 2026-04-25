# TASK-0480: Per-worker ID range constraint on recycle (R10 — partition-local free-list)

**Spec:** SPEC-22 §3.1 R10; §3.3 R25 (D4 preservation under I3').
**Requirements:** R10 (workers MUST draw recycled IDs only from the assigned ID range `[start, end)`; out-of-range IDs MUST NOT be used by that worker).
**Priority:** P0 (D4 preservation under distributed reduction).
**Status:** TODO
**Depends on:** TASK-0472 (create_agent free-list pop), TASK-0481 (build_subnet populates free-list per partition range — provides the precondition).
**Blocked by:** none
**Estimated complexity:** S (~30 LoC production + ~50 LoC tests)
**Bundle:** SPEC-22 Arena Management — Phase C (distributed integration).

## Context

R10 mandates that during distributed reduction, a worker's `create_agent` MUST only pop free-list IDs that fall within the worker's `[id_range.start, id_range.end)`. The mechanism is: `build_subnet` (TASK-0481) ALREADY pre-populates each partition's free-list with only in-range `None` slots. So R10 is enforced **by the precondition that the free-list contains only in-range IDs**, not by a runtime check inside `create_agent`. This task adds a **defensive debug assertion** at `create_agent`'s pop site to verify the precondition (i.e., a worker's `Net.id_range` is consulted if available, and the popped ID is checked against it in debug builds).

## Acceptance Criteria

- [ ] Add an OPTIONAL `pub id_range: Option<core::ops::Range<AgentId>>` field on `Net` (default `None` for non-partitioned contexts; set by `build_subnet` to `Some(partition.id_range.clone())` per TASK-0481).
- [ ] Field has `#[serde(skip)]` and rkyv skip — partition-context state, not on the wire.
- [ ] In `create_agent`'s recycle path, after `self.free_list.pop()` succeeds, add a `debug_assert!` that the popped `id` is within `id_range` if `id_range.is_some()`. In release builds the check is dead-code-eliminated.
- [ ] Test T9 (SPEC-22 §7.1): partition with ID range `[0, 100)` and `[100, 200)`; during reduction on partition 0, all recycled IDs are in `[0, 100)`; same for partition 1.
- [ ] No production change to `create_agent` semantics — the assertion fires only on a *bug* (free-list contains an out-of-range ID).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/net/core.rs` | modify | Add `pub id_range: Option<Range<AgentId>>` field with serde+rkyv skip; debug-assertion in `create_agent`. |

## Key Types / Signatures

```rust
pub struct Net {
    // ... existing fields (incl. free_list) ...
    /// SPEC-22 R10: the partition's owning ID range, if this Net belongs to a
    /// partitioned worker. Set by `build_subnet`; None for whole-net contexts.
    /// Not serialized (partition-context state).
    #[serde(skip)]
    #[cfg_attr(feature = "zero-copy", rkyv(with = rkyv::with::Skip))]
    pub id_range: Option<core::ops::Range<AgentId>>,
}

impl Net {
    pub fn create_agent(&mut self, symbol: Symbol) -> AgentId {
        if let Some(id) = self.free_list.pop() {
            // SPEC-22 R10: defensive — verify ID is in partition's range
            #[cfg(debug_assertions)]
            if let Some(ref range) = self.id_range {
                debug_assert!(range.contains(&id),
                    "SPEC-22 R10 violation: popped id {} not in partition range {:?}",
                    id, range);
            }
            // ... rest as TASK-0472 ...
            id
        } else {
            // ... fresh allocation ...
        }
    }
}
```

## Test Expectations (forward-ref)

TEST-SPEC-0480:
- T9 (SPEC-22 §7.1): full per-partition ID range compliance.
- `id_range_none_skips_assertion` — non-partitioned contexts unaffected.
- `id_range_some_traps_out_of_range_pop` (debug only).

## Invariants Touched

- D4 (ID Uniqueness After Distributed Reduction — preserved by free-list partition-locality).
- R10 (consumed defensively).

## Notes

- The actual prevention of out-of-range IDs in the free-list is in TASK-0481 (build_subnet free-list construction). This task is the *defense in depth* — a runtime check that catches bugs in `build_subnet` or `merge`.
- The `id_range` field is also consumed by TASK-0482 (RecyclePolicy::BorderClean strategy) for `border_entries` lookup.

## DAG Links

- **Predecessors:** TASK-0472, TASK-0481.
- **Successors:** TASK-0482 (RecyclePolicy uses `id_range`); TASK-0492 (sparse build_subnet sets `id_range`).
