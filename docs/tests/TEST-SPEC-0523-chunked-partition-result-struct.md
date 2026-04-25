# TEST-SPEC-0523: ChunkedPartitionResult struct (partitions + borders + stats)

**SPEC-21 §7 ID:** plumbing (T6 partial — full pipeline isomorphism via TASK-0567).
**Owning task:** TASK-0523.
**Parent spec:** SPEC-21 §3.3 R20, R21 (structurally compatible with PartitionPlan); §4.1 ChunkedPartitionResult struct.
**Type:** unit.
**Theory anchor:** ARG-002 Q5/C1-C3 (split/merge identity).

---

## Inputs / Fixtures

- A constructed `ChunkedPartitionResult` with 4 partitions, 8 border entries, and a stats instance.
- A constructed `PartitionPlan` (the SPEC-04 v1 type) for cross-conversion verification.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0523-01 | `result_constructible` | explicit fixture | construct via struct literal | succeeds; fields readable. |
| UT-0523-02 | `result_serde_round_trip` | the fixture | bincode encode → decode | decoded `== original` (`PartialEq` on the full struct). |
| UT-0523-03 | `from_chunked_to_partition_plan_preserves_partitions_one_to_one` | the fixture | `let plan: PartitionPlan = result.into();` | `plan.partitions.len() == result.partitions.len()` AND each partition's fields (`subnet`, `free_port_index`, `id_range`, `border_id_start`, `border_id_end`, `worker_id`) are byte-identical. |
| UT-0523-04 | `from_chunked_to_partition_plan_preserves_borders_one_to_one` | same fixture | inspect `plan.borders` | identical to `result.borders` (1:1 mapping; no transformation). |
| UT-0523-05 | `from_chunked_to_partition_plan_drops_stats` | same fixture | `plan` field set | `stats` field is NOT present in `PartitionPlan` (R21 — the conversion is information-shedding for the stats; merge protocol does not consume stats). |
| UT-0523-06 | `partition_field_set_matches_spec04_r21` | UT-0523-03 partition | grep field names: `subnet`, `free_port_index`, `id_range`, `border_id_start`, `border_id_end`, `worker_id` | all 6 fields present (R21 structural compat). |
| UT-0523-07 | `derives_present` | struct definition | grep `#[derive(...)]` | contains `Debug, Clone, PartialEq, Serialize, Deserialize`. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | An empty result (`partitions: vec![], borders: vec![]`) | constructs; serde OK; conversion to PartitionPlan produces an empty plan. |
| EC-2 | A result with mismatched border IDs (e.g., border_id 0 appears in 1 partition only) | constructs at the type level (no validation); C3 bijectivity is enforced AT FINALIZE-TIME in TASK-0570 / TEST-SPEC-T5, NOT at struct construction. |
| EC-3 | A future SPEC-04 amendment adding a partition field | UT-0523-06 MUST be updated; the `From` conversion impl in TASK-0523 MUST be updated synchronously; this is a sibling-spec coordination point also flagged in TEST-SPEC-0517. |

## Invariants asserted

- R20, R21 (structural compatibility with PartitionPlan).
- §4.1 ChunkedPartitionResult derive set.

## ARG/DISC/REF citation

- ARG-002 Q5/C1-C3.

## Determinism notes

Pure synchronous, no tokio. Bincode 1.x.

## Cross-test dependencies

- TEST-SPEC-T6 (streaming vs batch isomorphism) — exercises the `From` conversion in the actual pipeline run.
- TEST-SPEC-0517 (SPEC-04 split additive amendment) — share the partition-field-set coordination point.
- TEST-SPEC-0554 (orchestrator) — produces `ChunkedPartitionResult` instances at run time.
