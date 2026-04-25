# TEST-SPEC-0414: `CoordinatorState`/`Event`/`Action` enum surface (SPEC-13 A2)

**SPEC-20 §7 ID:** none direct (functional coverage in TASK-0436 / EG-U* FSM tests).
**Owning task:** TASK-0414.
**Parent spec:** SPEC-13 R21 (amended via SPEC-20 §3.8 A2). Closes SC-011.
**Type:** unit (sanity / surface-only).

---

## Inputs / Fixtures

None. This is a surface test — instantiation, Debug/PartialEq, and discriminant-stability checks only.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0414-01 | `coordinator_state_new_variants_construct` | — | construct `AcceptingMembershipChanges` and `SoloReducing` | Both compile and round-trip through `Debug`. |
| UT-0414-02 | `coordinator_state_partial_eq_self` | — | `assert_eq!(AcceptingMembershipChanges, AcceptingMembershipChanges)` for each new variant | Equality holds. |
| UT-0414-03 | `coordinator_state_inequality_with_existing_variants` | — | `assert_ne!(AcceptingMembershipChanges, WaitingForWorkers)` and similar pairs | Inequality holds — no accidental aliasing. |
| UT-0414-04 | `coordinator_event_all_new_variants_construct` | — | construct each of the 9 new events: `WorkerJoined(WorkerId(7))`, `WorkerLeft(WorkerId(7), LeaveKind::AfterResult)`, `WorkerConnectionLost(WorkerId(7))`, `MembershipWindowClosed`, `SelfPartitionReduced(stats)`, `SelfPartitionPanic("boom".into())`, `InitialWaitTimeout`, `SoloReductionComplete`, `SoloReduceBatchComplete` | All compile. |
| UT-0414-05 | `coordinator_event_debug_format_contains_variant_name` | each event from UT-0414-04 | `format!("{:?}", event)` | Output starts with the variant name (`"WorkerJoined"`, etc.). |
| UT-0414-06 | `coordinator_action_all_new_variants_construct` | — | construct each of the 9 new actions: `RegisterWorker`, `RemoveWorker`, `QueueWorkerForNextWindow`, `ReclaimPartition(WorkerId(7), RetainedSlot::Initial)`, `LogJoin`, `LogDeparture(_, DepartureKind::Timeout)`, `InvokeSplitAndDispatch(K_eff)`, `SpawnSelfPartition(p)`, `PollPendingConnections` | All compile and round-trip Debug. |
| UT-0414-07 | `retained_slot_two_variants` | — | `RetainedSlot::Initial`, `RetainedSlot::LastAcked` | Both construct; PartialEq across cases inequality. |
| UT-0414-08 | `departure_kind_four_variants` | — | construct `DepartureKind::{Timeout, ConnLost, LeaveAfter, LeaveUrgent}` | All four compile and PartialEq across pairs is correct. |
| UT-0414-09 | `existing_variants_unchanged` | — | grep for any existing CoordinatorState variant; assert their discriminant order is unchanged | Test reads the source via `include_str!` or hand-mirrored test fixture; new variants appended only at the end. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | `SelfPartitionPanic` payload is empty string | Variant constructible; not a special-case panic. |
| EC-2 | `WorkerLeft` with `LeaveKind::Urgent` | Variant constructible; carries kind correctly. |

## Invariants asserted

None directly. This task is type-surface only; functional invariants are TASK-0436's territory.

## ARG/DISC/REF citation

None.

## Determinism notes

Pure synchronous.

## Cross-test dependencies

- `LeaveKind` from TASK-0418 (wire variants) — this task may need TASK-0418 land to import `LeaveKind`. If TASK-0414 ships first (it has no predecessors), use a temporary local `LeaveKind` enum and migrate when TASK-0418 lands.
- EG-U10/U10a/b/c, EG-U16, EG-U7/a/b/c, EG-U6 ALL depend on these enums existing — every functional EG-U FSM test imports a subset of these variants.
