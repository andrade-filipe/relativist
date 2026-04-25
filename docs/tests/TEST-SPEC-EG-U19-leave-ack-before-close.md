# TEST-SPEC EG-U19: LeaveAck before close (R35a, SC-017)

**SPEC-20 §7.1 ID:** EG-U19
**Owning task(s):** TASK-0418, TASK-0441.
**Type:** unit.
**Test name:** `test_leave_ack_before_close`.

---

## Inputs / Fixtures

- Hybrid; K_remote=1.
- Worker w1 sends `LeaveRequest { kind: AfterResult }` after a successful `PartitionResult`.
- Test instruments the byte stream (e.g., a `tokio::io::duplex` with a recording wrapper) to capture exact write order.

## Expected behaviour

R35a: the coordinator MUST send `LeaveAck` BEFORE closing the TCP write half. The worker MUST NOT close its connection until it has received `LeaveAck` (this is a worker-side discipline; here we test the coordinator side, but the test scripts the worker to wait for LeaveAck before closing).

## Assertions

| # | Assertion |
|---|-----------|
| A1 | The instrumented stream's write log shows: `[LeaveAck bytes] ... [optional Shutdown bytes] ... [stream close]`. |
| A2 | The bytes for `LeaveAck` arrive at the worker BEFORE the worker observes EOF (read returns 0). |
| A3 | If the test scripts the worker to wait for LeaveAck before closing, the worker observes LeaveAck successfully (does NOT receive an EOF before LeaveAck). |
| A4 | If the worker closes before LeaveAck delivery (test variant: simulate the BAD pattern), the coordinator logs WARN per R35a discipline guidance. |

## Edge / negative cases

- EC-1: `LeaveRequest { kind: Urgent }` — same R35a discipline; LeaveAck before close.
- EC-2: coordinator-initiated `Shutdown` (not LeaveAck) — different message; this test does NOT cover Shutdown delivery (that's SPEC-06's territory).
- EC-3: worker's stream-write of LeaveRequest fails (RST mid-frame) — coordinator does not get to send LeaveAck; departure is treated as `WorkerConnectionLost` (different code path).

## Invariants asserted

- D6 (Protocol Termination) via R35a discipline.

## ARG/DISC/REF citation

None.

## Determinism notes

`#[tokio::test(flavor = "current_thread")]`. Stream is `tokio::io::duplex` with custom wrapper recording bytes in order. No timer dependency.

## Cross-test dependencies

- EG-U10/U10a/b/c all exercise the leave path; they assert the LeaveAck is observed but EG-U19 is the dedicated ordering anchor.
