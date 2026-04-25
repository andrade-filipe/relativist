# TASK-0590: SPEC-22 R10b Strategy B (`BorderClean`) wiring under streaming pipeline

**Spec:** SPEC-21 §3.7 R37b (G1 free-list interaction; closes SC-007); §3.8 A6 (consumer of TASK-0515).
**Requirements:** R37b — Strategy B: under `RecyclePolicy::BorderClean` (opt-in), workers MAY pop from the free-list **only for IDs not present in the worker's locally-cached `border_entries` set**. This is the precision path that allows recycling for non-border slots even during chunked dispatch.
**Priority:** P1 (precision-path opt-in; default ships with Strategy A per TASK-0589).
**Status:** TODO
**Depends on:** TASK-0515 (SPEC-22 R10b broadening amendment landed in spec text), TASK-0482 (SPEC-22 `RecyclePolicy::BorderClean` enum variant + `is_border_protected` helper — SPEC-22 production task), TASK-0577 / TASK-0578 (FSMs that maintain `border_entries` cache), TASK-0589 (sibling Strategy A — pre-coordinated branching).
**Blocked by:** TASK-0482 MUST land first (`is_border_protected` exists; `border_entries` cache exists); TASK-0589 SHOULD land first to establish the gate-condition skeleton.
**Estimated complexity:** M (~120 LoC: per-worker `border_entries` cache maintenance + gated free-list pop with id-membership check + integration tests).
**Bundle:** SPEC-21 Streaming Generation — Phase F (regression / polish / late-binding).

## Context

Per SPEC-21 R37b verbatim (line 258):

> Under `RecyclePolicy::BorderClean` (SPEC-22 R10b Strategy B, opt-in), workers MAY pop from the free-list **only for IDs not present in the worker's locally-cached `border_entries` set**.

Per SPEC-21 §3.8 A6 (closes SC-007), Strategy B's trigger condition is also broadened from delta-only to `(delta_mode || streaming_active) && id ∈ border_referenced_set`.

**Distinguishing from Strategy A (TASK-0589):** Strategy A is the conservative path — disable the entire free-list during streaming. Strategy B is the precision path — disable the free-list only for IDs the coordinator is currently tracking in `border_map` / `BorderGraph`. Strategy B preserves recycling for the (typically large) majority of slots that are NOT borders, at the cost of maintaining a per-worker `border_entries` cache synchronized with the coordinator's `BorderGraph`.

The per-worker `border_entries: HashSet<AgentId>` cache is updated:
- On `AssignPartition` receipt: insert all IDs that appear in the partition's border classification.
- On `BorderGraphUpdate` (if SPEC-19 ships such a message under streaming) or implicit update via partition metadata: refresh.
- Cleared on `Done` transition.

## Acceptance Criteria

- [ ] Worker maintains `border_entries: HashSet<AgentId>` cache, updated on each `AssignPartition` receipt under streaming mode.
- [ ] Worker arena's `Net::create_agent` gate (extending TASK-0589's branch) under `RecyclePolicy::BorderClean` becomes: `if (delta_mode || streaming_active) && border_entries.contains(&id) { skip free-list pop } else { pop normally }`.
- [ ] Per-id membership check is O(1) via HashSet (acceptable overhead per R37b).
- [ ] Integration test: streaming + Strategy B, 4 workers, 8 chunks, ep_annihilation with mixed border/non-border slots → border-id pops are zero, non-border-id pops occur (verified via debug counter on the two paths separately).
- [ ] Cross-strategy regression: same input under Strategy A and Strategy B produces **isomorphic** merged results (G1 / ARG-005 preserved by both).
- [ ] No regression on the 1181/1224 baseline.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/net/free_list.rs` | modify | Extend the gate from TASK-0589 with `BorderClean` branch + `border_entries` membership check. |
| `relativist-net/src/worker/border_cache.rs` | create | Per-worker `border_entries` cache maintenance. |
| `relativist-core/tests/spec21_r37b_strategy_b_streaming.rs` | create | Integration test verifying border-id pops are zero, non-border pops continue. |

## Key Types / Signatures

```rust
// In Net::create_agent, extending TASK-0589's branch:
if (cfg.delta_mode || worker_state.streaming_active)
    && cfg.recycle_under_delta == RecyclePolicy::BorderClean
{
    if let Some(id) = self.free_list.last().copied() {
        if worker_state.border_entries.contains(&id) {
            // protected — skip pop, fall through to next_id
        } else {
            self.free_list.pop(); // R3 path, but only for non-border
            return id;
        }
    }
}
```

## Test Expectations (forward-ref)

Reuse pattern from TEST-SPEC-0482 (Strategy B baseline) and TEST-SPEC-0515 (broadening). Production-level:
- UT-0590-01: streaming + Strategy B → zero pops for border IDs.
- UT-0590-02: streaming + Strategy B → non-border IDs are popped normally (precision is achieved).
- UT-0590-03: cross-strategy isomorphism — same input under A vs B → merged results isomorphic via `nets_isomorphic`.
- UT-0590-04: `border_entries` cache cleared on `Done` transition (no leak between runs).

## Invariants Touched

- G1 (BSP determinism under streaming) — preserved by precision gate.
- ARG-005 delta-recoverability — preserved under streaming via per-id protection.

## Notes

- The opt-in nature means CI default is Strategy A (TASK-0589); Strategy B is exercised only by tests that explicitly set `recycle_under_delta = BorderClean`.
- Cross-coordination with TASK-0591 (cargo-feature-gate alternative): same as Strategy A — feature-gate is an additional safety net, not a replacement.
- The `border_entries` cache is conceptually a SUBSET of the coordinator's `BorderGraph`; it does NOT need to be globally synchronized — worker-local view is sufficient because R10c protected-tombstone semantics already ensure that a freshly-popped non-border ID cannot CAUSE a cross-worker border violation.
- Consumed by ARG-005 closure validation under streaming, precision-path branch.

## DAG Links

- **Predecessors:** TASK-0515, TASK-0482, TASK-0578, TASK-0589.
- **Successors:** TASK-0591 (alternative path — orthogonal but documents trade-offs).
