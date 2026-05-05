# TEST-SPEC EG-U15b: protocol version mismatch — JoinRequest path (R0d, R37, R37a, R35-cross-spec-version-shape, NF-010)

**SPEC-20 §7.1 ID:** EG-U15b
**Owning task(s):** TASK-0417, TASK-0418, TASK-0419, TASK-0432.
**Type:** unit.
**Test name:** `test_protocol_version_mismatch_join_request_path`.

---

## Inputs / Fixtures

- Coordinator running v4 with `elastic_join = true`.
- FSM in `Running` (mid-session, after BSP loop has started).
- A fake worker connects and sends `JoinRequest { protocol_version: 3, .. }` (a v3 worker).

## Expected behaviour

NF-010 split: this exercises the **mid-session** rejection path that uses SPEC-20 R35's `JoinNack` payload shape:

1. Coordinator inspects first message; sees `JoinRequest{ version: 3 }`.
2. Returns `JoinNack { reason: JoinNackReason::ProtocolVersionMismatch { coordinator: 4, worker: 3 } }`.
3. Closes stream.

NF-009 (cross-spec version shape): the byte payload of `JoinNackReason::ProtocolVersionMismatch { coordinator: 4, worker: 3 }` MUST match the byte payload of `RegisterNackReason::ProtocolVersionMismatch { coordinator: 4, worker: 3 }` from EG-U15a.

## Assertions

| # | Assertion |
|---|-----------|
| A1 | The worker's read half observes `JoinNack { reason: ProtocolVersionMismatch { coordinator: 4, worker: 3 } }`. |
| A2 | The stream is closed after Nack. |
| A3 | No `WorkerId` is allocated. |
| A4 | The FSM does NOT transition to `AcceptingMembershipChanges` for this rejected request. |
| A5 | **NF-009 closure (mandatory):** the `ProtocolVersionMismatch` payload bytes from EG-U15a's RegisterNack and from this test's JoinNack are byte-identical when stripped of the outer enum-discriminant. Cross-test fixture: capture bytes from both tests; assert equality in this test or in a dedicated companion test. |

## Edge / negative cases

- EC-1: `worker = 5` (future version) — same `JoinNack`; payload shape unchanged.
- EC-2: `elastic_join = false` and v3 worker — coordinator may also Nack with `ElasticJoinDisabled` instead of `ProtocolVersionMismatch`. **Decision per TASK-0419**: version check happens FIRST; assert `ProtocolVersionMismatch` regardless of `elastic_join` setting (i.e., if the version is wrong we don't even get to the elastic_join check). Document the chosen ordering and assert it.
- EC-3: a v4 worker sends a malformed `JoinRequest` (truncated payload) → not this test's territory; covered by TASK-0419's protocol-violation path.

## Invariants asserted

- D6 (Protocol Termination).
- NF-009/NF-010 closure invariant (cross-spec payload shape alignment).

## ARG/DISC/REF citation

None.

## Determinism notes

`#[tokio::test(flavor = "current_thread")]`. No timers.

## Cross-test dependencies

- **EG-U15a** (RegisterNack path) — payload shape CROSS-COMPARISON is the joint NF-009/NF-010 closure.
- TEST-SPEC-0418 UT-0418-08 (wire-layer payload alignment).
- TEST-SPEC-0419 UT-0419-05/07 (handshake-handler layer).
