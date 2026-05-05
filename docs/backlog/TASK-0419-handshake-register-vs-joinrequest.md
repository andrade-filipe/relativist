# TASK-0419: Coordinator handshake branch — `Register` vs `JoinRequest` selection + cross-spec version shape

**Spec:** SPEC-20 §3.5 R37a (Register vs JoinRequest selection), R0d (version-mismatch full-rejoin), R35-cross-spec-version-shape (NF-009).
**Requirements:** R37a, R0d, R35-cross-spec-version-shape.
**Priority:** P0 (wire; blocker for any mid-session join).
**Status:** TODO
**Depends on:** TASK-0417, TASK-0418.
**Blocked by:** TASK-0417, TASK-0418.
**Estimated complexity:** S (~60-90 LoC)
**Bundle:** SPEC-20 Elastic Grid — wire protocol foundations.

## Context

A worker that connects to the coordinator's listener may be participating in the initial `WaitingForWorkers` window (uses `Register`) OR connecting mid-session (uses `JoinRequest`). The coordinator distinguishes by inspecting the first message on each new stream. Mixing the two on a single connection is a protocol violation per R37a. R0d mandates that version-mismatched `JoinRequest` payloads are rejected with `JoinNack { reason: ProtocolVersionMismatch { coordinator, worker } }` — the shape must match SPEC-19 R37's `RegisterNack` rejection (NF-009).

## Acceptance Criteria

- [ ] On every newly accepted TCP stream, the coordinator reads the first `Message`:
  - `Register(...)` → existing SPEC-06 path (v1 startup registration) — unchanged.
  - `JoinRequest { protocol_version, auth_token, worker_capabilities }` → elastic-join path; proceed to TASK-0432.
  - Any other variant → protocol violation; close connection with log.
- [ ] `Register` received mid-session (FSM state ≠ `WaitingForWorkers`) → rejected as protocol violation.
- [ ] `JoinRequest` received during `WaitingForWorkers` → valid per R37a (second sentence): worker MAY connect via `JoinRequest` at any state ≠ `WaitingForWorkers`. Clarify handling: accept either for flexibility, OR reject with `JoinNack { ElasticJoinDisabled }` during initial window. **Developer decision, documented in the code**: spec prose says `Register` is for initial, `JoinRequest` for mid-session; choose strict enforcement for clarity.
- [ ] Version-mismatch handling (R0d + R35-cross-spec-version-shape):
  - `JoinRequest.protocol_version != PROTOCOL_VERSION(4)` → respond with `JoinNack { reason: ProtocolVersionMismatch { coordinator: 4, worker: <received> } }`.
  - `Register` version-mismatch path → `RegisterNack` with a payload shape aligned 1:1 with the above (SPEC-19 R37 next revision owns the exact adoption).
- [ ] `elastic_join = false` + `JoinRequest` received → respond `JoinNack { ElasticJoinDisabled }`.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/protocol/coordinator.rs` | modify | Accept-loop branching on first message; rejection paths. |
| `relativist-core/src/protocol/errors.rs` *(optional)* | modify | Add `HandshakeViolation` variant if not present. |

## Test Expectations (forward-ref)

- EG-U14 (WorkerIdSpaceExhausted path).
- EG-U15a (Register path version-mismatch → `RegisterNack`).
- EG-U15b (JoinRequest path version-mismatch → `JoinNack`; assert payload shape identical to EG-U15a).
- TEST-SPEC-0419 additional cases: protocol violation when mixing `Register` and `JoinRequest` semantics.

## Invariants Touched

- D6 (Protocol Termination) — preserved via explicit rejection.

## Notes

- **Strict mode recommendation**: reject `JoinRequest` during initial window, reject `Register` mid-session. This closes the ambiguity without coupling to run history.

## DAG Links

- **Predecessors:** TASK-0417, TASK-0418.
- **Successors:** TASK-0432 (JoinRequest -> WorkerJoined flow), TASK-0441 (LeaveRequest).
