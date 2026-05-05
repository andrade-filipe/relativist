# TASK-0471: Add `free_list: Vec<AgentId>` field to `Net` struct + constructors

**Spec:** SPEC-22 §3.1 R1, R8; §4.1 (struct definition).
**Requirements:** R1 (free-list field), R8 (constructors initialize empty), R28 (always-on, no feature gate), R29 (SparseNet always available — informational).
**Priority:** P0 (foundational; every free-list-aware task depends on the field's existence).
**Status:** TODO
**Depends on:** TASK-0461 (SPEC-02 R2 amendment).
**Blocked by:** none
**Estimated complexity:** S (~30 LoC production + ~20 LoC tests)
**Bundle:** SPEC-22 Arena Management — Phase B (free-list core implementation).

## Context

SPEC-22 §4.1 specifies the complete post-SPEC-22 `Net` struct layout. The `free_list: Vec<AgentId>` field is added at the end of the struct, with documentation tying it to R1 (existence) and R8 (constructors). All existing fields remain unchanged; in particular `freeport_redirects` is preserved verbatim with `#[serde(skip)]` and the rkyv conditional skip attribute. R28 mandates the feature is always-on (no feature gate) — the field is unconditionally present.

## Acceptance Criteria

- [ ] Add `pub free_list: Vec<AgentId>` to the `Net` struct in `relativist-core/src/net/core.rs` (or current canonical location).
- [ ] Field is included in the `#[derive(...)]` propagation (Debug, Clone, PartialEq, Eq, Serialize, Deserialize, plus rkyv derives under `feature = "zero-copy"`).
- [ ] `Net::new()` initializes `free_list: Vec::new()`.
- [ ] `Net::with_capacity(capacity)` initializes `free_list: Vec::new()` (capacity does NOT pre-allocate the free-list — it grows on demand via `remove_agent`).
- [ ] `freeport_redirects` field is left unchanged; no derive or attribute changes.
- [ ] Rustdoc on the field cites SPEC-22 R1 and R10/R10b/R10c (partition-range and protected-tombstone constraints).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/net/core.rs` | modify | Add `free_list: Vec<AgentId>` field to `Net` struct; update `new()` and `with_capacity()` constructors. |

## Key Types / Signatures

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "zero-copy", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
pub struct Net {
    pub agents: Vec<Option<Agent>>,
    pub ports: Vec<PortRef>,
    pub redex_queue: VecDeque<(AgentId, AgentId)>,
    pub next_id: AgentId,
    pub root: Option<PortRef>,
    #[serde(skip)]
    #[cfg_attr(feature = "zero-copy", rkyv(with = rkyv::with::Skip))]
    pub freeport_redirects: HashMap<u32, PortRef>,
    /// SPEC-22 R1: free-list of recycled AgentId slots, LIFO (push/pop at end).
    /// Initialized empty by Net::new() and Net::with_capacity().
    pub free_list: Vec<AgentId>,
}
```

## Test Expectations (forward-ref for Stage 2 TEST-GENERATOR)

TEST-SPEC-0471:
- `net_new_initializes_empty_free_list`
- `net_with_capacity_initializes_empty_free_list`
- `net_serde_round_trip_preserves_free_list_field` — empty case (this task only; non-empty in TASK-0475)

## Invariants Touched

- I3' (uniqueness — the field's *existence* prepares the substrate; preservation is in TASK-0472/0473).
- R28 (always-on default — no feature gate).

## Notes

- This task does NOT yet wire `free_list` into `create_agent` / `remove_agent`; that's TASK-0472 / TASK-0473.
- `Send + Sync` for `Net` is preserved by construction (`Vec<AgentId>` is `Send + Sync` since `AgentId: Send + Sync`).
- The `static_assertions::assert_impl_all!(Net: Send, Sync)` compile-time check is added in TASK-0488 (Send + Sync compile-time assertion).

## DAG Links

- **Predecessors:** TASK-0461 (SPEC-02 R2 amendment).
- **Successors:** TASK-0472 (create_agent free-list pop), TASK-0473 (remove_agent free-list push), TASK-0475 (serde round-trip), TASK-0489 (to_sparse).
