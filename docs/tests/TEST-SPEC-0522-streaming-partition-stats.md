# TEST-SPEC-0522: StreamingPartitionStats struct

**SPEC-21 §7 ID:** plumbing (T1 partial — per-worker counts verification deferred to TASK-0530 strategy correctness).
**Owning task:** TASK-0522.
**Parent spec:** SPEC-21 §4.1 (StreamingPartitionStats); R3 (strategy `finalize()` returns stats); SC-021 closure (chunks_processed pipeline-owned).
**Type:** unit.
**Theory anchor:** None direct (observability surface).

---

## Inputs / Fixtures

- A `StreamingPartitionStats` instance: `{ per_worker_agent_counts: vec![25, 25, 25, 25], chunks_processed: 0, total_pending_resolved: 0 }` (or whatever the §4.1 field set is — exact fields per task acceptance).

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0522-01 | `stats_constructible` | explicit field literal | construct | succeeds; fields readable. |
| UT-0522-02 | `stats_debug_format` | the fixture | `{:?}` format | non-empty string; includes per_worker_agent_counts values. |
| UT-0522-03 | `stats_clone` | the fixture | `.clone()` | deep clone; mutating the clone does not affect the original. |
| UT-0522-04 | `stats_serde_round_trip` | the fixture | bincode encode → decode | decoded `== original`. |
| UT-0522-05 | `chunks_processed_zero_when_returned_by_strategy_finalize` | the strategy's `finalize()` return | inspect `.chunks_processed` | `== 0` (per SC-021 closure: chunks_processed is pipeline-owned, NOT strategy-owned; strategies always return 0 in this field). |
| UT-0522-06 | `chunks_processed_doc_documents_pipeline_ownership` | the field's Rustdoc | grep for "pipeline-owned" or "strategy MUST return 0" | substring present (mandatory per task notes — without this Rustdoc, future readers may incorrectly trust the strategy-returned value). |
| UT-0522-07 | `derives_present` | struct definition | grep `#[derive(...)]` | contains `Debug, Clone, PartialEq, Serialize, Deserialize`. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | `per_worker_agent_counts` is empty (0 workers) | constructs; serde round-trip OK. Downstream pipeline MUST handle 0-worker case as configuration error (out of this test's scope). |
| EC-2 | `chunks_processed = u32::MAX` | constructs; serde round-trip OK. |
| EC-3 | Mixed `per_worker_agent_counts` (e.g., `[100, 0, 50, 0]`) | constructs; the test does NOT assert balance — partition quality affects performance only, not correctness. |

## Invariants asserted

- §4.1 StreamingPartitionStats derive set.
- SC-021 closure (chunks_processed pipeline-owned).

## ARG/DISC/REF citation

- None at type level.

## Determinism notes

Pure synchronous. Bincode 1.x. Deterministic by construction.

## Cross-test dependencies

- TEST-SPEC-T1 (RoundRobin assignment correctness) — extends this with per-worker count verification under a real strategy run.
- TEST-SPEC-0530 (RoundRobin strategy) — produces `StreamingPartitionStats` instances; UT-0530-* asserts per-worker count correctness.
- TEST-SPEC-0554 (orchestrator) — owns the `chunks_processed` field at run time; UT-0554 asserts `result.stats.chunks_processed == ceil(total_agents / chunk_size)`.
