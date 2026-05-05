# TEST-SPEC EG-U15a: protocol version mismatch — Register path (R0d, R37, NF-010)

**SPEC-20 §7.1 ID:** EG-U15a
**Owning task(s):** TASK-0417, TASK-0418, TASK-0419.
**Type:** unit.
**Test name:** `test_protocol_version_mismatch_register_path`.

---

## Inputs / Fixtures

- Coordinator running v4 (`PROTOCOL_VERSION = 4`).
- FSM in `WaitingForWorkers` state (initial cohort window).
- A fake worker connects and sends `Register { protocol_version: 3, .. }` (a v3 worker).

## Expected behaviour

NF-010 split: this exercises the **initial-window** rejection path that uses SPEC-19 R37's `RegisterNack` payload shape:

1. Coordinator inspects first message; sees `Register{ version: 3 }`.
2. Returns `RegisterNack { reason: RegisterNackReason::ProtocolVersionMismatch { coordinator: 4, worker: 3 } }`.
3. Closes stream.

## Assertions

| # | Assertion |
|---|-----------|
| A1 | The worker's read half observes `RegisterNack { reason: ProtocolVersionMismatch { coordinator: 4, worker: 3 } }`. |
| A2 | The stream is closed by the coordinator after Nack write. |
| A3 | No `WorkerId` is allocated (active set unchanged). |
| A4 | The coordinator's FSM remains in `WaitingForWorkers` (no transition fired by this rejected handshake). |
| A5 | The bytes of the Nack payload are captured for cross-comparison with EG-U15b's `JoinNack` payload (NF-009 alignment). |

## Edge / negative cases

- EC-1: `worker = u32::MAX` value — no overflow; payload shape is identical.
- EC-2: `worker = 0` (degenerate) — same Nack; no special-case.
- EC-3: `worker = 5` (some future version) — same Nack with `worker: 5`.

## Invariants asserted

- D6 (Protocol Termination).

## ARG/DISC/REF citation

None.

## Determinism notes

`#[tokio::test(flavor = "current_thread")]`. Single read + single write; no timers.

## Cross-test dependencies

- EG-U15b is the JoinRequest counterpart; together they CLOSE NF-009 + NF-010 by asserting the payload shape is identical across the two paths.
- TEST-SPEC-0418 UT-0418-08 covers the same alignment at the wire-types layer.
- TEST-SPEC-0419 UT-0419-06/07 covers the same at the handshake-handler layer.
