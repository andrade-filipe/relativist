# TASK-0474: Free-list no-duplicates invariant — debug assertion + optional `HashSet` shadow

**Spec:** SPEC-22 §3.1 R5, R6 (closes SC-018).
**Requirements:** R5 (LIFO — `Vec::pop`/`push` at end), R6 (no duplicates — MUST verify by `debug_assert!(!self.free_list.contains(&id))` in `remove_agent`; OPTIONAL `HashSet<AgentId>` shadow under `#[cfg(debug_assertions)]`).
**Priority:** P0 (closure of SC-018 from Round 1).
**Status:** TODO
**Depends on:** TASK-0473 (remove_agent push site exists).
**Blocked by:** none
**Estimated complexity:** S (~40 LoC production + ~30 LoC tests; `HashSet` shadow path adds ~50 LoC in debug-cfg)
**Bundle:** SPEC-22 Arena Management — Phase B (free-list core implementation).

## Context

R6 mandates: "free-list MUST NOT contain duplicates". Round 1 SC-018 flagged the original SHOULD-strength assertion; Round 2 closes by upgrading to MUST with explicit assertion location and an OPTIONAL O(1) `HashSet<AgentId>` shadow under `#[cfg(debug_assertions)]` to avoid the O(n) cost of `Vec::contains` in debug builds. The shadow is OPTIONAL and is not part of the release-build state. R5 LIFO is preserved by `Vec::pop`/`push` at the end (O(1)).

## Acceptance Criteria

- [ ] In debug builds, `remove_agent` MUST execute `debug_assert!(!self.free_list.contains(&id))` immediately before pushing `id` to `free_list` (already partially in TASK-0473; this task formalizes and tests it).
- [ ] OPTIONAL: Add a `#[cfg(debug_assertions)] free_list_shadow: HashSet<AgentId>` field on `Net` to make the duplicate check O(1). The shadow is updated on push and pop. The shadow is NOT serialized (`#[serde(skip)]` and rkyv skip) and is NOT part of release-build state.
- [ ] If the shadow is added, replace the `contains` call with the O(1) shadow lookup.
- [ ] R5 LIFO is preserved (push/pop at the Vec end; no reordering).
- [ ] Test that `Vec::pop` returns the most-recently-pushed ID (LIFO check beyond TASK-0472 T2).
- [ ] Test T10 (free-list no-duplicate invariant): direct manipulation duplicate-add triggers the debug assertion.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/net/core.rs` | modify | Strengthen R6 assertion in `remove_agent`; OPTIONAL shadow `HashSet` field under `#[cfg(debug_assertions)]`. |

## Key Types / Signatures

```rust
pub struct Net {
    // ... existing fields ...
    pub free_list: Vec<AgentId>,
    /// SPEC-22 R6 (optional): O(1) duplicate-check shadow under debug_assertions.
    /// Not serialized; not part of release-build state.
    #[cfg(debug_assertions)]
    #[serde(skip)]
    #[cfg_attr(feature = "zero-copy", rkyv(with = rkyv::with::Skip))]
    free_list_shadow: std::collections::HashSet<AgentId>,
}
```

## Test Expectations (forward-ref)

TEST-SPEC-0474:
- T10 (SPEC-22 §7.1): direct manipulation duplicate-add triggers `debug_assert!`.
- `pop_returns_most_recently_pushed` — LIFO smoke test.
- (If shadow adopted) `shadow_consistency_after_push_pop_cycle`.

## Invariants Touched

- R5 (LIFO — preserved by Vec::pop/push at end).
- R6 (no duplicates — verified at the push site).

## Notes

- The OPTIONAL shadow is left to DEVELOPER judgement at Stage 3. Recommendation: add the shadow if the existing 1181 default tests show non-trivial slowdown from `Vec::contains` in debug; otherwise keep the simpler `contains` path.
- `Vec::contains` in release builds is dead-code-eliminated because `debug_assert!` is a no-op there.

## DAG Links

- **Predecessors:** TASK-0473.
- **Successors:** TASK-0495 (broader I3' debug assertions; R27 free-list consistency check).
