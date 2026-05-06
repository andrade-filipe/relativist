# TEST-SPEC EG-U7: departure reclaim initial (R23a-b, R24a)

**SPEC-20 §7.1 ID:** EG-U7
**Owning task(s):** TASK-0438 (departure detection), TASK-0440 (v1 reclaim+resplit), TASK-0436, TASK-0451 (logging).
**Type:** unit.
**Test name:** `test_departure_reclaim_initial`.

---

## Inputs / Fixtures

- v1 mode + hybrid; K_remote=2 (K_eff=3).
- A worker `w1` is dispatched its `InitialPartition` but disconnects (TCP close or timeout) BEFORE returning any `PartitionResult`.
- The coordinator retains `retained_initial[w1]` per R23b.

## Expected behaviour

R24a: w1's reclaim source is `retained_initial` (no successful round). The coordinator:
1. Detects departure (timeout or `WorkerConnectionLost`).
2. Pulls `retained_initial[w1]`.
3. Re-numbers it via `remap_partition_ids` and rebases `border_id` via `allocate_border_ids`.
4. Re-splits the surviving + reclaimed material into `K_eff_new = K_eff - 1`.
5. Round restarts with the updated K_eff.

## Assertions

| # | Assertion |
|---|-----------|
| A1 | `metrics.workers_departed_per_round` records 1 in the affected round. |
| A2 | `metrics.retained_initial_reclaims_per_round` increments by 1. |
| A3 | `metrics.retained_last_acked_reclaims_per_round` does NOT increment (w1 had no successful round). |
| A4 | After re-split, K_eff_new == K_eff_old - 1 == 2. |
| A5 | `canonicalise(final) == canonicalise(reduce_all(input))`. |
| A6 | The departed worker's logged INFO/WARN line includes worker id + departure kind. |

## Edge / negative cases

- EC-1: w1 disconnects 0ms after `InitialPartition` is sent — timeout-detected departure.
- EC-2: w1 disconnects via TCP RST — `WorkerConnectionLost` event variant fires.
- EC-3: graceful `LeaveRequest{Urgent}` before any result — covered by EG-U10a.

## Invariants asserted

- D3-elastic (R24c): no in-round mixed merge; reclaim happens at clean boundary.
- D4-elastic (R11a, R30): id ranges remain disjoint after re-split.
- G1 PRESERVED via ARG-006 P10/P11/P12.

## ARG/DISC/REF citation

ARG-006 (mixed-trace recoverability) — empirical signature for v1.

## Determinism notes

`#[tokio::test(flavor = "current_thread", start_paused = true)]`. Worker stream is `tokio::io::duplex`; departure scripted by closing the duplex on demand. Timeout fires via manual clock advance.

## Cross-test dependencies

- EG-U7a/b/c (other departure-reclaim shapes).
- EG-I3 is the integration counterpart and the formal ARG-006 empirical anchor.
