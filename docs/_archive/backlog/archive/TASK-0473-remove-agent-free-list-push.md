# TASK-0473: Modify `remove_agent` to push to free-list + purge `freeport_redirects`

**Spec:** SPEC-22 §3.1 R2, R7; §4.3 (remove_agent body).
**Requirements:** R2 (push id onto free-list after disconnect), R7 (no PortRef::AgentPort references to recycled IDs); §4.1 prose closure for `freeport_redirects` × recycle (closes SC-001 second surface); §4.3 `is_border_protected` guard (R10b/R10c — wiring deferred to TASK-0482).
**Priority:** P0 (foundational; the free-list push path).
**Status:** TODO
**Depends on:** TASK-0471 (free_list field exists), TASK-0464 (SPEC-02 R12 amendment).
**Blocked by:** none
**Estimated complexity:** S (~40 LoC production + ~60 LoC tests)
**Bundle:** SPEC-22 Arena Management — Phase B (free-list core implementation).

## Context

SPEC-22 §4.3 provides the complete `remove_agent` body. After existing SPEC-02 R12 behavior (mark slot None, disconnect ports), the new steps are:

1. Purge any `freeport_redirects` entry keyed by the agent's ID via `self.freeport_redirects.remove(&(id as u32))`. Closes SC-001 second surface — without this, a stale redirect would reference a different agent after the slot is recycled.
2. Push `id` onto `free_list` UNLESS `self.is_border_protected(id)` returns `true` (R10c protected-tombstone branch). In single-net (non-distributed) and v1 (non-delta) contexts, `is_border_protected` is a no-op returning `false`, so the push always happens. The actual border-state wiring is in TASK-0482.

## Acceptance Criteria

- [ ] Modify `Net::remove_agent` in `relativist-core/src/net/core.rs` per SPEC-22 §4.3 reference body.
- [ ] After existing port-disconnect loop and `agents[id] = None`: call `self.freeport_redirects.remove(&(id as u32))`.
- [ ] If `!self.is_border_protected(id)`: `self.free_list.push(id)`. Else: do nothing (slot becomes a protected tombstone — R10c).
- [ ] Add a private `fn is_border_protected(&self, _id: AgentId) -> bool { false }` placeholder method on `Net`. Real wiring lands in TASK-0482; this stub guarantees R10b/R10c semantics in non-distributed contexts.
- [ ] R6 no-duplicates assertion: `debug_assert!(!self.free_list.contains(&id))` immediately before the push (closes SC-018; covered fully in TASK-0474).
- [ ] Documentation cites SPEC-22 R2, R7, R10b, R10c.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/net/core.rs` | modify | Replace `Net::remove_agent` body; add private `is_border_protected` stub. |

## Key Types / Signatures

```rust
impl Net {
    pub fn remove_agent(&mut self, id: AgentId) {
        if let Some(agent) = self.agents[id as usize] {
            // ... existing port-disconnect loop (SPEC-02 R12) ...
            self.agents[id as usize] = None;

            // SC-001 second surface: purge stale freeport_redirects entry
            self.freeport_redirects.remove(&(id as u32));

            // R2 + R10b/R10c: push to free-list unless border-protected
            if !self.is_border_protected(id) {
                debug_assert!(!self.free_list.contains(&id));  // R6 (covered in TASK-0474)
                self.free_list.push(id);
            }
        }
    }

    /// SPEC-22 R10b/R10c default: never protected in pure-net contexts.
    /// Distributed call sites override via injected border state (TASK-0482).
    fn is_border_protected(&self, _id: AgentId) -> bool {
        false
    }
}
```

## Test Expectations (forward-ref)

TEST-SPEC-0473 — covers:
- T1, T3, T4 (basic recycling, exhaustion, port slot reinit) — see TASK-0472 forward-ref.
- T5: Reduction with recycling — 100 CON-CON annihilation pairs leaves `next_id == 200`, free-list = 200, `count_live_agents == 0`.
- `freeport_redirects_purged_on_recycle` — set a redirect entry keyed by an agent's ID; remove the agent; assert the entry is gone.

## Invariants Touched

- I3' (uniqueness — push is preceded by full slot clearing).
- D1c (FreePort bijectivity — preserved by the `freeport_redirects` purge).
- R7 (no AgentPort references to recycled IDs — guaranteed by the disconnect loop).

## Notes

- The `is_border_protected` stub method enables TASK-0482 to override behavior without re-touching `remove_agent`. The default `false` preserves single-net / v1 / non-delta semantics.
- The `debug_assert!(!self.free_list.contains(&id))` is partial here; the full R6 closure (with the optional `HashSet<AgentId>` shadow) lands in TASK-0474.

## DAG Links

- **Predecessors:** TASK-0471, TASK-0464.
- **Successors:** TASK-0474 (R6 no-duplicates closure), TASK-0482 (RecyclePolicy + border-protected wiring), TASK-0495 (I3' debug assertions).
