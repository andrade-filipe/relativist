# TEST-SPEC EG-U14: WorkerId exhaustion → JoinNack (R11, R35a, SC-023)

**SPEC-20 §7.1 ID:** EG-U14
**Owning task(s):** TASK-0418, TASK-0419, TASK-0420, TASK-0432.
**Type:** unit.
**Test name:** `test_worker_id_exhaustion_join_nack`.

---

## Inputs / Fixtures

- Hybrid coordinator with `next_worker_id` synthetically set to `u32::MAX` via test-only constructor / test-only setter (production code does not allow this — it's a fixture).
- A scripted `JoinRequest{ version: 4 }` arriving on a fresh stream.

## Expected behaviour

R11 caps `next_worker_id` at `u32::MAX`. When `next_worker_id == u32::MAX` and a new `JoinRequest` arrives, the coordinator responds with `JoinNack { reason: JoinNackReason::WorkerIdSpaceExhausted }` and closes the stream.

## Assertions

| # | Assertion |
|---|-----------|
| A1 | `JoinNack` is observed on the worker's stream with `reason == WorkerIdSpaceExhausted`. |
| A2 | The stream is closed by the coordinator after the Nack write. |
| A3 | The active worker set is unchanged (the would-be joiner is NOT registered). |
| A4 | A WARN log line is emitted naming the run as having reached WorkerId exhaustion. |
| A5 | Subsequent JoinRequests continue to receive the same Nack (state is sticky; no recovery). |

## Edge / negative cases

- EC-1: `next_worker_id == u32::MAX - 1`; one final assignment succeeds (WorkerId(u32::MAX - 1)); next request is Nacked.
- EC-2: a worker departs after exhaustion → R11 monotonic prevents reuse; the Nack persists.

## Invariants asserted

- D6 (Protocol Termination) — preserved via clean Nack.

## ARG/DISC/REF citation

None.

## Determinism notes

`#[tokio::test(flavor = "current_thread")]`. No timer dependency.

## Cross-test dependencies

None direct.
