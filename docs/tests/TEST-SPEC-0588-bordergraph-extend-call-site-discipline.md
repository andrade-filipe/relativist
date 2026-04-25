# TEST-SPEC-0588: `BorderGraph::extend_with_chunk_borders` call-site discipline (under `delta_mode && streaming_active`)

**SPEC-21 §7 ID:** plumbing only (production-side closure of TEST-SPEC-0516 amendment-level coverage; cross-spec with SPEC-19 amendment A7).
**Owning task:** TASK-0588.
**Parent spec:** SPEC-21 §3.7 R37f (BorderGraph update under delta+streaming; closes SC-017); §3.6 R36 (delta+pull compatibility — SHOULD elevated to MUST under conjunction); §3.8 A7 (consumer of TASK-0516).
**Type:** unit + integration (call-site wiring + delta+streaming end-to-end with regression-catcher variant).
**Theory anchor:** ARG-005 (delta border completeness — preserved under streaming via this call-site); ARG-001 G1 (BSP determinism under delta+streaming — restored by R37f).

---

## Inputs / Fixtures

- The post-TASK-0588 orchestrator (`generate_and_partition_chunked` from TASK-0554, AND/OR `coordinator_pull_dispatch_loop` from TASK-0577 if `dispatch_mode == Pull`) with the `new_borders_this_chunk` accumulator wired in.
- A 2-partition delta-mode + streaming scenario: 4 workers, 8 chunks, `ep_annihilation_pure(64)` with `chunk_size = 8`.
- `GridConfig` instances: `cfg_delta_streaming` (`delta_mode = true`, `chunk_size = 8`, streaming active), `cfg_delta_only` (`delta_mode = true`, `chunk_size = u32::MAX`, streaming inactive), `cfg_streaming_only` (`delta_mode = false`, `chunk_size = 8`).
- The `BorderGraph::extend_with_chunk_borders(&mut self, new_borders: &HashMap<u32, (PortRef, PortRef)>)` method from SPEC-19 (cross-spec; implementation owned by SPEC-19).
- The `nets_isomorphic` helper for batch-vs-streaming comparison.
- A "regression-catcher" build variant of the orchestrator with the extension call DISABLED (gated on `#[cfg(test)]` test-flag), used exclusively for UT-0588-02.

## Unit Tests (call-site wiring)

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0588-01 | `extension_called_after_each_install_connection_yielding_border` | `cfg_delta_streaming`; one chunk produces 2 border wires | observe call-trace of `BorderGraph::extend_with_chunk_borders` | called exactly ONCE for that chunk; the `new_borders` HashMap contains exactly 2 entries with the correct `(border_id, (PortRef, PortRef))` keys. |
| UT-0588-02 | `regression_catcher_without_extension_fails` | `cfg_delta_streaming`; orchestrator built with extension call DISABLED (`#[cfg(test)] #[cfg(feature = "regression_catcher")]` gate); 8 chunks | run pipeline + merge | merged result is NOT isomorphic to batch baseline (assertion fails on the regression-catcher build); proves the gate is load-bearing per TASK-0588 acceptance line 35 + UT-0588-02 explicit. (Captured as `#[should_panic]` or explicit `assert!(!nets_isomorphic(...))` — document choice.) |
| UT-0588-03 | `idempotency_call_twice_same_borders` | `BorderGraph` with some prior state; call `extend_with_chunk_borders(&new_borders)` twice with the same input | check final `BorderGraph` state | identical to single-call result. (R37f line 21 — idempotency — TASK-0588 acceptance line 33.) |
| UT-0588-04 | `no_op_when_new_borders_empty` | `BorderGraph` with prior state; call with `new_borders = HashMap::new()` | check | `BorderGraph` UNCHANGED; method returns immediately. (R37f line 21 — no-op — TASK-0588 acceptance line 34.) |
| UT-0588-05 | `gate_no_op_when_delta_mode_false` | `cfg_streaming_only`; chunk produces border wires | observe call-trace | `extend_with_chunk_borders` is NOT called. (R37f conjunction gate — TASK-0588 acceptance line 32.) |
| UT-0588-06 | `gate_no_op_when_streaming_inactive` | `cfg_delta_only` (chunk_size = u32::MAX → R26 short-circuit; streaming inactive) | observe | `extend_with_chunk_borders` is NOT called from the streaming pipeline. (Joint with TEST-SPEC-0567 short-circuit; cross-cuts batch path which has its own border discovery.) |
| UT-0588-07 | `accumulator_cleared_between_chunks` | `cfg_delta_streaming`; chunk 1 produces 2 borders, chunk 2 produces 1 border | observe `new_borders_this_chunk` between chunks | after chunk 1's `extend_with_chunk_borders` call, accumulator is cleared; chunk 2's accumulator starts empty and gathers exactly 1 entry. |

## Unit Tests (call-site placement order)

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0588-08 | `extension_called_BEFORE_next_chunk_assign_partition` | `cfg_delta_streaming`; chunks 1 and 2 | observe message-emission timeline | `extend_with_chunk_borders` is invoked AFTER chunk 1's `install_connection` calls AND BEFORE chunk 2's `AssignPartition` send. (Call-site discipline per TASK-0588 acceptance line 30.) |
| UT-0588-09 | `pull_mode_extension_in_generating_next_state` | `cfg_delta_streaming` with `dispatch_mode = Pull` | observe coordinator FSM trace | `extend_with_chunk_borders` is invoked from the `GeneratingNext → AwaitingResults` transition (TASK-0588 acceptance line 29 + cross-cut with TEST-SPEC-0577). |

