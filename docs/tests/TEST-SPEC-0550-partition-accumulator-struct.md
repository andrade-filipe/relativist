# TEST-SPEC-0550: PartitionAccumulator struct (AccumulatorNet enum default Sparse)

**SPEC-21 §7 ID:** plumbing (gates §4.9 SparseNet adoption; SC-006 closure).
**Owning task:** TASK-0550.
**Parent spec:** SPEC-21 §4.9 PartitionAccumulator (with `AccumulatorNet { Sparse(SparseNet), Dense(Net) }` enum); SPEC-22 R22 (SparseNet adoption); SC-006 closure.
**Type:** unit.
**Theory anchor:** AC-010 (HVM4 WNF Evaluation — frame-reuse pattern informs the accumulator design).

---

## Inputs / Fixtures

- A fresh `PartitionAccumulator::new(WorkerId(0))` (default constructor with no agents).
- The `AccumulatorNet` enum exposed via the accumulator's internal `subnet: AccumulatorNet` field (test-only access).

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0550-01 | `default_subnet_is_sparse_variant` | a fresh accumulator | inspect the `subnet` discriminant | `AccumulatorNet::Sparse(_)` variant (NOT Dense). This is the SC-006 closure default; failure indicates a future refactor flipped the default. |
| UT-0550-02 | `default_sparse_is_empty` | a fresh accumulator | call `live_agent_count()` | `== 0`. |
| UT-0550-03 | `default_min_max_assigned_id_none` | a fresh accumulator | inspect `min_assigned_id` and `max_assigned_id` | both `None` (no agents added yet). |
| UT-0550-04 | `default_free_port_index_empty` | a fresh accumulator | inspect `free_port_index` | empty (no border references registered). |
| UT-0550-05 | `worker_id_field_correct` | construct accumulators with `WorkerId(0..4)` | inspect `worker_id` on each | matches the constructor argument. |
| UT-0550-06 | `multi_worker_construction_independent` | 4 accumulators built in parallel | each one's internal state | independent (no shared SparseNet, no shared free_port_index). |
| UT-0550-07 | `derives_present_on_accumulator_net_enum` | `AccumulatorNet` enum definition | grep | contains `Debug` (mandatory for diagnostics); other derives per SPEC-21 §4.9. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | `WorkerId(u32::MAX)` constructor | succeeds; no clamp; UT-0550-05 holds at the boundary. |
| EC-2 | Future amendment that adds a third `AccumulatorNet` variant (e.g., `Hybrid`) | UT-0550-01 MUST be updated; SC-006 closure rationale MUST be re-examined. Coordinate with ESPECIALISTA EM SPECS. |
| EC-3 | A constructor variant `PartitionAccumulator::with_initial_dense(...)` (test-only or future) | the test MUST then assert which constructor was used to enforce the SC-006 default discipline. |

## Invariants asserted

- §4.9 default-Sparse construction (closes SC-006).
- AccumulatorNet enum surface (Sparse variant present and default).
- T1 / I1 / I2 (preserved — SparseNet honors these per SPEC-22 R26).
- D4 (preserved post-finalize — TASK-0552 enforces).
- I3' (preserved — SparseNet `agents: HashMap<AgentId, Agent>` per SPEC-22 R13/R29).

## ARG/DISC/REF citation

- AC-010 (HVM4 WNF Evaluation — frame-reuse pattern informs §4.9 PartitionAccumulator design).

## Determinism notes

Pure synchronous, no tokio, no RNG. The test does NOT iterate the SparseNet's internal HashMap (which has non-deterministic iteration order); UT-0550-02 calls `live_agent_count()` (deterministic via R14).

## Cross-test dependencies

- **SPEC-22 fixture reuse (mandatory):** TEST-SPEC-0486 (SparseNet struct), TEST-SPEC-T11 (SparseNet construction and count). DO NOT duplicate the SparseNet semantics tests; cite them.
- TEST-SPEC-0551 (add_agent_connect) — extends this with mutation tests.
- TEST-SPEC-0552 (finalize) — extends with the Sparse → Dense conversion path.
- TEST-SPEC-0491 (is_behaviorally_equal helper) — shared equivalence-test infrastructure.
