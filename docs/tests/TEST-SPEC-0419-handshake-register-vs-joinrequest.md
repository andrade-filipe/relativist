# TEST-SPEC-0419: Coordinator handshake `Register` vs `JoinRequest` selection (R37a, R0d, NF-009)

**SPEC-20 §7 ID:** none direct (functional cases EG-U14, EG-U15a, EG-U15b consume this).
**Owning task:** TASK-0419.
**Parent spec:** SPEC-20 §3.5 R37a, R0d, R35-cross-spec-version-shape.
**Type:** unit (handshake state machine; uses `tokio::test` + a controlled mock TCP stream).

---

## Inputs / Fixtures

- A mock duplex stream (`tokio::io::duplex(64KB)`) so both ends are observable.
- A coordinator handshake handler `accept_first_message(stream, fsm_state, config) -> HandshakeOutcome`.
- Pre-built sample `Message::Register(...)` and `Message::JoinRequest { protocol_version: 4, ... }` and `Message::JoinRequest { protocol_version: 3, ... }`.
- A controlled `FsmState` enum value: `WaitingForWorkers` vs `Running`/`AcceptingMembershipChanges`.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0419-01 | `register_during_waiting_accepted` | mock stream pre-loaded with `Register{ id, version: 4 }`; FSM = `WaitingForWorkers` | `accept_first_message(stream, ..)` | `HandshakeOutcome::RegisterAccepted { worker_id }`. |
| UT-0419-02 | `join_request_v4_during_running_accepted_when_elastic_join_true` | stream pre-loaded with `JoinRequest{ version: 4, .. }`; FSM = `Running`; `config.elastic_join = true` | same | `HandshakeOutcome::JoinAccepted { .. }`. |
| UT-0419-03 | `register_during_running_rejected_protocol_violation` | `Register` arriving when FSM = `Running` | same | `HandshakeOutcome::RejectedProtocolViolation { reason }`; the connection close is logged at WARN. |
| UT-0419-04 | `join_request_during_waiting_strict_rejected_with_elastic_join_disabled` | `JoinRequest` arriving when FSM = `WaitingForWorkers`; coordinator chose strict mode (per task notes) | same | `HandshakeOutcome::RejectedJoinNack { reason: JoinNackReason::ElasticJoinDisabled }` AND the coordinator wrote a `JoinNack` to the stream before close. |
| UT-0419-05 | `join_request_v3_rejected_with_protocol_version_mismatch` | `JoinRequest{ version: 3, .. }`; FSM = `Running`; `config.elastic_join = true` | same | Coordinator wrote `JoinNack { reason: JoinNackReason::ProtocolVersionMismatch { coordinator: 4, worker: 3 } }` to the stream before close. |
| UT-0419-06 | `register_v3_rejected_with_register_nack_protocol_version_mismatch` | `Register{ version: 3, .. }`; FSM = `WaitingForWorkers` | same | Coordinator wrote `RegisterNack { reason: RegisterNackReason::ProtocolVersionMismatch { coordinator: 4, worker: 3 } }` to the stream before close. |
| UT-0419-07 | `nf_009_register_nack_and_join_nack_payload_shape_aligned` | the bytes captured in UT-0419-05 stream-write and UT-0419-06 stream-write | extract just the `*Nack` payload and compare the bincode-encoded `ProtocolVersionMismatch` field tail | The two payloads' field-tail bytes are identical (mirrors UT-0418-08 at the protocol-handler layer). |
| UT-0419-08 | `elastic_join_disabled_in_config_rejects_join_request` | `JoinRequest{ version: 4 }`; FSM = `Running`; `config.elastic_join = false` | same | `JoinNack { ElasticJoinDisabled }` written; connection closed. |
| UT-0419-09 | `unexpected_first_message_other_variant_rejected` | mock stream pre-loaded with e.g. `Heartbeat` or `PartitionResult` (any non-Register/JoinRequest variant) | same | `HandshakeOutcome::RejectedProtocolViolation`; connection closed. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Connection closes mid-handshake (worker drops the TCP stream after a partial frame) | `HandshakeOutcome::RejectedConnectionLost` (or analogous); coordinator does not panic. |
| EC-2 | Worker sends `Register` on the v4 wire format with `version: 4` (compatible) | Accepted. |
| EC-3 | Worker sends `JoinRequest` with `version: u32::MAX` | Rejected with `ProtocolVersionMismatch { coordinator: 4, worker: u32::MAX }`; no overflow. |

## Invariants asserted

- D6 (Protocol Termination) — preserved; rejection paths always close the connection cleanly with a Nack written first.

## ARG/DISC/REF citation

None.

## Determinism notes

Async test (`#[tokio::test]`); but the mock duplex stream eliminates real TCP non-determinism. Each test runs single-threaded with `flavor = "current_thread"` to avoid scheduler-dependent ordering. No timers fire; handshake is one read + optional one write per test.

## Cross-test dependencies

- UT-0419-07 is the runtime-handler-layer NF-009 closure complement to TEST-SPEC-0418 UT-0418-08.
- EG-U15a (Register path version-mismatch) and EG-U15b (JoinRequest path version-mismatch) use UT-0419-05 and UT-0419-06 as their per-message-handler unit fixtures and add an end-to-end rejection assertion.
