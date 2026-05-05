# TEST-SPEC EG-U6a: join-window boundary race (R10a-b, SC-007)

**SPEC-20 §7.1 ID:** EG-U6a
**Owning task(s):** TASK-0434 (pending queue), TASK-0435 (drain-then-arm), TASK-0436 (FSM).
**Type:** unit (deterministic concurrency injection).
**Test name:** `test_join_window_boundary_race`.

---

## Inputs / Fixtures

- Hybrid mode, K_remote=1.
- A joiner that arrives EXACTLY when the join-window timer is about to fire.
- An instrumented FSM driver that lets the test pause execution between specific events using `tokio::yield_now().await`.

## Expected behaviour

R10a-b drain-then-arm protocol: when `MembershipWindowClosed` and `WorkerJoined` race, the FSM:
1. Drains all pending `WorkerJoined` events FIRST (they joined within the window).
2. THEN arms the `MembershipWindowClosed` transition (closes the window).

Result: the late-arriving joiner is assigned to the *current* opening window, NOT punted to the next one.

## Assertions

| # | Assertion |
|---|-----------|
| A1 | Test injects events: timer fire ordered AFTER WorkerJoined emission. Verify via captured event timestamps that both events are within `<10us` of each other (the simulated race). |
| A2 | The FSM processes `WorkerJoined` first; the joiner gets an immediate `JoinAck` for the opening round. |
| A3 | `MembershipWindowClosed` then closes the window; subsequent joins go to the next window. |
| A4 | The reverse race (timer fires FIRST observably, joiner arrives <1us later) → the joiner is buffered in `pending_connections_queue` and drained at the NEXT `AcceptingMembershipChanges`. (A separate sub-test for this.) |

## Edge / negative cases

- EC-1: simultaneous arrival of 3 joiners with the timer on the verge of firing → all 3 are drained before the timer; assert deterministic ordering by `WorkerId` assignment monotonicity (R11).
- EC-2: timer fires twice (only legal once) — guard against double-fire.

## Invariants asserted

- D6 (Protocol Termination).

## ARG/DISC/REF citation

None.

## Determinism notes

**Race tests are non-deterministic by nature unless the runtime is controlled.** Strategy:
- `#[tokio::test(flavor = "current_thread", start_paused = true)]`.
- Use `tokio::time::advance(window_max - epsilon)` to bring the clock to right before timer fire.
- Inject `WorkerJoined` event via a controlled channel; explicitly `tokio::yield_now().await` to let the FSM consume it.
- Then `tokio::time::advance(epsilon * 2)` to fire the timer.
- Verify event ordering via captured trace.

If the FSM's drain-then-arm is implemented via inspection of the internal event queue (atomic read of "are there pending WorkerJoined events?" before arming the timer), both event-injection orderings are tested explicitly.

## Cross-test dependencies

- EG-U6 (mid-round queued joins).
- EG-U11 (join + departure same round).
