# TASK-0491: `Net::is_behaviorally_equal` helper + R21 round-trip closure (closes SC-014)

**Spec:** SPEC-22 §3.2 R21 (closes SC-014); §7.2 T14, T15; §7.1 T8.
**Requirements:** R21 (behavioral equality definition); helper `Net::is_behaviorally_equal(&self, other: &Net) -> bool` MUST be provided in `src/net/core.rs` and used by tests T14/T8 instead of `==`. Round-trip MUST satisfy: `Net::to_sparse().to_dense(None)` produces a net **behaviorally equal** to original; `SparseNet::to_dense(None).to_sparse()` produces a sparse net **structurally equal** (full `==`) to the original.
**Priority:** P0 (test-surface anchor; closes SC-014).
**Status:** TODO
**Depends on:** TASK-0489 (to_sparse), TASK-0490 (to_dense), TASK-0471 (Net.free_list).
**Blocked by:** none
**Estimated complexity:** S (~80 LoC production + ~100 LoC tests)
**Bundle:** SPEC-22 Arena Management — Phase D (SparseNet).

## Context

R21 defines behavioral equality verbatim:

> Two `Net` values `n1` and `n2` are *behaviorally equal* iff for every sequence of public-API operations [...], the observable post-state of `n1` after the sequence and the observable post-state of `n2` after the same sequence agree on every public field projection (live-agent set, port-target relation, redex queue contents up to ordering, root, `next_id`, `freeport_redirects`). Specifically: `n1.agents.iter().filter(|x| x.is_some()).collect::<Vec<_>>() == n2.agents.iter().filter(|x| x.is_some()).collect::<Vec<_>>()`, and analogous projections for `ports` (only `AgentPort` entries, ignoring trailing `DISCONNECTED`) and `freeport_redirects` (full equality). Byte-equality of the underlying `Vec`s is NOT required: `agents.len()` and `ports.len()` MAY differ between `n1` and `n2` because trailing `None`/`DISCONNECTED` slots are trimmed by `to_sparse().to_dense()`.

The helper enables T14 / T8 to assert post-conversion equality without flagging a benign `agents.len()` divergence.

## Acceptance Criteria

- [ ] Implement `Net::is_behaviorally_equal(&self, other: &Net) -> bool` in `relativist-core/src/net/core.rs` per the R21 definition.
- [ ] Compare live-agent sets: `self.agents.iter().filter(|x| x.is_some()).collect::<Vec<_>>() == other.agents.iter().filter(|x| x.is_some()).collect::<Vec<_>>()`.
- [ ] Compare port-target relations: project both ports vectors to `(agent_id, port_id) -> AgentPort(target)` entries (i.e., for every live AgentPort source, compare the target). Trailing DISCONNECTED slots are ignored.
- [ ] Compare `redex_queue` up to ordering (canonical sort or HashSet equality).
- [ ] Compare `root`, `next_id`, `freeport_redirects` directly (full equality).
- [ ] Free-list comparison: set-equality (LIFO order is a Vec-serde detail; behavioral equality only requires same set).
- [ ] R21 round-trip 1: `Net::to_sparse().to_dense(None)` produces a net `is_behaviorally_equal` to the original.
- [ ] R21 round-trip 2: `SparseNet::to_dense(None).to_sparse()` produces a sparse net **structurally equal** (`==`) to the original (sparse representations have no trailing-slot ambiguity).
- [ ] Test T14 uses `is_behaviorally_equal`.
- [ ] Test T15 uses full `==` on SparseNet.
- [ ] Test T8 (serde round-trip from TASK-0475) migrates to `is_behaviorally_equal` once this helper lands.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/net/core.rs` | modify | Add `impl Net { pub fn is_behaviorally_equal(&self, other: &Net) -> bool }`. |

## Key Types / Signatures

```rust
impl Net {
    /// SPEC-22 R21 behavioral equality: two nets are equivalent iff they agree
    /// on the live-agent set, port-target relation, redex queue (up to ordering),
    /// root, next_id, freeport_redirects, and free-list set.
    /// Trailing None/DISCONNECTED slots and trailing port-array padding are ignored.
    pub fn is_behaviorally_equal(&self, other: &Net) -> bool;
}
```

## Test Expectations (forward-ref)

TEST-SPEC-0491:
- T14: dense → sparse → dense round-trip via `is_behaviorally_equal`.
- T15: sparse → dense → sparse via `==`.
- `behaviorally_equal_ignores_trailing_none_slots`.
- `behaviorally_equal_ignores_trailing_disconnected_ports`.
- `behaviorally_equal_redex_queue_order_independent`.
- `behaviorally_equal_distinguishes_freeport_redirects` — different redirects ⇒ not equal.

## Invariants Touched

- R21 (consumed; helper IS the closure of SC-014).

## Notes

- The redex-queue comparison up-to-ordering is necessary because reduction strategy may permute the queue without changing behavior. Use `HashSet<(AgentId, AgentId)>` projection.
- For the `ports` projection, iterate over `self.agents.iter().flatten()`, collect `(agent.id, p, ports[port_index(agent.id, p)])` for `p in 0..total_ports(agent.symbol)`, skipping DISCONNECTED entries.

## DAG Links

- **Predecessors:** TASK-0489, TASK-0490, TASK-0471.
- **Successors:** TASK-0475 (T8 migration), TASK-0492 (T16 G1 round-trip uses `is_behaviorally_equal`).
