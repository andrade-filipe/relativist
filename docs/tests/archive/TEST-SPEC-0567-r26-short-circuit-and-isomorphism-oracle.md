# TEST-SPEC-0567: R26 short-circuit + T6/T8 isomorphism oracle (`chunk_size = u32::MAX` → `split()`)

**SPEC-21 §7 ID:** owner of T6 (streaming-vs-batch equivalence) and T8 (chunk-size independence) at the integration level. Mirrors TEST-SPEC-T6-streaming-vs-batch-equivalence and TEST-SPEC-T8-chunk-size-independence (the spec-catalog T-tests delegate to this plumbing file for short-circuit + isomorphism oracle coverage).
**Owning task:** TASK-0567.
**Parent spec:** SPEC-21 §3.4 R26 (short-circuit; closes SC-014); §3.8 A8 (split() unchanged + chunked alternative entry point).
**Type:** unit + integration (orchestrator branch + isomorphism harness across benchmarks).
**Theory anchor:** ARG-002 (split/merge identity — extended for streaming via R10/R11/R26 isomorphism contract).

---

## Inputs / Fixtures

- The post-TASK-0554 orchestrator `generate_and_partition_chunked(stream, num_workers, chunk_size, strategy)`.
- A fresh `GridConfig::default()` with `chunk_size = u32::MAX` (sentinel) and another with `chunk_size = 256`.
- Three benchmarks: `ep_annihilation_pure(1024)`, `dual_tree(8)`, and a mixed benchmark (`mixed_net` if registered, else fall back to `ep_annihilation_pure(512)` + `dual_tree(6)` overlay).
- The `nets_isomorphic` helper (SPEC-00 §6.12 / SPEC-22 R21 `is_behaviorally_equal`).
- A test-only counter or `tracing` event spy: `streaming_short_circuit_taken: AtomicBool` (instrumentation gated on `#[cfg(test)]`) toggled at the top of the orchestrator's `chunk_size == u32::MAX` branch.

## Unit Tests (R26 short-circuit branch)

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0567-01 | `short_circuit_taken_when_chunk_size_is_u32_max` | `cfg.chunk_size == u32::MAX`; `ep_annihilation_pure(64)` stream | call `generate_and_partition_chunked(...)` | `streaming_short_circuit_taken.load() == true`; the streaming-pipeline code path is NOT entered. |
| UT-0567-02 | `streaming_path_taken_when_chunk_size_below_max` | `cfg.chunk_size = 16`; same input | same call | `streaming_short_circuit_taken.load() == false`; the streaming pipeline IS entered. |
| UT-0567-03 | `short_circuit_uses_split_with_contiguous_id_strategy` | short-circuit triggered | observe the strategy passed into the inner `split(...)` call | `ContiguousIdStrategy` (SPEC-04 R22) regardless of `cfg.streaming_strategy`. (TASK-0567 NOTE line 69 — explicit independence from T9 axis.) |
| UT-0567-04 | `short_circuit_returns_chunked_partition_result` | short-circuit triggered | inspect return type | `ChunkedPartitionResult` constructed via `from_partition_plan(plan)` (R20-R21 conversion); structurally compatible with the streaming-path return. |
| UT-0567-05 | `short_circuit_chunk_size_below_max_negative_control` | `cfg.chunk_size = u32::MAX - 1` | call orchestrator | streaming path taken (NOT short-circuit); the sentinel is strictly `u32::MAX`. |

## Integration tests (T6 isomorphism harness)

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| IT-0567-T6-01 | `t6_ep_annihilation_streaming_isomorphic_to_batch` | `ep_annihilation_pure(1024)` | (a) batch path: `split(make_net(1024), 4, ContiguousIdStrategy)` → merge → `merged_batch`; (b) streaming path: `generate_and_partition_chunked(make_net_stream(1024, chunk_size=256), 4, RoundRobin)` → merge → `merged_streaming` | `nets_isomorphic(merged_batch, merged_streaming) == true`. **Bit-identical layout NOT asserted** (free-list / SparseNet / freeport_redirects per R26 closure of SC-014). |
| IT-0567-T6-02 | `t6_dual_tree_streaming_isomorphic_to_batch` | `dual_tree(8)` | same comparison at chunk_size = 64 | `nets_isomorphic == true`. |
| IT-0567-T6-03 | `t6_mixed_streaming_isomorphic_to_batch` | mixed benchmark | same comparison | `nets_isomorphic == true`. |

