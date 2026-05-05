# TASK-0451: INFO/WARN logging on join (R17) and departure (R28)

**Spec:** SPEC-20 §3.2 R17 (INFO log on join), §3.3.4 R28 (WARN log on departure).
**Requirements:** R17 (SHOULD), R28 (SHOULD).
**Priority:** P2 (observability; non-blocking).
**Status:** TODO
**Depends on:** TASK-0432 (join handler), TASK-0441 (leave handler), TASK-0440+0443 (reclaim).
**Blocked by:** TASK-0432, TASK-0441.
**Estimated complexity:** S (~40-60 LoC production + ~40 LoC tests)
**Bundle:** SPEC-20 Elastic Grid — Phase 3 Observability.

## Context

R17: log each worker join event at INFO including `K_eff_new`, `worker_id`, `partition_index`, round number at first participation. R28: log each departure at WARN including `worker_id`, departure type (`timeout | connection_loss | leave_after_result | leave_urgent`), round number, retained slot consumed.

## Acceptance Criteria

- [ ] On `WorkerJoined(id)` registration, emit `tracing::info!` with all required fields.
- [ ] On any departure action (`LogDeparture(id, DepartureKind)`), emit `tracing::warn!` with all required fields.
- [ ] Test verifies log message format (use `tracing_test` or equivalent).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/protocol/coordinator.rs` | modify | Insert tracing macros at join/departure points. |

## Test Expectations (forward-ref)

- Covered as side effect of EG-U7, EG-U10, EG-U11 (assertions on logged fields).

## Invariants Touched

- None.

## DAG Links

- **Predecessors:** TASK-0432, TASK-0441, TASK-0440, TASK-0443.
- **Successors:** none.
