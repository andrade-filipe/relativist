# TASK-0475: Serde + bincode round-trip for `Net.free_list`

**Spec:** SPEC-22 §3.1 R9; §7.1 T8 (serialization round-trip).
**Requirements:** R9 (free-list MUST be included in serde serialization/deserialization; deserialized net MUST have a valid free-list — every ID in free-list corresponds to a `None` slot in the arena).
**Priority:** P0 (wire compatibility / persisted state correctness).
**Status:** TODO
**Depends on:** TASK-0471 (field exists), TASK-0473 (remove_agent path produces free-list state).
**Blocked by:** none
**Estimated complexity:** S (~30 LoC production + ~80 LoC tests; mostly test surface)
**Bundle:** SPEC-22 Arena Management — Phase B (free-list core implementation).

## Context

The `free_list: Vec<AgentId>` field participates in serde + bincode by default (no `#[serde(skip)]`). R9 mandates: deserialized net's free-list must be valid (every ID corresponds to a `None` slot in the arena); every `None` slot in the arena SHOULD be in the free-list (slots that were `None` before free-list introduction MAY not be in the free-list for backward compatibility — i.e., a v3-format net deserialized into a v3 binary may have `None` slots NOT in the free-list, and that's acceptable; the slots are simply not eligible for recycling until a future `remove_agent` re-adds them).

## Acceptance Criteria

- [ ] Confirm `free_list: Vec<AgentId>` participates in serde via the existing `#[derive(Serialize, Deserialize)]` on `Net`. No `#[serde(skip)]`.
- [ ] Confirm `free_list` participates in rkyv under `feature = "zero-copy"` via the existing `#[cfg_attr(feature = "zero-copy", derive(...))]`. No rkyv skip.
- [ ] Add a `Net::validate_free_list(&self) -> Result<(), NetError>` helper that verifies R9 post-condition: for every `id` in `free_list`, `agents[id as usize].is_none()`. (This is also the R9 deserializer-side post-condition check.)
- [ ] Document in Rustdoc the v3-vs-v2 compatibility note: v3 deserializers MAY tolerate v2 nets as having empty `free_list` (deserializer-defined per SPEC-22 §6).
- [ ] Test T8 (SPEC-22 §7.1): build net with non-empty free-list, serialize via bincode, deserialize, assert `is_behaviorally_equal` (helper from TASK-0491) and free-list set-equality.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/net/core.rs` | modify | Add `Net::validate_free_list` helper. Confirm derive propagation includes `free_list`. |
| `relativist-core/src/error.rs` | modify | Add `NetError::FreeListInvalid { id: AgentId, reason: &'static str }` variant. |

## Key Types / Signatures

```rust
impl Net {
    /// SPEC-22 R9 post-condition: every ID in free_list MUST correspond to a None slot.
    pub fn validate_free_list(&self) -> Result<(), NetError> {
        for &id in &self.free_list {
            if self.agents.get(id as usize).and_then(|s| s.as_ref()).is_some() {
                return Err(NetError::FreeListInvalid { id, reason: "slot is Some" });
            }
        }
        Ok(())
    }
}
```

## Test Expectations (forward-ref)

TEST-SPEC-0475:
- T8 (full): bincode round-trip; assert `is_behaviorally_equal` (the helper test joins with TASK-0491's R21 closure).
- `validate_free_list_rejects_some_slot` — push an ID whose slot is Some; assert validation fails.
- `serde_round_trip_preserves_free_list_order` — LIFO order preserved by `Vec` serde.

## Invariants Touched

- R9 (post-condition validity).
- R26 (round-trip identity, indirectly).

## Notes

- T8 forward-references the `is_behaviorally_equal` helper authored in TASK-0491. If TASK-0475 lands first, the test can fall back to byte-equality with explicit normalization (trailing-slot trim) and migrate to `is_behaviorally_equal` once TASK-0491 lands.
- The wire-version rejection clause (R9a) is in TASK-0476, NOT here. This task only handles the structural serde participation.

## DAG Links

- **Predecessors:** TASK-0471, TASK-0473.
- **Successors:** TASK-0476 (PROTOCOL_VERSION bump; T8a test), TASK-0491 (is_behaviorally_equal helper).
