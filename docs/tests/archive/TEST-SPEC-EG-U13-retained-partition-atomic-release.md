# TEST-SPEC EG-U13: retained partition atomic release (R31 MUST, SC-013)

**SPEC-20 §7.1 ID:** EG-U13
**Owning task(s):** TASK-0439 (retained-state), TASK-0452 (debug assertions).
**Type:** unit.
**Test name:** `test_retained_partition_atomic_release`.

---

## Inputs / Fixtures

- Hybrid; K_remote=1; v1 mode.
- Worker w1 completes round N successfully → coordinator updates `retained_last_acked[w1]` to the round-N partition.
- Round N+1 begins; coordinator dispatches to w1.
- Test instruments the coordinator to inspect `retained_last_acked[w1]` at multiple discrete points.

## Expected behaviour

R31 (MUST): `retained_last_acked[w1]_round_n` is held until the round N+1 dispatch is FULLY transmitted on the wire. Only after the entire `InitialPartition` (or delta payload) for round N+1 has been written to w1's stream does the coordinator atomically release the round-N snapshot.

If w1 disconnects DURING round-N+1 dispatch (after partial write), `retained_last_acked[w1]_round_n` MUST still be available for reclaim (the new payload was never fully delivered, so reclaiming the round-N state is correct).

## Assertions

| # | Assertion |
|---|-----------|
| A1 | At the start of round N+1 dispatch, `retained_last_acked[w1] == round_n_partition` (still held). |
| A2 | At the end of round N+1 dispatch (entire payload written), the coordinator atomically refreshes: `retained_last_acked[w1]` is now either (a) cleared (if no result yet for round N+1) OR (b) set to round-N+1 once that result arrives — depending on implementation. **Crucial assertion: there is NO point in time where `retained_last_acked[w1]` is None or stale while w1 has unacknowledged work in flight.** |
| A3 | If w1 disconnects mid-dispatch (instrumented), reclaim uses round-N partition (not round-N+1's, which never fully landed). |
| A4 | Memory bound: the coordinator never holds more than ONE retained snapshot per worker per slot type (`retained_initial` and `retained_last_acked`). |

## Edge / negative cases

- EC-1: w1 disconnects EXACTLY at the moment dispatch completes — race resolution: the coordinator's atomic release happens-before the disconnect detection; if so, the new round-N+1 dispatch is treated as "delivered", reclaim uses the (newer) round-N partition only if no further result was received. Document the exact happens-before.
- EC-2: 0-byte dispatch (no work to do; round terminates) — refresh is still atomic.
- EC-3: w1 receives the dispatch but the response never arrives → covered by EG-U7b.

## Invariants asserted

- D5 (state ownership; no transient double-ownership).
- R31 atomic refresh discipline.

## ARG/DISC/REF citation

None directly; underpins ARG-006 P11 (retained-snapshot consistency).

## Determinism notes

`#[tokio::test(flavor = "current_thread", start_paused = true)]`. Stream writes are observed step-by-step via instrumentation; checkpoints are taken between explicit `tokio::yield_now().await`s.

## Cross-test dependencies

- EG-U7b/c (reclaim from last-acked).
- TEST-SPEC-0411 / TEST-SPEC-0412 underpin the materialisation pieces.
