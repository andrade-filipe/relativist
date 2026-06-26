# TEST-SPEC EG-U5-delta: dynamic join re-partition (delta) (R12-delta)

**SPEC-20 §7.1 ID:** EG-U5-delta
**Owning task(s):** TASK-0446.
**Type:** unit.
**Test name:** `test_dynamic_join_repartition_delta`.

---

## Inputs / Fixtures

- Delta mode (`delta_mode = true`); hybrid coordinator on.
- Initial K_remote = 2; K_eff = 3.
- 1 worker joins mid-run after round 1.

## Expected behaviour

R12-delta: on join, the coordinator broadcasts `FinalStateRequest` to all active workers, collects their final `Partition` payloads, runs `reconstruct(&bg, survivors, vec![])` to obtain a fresh full `Net`, re-`split`s into K_eff_new=4 partitions, sends `InitialPartition` to each (including the joiner). The joiner receives a fresh `InitialPartition`, NOT a delta.

## Assertions

| # | Assertion |
|---|-----------|
| A1 | After round 1, FSM transitions: `... → AcceptingMembershipChanges → FinalStateRequestSent → CollectingFinalStates → Reconstructing → Partitioning(K_eff=4) → ...`. (Match each FSM state via captured trace.) |
| A2 | All active workers receive `FinalStateRequest`; all reply with their final `Partition`. |
| A3 | `reconstruct(&bg, survivors, vec![])` is invoked; result is a complete `Net`. |
| A4 | `split(net, 4)` produces 4 partitions. |
| A5 | All 4 workers receive `InitialPartition` (not delta). |
| A6 | `metrics.join_round_overhead_ms_per_round[1] > 0` (R12a wire cost recorded). |
| A7 | `canonicalise(final) == canonicalise(reduce_all(input))`. |

## Edge / negative cases

- EC-1: a worker times out on `FinalStateRequest` → coordinator treats it as a departure; integration with EG-U7 / EG-U10a logic.
- EC-2: 2 joins simultaneously → still one FinalStateRequest cycle, K_eff_new = 5.
- EC-3: join during round 0 (before any reduction occurred) — degenerates to "everyone gets InitialPartition again"; assert no error.

## Invariants asserted

- D3 (Border Completeness) preserved via reconstruct (SPEC-19 R38).
- G1 conditional on ARG-005 for delta-optimized path; conservative path uses only ARG-001 + ARG-002.

## ARG/DISC/REF citation

ARG-005 (delta border completeness; conservative path independent of optimized).

## Determinism notes

`#[tokio::test(flavor = "current_thread", start_paused = true)]`. Worker scripts deterministic.

## Cross-test dependencies

- EG-I2-delta integration test.
- EG-B3 measures the `c_o_join` cost of the FinalStateRequest cycle.
