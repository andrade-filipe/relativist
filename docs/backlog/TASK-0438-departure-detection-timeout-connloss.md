# TASK-0438: Departure detection — `collect_timeout` + TCP-error ConnectionLost (R18, R19)

**Spec:** SPEC-20 §3.3.1 R18 (timeout detection, closes SC-011 partial), R19 (connection-loss detection).
**Requirements:** R18, R19.
**Priority:** P0 (foundation of all departure behavior).
**Status:** TODO
**Depends on:** TASK-0413 (SPEC-06 R25 amendment), TASK-0414 (event enums), TASK-0426 (TimerKind::Collect), TASK-0436 (FSM transition Phase C).
**Blocked by:** TASK-0413, TASK-0436 Phase C.
**Estimated complexity:** S (~70-100 LoC production + ~80 LoC tests)
**Bundle:** SPEC-20 Elastic Grid — Phase 2.3 Dynamic Departure.

## Context

R18: if a worker does not return its current-round result within `collect_timeout` AND `elastic_departure = true`, treat as departed (emit `PhaseTimeout(id)` → FSM reclaim path), rather than v1 fatal. When `elastic_departure = false`, v1 fatal preserved exactly. R19: TCP I/O error immediately emits `WorkerConnectionLost(id)` without waiting for timeout.

## Acceptance Criteria

- [ ] Existing `collect_timeout` (SPEC-06 R30, default 600s in v1; R33 may override for elastic) is reused; no new heartbeat protocol.
- [ ] `PhaseTimeout` fires per-worker when their result is overdue: emit `CoordinatorEvent::PhaseTimeout(worker_id)`.
- [ ] TCP I/O error on a worker stream (send or recv) → emit `CoordinatorEvent::WorkerConnectionLost(worker_id)` immediately, without waiting for `collect_timeout`.
- [ ] FSM transitions (TASK-0436 Phase C):
  - `elastic_departure = true` → recovery path (reclaim + remove).
  - `elastic_departure = false` → v1 fatal `Error` (consistent with SPEC-06 R25 amendment A1 in TASK-0413).
- [ ] Collection continues for remaining workers; coordinator proceeds to `Merging` when all responsive workers have returned or timed out.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/protocol/coordinator.rs` | modify | Per-worker collect timer; I/O error → ConnectionLost event. |

## Test Expectations (forward-ref)

- EG-I3 `test_elastic_departure_correctness_v1` (R18-R26, G1 CLOSED via ARG-006).
- EG-I3-delta (R18-R26, G1 CONDITIONAL/CLOSED-for-conservative).
- EG-U7 `test_departure_reclaim_initial` (R23a-b, R24a).

## Invariants Touched

- D6 — preserved via fatal fallback when elastic off.

## Notes

- **Per-worker vs aggregate timeout**: R18 mandates per-worker detection (not a single aggregate timer).

## DAG Links

- **Predecessors:** TASK-0413, TASK-0414, TASK-0426, TASK-0436.
- **Successors:** TASK-0443 (reclaim logic), TASK-0441 (graceful leave).
