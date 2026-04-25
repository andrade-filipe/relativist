# TEST-SPEC EG-U10a: graceful leave urgent v1 (R22b, SC-008)

**SPEC-20 §7.1 ID:** EG-U10a
**Owning task(s):** TASK-0440, TASK-0441, TASK-0436.
**Type:** unit.
**Test name:** `test_graceful_leave_urgent_v1`.

---

## Inputs / Fixtures

- v1 mode + hybrid; K_remote=2.
- Worker `w1` sends `LeaveRequest { kind: Urgent }` MID-ROUND (i.e., before `PartitionResult` for the current round is sent).

## Expected behaviour

R22b: Urgent leave is treated like a timeout for the current round:
1. Coordinator sends `LeaveAck` immediately.
2. The current round's collect path sees w1 as departed; the coordinator triggers timeout-recovery (same code path as EG-U7).
3. R24a (or R24b): reclaim `retained_initial` (if no successful round) or `retained_last_acked` (if at least one).
4. Re-split for K_eff_new = K_eff - 1.
5. The current round restarts (not the next round; the dispatched payload is invalidated).

## Assertions

| # | Assertion |
|---|-----------|
| A1 | `LeaveAck` is sent to w1 before the stream is closed. |
| A2 | The coordinator does NOT wait for `PartitionResult` from w1 in the current round. |
| A3 | Reclaim path is taken (either initial or last-acked, depending on prior round count). |
| A4 | The current round's K_eff is updated to K_eff - 1; partitions are re-split for the **same round number** (round restart, not advance). |
| A5 | `canonicalise(final) == canonicalise(reduce_all(input))`. |

## Edge / negative cases

- EC-1: w1 sends Urgent but its `PartitionResult` arrived nanoseconds before — coordinator already accepted it; treats Urgent as "leave for next round", effectively R22a behaviour. Document the race resolution.
- EC-2: 2 workers urgent-leave simultaneously — coalesced into one re-split (same as EG-U8 multi-departure path).

## Invariants asserted

- D3-elastic (R24c).
- G1 PRESERVED via ARG-006.

## ARG/DISC/REF citation

ARG-006.

## Determinism notes

`#[tokio::test(flavor = "current_thread", start_paused = true)]`. Urgent-leave timing scripted via manual clock advance to ensure it arrives mid-round.

## Cross-test dependencies

- EG-U10b (delta-mode counterpart).
- EG-U10c (after-result-no-result-received upgrade).
