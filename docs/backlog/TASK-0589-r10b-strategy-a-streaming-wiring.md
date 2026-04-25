# TASK-0589: SPEC-22 R10b Strategy A (`DisableUnderDelta`) wiring under streaming pipeline

**Spec:** SPEC-21 §3.7 R37b (G1 free-list interaction; closes SC-007); §3.8 A6 (consumer of TASK-0515).
**Requirements:** R37b — Strategy A: under `RecyclePolicy::DisableUnderDelta` (default), workers MUST NOT pop from the free-list while delta mode is active AND chunked dispatch is in progress; this is the conservative gating that closes G1 by construction.
**Priority:** P0 (G1 / ARG-005 closure under streaming; default policy wiring).
**Status:** TODO
**Depends on:** TASK-0515 (SPEC-22 R10b broadening amendment landed in spec text), TASK-0482 (SPEC-22 `RecyclePolicy` enum + `GridConfig.recycle_under_delta` + `is_border_protected` wiring — SPEC-22 production task), TASK-0577 / TASK-0578 (FSMs that drive chunked-dispatch state into the worker arena), TASK-0554 (orchestrator that sets `streaming_active` flag).
**Blocked by:** TASK-0482 MUST land first (Strategy A enum variant exists; this task extends the gate condition).
**Estimated complexity:** S (~80 LoC: gate-condition extension at free-list pop sites + integration tests).
**Bundle:** SPEC-21 Streaming Generation — Phase F (regression / polish / late-binding).

## Context

Per SPEC-21 R37b verbatim (line 257):

> Under `GridConfig.recycle_under_delta == RecyclePolicy::DisableUnderDelta` (SPEC-22 R10b Strategy A, default), workers MUST NOT pop from the free-list while delta mode is active **AND** chunked dispatch is in progress.

SPEC-22 R10b Strategy A originally gates only on `delta_mode == true` (line 340 *Old text*). SPEC-21 §3.8 A6 (closes SC-007) broadens the gate condition to `(delta_mode || streaming_active) && id ∈ border_referenced_set`. This task wires the **streaming_active** half of the broadened condition into the existing Strategy A code path.

**Implementation note (R37b conservative reading per Round 2):** the simplest interpretation is "Strategy A disables the entire free-list during chunked dispatch" — pop returns `None` whenever `streaming_active == true` (regardless of border membership). This is the conservative, default-safe path. Strategy B (TASK-0590) is the precision path that uses border membership.

The `streaming_active` flag is set by the worker upon receiving its first chunked `AssignPartition` (i.e., when the coordinator is in `DispatchingFirst` or `GeneratingNext`) and cleared upon receiving `NoMoreWork` + completing `FinalReduction`.

## Acceptance Criteria

- [ ] Worker arena's `Net::create_agent` (or wherever the SPEC-22 free-list pop happens — TASK-0472) gains an additional gate clause: `if cfg.recycle_under_delta == RecyclePolicy::DisableUnderDelta && (delta_mode || streaming_active) { skip free-list pop }`.
- [ ] `streaming_active` flag is plumbed from worker FSM (TASK-0578 chunk-dispatch state) into the arena.
- [ ] When the gate triggers, `next_id`-increment fresh allocation is used instead (R37b conservative path).
- [ ] Integration test (streaming + recycle gated): 4 workers, 8 chunks, ep_annihilation → no free-list pops occur during streaming phase; verified via debug counter in `Net.free_list_pops`.
- [ ] Regression test (push mode unaffected): with `dispatch_mode == Push`, `streaming_active` is never set, free-list pops continue per SPEC-22 R3.
- [ ] No regression on the 1181/1224 baseline (pure additive gate).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/net/free_list.rs` (or wherever TASK-0472 placed the pop site) | modify | Extend gate: `(delta_mode \|\| streaming_active)`. |
| `relativist-net/src/worker/streaming_state.rs` | create | Worker-side `streaming_active` flag plumbed from FSM. |
| `relativist-core/tests/spec21_r37b_strategy_a_streaming.rs` | create | Integration test verifying no pops during streaming. |

## Key Types / Signatures

```rust
// In Net::create_agent (extends TASK-0472):
if cfg.recycle_under_delta == RecyclePolicy::DisableUnderDelta
    && (cfg.delta_mode || worker_state.streaming_active)
{
    // skip free-list pop; fall through to next_id increment (R37b conservative)
} else if let Some(id) = self.free_list.pop() {
    // SPEC-22 R3 path
}
```

## Test Expectations (forward-ref)

Reuse pattern from TEST-SPEC-0482 (Strategy A baseline) and TEST-SPEC-0515 (broadening amendment-level). Production-level:
- UT-0589-01: streaming + delta_mode=true + Strategy A → zero free-list pops during chunked phase.
- UT-0589-02: streaming + delta_mode=false + Strategy A → still zero free-list pops (broadening triggers on streaming alone).
- UT-0589-03: push mode + delta_mode=false + Strategy A → free-list pops occur normally (SPEC-22 R3 unchanged).
- UT-0589-04: ARG-005 oracle — delta+streaming results match batch baseline byte-equality (Strategy A is the default-safe path).

## Invariants Touched

- G1 (BSP determinism under streaming) — preserved by Strategy A conservative gate.
- ARG-005 delta-recoverability — preserved under streaming via this gate.

## Notes

- The `streaming_active` flag MUST be cleared upon `FinalReduction → SendFinalResult` to allow post-streaming reductions to recycle normally. Document explicitly in the worker FSM hook.
- Cross-coordination with TASK-0590 (Strategy B): the two strategies are mutually exclusive at runtime (selected via `GridConfig.recycle_under_delta`); this task ships only the Strategy A path.
- Cross-coordination with TASK-0591 (cargo-feature-gate alternative): when `feature = "streaming-no-recycle"` is enabled, this task's gate becomes redundant (the entire free-list is disabled at compile time). The gate MUST remain present and CORRECT regardless of feature flag — TASK-0591's feature-gate is an ADDITIONAL safety net, not a replacement.
- Consumed by ARG-005 closure validation under streaming.

## DAG Links

- **Predecessors:** TASK-0515, TASK-0482, TASK-0578, TASK-0554.
- **Successors:** TASK-0590 (sibling strategy), TASK-0591 (alternative path).
