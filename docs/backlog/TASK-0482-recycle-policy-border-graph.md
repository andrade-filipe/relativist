# TASK-0482: `RecyclePolicy` enum + `GridConfig.recycle_under_delta` + `is_border_protected` wiring (R10b/R10c — Strategy A and Strategy B)

**Spec:** SPEC-22 §3.1 R10b, R10c; §3.8 A10 (consumes SPEC-19 amendment).
**Requirements:** R10b (two normative strategies — `RecyclePolicy::DisableUnderDelta` (default, Strategy A) and `RecyclePolicy::BorderClean` (Strategy B)); R10c (protected-tombstone semantics: slot stays `None`, ports `DISCONNECTED`, ID NOT in `free_list`, persists until next `reconstruct`/clean-boundary moment).
**Priority:** P0 (G1 preservation under delta mode; closes SC-005).
**Status:** TODO
**Depends on:** TASK-0469 (SPEC-19 §3.2 amendment), TASK-0473 (remove_agent push site), TASK-0472 (create_agent pop site), TASK-0480 (id_range field).
**Blocked by:** none
**Estimated complexity:** M (~150 LoC production + ~120 LoC tests; touches `Net`, `GridConfig`, worker dispatch)
**Bundle:** SPEC-22 Arena Management — Phase C (distributed integration).

## Context

R10b authors two normative implementation strategies for free-list × `BorderGraph` slot-id stability under `delta_mode == true`:

- **Strategy A (`RecyclePolicy::DisableUnderDelta`, DEFAULT):** workers MUST NOT pop from the free-list during a delta-mode round. `create_agent` falls back to `next_id` allocation. The free-list still accumulates pushes from `remove_agent` (or instead protected-tombstones — see R10c). Drained at the next clean partition boundary (`reconstruct` per SPEC-19 R38).
- **Strategy B (`RecyclePolicy::BorderClean`):** workers MAY pop from the free-list only if the popped ID is NOT in the partition's `border_entries` set (SPEC-04 R20-R22 — partition-local, O(1) HashSet shadow). If border-referenced, re-push (or stash) and allocate fresh.

R10c: `remove_agent` on a border-referenced ID under delta mode marks the slot as a *protected tombstone* — `agents[id] = None`, ports `DISCONNECTED`, ID NOT in `free_list`. Optionally tracked via `Net::protected_tombstones: HashSet<AgentId>` (debug builds) for assertion validation.

The choice between strategies is a `GridConfig.recycle_under_delta: RecyclePolicy` field. The threat model R10b prevents (verbatim): round N produces border `B = (border_id, AgentPort(47, 0), AgentPort(123, 0))`; round N+1 worker recycles ID 47 to a different `Symbol`; coordinator dispatches `CommutationBatch` indexing `AgentPort(47, 0)`; worker's local `agents[47]` resolves to a different rule than BorderGraph computed → G1 violation.

## Acceptance Criteria

