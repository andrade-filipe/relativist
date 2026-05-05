# TASK-0432: `JoinRequest` handshake handler — authenticate, assign `WorkerId`, send `JoinAck`

**Spec:** SPEC-20 §3.2 R9 (accept mid-run), R11 (WorkerId allocation from monotonic counter starting at 1), R11a (partition_index decoupling), R17 (INFO logging), R35a (ack semantics), R0d (version mismatch rejection).
**Requirements:** R9, R11, R11a, R17, R35a, R0d.
**Priority:** P0 (dynamic joining cannot work without this).
**Status:** TODO
**Depends on:** TASK-0418 (JoinRequest/JoinAck variants), TASK-0419 (handshake branch), TASK-0420 (WorkerId counter).
**Blocked by:** TASK-0419.
**Estimated complexity:** M (~100-150 LoC production + ~80 LoC tests)
**Bundle:** SPEC-20 Elastic Grid — Phase 2.2 Dynamic Joining.

## Context

A mid-session connection received by the coordinator (TASK-0419's detection) must complete a `JoinRequest` / `JoinAck` handshake before the worker is included in `W_active`. The worker receives its assigned `WorkerId`, `partition_index` (at the next round boundary — NOT the current round), and `next_round_number`. Tokens and TLS (if enabled, SPEC-10) are validated at this step.

## Acceptance Criteria

- [ ] On `Message::JoinRequest` received, validate `protocol_version == PROTOCOL_VERSION(4)` else send `JoinNack { ProtocolVersionMismatch { coordinator, worker } }` (R0d + R35-cross-spec-version-shape).
- [ ] Validate `auth_token` per SPEC-10 (if configured); failure → `JoinNack { AuthenticationFailed }`.
- [ ] If `elastic_join = false` → `JoinNack { ElasticJoinDisabled }`.
- [ ] Allocate `worker_id = next_worker_id; next_worker_id += 1` (TASK-0420).
- [ ] If `next_worker_id` would overflow `u32::MAX` → `JoinNack { WorkerIdSpaceExhausted }` (R11 + SC-023).
- [ ] Compute the partition_index the worker WILL occupy in the next round (TASK-0420 helper, R11a).
- [ ] Compute `next_round_number` as current_round + 1 (or more if mid-round per R16).
- [ ] Send `JoinAck { assigned_worker_id, partition_index, next_round_number }`.
- [ ] Emit `CoordinatorEvent::WorkerJoined(worker_id)` to the FSM.
- [ ] INFO-level log per R17 with `K_eff_new`, `worker_id`, `partition_index`, `round_number`.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/protocol/coordinator.rs` | modify | JoinRequest processing function. |

## Test Expectations (forward-ref)

- EG-U5 `test_dynamic_join_repartition_v1` (R12-v1, R13).
- EG-U6 `test_dynamic_join_mid_round_queued` (R10b, R16).
- EG-U14 `test_worker_id_exhaustion_join_nack` (R11, SC-023).
- EG-U15b `test_protocol_version_mismatch_join_request_path` (R0d, R37, R37a, NF-010).

## Invariants Touched

- D4-elastic — preserved (partition_index dense; WorkerId sparse).

## Notes

- **JoinAck timing**: sent BEFORE the new worker is put into `W_active` for the next round (the state transition is atomic with the ack).
- **No mid-round dispatch**: R16 — workers joining mid-round are queued via R10b and registered only at the next join window.

## DAG Links

- **Predecessors:** TASK-0418, TASK-0419, TASK-0420.
- **Successors:** TASK-0434 (pending-connections drain), TASK-0435 (join window timers), TASK-0436 (FSM wiring), TASK-0433 (v1 rejoin repartition).
