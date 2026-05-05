# TASK-0435: Join-window drain-then-arm protocol with `JoinWindowMin`/`JoinWindowMax` timers

**Spec:** SPEC-20 §3.2 R10 (AcceptingMembershipChanges entry), R10a (drain-then-arm), closes SC-007.
**Requirements:** R10 (window state transition), R10a (drain → min timer → max timer → Partitioning).
**Priority:** P0 (correct window timing = correctness).
**Status:** TODO
**Depends on:** TASK-0426 (TimerKind), TASK-0432 (JoinRequest handler), TASK-0434 (pending_connections_queue).
**Blocked by:** TASK-0434.
**Estimated complexity:** M (~120-170 LoC production + ~80 LoC tests)
**Bundle:** SPEC-20 Elastic Grid — Phase 2.2 Dynamic Joining.

## Context

On `AcceptingMembershipChanges` entry, the coordinator MUST first drain all pending TCP connections (from the queue populated by TASK-0434). Each drained connection completes the `Register`/`JoinRequest` handshake. Then arm `JoinWindowMin`; on `MembershipWindowClosed_min`, if no new pending arrived during drain, transition to `Partitioning`; else arm `JoinWindowMax - JoinWindowMin`, transition to `Partitioning` on drain-empty observation OR the max timer.

## Acceptance Criteria

- [ ] On `CheckTermination × NotNormalForm [elastic_join || elastic_departure] → AcceptingMembershipChanges` transition, the action performs:
  - `PollPendingConnections` (drain — complete handshakes for all currently-queued streams).
  - `StartTimer(TimerKind::JoinWindowMin, cfg.join_window_min)`.
- [ ] During `AcceptingMembershipChanges`, incoming `WorkerJoined` events add to `W_active` via action `RegisterWorker`; departures handled (TASK-0441).
- [ ] On `MembershipWindowClosed` timer:
  - If no new pending arrived during the drain pass → `→ Partitioning`, action `InvokeSplitAndDispatch(K_eff_new)`.
  - If new pending arrived → re-drain; arm `JoinWindowMax - JoinWindowMin`; on next drain-empty OR max-expiry → `→ Partitioning`.
- [ ] Respect R16 (workers connecting during an active round are buffered and registered ONLY at the next window).
- [ ] Determinism: test-injected `tokio::yield_now()` (EG-U6a) must produce deterministic round assignment.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/protocol/coordinator.rs` | modify | `AcceptingMembershipChanges` state handler + timer arming. |

## Test Expectations (forward-ref)

- EG-U6a `test_join_window_boundary_race` (R10a-b, SC-007).
- EG-U11 `test_join_and_departure_same_round` (§4.2.3, R26).

## Invariants Touched

- D6 — preserved via bounded window duration.

## Notes

- **Max guard**: bounded by `join_window_max` (default 500 ms) to cap adversarial drag-out.

## DAG Links

- **Predecessors:** TASK-0426, TASK-0432, TASK-0434.
- **Successors:** TASK-0433 (v1 repartition), TASK-0436 (FSM wiring), EG-U6a.
