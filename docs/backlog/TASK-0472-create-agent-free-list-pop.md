# TASK-0472: Modify `create_agent` to pop from free-list before `next_id` allocation

**Spec:** SPEC-22 §3.1 R3, R4, R5; §4.2 (create_agent body).
**Requirements:** R3 (free-list pop OR next_id increment), R4 (slot reuse semantics — Some(Agent), DISCONNECTED ports, no arena expansion), R5 (LIFO ordering).
**Priority:** P0 (foundational; the recycle path).
**Status:** TODO
**Depends on:** TASK-0471 (free_list field exists), TASK-0463 (SPEC-02 R11 amendment), TASK-0462 (SPEC-02 R10 amendment).
**Blocked by:** none
**Estimated complexity:** S (~50 LoC production + ~80 LoC tests)
**Bundle:** SPEC-22 Arena Management — Phase B (free-list core implementation).

## Context

SPEC-22 §4.2 provides the complete `create_agent` body. The recycle path:
1. `self.free_list.pop()` (LIFO, R5).
2. Defensive `debug_assert!` that `agents[id].is_none()` and port slots are `DISCONNECTED`.
3. `agents[id] = Some(Agent { symbol, id })`; re-initialize port slots to `DISCONNECTED` (defensive; R4(b)).
4. Do NOT expand arena (R4(c)) and do NOT increment `next_id` (R3 explicit).

The fresh-allocation path is preserved: increment `next_id`, expand arena/ports if needed, write the new agent. R10 (per-worker ID range) is enforced at the call-site level (i.e., when `create_agent` runs inside a worker context, the free-list it operates on contains only IDs from `[id_range.start, id_range.end)` — guaranteed by `build_subnet` per TASK-0481).

## Acceptance Criteria

- [ ] Modify `Net::create_agent` in `relativist-core/src/net/core.rs` to first attempt `self.free_list.pop()`.
- [ ] On free-list hit: defensive `debug_assert!(self.agents[id as usize].is_none())` and `debug_assert!(self.ports[base + offset] == DISCONNECTED)` for each port slot. Then write `Some(Agent { symbol, id })` and re-initialize the 3 port slots to `DISCONNECTED`. Do NOT increment `next_id`. Do NOT expand arena.
- [ ] On free-list miss (empty): existing fresh-allocation path unchanged — increment `next_id`, expand arena/ports if needed.
- [ ] Returns the assigned `AgentId` regardless of path.
- [ ] Postcondition: `agents[returned_id].is_some()`, `agents[returned_id].symbol == symbol`.
- [ ] R28 (always-on): no feature gate around the free-list-pop branch.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/net/core.rs` | modify | Replace `Net::create_agent` body with the SPEC-22 §4.2 reference implementation. |

## Key Types / Signatures

(Same signature as before; only body changes.)

```rust
impl Net {
    pub fn create_agent(&mut self, symbol: Symbol) -> AgentId {
        if let Some(id) = self.free_list.pop() {
            // Reuse recycled slot (R3, R4, R5 LIFO)
            // ... (see SPEC-22 §4.2 for full body)
            id
        } else {
            // Fresh allocation (existing path; R3 fall-through)
            // ... (existing body)
        }
    }
}
```

## Test Expectations (forward-ref)

TEST-SPEC-0472 — covers SPEC-22 §7.1:
- T1: Basic recycling — create 3, remove middle, create reuses middle ID.
- T2: LIFO ordering — create 5 (IDs 0-4), remove IDs 1, 3, 2 in order, create 3 receives IDs 2, 3, 1; `next_id` unchanged at 5. (Pure-driver test per SC-019/SC-020 annotation.)
- T3: Free-list exhaustion — create 3, remove all, create 5: first 3 reuse, last 2 fresh.
- T4: Port slot reinitialization — recycled ERA's auxiliary slots are DISCONNECTED (no stale CON connections).
- T6: Commutation recycling (CON-DUP) — `next_id == 4` (original 2 + 2 new), free-list empty.

## Invariants Touched

- I3' (uniqueness — preserved: pop removes ID from free-list before assignment; only one live agent ever holds the ID).
- R5 (LIFO — `Vec::pop` from end is O(1) and preserves LIFO).
- T1 (Port Linearity — preserved: ports re-initialized to DISCONNECTED before `connect` calls).

## Notes

- The defensive `debug_assert!` for the port slot is a safeguard against R7 violation. In release builds these are compiled out; the postcondition is preserved by the `remove_agent` invariant.
- The `is_border_protected(id)` check is NOT in `create_agent` (it's in `remove_agent` per R10c). However, R10b under Strategy B requires that *if* we pop a border-referenced ID, we re-push and try fresh — this conditional check lives in TASK-0482's wiring (the `RecyclePolicy::BorderClean` strategy intercepts the pop site). For now (this task), assume the free-list contains only safe-to-recycle IDs; TASK-0482 strengthens this.

## DAG Links

- **Predecessors:** TASK-0471, TASK-0463, TASK-0462.
- **Successors:** TASK-0480 (per-worker ID range constraint), TASK-0482 (RecyclePolicy wiring), TASK-0497 (SPEC-03 assertion audit).
