# TASK-0437: Delta-mode self-worker symmetry — full worker-side delta loop (NF-003)

**Spec:** SPEC-20 §3.1 R4-delta (apply_deltas includes self), R4-delta-self-symmetry (closes NF-003).
**Requirements:** R4-delta, R4-delta-self-symmetry (critical — forbids short-circuiting `reduce_all` on full self-partition).
**Priority:** P0 (delta hybrid correctness; D2 invariant on delta).
**Status:** TODO
**Depends on:** TASK-0423 (v1 self-spawn), SPEC-19 R24 worker delta loop (existing).
**Blocked by:** TASK-0423.
**Estimated complexity:** M (~130-180 LoC production + ~100 LoC tests including instrumented wire-symmetry test)
**Bundle:** SPEC-20 Elastic Grid — Phase 2.1 Hybrid Coordinator (delta mode).

## Context

NF-003 mandates that in delta mode the in-process self-worker executes the *full* worker-side delta state machine specified by SPEC-19 R24: (i) maintain own `partition` + `border_id` endpoint state, (ii) receive `RoundStart(deltas)` from coordinator orchestration role through `ChannelTransport`, (iii) `apply_border_deltas(partition, deltas)`, (iv) `reduce_all(partition)`, (v) emit `RoundResult(deltas)` back through channel. Short-circuiting `reduce_all` on the full self-partition would (a) bypass per-round delta-emission protocol, (b) violate D2 (Local Reduction Equivalence) via code-path divergence.

## Acceptance Criteria

- [ ] In `spawn_self_partition` (TASK-0423), add a delta-mode branch: when `cfg.delta_mode == true`, the spawn_blocking task runs the **full SPEC-19 R24 worker delta loop**, not a short-circuit `reduce_all`.
- [ ] Self-worker maintains its own `partition` + `border_id` endpoint state (structurally a delta-mode worker, not a coordinator).
- [ ] `RoundStart` messages from the coordinator orchestration role flow through the channel; self-worker calls `apply_border_deltas(partition, deltas)` then `reduce_all(partition)` then emits `RoundResult(deltas)`.
- [ ] Coordinator consumes the self-worker's `RoundResult` identically to any remote worker's — no special branch in `BorderGraph.apply_deltas`.
- [ ] FINAL — prohibition on short-circuit: add an inline doc block forbidding the optimization, with reference to NF-003 rationale.
- [ ] Instrumented wire-symmetry test (EG-U4-delta-wire-symmetry): run the same partition through (a) self-worker via `ChannelTransport` and (b) remote worker over (simulated) transport; assert `RoundResult.border_deltas` structural equivalence.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/protocol/self_worker.rs` | modify | Delta-mode branch: invoke SPEC-19 R24 worker loop. |
| `relativist-core/src/merge/` (BorderGraph.apply_deltas site) | verify | No special-case for self-worker. |

## Test Expectations (forward-ref)

- EG-U4-delta `test_hybrid_apply_deltas_includes_self` (R4-delta).
- EG-U4-delta-wire-symmetry `test_self_worker_delta_round_result_shape_matches_remote` (R4-delta-self-symmetry, NF-003).
- EG-I1-delta integration (R1, R4-delta, G1 CONDITIONAL/CLOSED-for-conservative).

## Invariants Touched

- D2 (Local Reduction Equivalence) — preserved exclusively by this symmetry.

## Notes

- **Forbidden optimization**: "reduce_all(full_self_partition) once per round" is a regression hazard. The cost of the delta loop is purely structural (same interactions, different emission cadence); performance argument against the short-circuit is moot because correctness fails.

## DAG Links

- **Predecessors:** TASK-0423.
- **Successors:** TASK-0446 (delta rejoin), TASK-0443 (delta departure).