## Integration tests (R37f conjunction full pipeline)

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| IT-0588-01 | `delta_streaming_isomorphic_to_batch_baseline` | `cfg_delta_streaming`; 4 workers; 8 chunks; `ep_annihilation_pure(64)` | run streaming pipeline + merge; compare to `split(make_net(64), 4) + delta-merge` batch baseline | `nets_isomorphic(merged_streaming, merged_batch) == true`. (TASK-0588 acceptance line 35.) |
| IT-0588-02 | `delta_streaming_dual_tree_isomorphic` | `cfg_delta_streaming`; `dual_tree(8)`; chunk_size = 16 | same | `nets_isomorphic == true`. |
| IT-0588-03 | `mid_stream_border_emergence_correctly_extended` | a stream where chunk 3 introduces a border that resolves to a port created in chunk 1 (cross-chunk forward reference) | run pipeline; observe `BorderGraph` state after chunk 3 | the new border is in the `BorderGraph`; merged result is correct. (Tests the cross-chunk extension semantics directly.) |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Chunk produces ZERO border wires (all internal) | `new_borders_this_chunk` stays empty; `extend_with_chunk_borders` is called with empty map (UT-0588-04 no-op path); `BorderGraph` unchanged. |
| EC-2 | Same `border_id` appears in two consecutive chunks (e.g., a border wire that crosses chunk N+1 mid-resolution) | UT-0588-03 idempotency holds; second call is a no-op for the duplicate id. |
| EC-3 | `BorderGraph` is the coordinator's view; worker-local `border_entries` cache (TEST-SPEC-0590) is updated independently | the call-site here updates ONLY the coordinator's `BorderGraph`; worker caches are propagated via `AssignPartition` payload metadata. (Cross-spec with TEST-SPEC-0590.) |
| EC-4 | M5 milestone gate (`ep_con 100M coordinator-side`) | enabled by this amendment but out-of-scope for SPEC-21 per TASK-0588 NOTE line 78 (cross-cut with TEST-SPEC-0516 line 58). |

## Invariants asserted

- R37f (BorderGraph update under `delta_mode && streaming_active`; closes SC-017).
- R36 (delta+pull compatibility — SHOULD elevated to MUST under conjunction).
- §3.8 A7 (SPEC-21 amendment consuming SPEC-19 A10).
- G1 (BSP determinism under delta+streaming) — restored by R37f call-site.
- ARG-005 INV-REC (delta-recoverability — preserved under streaming via this call-site).
- D2 / D3 (border completeness, cross-round border discovery) — preserved across chunks.

## ARG/DISC/REF citation

- ARG-005 INV-REC (delta border completeness, extended to streaming).
- ARG-001 G1 (preserved under delta+streaming).

## Determinism notes

**CROSS-SPEC OWNERSHIP SPLIT:** This task ships the CALL-SITE DISCIPLINE only. The `extend_with_chunk_borders` IMPLEMENTATION lives in SPEC-19 (cross-spec). Tests in this TEST-SPEC depend on the SPEC-19 method existing as code; if SPEC-19 implementation lags, this task's integration tests CANNOT pass. The dependency is documented in TASK-0588 line 8 ("Blocked by SPEC-19 implementation").

**REGRESSION-CATCHER BUILD VARIANT (UT-0588-02):** The without-call variant requires a `#[cfg(feature = "regression_catcher_no_extend")]` gate. The CI matrix MUST exercise this gate (separately from the default build) to confirm the assertion FAILS without the call-site. This is a load-bearing test for the gate's necessity per TASK-0588 acceptance line 35.

**TOKIO ORDERING:** Under `dispatch_mode = Pull` (UT-0588-09), the FSM-driven extension call MUST fire from `GeneratingNext → AwaitingResults` synchronously within the same `tokio::select!` arm. Use `#[tokio::test(flavor = "current_thread")]` to ensure deterministic ordering. Multi-thread runtimes would race the extension against the `AssignPartition` send.

**ACCUMULATOR LIFETIME:** The `new_borders_this_chunk` accumulator is per-chunk-scoped; UT-0588-07 asserts the clear-between-chunks invariant. Forgetting to clear would cause the accumulator to grow unboundedly and pass duplicate borders to `extend_with_chunk_borders` (idempotency would mask the bug functionally but inflate latency).

**ARG-005 CLOSURE ANCHOR:** This task is the operational closure of ARG-005 INV-REC under streaming. The without-extension variant (UT-0588-02) is the regression-catcher that fails closed if the call-site is removed.

## Cross-test dependencies

- **TEST-SPEC-0516 (SPEC-19 BorderGraph::extend_with_chunk_borders amendment-level coverage)** — predecessor; this task is the production-side call-site closure.
- **TEST-SPEC-0553 (`install_connection` helper)** — predecessor; the call-site that produces `new_borders` per chunk.
- **TEST-SPEC-0554 (orchestrator)** — predecessor; the host function for the call-site wiring.
- **TEST-SPEC-0577 (coordinator FSM)** — sibling; under `dispatch_mode = Pull`, the call fires from `GeneratingNext` (UT-0588-09).
- **SPEC-19 amendment A7 (cross-spec)** — provides the `extend_with_chunk_borders` IMPLEMENTATION; this task depends on its prior landing.
- **TEST-SPEC-T6 / T7 (streaming-vs-batch + end-to-end reduction equivalence)** — full delta+streaming exercise via this amendment per TEST-SPEC-0516 line 57; integration-level closure shared.
- **TEST-SPEC-0590 (R10b Strategy B `border_entries` cache)** — cross-cut on EC-3; worker-local cache is propagated independently.
