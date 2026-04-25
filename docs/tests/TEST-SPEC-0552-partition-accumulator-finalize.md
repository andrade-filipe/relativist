# TEST-SPEC-0552: PartitionAccumulator::finalize (Sparse → Dense via to_dense(id_range))

**SPEC-21 §7 ID:** plumbing (gates §4.9 finalize; T14a partition-scoped to_dense; SC-006 closure).
**Owning task:** TASK-0552.
**Parent spec:** SPEC-21 §4.9 finalize; §3.5 R23 (dense-finalized sizing); SPEC-22 R20 (partition-scoped to_dense), R30 (DenseAllocationExceedsThreshold).
**Type:** unit + integration (cross-spec dependency on SPEC-22).
**Theory anchor:** AC-010 (HVM4 frame-reuse — finalize is the frame-emission point).

---

## Inputs / Fixtures

- A "small contiguous" fixture: 100 agents at IDs [0, 100), all live; `id_range = [0, 100)`.
- A "sparse-acceptable" fixture: 100 live agents at IDs [0, 400) (i.e., id_range covers 400 slots, 4× live count — exactly at threshold); `id_range = [0, 400)`.
- A "dense-rejection" fixture: 100 live agents at IDs [0, 10_000) (id_range = 10_000, 100× live count, well above 4× threshold).
- A fixture exercising `freeport_redirects` — 5 wires registered as border via `connect((agent, port), FreePort(b))`.
- An accumulator pre-loaded as `AccumulatorNet::Dense(Net)` (the alternative variant — short-circuit case).

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0552-01 | `finalize_small_contiguous_returns_partition` | the small-contiguous fixture; `id_range = [0, 100)` | call `finalize(id_range)` | returns `Ok(Partition)`. |
| UT-0552-02 | `finalized_partition_subnet_is_dense_net` | UT-0552-01's result | inspect `partition.subnet` | is the dense `Net` type (NOT SparseNet). |
| UT-0552-03 | `finalized_subnet_agents_len_matches_id_range` | UT-0552-01's result | `partition.subnet.agents.len()` | `== 100` (R23: dense-finalized sizing equals `id_range.end - id_range.start`). |
| UT-0552-04 | `finalize_dense_rejection_above_threshold` | the dense-rejection fixture; `id_range = [0, 10_000)`, 100 live | call `finalize(id_range)` | returns `Err(PartitionError::DenseAllocationExceedsThreshold)` (SPEC-22 R30). |
| UT-0552-05 | `finalize_at_exactly_4x_threshold` | the sparse-acceptable fixture; `id_range = [0, 400)`, 100 live | call `finalize(id_range)` | returns `Ok(Partition)`. (Boundary: exactly 4× should pass per SPEC-22 R30 strict-greater-than discipline; the test asserts the documented inequality.) |
| UT-0552-06 | `freeport_redirects_preserved_through_finalize` | a sparse fixture with 5 freeport_redirects entries | call `finalize`; inspect resulting Net's `freeport_redirects` | all 5 entries preserved (delegated to SPEC-22 R13 / TEST-SPEC-T14). |
| UT-0552-07 | `dense_variant_short_circuits_finalize` | `AccumulatorNet::Dense(Net)` accumulator | call `finalize` | returns the inner Net wrapped in a Partition; no Sparse → Dense conversion happens (no-op variant). |
| UT-0552-08 | `partition_field_set_complete` | UT-0552-01's result | inspect `Partition` fields | all 6 SPEC-04 R21 fields present: `subnet`, `free_port_index`, `id_range`, `border_id_start`, `border_id_end`, `worker_id`. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | `id_range` is empty (`.start == .end`) | finalize returns `Ok(Partition)` with empty subnet; downstream merge MUST tolerate empty partitions. |
| EC-2 | Live agents exist outside `id_range` (e.g., agent at id 50, id_range = [10, 20)) | finalize MUST error: `PartitionError::AgentOutsideIdRange { agent_id, id_range }` or equivalent. The accumulator's worker assignment was wrong. |
| EC-3 | A live agent at id_range.end - 1 (boundary) | included in finalized partition; `subnet.agents[id_range.end - 1 - id_range.start]` is the dense slot containing that agent. |
| EC-4 | Calling finalize twice on the same accumulator | second call MUST error or be undefined (the accumulator was consumed by the first finalize). The test asserts the documented behavior. |

## Invariants asserted

- §4.9 finalize contract.
- R23 (dense-finalized sizing).
- D4 (ID Uniqueness After Distributed Reduction) — preserved via `id_range` scoping.
- SPEC-22 R30 (DenseAllocationExceedsThreshold rejection) — UT-0552-04.
- SPEC-22 R13 (freeport_redirects preservation) — UT-0552-06.
- §4.9 SC-006 closure (SparseNet → Dense conversion at finalize-time).

## ARG/DISC/REF citation

- AC-010 (HVM4 frame-reuse — finalize is the analog of the WNF frame-emission point).

## Determinism notes

The Sparse → Dense conversion iterates the SparseNet's `agents: HashMap<AgentId, Agent>` to populate the dense `Vec`. HashMap iteration order is non-deterministic; the conversion MUST sort by `AgentId` (or equivalently, use the dense slot index as the iteration key, walking 0..id_range.len() and looking up via HashMap::get). This guarantees the resulting dense Net has byte-identical layout regardless of HashMap seed. UT-0552-06 implicitly tests this via behavioral equality (TEST-SPEC-0491 helper).

The `DenseAllocationExceedsThreshold` discriminator (UT-0552-04 / UT-0552-05) is `id_range_size > 4 * live_agent_count`. The exact `>` vs `>=` is per SPEC-22 R30; the test reads the spec's inequality and asserts the boundary case (UT-0552-05) accordingly. Document the chosen discrimination in the test code.

## Cross-test dependencies

- TEST-SPEC-0550, TEST-SPEC-0551 (accumulator construction + mutation) — prerequisites.
- **SPEC-22 fixture reuse (mandatory):**
  - TEST-SPEC-0490 (sparse-to-dense id_range) — UT-0552-02/03 build on this; do NOT duplicate the to_dense(id_range) tests.
  - TEST-SPEC-0491 (is_behaviorally_equal helper) — used in UT-0552-06 for the freeport_redirects round-trip check.
  - TEST-SPEC-0484 (DenseAllocationExceedsThreshold) — UT-0552-04 invokes the same error path.
  - TEST-SPEC-T14a (partition-scoped to_dense) — direct sibling.
- TEST-SPEC-T10 (peak memory) — depends on finalize being instrumented; full T10 in the orchestrator-level test (TASK-0584, out of scope wave 1).
