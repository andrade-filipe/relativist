# TASK-0413: [SPEC-06 amendment A1] Conditional `elastic_departure` clause on R25 / PhaseTimeout path

**Spec:** SPEC-20 §3.8 A1 (closes SC-011). Modifies SPEC-06 R25 and the interaction with SPEC-13 R21 `WaitingForResults × PhaseTimeout → Error`.
**Requirements:** A1 conditional clause; coordinates with R18 (elastic-timeout detection) and R19 (connection-loss detection).
**Priority:** P0 (blocker for TASK-0438, TASK-0439 departure-detection FSM wiring).
**Status:** TODO
**Depends on:** none (amendment to existing behavior; activated only when `elastic_departure = true`).
**Blocked by:** none
**Estimated complexity:** S (~30-50 LoC production + ~40 LoC tests)
**Bundle:** SPEC-20 Elastic Grid — predecessor-spec amendment cluster.
**Tag:** `[SPEC-06 amendment]`

## Context

SPEC-06 R25 mandates that the coordinator abort the grid loop and return an error when a worker connection is lost during execution. SPEC-20 `elastic_departure = true` flips this into a recoverable path (reclaim state, remove worker from `W_active`, continue). The amendment is strictly conditional: when `elastic_departure = false`, v1 behavior is preserved bit-exactly.

## Acceptance Criteria

- [ ] In the coordinator's I/O error path (`relativist-core/src/protocol/coordinator.rs` R25 site), branch on `GridConfig.elastic_departure`:
  - `false` → v1 fatal path (unchanged).
  - `true` → emit `CoordinatorEvent::WorkerConnectionLost(id)` into the FSM instead of aborting.
- [ ] Same branching for `PhaseTimeout(id)` in `WaitingForResults`:
  - `false` → v1 fatal `Error` transition.
  - `true` → emit `CoordinatorEvent::PhaseTimeout(id)` into the FSM for elastic reclaim handling.
- [ ] No new CLI flag — toggled exclusively through `GridConfig.elastic_departure` (TASK-0415).
- [ ] Backward compatibility: with `elastic_departure = false`, the 1181 default / 1224 zero-copy baseline tests pass without regression.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/protocol/coordinator.rs` | modify | Add conditional branches on `elastic_departure` at R25 and PhaseTimeout sites. |
| `relativist-core/src/net/fsm/` *(SPEC-13 R21 transitions)* | read-only dep | Emits events consumed by TASK-0436 (FSM transition table extension). |

## Test Expectations (forward-ref)

Covered by EG-U18 preconditions and by EG-I3 (departure correctness v1), EG-U10a/b (urgent leave).

## Invariants Touched

- D6 (Protocol Termination) — preserved: elastic path adds at most one extra round per departure.

## Notes

- This task is a small, surgical branching of existing behavior. The heavy lifting (retained-state management, reclaim logic) lives in TASK-0438 and TASK-0443.

## DAG Links

- **Predecessors:** none.
- **Successors:** TASK-0436 (FSM transition table), TASK-0438 (departure detection).
