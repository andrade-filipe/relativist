# TASK-0490: `SparseNet::to_dense(id_range: Option<Range<AgentId>>)` conversion with partition scoping (R20 — closes SC-006)

**Spec:** SPEC-22 §3.2 R20, R21; §4.6 (conversion body) + §4.6 backward-compat note.
**Requirements:** R20 (determine max_id; allocate Vec<Option<Agent>> of size max_id+1 and Vec<PortRef> of size (max_id+1)*3; insert all agents and port entries; copy redex_queue/next_id/root; copy freeport_redirects); §4.6 SC-006 fix: signature now takes `Option<Range<AgentId>>` to scope free-list population to the partition's range.

**Critical**: `to_dense` is a SIGNATURE CHANGE from earlier drafts. Old: `to_dense(&self) -> Net`. New: `to_dense(&self, id_range: Option<Range<AgentId>>) -> Net`. Existing call sites must migrate to `to_dense(None)`.

**Priority:** P0 (closes SC-006; required for R22 sparse-then-dense build_subnet).
**Status:** TODO
**Depends on:** TASK-0486 (SparseNet), TASK-0487 (SparseNet ops), TASK-0471 (Net.free_list).
**Blocked by:** none
**Estimated complexity:** S (~70 LoC production + ~120 LoC tests including the partition-scoped T14a)
**Bundle:** SPEC-22 Arena Management — Phase D (SparseNet).

## Context

§4.6 provides the reference body. Compute `max_id = self.agents.keys().max().copied().unwrap_or(0)`; `arena_len = max_id + 1`. Allocate dense vectors. Insert all agents and port entries. Copy redex_queue, next_id, root. **Copy `freeport_redirects.clone()`** (closes SC-001 second surface). Free-list initially empty.

**Free-list population (SC-006 fix):**
- `id_range = Some(range)`: free-list is populated only with `None` indices in `[range.start, range.end)`. This is the partition-context call site (R10/R10a). Indices outside the range are NOT added (D4 violation prevented).
- `id_range = None`: whole-net case. Every `None` index in the arena is added to the free-list. Use only in non-partitioned contexts.

The signature change is a hard-break from the earlier draft `to_dense(&self) -> Net`. All call sites must explicitly pass `None` or `Some(range)`.

### `to_dense(id_range)` call-site audit (the spec corpus)

(Required by Round 2 §"Round 3 confirmation suggestions" item 2.) Within the SPEC corpus, `to_dense` call sites are:

- **SPEC-22 §4.6 backward-compat note**: documents the migration; no call site, just text.
- **SPEC-22 §3.8 A7 (SPEC-04 build_subnet amendment)**: amended to call `to_dense(Some(partition.id_range.clone()))` when the sparse path is taken (R22 threshold).
- **SPEC-22 R10a + R22**: same call site as A7.
- **SPEC-22 §7.2 T14**: tests the whole-net case via `to_dense(None)`.
- **SPEC-22 §7.2 T14a**: tests the partition-scoped case via `to_dense(Some(50..100))` and `to_dense(Some(100..200))`.
- **SPEC-22 §7.2 T15**: round-trip Sparse → Dense → Sparse via `to_dense(None)`.
- **SPEC-22 §7.2 T16**: integration test that exercises sparse build_subnet → reduce → merge; the build_subnet path under R22 calls `to_dense(Some(partition.id_range.clone()))`.

No other SPEC-corpus call sites reference `to_dense`. Implementation-side migration (i.e., scanning `src/` for any earlier no-arg `to_dense()` invocations) is a Stage 3 DEVELOPER chore — not in this spec-corpus audit.

## Acceptance Criteria

- [ ] Implement `SparseNet::to_dense(&self, id_range: Option<core::ops::Range<AgentId>>) -> Net` in `relativist-core/src/net/sparse.rs` per SPEC-22 §4.6 reference body.
- [ ] Compute `max_id = self.agents.keys().max().copied().unwrap_or(0)`; `arena_len = max_id + 1`.
- [ ] Allocate `Vec<Option<Agent>>` of size `arena_len` filled with `None`; `Vec<PortRef>` of size `arena_len * PORTS_PER_SLOT` filled with `DISCONNECTED`.
- [ ] Insert all agents into the dense arena.
- [ ] Insert all port entries via `port_index(id, port)`.
- [ ] Copy `redex_queue.clone()`, `next_id`, `root`, `freeport_redirects.clone()`.
- [ ] Free-list construction:
  ```rust
  let (lo, hi) = match id_range {
      Some(r) => (r.start as usize, (r.end as usize).min(arena_len)),
      None => (0, arena_len),
  };
  for i in lo..hi {
      if net.agents[i].is_none() {
          net.free_list.push(i as AgentId);
      }
  }
  ```
- [ ] Set `net.id_range = id_range.clone()` (for downstream R10 defensive check; partition-scoped contexts inherit the range).
- [ ] Test T14a (SPEC-22 §7.2): SparseNet at IDs `{50, 51, 75, 99, 130, 175}`. `to_dense(Some(50..100))` produces free-list exactly `{52, 53, ..., 74, 76, ..., 98}`; IDs in `[0, 50)` and `[100, max_id]` MUST NOT appear. `to_dense(Some(100..200))` produces free-list exactly `{100, 101, ..., 129, 131, ..., 174, 176, ..., 199}`.
- [ ] Test T14 (whole-net case via `to_dense(None)`): every `None` index in the arena is in the free-list.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/net/sparse.rs` | modify | Add `impl SparseNet { pub fn to_dense(&self, id_range: Option<Range<AgentId>>) -> Net }`. |

## Key Types / Signatures

```rust
impl SparseNet {
    pub fn to_dense(&self, id_range: Option<core::ops::Range<AgentId>>) -> Net;
}
```

## Test Expectations (forward-ref)

TEST-SPEC-0490:
- T14: round-trip whole-net (joint with TASK-0489 / TASK-0491).
- T14a: partition-scoped — exact free-list set per the SPEC-22 §7.2 fixtures.
- `to_dense_none_populates_full_free_list`.
- `to_dense_some_with_empty_range_yields_empty_free_list`.
- `to_dense_preserves_freeport_redirects` (SC-001 second surface).

## Invariants Touched

- D4 (preserved — partition-scoped free-list confined to range).
- D1c (FreePort bijectivity — `freeport_redirects` copy).
- R10 (precondition supplier for partition contexts).

## Notes

- The signature change requires call-site migration documented in §4.6 of the spec; Stage 3 DEVELOPER will scan `src/` for legacy no-arg `to_dense()` callers (none in spec corpus per audit above; implementation-side audit is a developer chore).
- R10a's "MUST upgrade" depends on this signature: see TASK-0466's R10a strengthening.

## DAG Links

- **Predecessors:** TASK-0486, TASK-0487, TASK-0471.
- **Successors:** TASK-0491 (round-trip helper / R21), TASK-0492 (sparse build_subnet at threshold).