- [ ] Define `pub enum RecyclePolicy { DisableUnderDelta, BorderClean }` in `relativist-core/src/merge/config.rs` (or wherever GridConfig lives) with `derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)`. `Default::default() == RecyclePolicy::DisableUnderDelta`.
- [ ] Extend `GridConfig` with `pub recycle_under_delta: RecyclePolicy` (default `DisableUnderDelta`) — coordinate with TASK-0415 (SPEC-20 GridConfig) to avoid serde collision.
- [ ] Add a `Net::border_entries_shadow: Option<HashSet<AgentId>>` field with `#[serde(skip)]` and rkyv skip (partition-local, debug-state). Set by `build_subnet` for delta-mode workers.
- [ ] Add a `Net::protected_tombstones: Option<HashSet<AgentId>>` field with `#[cfg(debug_assertions)]`, `#[serde(skip)]`, and rkyv skip — debug-only tracking for R10c assertion validation.
- [ ] Override `Net::is_border_protected(&self, id: AgentId) -> bool`: returns `true` IFF `border_entries_shadow.as_ref().map_or(false, |s| s.contains(&id))` — i.e., id is referenced in the partition's border-entries set. Returns `false` in non-distributed (border_entries_shadow is `None`) contexts.
- [ ] Strategy A wiring: in `create_agent`, before `self.free_list.pop()`, add a runtime check: if `self.recycle_policy == DisableUnderDelta && self.is_in_delta_round` (a Net flag set by the worker dispatch loop), SKIP the pop and fall through to `next_id` allocation.
- [ ] Strategy B wiring: in `create_agent`'s recycle path, AFTER `self.free_list.pop()` returns `Some(id)`, check `self.is_border_protected(id)`; if `true`, re-push to free-list (or stash in a side-list) and recurse / fall through to `next_id`.
- [ ] In `remove_agent`: the `is_border_protected(id)` guard wired in TASK-0473 now consults the populated `border_entries_shadow`. R10c: when guard returns `true`, slot stays `None`, ports `DISCONNECTED`, ID NOT pushed to `free_list`. In debug builds, ID is added to `protected_tombstones` shadow.
- [ ] In `create_agent`'s recycle path (debug only): `debug_assert!(!self.protected_tombstones.as_ref().map_or(false, |s| s.contains(&new_id)))` — guards against accidental recycle of a protected tombstone.
- [ ] Add `Net::recycle_policy: RecyclePolicy` field, set by the worker dispatch loop from `GridConfig.recycle_under_delta`.
- [ ] Add `Net::is_in_delta_round: bool` flag, set true at delta-round entry and false at `reconstruct` boundary (per SPEC-19 R38).
- [ ] At `reconstruct` clean-boundary: drain the `protected_tombstones` shadow into `free_list` (closes the protected-tombstone reclaim path).
- [ ] **Acceptance criteria for both code paths under `GridConfig.recycle_under_delta`:**
  - **Strategy A (`DisableUnderDelta`):** T9a — given a 2-partition delta-mode scenario with default policy, worker 0 owns IDs `[0, 100)` with border at `AgentPort(47, 0)`; agent 47 consumed in round N; assert `agents[47] == None`, `free_list does NOT contain 47`, on next `create_agent` returned ID is NOT 47, after `reconstruct` ID 47 is reclaimable.
  - **Strategy B (`BorderClean`):** T9b — same scenario with `RecyclePolicy::BorderClean`; pre-populate `border_entries_shadow == {47}`; trigger `remove_agent(47)`; assert (a) free-list does NOT contain 47, (b) free-list still receives non-border-referenced freed IDs (e.g., remove ID 50 — assert 50 IS in free-list).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/merge/config.rs` | modify | Add `RecyclePolicy` enum and `GridConfig.recycle_under_delta` field. |
| `relativist-core/src/net/core.rs` | modify | Add `border_entries_shadow`, `protected_tombstones` (debug), `recycle_policy`, `is_in_delta_round` fields. Override `is_border_protected`. Wire Strategy A/B into `create_agent`. |
| `relativist-core/src/partition/helpers.rs` | modify | `build_subnet` populates `border_entries_shadow` from `border_entries[i]` for delta-mode workers. |
| `relativist-core/src/merge/grid.rs` *(or wherever the worker round dispatch lives)* | modify | Toggle `is_in_delta_round` at delta-round entry/exit. Drain `protected_tombstones` at `reconstruct`. |

## Key Types / Signatures

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RecyclePolicy {
    #[default]
    DisableUnderDelta,
    BorderClean,
}

pub struct GridConfig {
    // ... existing fields (incl. SPEC-20 fields from TASK-0415) ...
    pub recycle_under_delta: RecyclePolicy,  // SPEC-22 R10b
}

pub struct Net {
    // ... existing fields ...
    #[serde(skip)]
    #[cfg_attr(feature = "zero-copy", rkyv(with = rkyv::with::Skip))]
    pub border_entries_shadow: Option<std::collections::HashSet<AgentId>>,

    #[cfg(debug_assertions)]
    #[serde(skip)]
    pub protected_tombstones: Option<std::collections::HashSet<AgentId>>,

    pub recycle_policy: RecyclePolicy,
    pub is_in_delta_round: bool,
}
```

## Test Expectations (forward-ref)

TEST-SPEC-0482 — covers SPEC-22 §7.1:
- T9a: Strategy A — `DisableUnderDelta` protected tombstone scenario.
- T9b: Strategy B — `BorderClean` selective recycle.
- `protected_tombstone_drained_at_reconstruct`.
- `default_policy_is_disable_under_delta`.
- `non_distributed_context_unaffected_by_recycle_policy` — `border_entries_shadow == None` means is_border_protected always returns `false`.

## Invariants Touched

- D2 (Border completeness) — preserved by recycle restriction.
- D3 (Cross-round border discovery) — preserved.
- G1 (under delta mode) — protected by R10b.
- I3' stability subclause — border-referenced IDs preserved across rounds via protected tombstones.
- ARG-005 INV-REC — Strategy A/B both honor the soundness condition (the basis (B_k, {N_w,k}) ~ μ_k holds because slot-level isomorphism is preserved).

## Notes

- The `is_in_delta_round` flag is set at delta-round entry by the worker dispatch loop (interaction surface is in `merge/grid.rs` or `protocol/worker.rs`). The exact wiring depends on whether SPEC-19's worker loop is fully implemented; if not, this task adds the flag plumbing scaffolding only.
- Coordinate with TASK-0415 (SPEC-20 GridConfig) on the field-list ordering and serde-default attributes.
- The protected_tombstones shadow is `#[cfg(debug_assertions)]` only; release builds rely on the `is_border_protected` guard's correctness without redundant tracking.
- Strategy B is opt-in for benchmarks where measurement of the recycle benefit under delta mode is desired (per R10b prose).

## DAG Links

- **Predecessors:** TASK-0469, TASK-0473, TASK-0472, TASK-0480.
- **Successors:** TASK-0492 (sparse build_subnet may also set border_entries_shadow), TASK-0495 (R27 debug assertions for protected tombstones), TASK-0500 (regression).
