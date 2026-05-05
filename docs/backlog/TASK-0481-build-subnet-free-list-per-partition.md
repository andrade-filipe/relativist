# TASK-0481: `build_subnet` populates partition free-list with in-range `None` slots (R10a)

**Spec:** SPEC-22 §3.1 R10a; §3.8 A7 (consumes the SPEC-04 amendment).
**Requirements:** R10a (build_subnet MUST populate the free-list of each partition with the `None` slots that fall within `[id_range.start, id_range.end)`; IDs outside the range are forbidden in this partition's free-list — closes SC-006). MUST upgrade closes SC-009 at the dense path.
**Priority:** P0 (D4 preservation; partition-correct free-list).
**Status:** TODO
**Depends on:** TASK-0466 (SPEC-04 §4.5 amendment), TASK-0471 (free_list field exists), TASK-0480 (id_range field exists).
**Blocked by:** none
**Estimated complexity:** S (~50 LoC production + ~80 LoC tests)
**Bundle:** SPEC-22 Arena Management — Phase C (distributed integration).

## Context

`build_subnet` produces a partition subnet for worker `i`. After live agents are placed in the dense arena, the function must walk `[partition.id_range.start, partition.id_range.end)` and push every index whose `agents[i]` is `None` onto the partition's `free_list`. IDs outside the partition's range MUST NOT be added — they belong to other partitions; using them would violate D4. The MUST upgrade (was SHOULD in v1 of SPEC-22) closes SC-009: under `ContiguousIdStrategy`, the dense allocation `vec![None; max_id + 1]` would be pathological at M5 scale (100M agents). This task implements the dense-path R10a; the sparse-then-dense fallback (R22) is in TASK-0492.

## Acceptance Criteria

- [ ] Modify `build_subnet` in `relativist-core/src/partition/helpers.rs` (or current canonical location) to, after live agents are placed in the dense arena, walk `[partition.id_range.start, partition.id_range.end)` and push every `None` index onto the partition's `free_list`.
- [ ] The free-list is populated in any LIFO-compatible order; recommended: ascending iteration + push, so the most recent (highest in-range) `None` index ends up at the top of the stack.
- [ ] Set the partition `Net.id_range = Some(partition.id_range.clone())` (consumed by TASK-0480 defensive check and TASK-0482 RecyclePolicy).
- [ ] Edge case: when `partition.id_range.end > arena_len`, only iterate up to `arena_len.min(partition.id_range.end)` to avoid out-of-bounds.
- [ ] Edge case: an empty `id_range` (start == end) results in an empty free-list — no error.
- [ ] Test: build a net with 200 agents (IDs 0-199); split into 2 partitions `[0, 100)` and `[100, 200)`; remove agents 50, 75, 90 from net BEFORE split; assert partition 0's free-list is exactly `{50, 75, 90}` (in some LIFO-valid order); partition 1's free-list is empty.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/partition/helpers.rs` | modify | Extend `build_subnet` per SPEC-22 R10a — populate free-list + set `id_range`. |

## Key Types / Signatures

```rust
pub fn build_subnet(
    net: &Net,
    worker_agents: &HashSet<AgentId>,
    sigma: &SubstitutionTable,
    border_entries: &[BorderEntry],
    id_range: core::ops::Range<AgentId>,
) -> Net {
    // ... existing logic to place agents and ports ...
    let mut subnet = /* the constructed Net */;
    subnet.id_range = Some(id_range.clone());

    // SPEC-22 R10a: populate free-list with in-range None slots.
    let lo = id_range.start as usize;
    let hi = (id_range.end as usize).min(subnet.agents.len());
    for i in lo..hi {
        if subnet.agents[i].is_none() {
            subnet.free_list.push(i as AgentId);
        }
    }
    subnet
}
```

## Test Expectations (forward-ref)

TEST-SPEC-0481:
- `build_subnet_populates_free_list_in_range`.
- `build_subnet_excludes_out_of_range_none_slots`.
- `build_subnet_empty_id_range_yields_empty_free_list`.
- T9 (per-partition recycle stays in range) — joint coverage with TASK-0480.

## Invariants Touched

- D4 (preserved — partition free-list confined to assigned range).
- R10 (precondition supplier — the runtime check in TASK-0480 verifies what this task constructs).

## Notes

- This task is the dense-path R10a implementation. The sparse-path R22 (4× threshold) is in TASK-0492.
- `border_entries` is unchanged here; protected-tombstone tracking uses a separate HashSet shadow added in TASK-0482.

## DAG Links

- **Predecessors:** TASK-0466, TASK-0471, TASK-0480.
- **Successors:** TASK-0480 (defensive check), TASK-0482 (RecyclePolicy::BorderClean uses border_entries), TASK-0492 (sparse build_subnet at threshold).
