# TASK-0588: `BorderGraph::extend_with_chunk_borders` call-site discipline (under `delta_mode && streaming_active`)

**Spec:** SPEC-21 §3.7 R37f (BorderGraph update under delta+streaming; closes SC-017); §3.6 R36 (delta+pull compatibility — SHOULD elevated to MUST under the conjunction); §3.8 A7 (consumer of TASK-0516).
**Requirements:** R37f (under `delta_mode && streaming_active`, R36 elevates SHOULD → MUST; coordinator MUST call `BorderGraph::extend_with_chunk_borders(&new_borders)` after each `install_connection` invocation that yields a border wire, before chunk N+1's `AssignPartition`); R36 baseline.
**Priority:** P0 (closes SC-017; ARG-005 delta-recoverability gate under streaming).
**Status:** TODO
**Depends on:** TASK-0516 (SPEC-19 amendment A7 landed — `extend_with_chunk_borders` method signature in spec text), TASK-0553 (`install_connection` helper — call-site producer of `new_borders`), TASK-0554 (`generate_and_partition_chunked` orchestrator — call-site context), TASK-0577 (coordinator FSM — drives the call from `GeneratingNext` state).
**Blocked by:** SPEC-19 implementation of `extend_with_chunk_borders` (cross-spec, owned by SPEC-19; TASK-0516 is the spec amendment, the IMPL lives in SPEC-19's task list).
**Estimated complexity:** M (~120 LoC call-site wiring in coordinator + per-chunk new_borders capture; ~180 LoC delta+streaming integration tests).
**Bundle:** SPEC-21 Streaming Generation — Phase F (regression / polish / late-binding).

## Context

Per SPEC-21 §3.8 A7 (closes SC-017 per Round 2 closure log line 1089), under the conjunction `delta_mode && streaming_active`, R36 elevates from **SHOULD** to **MUST**. The coordinator MUST call:

```rust
BorderGraph::extend_with_chunk_borders(&mut self, new_borders: &HashMap<u32, (PortRef, PortRef)>)
```

after each `install_connection` invocation that yields a border wire, before chunk N+1's `AssignPartition` is dispatched. The method is **idempotent** on previously-seen border IDs and is a **no-op if `new_borders.is_empty()`**.

**Ownership split (per SC-017 closure):** SPEC-19 owns the `extend_with_chunk_borders` IMPLEMENTATION; SPEC-21 owns the **CALL-SITE DISCIPLINE** (this task). Without this call-site, the coordinator's `BorderGraph` becomes stale after chunk 1 under combined delta+streaming, missing cross-chunk active pairs and silently violating G1.

This task captures, after each `install_connection` call inside the streaming pipeline, the border wires that were newly created (i.e., the `(PortRef, PortRef)` pairs added to the coordinator's `border_map` during this chunk) into a local `new_borders` map and passes that map to `BorderGraph::extend_with_chunk_borders` BEFORE the next chunk's `AssignPartition` is dispatched.

## Acceptance Criteria

- [ ] Coordinator (in the `generate_and_partition_chunked` orchestrator from TASK-0554, OR in the pull-dispatch loop from TASK-0577 if `dispatch_mode == Pull`) maintains a `new_borders_this_chunk: HashMap<u32, (PortRef, PortRef)>` accumulator.
- [ ] After each `install_connection` call (TASK-0553) that classifies a wire as **border**, the new border entry is added to `new_borders_this_chunk`.
- [ ] BEFORE dispatching chunk N+1's `AssignPartition`, the coordinator calls `border_graph.extend_with_chunk_borders(&new_borders_this_chunk)` and clears the accumulator.
- [ ] The call is **gated** on `delta_mode && streaming_active`: when either flag is false, the call site is a no-op (R37f conjunction).
- [ ] Idempotency verified: calling `extend_with_chunk_borders` twice with the same `new_borders` produces the same `BorderGraph` state.
- [ ] No-op verified: `new_borders.is_empty()` → method returns immediately, `BorderGraph` unchanged.
- [ ] Integration test (delta + streaming, 4 workers, 8 chunks): merged result with extension calls is **isomorphic** to merged result of the equivalent batch path (SPEC-19 + SPEC-04 split() with the same input net); without the extension calls, the test FAILS due to stale `BorderGraph` (regression catcher).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/partition/streaming.rs` (or wherever TASK-0554 places the orchestrator) | modify | Add `new_borders_this_chunk` accumulator; call `extend_with_chunk_borders` between chunks. |
| `relativist-net/src/coordinator/pull_dispatch.rs` (TASK-0577) | modify | Wire the same accumulator into the `GeneratingNext → AwaitingResults` transition under pull mode. |
| `relativist-core/tests/spec21_delta_streaming_bordergraph.rs` | create | Integration test exercising R37f conjunction with regression-catcher (without-call variant FAILS). |

## Key Types / Signatures

```rust
// inside the per-chunk loop (TASK-0554 / TASK-0577 GeneratingNext):
let mut new_borders_this_chunk: HashMap<u32, (PortRef, PortRef)> = HashMap::new();
// ... for each agent in batch ...
//     for each connection_directive ...
//         install_connection(...) // TASK-0553
//         if classified as border { new_borders_this_chunk.insert(border_id, (a, b)); }
// ... before next chunk's AssignPartition:
if cfg.delta_mode && streaming_active {
    border_graph.extend_with_chunk_borders(&new_borders_this_chunk);
}
new_borders_this_chunk.clear();
```

## Test Expectations (forward-ref)

Reuse coverage from TEST-SPEC-0516 (amendment-level — flagged this task at INDEX line 56). Production-level coverage:
- UT-0588-01: delta+streaming, 8 chunks, 4 workers, `ep_annihilation` → merged result isomorphic to batch baseline.
- UT-0588-02 (regression catcher): same input WITHOUT the extension calls → assertion fails (proves the gate is load-bearing).
- UT-0588-03: idempotency — call extension twice with same `new_borders` → identical `BorderGraph`.
- UT-0588-04: no-op — empty `new_borders` → `BorderGraph` unchanged.
- TEST-SPEC-T6 / T7 partial — full delta+streaming exercise via this amendment per TEST-SPEC-0516 line 57.

## Invariants Touched

- G1 (BSP determinism under delta+streaming) — restored by R37f.
- ARG-005 delta-recoverability — preserved under streaming via this call-site.

## Notes

- This task does NOT implement `extend_with_chunk_borders` itself — that lives in SPEC-19 (cross-spec). This task is purely the CALL-SITE DISCIPLINE.
- The `streaming_active` flag is the same one consumed by TASK-0589 / TASK-0590 (R10b broadening). It is set by entering the chunked-dispatch phase of the orchestrator.
- M5 milestone gate (`ep_con 100M coordinator-side`) is enabled by this amendment but out-of-scope for SPEC-21 (per TEST-SPEC-0516 line 58).
- Consumed by ARG-005 closure validation (cross-spec concern).

## DAG Links

- **Predecessors:** TASK-0516, TASK-0553, TASK-0554, TASK-0577.
- **Successors:** none (terminal call-site discipline).
