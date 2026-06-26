# TASK-0436: Extended FSM transition table (§4.1.4) — all new rows for elastic grid

**Spec:** SPEC-20 §4.1.4 extended transition table (closes SC-012, SC-018). Integration of all new states, events, actions introduced by TASK-0414.
**Requirements:** A2 (SPEC-13 amendment landed in TASK-0414); plus R10, R10a, R10b, R15, R6, R22a, R22b, R22c, R18, R19, R3a, R5a.
**Priority:** P0 (the FSM is the authoritative event processor for all SPEC-20 behavior).
**Status:** TODO
**Depends on:** TASK-0414 (enums), TASK-0426 (TimerKind), TASK-0420 (WorkerId), TASK-0422 (select loop).
**Blocked by:** TASK-0414, TASK-0426.
**Estimated complexity:** L (~200-300 LoC production + ~150 LoC tests) — *if single-task size exceeds target, split into 436a (hybrid/solo rows) / 436b (join rows) / 436c (departure rows).*
**Bundle:** SPEC-20 Elastic Grid — Phase 2.1-2.3 FSM backbone.

## Context

The authoritative FSM transition table in §4.1.4 specifies ~25 new rows covering hybrid dispatch, solo mode, join window, graceful/urgent leave, timeout/conn-loss recovery. This task lands the transition function extension; runtime handlers (TASK-0423 spawn, TASK-0432 join, TASK-0438 departure detect, TASK-0443 reclaim) consume the actions.

## Acceptance Criteria (split allowed)

### Phase A — Hybrid / Solo rows (~70-100 LoC)
- [ ] `Init × ConfigLoaded → WaitingForWorkers`: +`StartTimer(InitialWait)`.
- [ ] `WaitingForWorkers × InitialWaitTimeout → SoloReducing [K=0 && hybrid]` / `Error [K=0 && !hybrid]`.
- [ ] `WaitingForWorkers × WorkerConnected [count >= min] → Partitioning`: +`CancelTimer(InitialWait)`.
- [ ] `Dispatching × AllDispatched [hybrid] → WaitingForResults`: +`SpawnSelfPartition`.
- [ ] `WaitingForResults × SelfPartitionReduced`: `StoreResult(0, stats)` (self finished first).
- [ ] `WaitingForResults × SelfPartitionPanic → Error`.
- [ ] `SoloReducing × SoloReduceBatchComplete [redexes_remain] → SoloReducing` (self-loop).
- [ ] `SoloReducing × SoloReductionComplete → Done`.
- [ ] `SoloReducing × WorkerJoined(id) → CheckTermination`: +`QueueWorkerForNextWindow(id)` (deferred transition after in-flight batch per R5a).

### Phase B — Join rows (~60-90 LoC)
- [ ] `Partitioning × WorkerJoined(id) → Partitioning`: +`QueueWorkerForNextWindow(id)`, `LogJoin`.
- [ ] `Dispatching × WorkerJoined(id) → Dispatching`: same.
- [ ] `WaitingForResults × WorkerJoined(id) → WaitingForResults`: same (R10b).
- [ ] `Merging × WorkerJoined(id) → Merging`: same.
- [ ] `CheckTermination × NotNormalForm [elastic_join || elastic_departure] → AcceptingMembershipChanges`: +`StartTimer(JoinWindowMin), PollPendingConnections`.
- [ ] `CheckTermination × NotNormalForm [!elastic_join && !elastic_departure] → Partitioning`: +`InvokeSplitAndDispatch(K_eff)`.
- [ ] `AcceptingMembershipChanges × WorkerJoined(id) → AcceptingMembershipChanges`: +`RegisterWorker(id), LogJoin` (R10a drain).
- [ ] `AcceptingMembershipChanges × MembershipWindowClosed → Partitioning`: +`InvokeSplitAndDispatch(K_eff_new)`.

### Phase C — Departure rows (~60-90 LoC)
- [ ] `WaitingForResults × PhaseTimeout(id) [elastic_departure] → WaitingForResults`: +`ReclaimPartition(id, auto), RemoveWorker(id), LogDeparture(Timeout)` (R18).
- [ ] `WaitingForResults × PhaseTimeout(id) [!elastic_departure] → Error`.
- [ ] `WaitingForResults × WorkerConnectionLost(id) [elastic_departure] → WaitingForResults`: +`ReclaimPartition(id, auto), RemoveWorker(id), LogDeparture(ConnLost)` (R19).
- [ ] `WaitingForResults × WorkerConnectionLost(id) [!elastic_departure] → Error`.
- [ ] `WaitingForResults × WorkerLeft(id, AfterResult) → WaitingForResults`: +`StoreResult(id, prev_result), RemoveWorkerForNextRound(id), LogDeparture(LeaveAfter), Send LeaveAck(id)` (R22a).
- [ ] `WaitingForResults × WorkerLeft(id, Urgent) → WaitingForResults`: +`ReclaimPartition(id, auto), RemoveWorker(id), LogDeparture(LeaveUrgent), Send LeaveAck(id)` (R22b).
- [ ] `AcceptingMembershipChanges × WorkerLeft(id, _) → AcceptingMembershipChanges`: +`RemoveWorker(id), LogDeparture, Send LeaveAck(id)`.

### Overall
- [ ] FSM is TOTAL over the new event set in all new states (SC-012 closure).
- [ ] All new transitions emit a `LogTransition` action for observability (SPEC-11).
- [ ] Unit tests exercise every new row (one test per row minimum).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/net/fsm/coordinator.rs` (SPEC-13 R21 site) | modify | Extend transition function. |
| `relativist-core/src/net/fsm/tests.rs` | modify | Transition-row unit tests. |

## Key Sizing Guidance

If the single-task diff exceeds ~250 LoC, split into:
- `TASK-0436a` Phase A (hybrid/solo)
- `TASK-0436b` Phase B (join)
- `TASK-0436c` Phase C (departure)

with explicit dependency ordering `0436a → 0436b → 0436c`.

## Test Expectations (forward-ref)

- EG-U1, EG-U1a, EG-U1b (hybrid / solo / WorkerId 0).
- EG-U6, EG-U6a (join window).
- EG-U7, EG-U7a/b/c (departure reclaim paths).
- EG-U10, EG-U10a/b/c (graceful leave variants).
- EG-U11 (join + departure same round).
- EG-U16 (self-partition panic → Error).
- EG-U18 (initial_wait supersedes worker_connect).

## Invariants Touched

- D6 (Protocol Termination) — preserved by FSM totality.
- All other SPEC-20 invariants reached via the actions triggered by these transitions.

## Notes

- **Determinism**: transition function is pure; all side-effects via actions emitted to the runtime.
- **Totality**: every `(State, Event)` pair must have a defined transition, even if it's `self → self` with `NoAction`.

## DAG Links

- **Predecessors:** TASK-0414, TASK-0426, TASK-0420, TASK-0422.
- **Successors:** TASK-0423 (SpawnSelfPartition), TASK-0432 (JoinRequest handler), TASK-0438 (departure detect), TASK-0443 (reclaim).
