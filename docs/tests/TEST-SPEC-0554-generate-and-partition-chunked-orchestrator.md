# TEST-SPEC-0554: generate_and_partition_chunked orchestrator (full pipeline integration)

**SPEC-21 §7 ID:** T5 (streaming pipeline produces valid partitions); T6 partial; T8 partial.
**Owning task:** TASK-0554.
**Parent spec:** SPEC-21 §3.3 R17, R18, R19, R22; §4.6 chunked pipeline architecture; SC-021 closure (chunks_processed pipeline-owned).
**Type:** integration + property.
**Theory anchor:** ARG-001 G1 (full-cycle equivalence); ARG-002 Q5/C1-C3 (split/merge identity).

---

## Inputs / Fixtures

- `ep_annihilation_stream(100, chunk_size=20)` with `num_workers = 4` and `RoundRobinStreamingStrategy`.
- `dual_tree_stream(8, chunk_size=2)` with `num_workers = 2`.
- A peak-memory instrumentation hook (e.g., `jemalloc_ctl::stats::allocated`).
- The `nets_isomorphic` / `is_behaviorally_equal` helper (SPEC-22 R21).

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0554-01 | `t5_full_integration_ep_annihilation_100_4_workers` | `ep_annihilation_stream(100)`, chunk_size=20, num_workers=4 | invoke `generate_and_partition_chunked(...)` | `Ok(ChunkedPartitionResult)`; `result.partitions.len() == 4`; total agents across partitions = 200; total wires (internal + border) = 100. C1/C2/C3 verified at the §4.8 SPEC-04 assertion sites. |
| UT-0554-02 | `chunks_processed_count_correct` | UT-0554-01 fixture | inspect `result.stats.chunks_processed` | `== 5` (`ceil(100 / 20)`; SC-021 closure validation; the orchestrator OWNS this field; the strategy returned 0 per TEST-SPEC-0522). |
| UT-0554-03 | `pending_store_empty_post_run` | UT-0554-01 fixture | post-pipeline: inspect the orchestrator's pending-connection store (test-only accessor) | empty (R19). |
| UT-0554-04 | `pending_store_non_empty_returns_error` | a malformed generator emitting `Pending { target_agent: 999 }` with no batch ever containing agent 999 | invoke `generate_and_partition_chunked` | returns `Err(PipelineError::UnresolvedPending { agent_id: 999 })` or equivalent. (R19 negative path; T4.) |
| UT-0554-05 | `t6_streaming_vs_batch_isomorphism_ep_annihilation` | `ep_annihilation_pure(100)` via (a) `split(make_net, 4)` and (b) `generate_and_partition_chunked` | merge both via SPEC-05 `merge`; compare merged Nets | `nets_isomorphic == true`. |
| UT-0554-06 | `t6_streaming_vs_batch_isomorphism_dual_tree` | `dual_tree(8)` via both paths | same comparison | `nets_isomorphic == true`. (UT-0542-06 / T7 integrates further with reduction.) |
| UT-0554-07 | `r22_one_batch_in_flight_peak_memory` | `ep_annihilation_stream(10_000, chunk_size=100)`, 4 workers; instrument peak memory | post-run: peak allocation | bounded by `O(chunk_size + accumulator_sizes + border_count + pending_size)`. The test asserts peak `< 4 * stream_total_memory` (a loose ceiling — full T10 in TASK-0584 with strict bound). |
| UT-0554-08 | `chunk_size_one_works_end_to_end` | `ep_annihilation_stream(20, chunk_size=1)` | run | `Ok`; `nets_isomorphic` to baseline. |

## Property tests

| ID | Property | Generator | Assertion |
|----|----------|-----------|-----------|
| PT-0554-01 | T8 chunk-size independence | proptest: `(benchmark, size, num_workers, chunk_size)` with chunk_size in [1..size] | merged result is `nets_isomorphic` to `reduce_all(make_net(size))`. (Joint with TEST-SPEC-T8.) |
| PT-0554-02 | C1 / C2 / C3 hold for any pipeline run | proptest: same generator | post-finalize: §4.8 SPEC-04 assertions pass. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | `chunk_size > total_agents` (single chunk) | pipeline runs the single chunk; `chunks_processed == 1`; result is correct. |
| EC-2 | `num_workers = 1` | all agents go to worker 0; no border wires; result trivial. |
| EC-3 | `total_agents = 0` (empty stream) | pipeline returns `Ok(ChunkedPartitionResult)` with 0 partitions or empty partitions; downstream merge MUST tolerate. |
| EC-4 | A chunk emits 0 agents but 1 Pending directive whose target is in a previous chunk | install_connection invoked; resolve as either internal or border wire depending on owner mapping. |
| EC-5 | Generator panics mid-stream | the orchestrator MUST propagate the panic without leaving accumulators in inconsistent state (test asserts no double-finalize is possible). |

## Invariants asserted

- R17 (function exists and signature matches §3.3).
- R18 (chunk-by-chunk processing sequence).
- R19 (empty-pending-store post-run; UT-0554-03 / UT-0554-04).
- R22 (one-batch-in-flight memory bound).
- D1 (Split/Merge Identity, extended) — UT-0554-05 / UT-0554-06.
- C1 / C2 / C3 — established per SPEC-04 §4.8 assertions invoked at finalize.
- T5 (streaming pipeline produces valid partitions).
- T6 (streaming vs batch isomorphism) — partial.
- T8 (chunk size independence) — partial.

## ARG/DISC/REF citation

- ARG-001 G1.
- ARG-002 Q5/C1-C3.

## Determinism notes

The orchestrator is single-threaded by design (R22 — one batch in flight). Tests MUST use `#[tokio::test(flavor = "current_thread")]` if any async path is exercised; pure-Core code paths use plain `#[test]`.

The peak-memory instrumentation (UT-0554-07) requires either `jemalloc_ctl` (Linux/macOS) or a custom allocator wrapper; the test MAY be `#[cfg(unix)]`-gated and not run on Windows CI. Document the gate in the test code.

UT-0554-05 / UT-0554-06 isomorphism check uses `is_behaviorally_equal` (SPEC-22 R21 / TEST-SPEC-0491). Border-id absolute integers are NOT compared (per TEST-SPEC-0510 cross-path note); only structural / behavioral equality.

## Cross-test dependencies

- TEST-SPEC-0510 (SPEC-04 R12 amendment) — UT-0554-05 cross-path comparison MUST account for distinct border-id ranges per the amendment.
- TEST-SPEC-0517 (SPEC-04 split additive amendment) — UT-0554-05 invokes the unmodified `split()` path.
- TEST-SPEC-0523 (ChunkedPartitionResult struct) — UT-0554-01's return type.
- TEST-SPEC-0530 (RoundRobin strategy) — used in UT-0554-01.
- TEST-SPEC-0541, TEST-SPEC-0542 (generator overrides) — produce input streams.
- TEST-SPEC-0550..0553 (accumulator + install_connection) — internal building blocks.
- TEST-SPEC-T5 (this TEST-SPEC IS T5).
- TEST-SPEC-T6, T7, T8 (full equivalence and chunk-size independence) — sibling spec-catalog tests.
