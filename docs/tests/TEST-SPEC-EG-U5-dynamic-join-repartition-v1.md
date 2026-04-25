# TEST-SPEC EG-U5: dynamic join re-partition v1 (R12-v1, R13)

**SPEC-20 §7.1 ID:** EG-U5
**Owning task(s):** TASK-0421 (id-range recompute), TASK-0432 (handshake handler), TASK-0433 (v1 re-partition on join).
**Type:** unit.
**Test name:** `test_dynamic_join_repartition_v1`.

---

## Inputs / Fixtures

- v1 mode (`delta_mode = false`); hybrid coordinator on.
- Initial cohort: K_remote = 2 (so K_eff = 3 with hybrid).
- Mid-run: 1 worker joins after round 1 completes.

## Expected behaviour

After the join, the next round's `K_eff = 4`. `compute_id_ranges(4)` yields 4 disjoint ranges. v1 re-partition flow: the coordinator merges the post-round-1 partitions back to a full `Net`, then re-`split`s into 4 partitions, dispatches.

## Assertions

| # | Assertion |
|---|-----------|
| A1 | After round 1: `metrics.effective_slots_per_round[0] == 3`. |
| A2 | After the join arrives in `AcceptingMembershipChanges`: `K_eff_new == 4`. |
| A3 | Round 2 starts with 4 partitions; ID ranges from `compute_id_ranges(4)` are disjoint (cross-check via TEST-SPEC-EG-U12). |
| A4 | `metrics.effective_slots_per_round[1] == 4`. |
| A5 | `metrics.workers_joined_per_round[1] == 1` (the join is recorded against the round 2 boundary). |
| A6 | `canonicalise(final_result) == canonicalise(reduce_all(input_net))`. |

## Edge / negative cases

- EC-1: 2 workers join simultaneously between rounds → K_eff increases by 2; both get distinct WorkerIds in monotonic order.
- EC-2: a join arrives in the same window as a departure → see EG-U11.
- EC-3: the join arrives before any reduction has occurred (still in `WaitingForWorkers`-like state) → handled via `Register` not `JoinRequest`; not this test's territory.

## Invariants asserted

- D4-elastic (R11a, R13): `partition_index` ↔ id range mapping holds across K_eff change.

## ARG/DISC/REF citation

None (v1 join correctness is a corollary of ARG-001 P2, not separately gated).

## Determinism notes

`#[tokio::test(flavor = "current_thread", start_paused = true)]`. Worker connect ordering scripted via duplex streams.

## Cross-test dependencies

- EG-U12 covers the disjointness of ranges.
- EG-I2 is the end-to-end integration version.
- EG-U5-delta is the delta-mode counterpart.