## Integration tests (T8 chunk-size independence)

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| IT-0567-T8-01 | `t8_chunk_size_independence_ep_annihilation_1024` | `ep_annihilation_pure(1024)` | run streaming pipeline at `chunk_size ∈ {2, 8, 64, 512, 1024}`; merge each | pairwise `nets_isomorphic(merged_k_i, merged_k_j) == true` for all pairs `(i, j)`. |
| IT-0567-T8-02 | `t8_chunk_size_one_edge_case` | `ep_annihilation_pure(64)` with `chunk_size = 1` | run streaming + merge | result isomorphic to `chunk_size = 64` baseline. (Minimal-chunk regression catcher per SPEC-21 R26 conjunction.) |
| IT-0567-T8-03 | `t8_chunk_size_equal_to_size_isomorphic_to_short_circuit` | `ep_annihilation_pure(1024)` with `chunk_size = 1024` (NOT `u32::MAX`) | run streaming pipeline; compare to `chunk_size = u32::MAX` short-circuit run | `nets_isomorphic == true`. (Single-chunk streaming converges to short-circuit semantically, even though paths differ.) |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | `chunk_size = u32::MAX` AND `dispatch_mode = Pull` | short-circuit wins (per TEST-SPEC-0512 EC-3); `tracing` warning logged; merged result equals batch baseline. |
| EC-2 | Empty stream (`size = 0`) with `chunk_size = u32::MAX` | short-circuit branch produces an empty `Net`; `split(empty_net, 4, ...)` returns 4 empty partitions; merge yields empty net (matches batch baseline). |
| EC-3 | Stream with size = 1 (one agent) and `chunk_size = u32::MAX` | short-circuit; `split` distributes the single agent to worker 0; other workers empty. Same as batch. |
| EC-4 | Partial materialization mid-stream raises panic | short-circuit's eager-collect MUST propagate the panic (no `catch_unwind`); document explicitly. |

## Invariants asserted

- R26 (short-circuit semantics; closes SC-014).
- §3.8 A8 (SPEC-04 split() UNCHANGED + chunked alternative entry point).
- D1' (extended split/merge identity for streaming — verified by isomorphism, not byte-equality, per R26 closure note).
- I3' (uniqueness preserved in both paths).
- T1 (port linearity preserved in both paths).
- T6 (streaming-vs-batch equivalence; full integration-level closure).
- T8 (chunk-size independence; full integration-level closure).

## ARG/DISC/REF citation

- ARG-002 (split/merge identity — Q5/C1-C3 extension to streaming via R10/R11/R26).

## Determinism notes

`nets_isomorphic` is deterministic (canonical-form comparison). The orchestrator's short-circuit branch is purely synchronous (eager-collect + `split()` are synchronous APIs); no tokio race possible. The `streaming_short_circuit_taken` AtomicBool is reset between tests via `#[cfg(test)]` test-fixture lifecycle (not shared across tests).

The streaming path under chunk_size = 1 (IT-0567-T8-02) MAY exercise tokio if the orchestrator is async-wrapped at TASK-0554; in that case `#[tokio::test(flavor = "current_thread")]` is required for deterministic ordering of pending-store resolution.

The pairwise comparison in IT-0567-T8-01 is O(k^2) for k chunk sizes; with k = 5 this is 10 comparisons; acceptable for a test suite. DO NOT add chunk_size sweep beyond this set without revising the harness budget.

## Cross-test dependencies

- TEST-SPEC-0540 (`make_net_stream` default-impl path equivalence) — provides the materialization path used by the short-circuit branch.
- TEST-SPEC-0541 (`ep_annihilation` native streaming override) — provides the chunked-path benchmark.
- TEST-SPEC-0542 (`dual_tree` streaming override) — provides the chunked-path benchmark.
- TEST-SPEC-0517 (SPEC-04 split() additive amendment) — establishes the unchanged-split contract that the short-circuit relies on.
- TEST-SPEC-0554 (orchestrator) — provides `generate_and_partition_chunked` host function.
- TEST-SPEC-0565 (GridConfig streaming fields) — provides the `chunk_size` field plumbing.
- TEST-SPEC-T6-streaming-vs-batch-equivalence — spec-catalog mirror; this plumbing file owns the integration-level closure.
- TEST-SPEC-T8-chunk-size-independence — spec-catalog mirror; same delegation.
- TEST-SPEC-0531 (Fennel strategy) — varied along T9 axis (T9 does NOT exercise the short-circuit per TASK-0567 NOTE line 69).
