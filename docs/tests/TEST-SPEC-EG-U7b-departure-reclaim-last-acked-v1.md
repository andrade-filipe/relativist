# TEST-SPEC EG-U7b: departure reclaim last-acked v1 (R23c-v1, R24b)

**SPEC-20 §7.1 ID:** EG-U7b
**Owning task(s):** TASK-0439 (retained-state), TASK-0440 (v1 reclaim+resplit).
**Type:** unit.
**Test name:** `test_departure_reclaim_last_acked_v1`.

---

## Inputs / Fixtures

- v1 mode + hybrid; K_remote=2.
- A worker `w1` successfully completes 2 rounds (returns `PartitionResult` for round 1 and round 2). After round 2, the coordinator updates `retained_last_acked[w1] = round_2_partition`.
- During round 3, `w1` departs.

## Expected behaviour

R24b: the reclaim source is `retained_last_acked` (the round-2 partition), NOT `retained_initial`. R23c-v1: the snapshot is the *Partition* state at the moment the round-2 ack was sent.

## Assertions

| # | Assertion |
|---|-----------|
| A1 | `metrics.retained_last_acked_reclaims_per_round` increments by 1. |
| A2 | `metrics.retained_initial_reclaims_per_round` does NOT increment. |
| A3 | The reclaimed partition has the SAME structure as `retained_last_acked[w1]` (i.e., the round-2 state, not the round-0 initial). |
| A4 | After re-split, `canonicalise(final) == canonicalise(reduce_all(input))`. |
| A5 | The reclaimed partition's `AgentId` set is renumbered via `remap_partition_ids` (cross-check via TEST-SPEC-0411 properties). |

## Edge / negative cases

- EC-1: w1 successfully completes round N, then disconnects between rounds (no in-flight result for round N+1) — same path; reclaim from `retained_last_acked[w1] = round_N_partition`.
- EC-2: w1 has an in-flight `PartitionResult` for round N+1 that arrives AFTER the coordinator decides to reclaim — late result is dropped (R31 atomic refresh; cross-test EG-U13).
- EC-3: w1 successfully completes 0 rounds (only `InitialPartition` was sent, no `PartitionResult` returned) → falls through to EG-U7 (initial reclaim) instead.

## Invariants asserted

- D5 (state ownership) preserved via R31 atomic refresh.
- G1 PRESERVED via ARG-006 P10/P11/P12.

## ARG/DISC/REF citation

ARG-006.

## Determinism notes

`#[tokio::test(flavor = "current_thread", start_paused = true)]`. Worker disconnect scripted via duplex stream close.

## Cross-test dependencies

- EG-U7c is the delta-mode counterpart.
- EG-U13 (atomic refresh).
