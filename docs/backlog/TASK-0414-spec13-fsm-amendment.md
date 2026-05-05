# TASK-0414: [SPEC-13 amendment A2] Register new CoordinatorState / Event / Action enums

**Spec:** SPEC-20 §3.8 A2 (closes SC-011). Amends SPEC-13 R21 transition table.
**Requirements:** A2; directly supports §4.1.1-§4.1.4.
**Priority:** P0 (blocker for all SPEC-20 FSM work — TASK-0430, TASK-0431, TASK-0436).
**Status:** TODO
**Depends on:** none (additive to existing enums).
**Blocked by:** none
**Estimated complexity:** S (~60-90 LoC of new enum variants + accessors + Debug/PartialEq derives)
**Bundle:** SPEC-20 Elastic Grid — predecessor-spec amendment cluster.
**Tag:** `[SPEC-13 amendment]`

## Context

SPEC-13 R21's transition table is amended by SPEC-20 to introduce new FSM states (`AcceptingMembershipChanges`, `SoloReducing`), events (`WorkerJoined`, `WorkerLeft(id, kind)`, `WorkerConnectionLost`, `MembershipWindowClosed`, `SelfPartitionReduced`, `SelfPartitionPanic`, `InitialWaitTimeout`, `SoloReductionComplete`, `SoloReduceBatchComplete`), and actions (`RegisterWorker`, `RemoveWorker`, `ReclaimPartition`, `QueueWorkerForNextWindow`, `LogJoin`, `LogDeparture`, `InvokeSplitAndDispatch`, `SpawnSelfPartition`, `PollPendingConnections`).

This task ONLY lands the enum-variant surface; the transition logic itself is TASK-0436, and FSM consumers (protocol, merge, partition) are subsequent tasks.

## Acceptance Criteria

- [ ] Extend `CoordinatorState` enum in the SPEC-13 FSM module with `AcceptingMembershipChanges`, `SoloReducing` variants.
- [ ] Extend `CoordinatorEvent` enum with all 9 new events listed in §4.1.2.
- [ ] Extend `CoordinatorAction` enum with all 9 new actions listed in §4.1.2.
- [ ] Add `RetainedSlot { Initial, LastAcked }` helper enum consumed by `ReclaimPartition(WorkerId, RetainedSlot)`.
- [ ] Add `DepartureKind { Timeout, ConnLost, LeaveAfter, LeaveUrgent }` helper enum consumed by `LogDeparture`.
- [ ] All new variants derive `Debug, Clone, PartialEq, Eq` (per project conventions).
- [ ] Module compiles cleanly; existing FSM transition function is untouched (kept exhaustive via `#[non_exhaustive]` or placeholder `_ => unreachable!()` pending TASK-0436).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/net/fsm/coordinator.rs` *(or SPEC-13 R19-R21 site)* | modify | Extend the three enums with new variants; define `RetainedSlot`, `DepartureKind` helper enums. |

## Key Types / Signatures

```rust
pub enum CoordinatorState {
    // ... existing ...
    AcceptingMembershipChanges,
    SoloReducing,
}

pub enum CoordinatorEvent {
    // ... existing ...
    WorkerJoined(WorkerId),
    WorkerLeft(WorkerId, LeaveKind),
    WorkerConnectionLost(WorkerId),
    MembershipWindowClosed,
    SelfPartitionReduced(WorkerRoundStats),
    SelfPartitionPanic(String),
    InitialWaitTimeout,
    SoloReductionComplete,
    SoloReduceBatchComplete,
}

pub enum CoordinatorAction {
    // ... existing ...
    RegisterWorker(WorkerId),
    RemoveWorker(WorkerId),
    QueueWorkerForNextWindow(WorkerId),
    ReclaimPartition(WorkerId, RetainedSlot),
    LogJoin(WorkerId),
    LogDeparture(WorkerId, DepartureKind),
    InvokeSplitAndDispatch(u32 /* K_eff */),
    SpawnSelfPartition(Partition),
    PollPendingConnections,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetainedSlot { Initial, LastAcked }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepartureKind { Timeout, ConnLost, LeaveAfter, LeaveUrgent }
```

## Test Expectations (forward-ref)

TEST-SPEC-0414 formalizes Debug/PartialEq sanity; functional coverage is in TASK-0436 transitions.

## Invariants Touched

- None directly (type-surface change only).

## Notes

- `LeaveKind { AfterResult, Urgent }` is defined by R21 and landed in TASK-0427 (wire protocol). This task references it as a forward dep; if R21 ships first, import directly.
- Keep the new variants as the last variants of each enum so discriminant stability is preserved for any serde-derived downstream code.

## DAG Links

- **Predecessors:** none.
- **Successors:** TASK-0430 (hybrid FSM), TASK-0431 (solo FSM), TASK-0436 (FSM transition table wiring), TASK-0438 (departure detection).
