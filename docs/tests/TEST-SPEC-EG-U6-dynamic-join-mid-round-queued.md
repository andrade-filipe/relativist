# TEST-SPEC EG-U6: dynamic join mid-round queued (R10b, R16)

**SPEC-20 §7.1 ID:** EG-U6
**Owning task(s):** TASK-0432 (handshake), TASK-0434 (pending queue), TASK-0436 (FSM).
**Type:** unit.
**Test name:** `test_dynamic_join_mid_round_queued`.

---

## Inputs / Fixtures

- Hybrid mode, K_remote=2, mid-round (FSM in `WaitingForResults`).
- A scripted joiner that connects and sends `JoinRequest{ version: 4 }` while results are still being collected.

## Expected behaviour

R16: a `JoinRequest` arriving when FSM is NOT in `AcceptingMembershipChanges` is buffered in `pending_connections_queue`. R10b: it is NOT immediately dispatched. The current round completes; the FSM transitions through `CheckTermination → AcceptingMembershipChanges`; the queued joiner is then drained via `PollPendingConnections` and assigned a WorkerId.

## Assertions

| # | Assertion |
|---|-----------|
| A1 | At the moment the JoinRequest arrives, FSM state is `WaitingForResults` (not `AcceptingMembershipChanges`). |
| A2 | `pending_connections_queue.len() == 1` immediately after arrival; no `JoinAck` sent yet. |
| A3 | The current round's PartitionResults complete uninterrupted. |
| A4 | After CheckTermination → `AcceptingMembershipChanges`, `pending_connections_queue` is drained and the joiner receives a `JoinAck`. |
| A5 | Round 2 dispatches K_eff_new = K_eff_old + 1 partitions. |

## Edge / negative cases

- EC-1: 3 joiners arrive mid-round → all 3 buffered; all 3 drained at the next `AcceptingMembershipChanges`.
- EC-2: a joiner connects, sends JoinRequest, then disconnects before the next window opens — coordinator detects the dead stream during drain; logs WARN; skips the joiner.
- EC-3: the run terminates before the next window opens — pending queue is drained and each pending joiner receives a final `JoinNack` or close (developer choice; document).

## Invariants asserted

- D6 (Protocol Termination).

## ARG/DISC/REF citation

None.

## Determinism notes

`#[tokio::test(flavor = "current_thread", start_paused = true)]`. Joiner stream is `tokio::io::duplex`; arrival timing controlled via manual clock advance.

## Cross-test dependencies

EG-U6a covers the boundary race; EG-U11 covers join+departure same round.
