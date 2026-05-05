# TASK-0495: I3' uniqueness debug assertions in `remove_agent` / `create_agent` (R24, R25, R27)

**Spec:** SPEC-22 §3.3 R24, R25, R27.
**Requirements:** R24 (I3' uniqueness statement), R25 (D4 preservation under I3'), R27 (debug assertions for free-list consistency: post-`remove_agent` recycle path, post-`remove_agent` protected-tombstone path, post-`create_agent` recycle path, periodic no-port-references-to-free-list-IDs).
**Priority:** P0 (closes the assertion side of the I3 → I3' migration).
**Status:** TODO
**Depends on:** TASK-0460 (I3' amendment), TASK-0472 (create_agent), TASK-0473 (remove_agent), TASK-0482 (RecyclePolicy + protected tombstones).
**Blocked by:** none
**Estimated complexity:** S (~80 LoC production assertions + ~80 LoC tests)
**Bundle:** SPEC-22 Arena Management — Phase E (invariant amendments).

## Context

R27 specifies four assertion families:

1. **After `remove_agent` (recycled path, R10b not triggered):** the freed ID is in the free-list, `agents[id] == None`, port slots are `DISCONNECTED`.
2. **After `remove_agent` (protected tombstone path, R10c):** `agents[id] == None`, port slots `DISCONNECTED`, ID NOT in free-list, ID IS in `protected_tombstones` shadow (debug builds only).
3. **After `create_agent` (recycled path):** ID is no longer in free-list, `agents[id] == Some(_)`, free-list has no duplicates, returned ID is NOT in `protected_tombstones` shadow.
4. **Periodic (debug or per-step):** no free-list ID is referenced by any `PortRef::AgentPort(id, _)` in the port array.

R24 / R25 are stated as the I3' definition + D4 compatibility rationale; the testable surface is family (1)-(4) above.

## Acceptance Criteria

- [ ] Add the four assertion families to the appropriate code paths in `relativist-core/src/net/core.rs`:
  - Family 1: end of `remove_agent` recycle branch (after free_list.push).
  - Family 2: end of `remove_agent` protected-tombstone branch.
  - Family 3: end of `create_agent` recycle branch (after agents[id] = Some(...)).
  - Family 4: a `Net::assert_no_free_list_port_refs(&self)` helper invoked periodically (e.g., at the top of `reduce_step` in debug builds, OR in a dedicated `Net::debug_check_invariants(&self)` method).
- [ ] All assertions wrapped in `debug_assert!` (no runtime cost in release).
- [ ] The `protected_tombstones` shadow consultation is gated on `#[cfg(debug_assertions)]` (per TASK-0482 design).
- [ ] Add a comment block above each assertion citing SPEC-22 R27 and the specific bullet (1/2/3/4).
- [ ] Test T7 (SPEC-22 §7.1): non-trivial net (Church(3) + Church(2) addition); reduce_all with free-list enabled; assert T1 (bidirectionality), I2 (reference validity), I3' (uniqueness — `Net::debug_check_invariants` passes).
- [ ] Test that R27 family 4 catches a synthetic violation: artificially put a free-list ID into a port slot; assert the periodic check fires.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/net/core.rs` | modify | Add the four assertion families; add `assert_no_free_list_port_refs` helper. |

## Key Types / Signatures

```rust
impl Net {
    /// SPEC-22 R27 family (4): periodic check that no PortRef references a
    /// free-list AgentId.
    #[cfg(debug_assertions)]
    pub fn assert_no_free_list_port_refs(&self) {
        let free_set: std::collections::HashSet<_> = self.free_list.iter().copied().collect();
        for port in &self.ports {
            if let PortRef::AgentPort(id, _) = port {
                debug_assert!(
                    !free_set.contains(id),
                    "SPEC-22 R7/R27 violation: PortRef references free-list ID {}",
                    id
                );
            }
        }
    }
}
```

## Test Expectations (forward-ref)

TEST-SPEC-0495:
- T7: post-recycling structural invariants on Church(3) + Church(2).
- `r27_family_1_post_remove_agent_recycle`.
- `r27_family_2_post_remove_agent_protected_tombstone`.
- `r27_family_3_post_create_agent_recycle`.
- `r27_family_4_no_free_list_port_refs`.

## Invariants Touched

- I3' (consumed; assertions verify uniqueness at every state transition).
- T1, I1, I2 (preserved by R7's no-AgentPort-to-free-list-IDs constraint, verified by family 4).
- D4 (preserved by R10/R25; verified transitively).

## Notes

- R27a (SPEC-03 in-rule assertion audit) is a SEPARATE task: TASK-0497.
- Family 4 may be too expensive to call per-step; it's recommended to call it via a `--debug-check-invariants` test-only feature or once at end of `reduce_all`. The DEVELOPER decides at Stage 3.

## DAG Links

- **Predecessors:** TASK-0460, TASK-0472, TASK-0473, TASK-0482.
- **Successors:** TASK-0497 (SPEC-03 in-rule audit), TASK-0500 (regression).
