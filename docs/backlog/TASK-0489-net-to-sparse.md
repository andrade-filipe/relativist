# TASK-0489: `Net::to_sparse()` conversion (R19)

**Spec:** SPEC-22 §3.2 R19; §4.6 (conversion body).
**Requirements:** R19 (Iterate dense arena; insert only `Some(agent)` entries; for each live agent copy port entries from flat array, skipping ERA aux and DISCONNECTED; copy redex_queue, next_id, root directly; copy `freeport_redirects` (closes SC-001 second surface). O(arena_len).
**Priority:** P0 (consumed by R22 sparse build_subnet and R21 round-trip).
**Status:** TODO
**Depends on:** TASK-0486 (SparseNet struct), TASK-0487 (SparseNet ops), TASK-0471 (Net.free_list — for completeness on the Net side).
**Blocked by:** none
**Estimated complexity:** S (~50 LoC production + ~80 LoC tests)
**Bundle:** SPEC-22 Arena Management — Phase D (SparseNet).

## Context

SPEC-22 §4.6 provides the reference body. Walk `self.agents.iter().flatten()`; for each live agent, insert into sparse.agents and copy per-port entries (skipping ERA aux via `total_ports(symbol)` bound and skipping DISCONNECTED). Copy `redex_queue`, `next_id`, `root`. **Critical:** copy `freeport_redirects` (closes SC-001 second surface — earlier drafts silently dropped this).

## Acceptance Criteria

- [ ] Implement `Net::to_sparse(&self) -> SparseNet` in `relativist-core/src/net/core.rs` (or a free function in `sparse.rs` — DEVELOPER's choice; `impl Net` is preferred for ergonomics).
- [ ] Iterate over `self.agents.iter().flatten()` — skips `None` slots automatically.
- [ ] For each live agent, copy port entries `(agent.id, p)` for `p in 0..total_ports(agent.symbol)` from the flat port array, skipping DISCONNECTED.
- [ ] Copy `redex_queue` (clone), `next_id`, `root` directly.
- [ ] **Copy `freeport_redirects.clone()` to the sparse net** (closes SC-001 second surface).
- [ ] Free-list is NOT copied to SparseNet — sparse representation has no tombstones, so the free-list concept does not apply directly. (R13 does not list a free-list field on SparseNet; this is consistent.)
- [ ] Complexity: O(arena_len).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/net/core.rs` | modify | Add `impl Net { pub fn to_sparse(&self) -> SparseNet }`. |

## Key Types / Signatures

(Per SPEC-22 §4.6 — copy verbatim.)

```rust
impl Net {
    pub fn to_sparse(&self) -> SparseNet {
        let live_count = self.count_live_agents();
        let mut sparse = SparseNet::with_capacity(live_count);
        for agent in self.agents.iter().flatten() {
            sparse.agents.insert(agent.id, *agent);
            let num_ports = total_ports(agent.symbol);
            for p in 0..num_ports {
                let idx = port_index(agent.id, p);
                let target = self.ports[idx];
                if target != DISCONNECTED {
                    sparse.ports.insert((agent.id, p), target);
                }
            }
        }
        sparse.redex_queue = self.redex_queue.clone();
        sparse.next_id = self.next_id;
        sparse.root = self.root;
        sparse.freeport_redirects = self.freeport_redirects.clone();
        sparse
    }
}
```

## Test Expectations (forward-ref)

TEST-SPEC-0489:
- `to_sparse_skips_none_slots`.
- `to_sparse_skips_disconnected_ports`.
- `to_sparse_skips_era_auxiliary_ports`.
- `to_sparse_preserves_freeport_redirects` (closes SC-001 second surface).
- T14 / T15 round-trips — joint with TASK-0490 / TASK-0491.

## Invariants Touched

- D1c (FreePort bijectivity — preserved by `freeport_redirects` copy).
- I6 sparse equivalent (R17 — ERA aux not copied).

## Notes

- The conversion is lossy with respect to the free-list (intentional — SparseNet has no free-list). The reverse (`to_dense`) re-derives the free-list from `None` slots within the optional `id_range` (TASK-0490).

## DAG Links

- **Predecessors:** TASK-0486, TASK-0487, TASK-0471.
- **Successors:** TASK-0490 (to_dense), TASK-0491 (round-trip helper), TASK-0492 (sparse build_subnet uses to_sparse).
