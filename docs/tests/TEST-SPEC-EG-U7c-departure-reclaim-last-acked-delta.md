# TEST-SPEC EG-U7c: departure reclaim last-acked delta (R23c-delta, R24b)

**SPEC-20 §7.1 ID:** EG-U7c
**Owning task(s):** TASK-0439, TASK-0443.
**Type:** unit.
**Test name:** `test_departure_reclaim_last_acked_delta`.

---

## Inputs / Fixtures

Same as EG-U7b but in delta mode:
- `delta_mode = true`; hybrid on; K_remote=2.
- `w1` completes 2 rounds; `retained_last_acked[w1] = (BorderGraph snapshot, last RoundResult deltas)`.
- `w1` departs in round 3.

## Expected behaviour

R23c-delta: the snapshot is a *pair* `(BorderGraph snapshot, last RoundResult deltas)`. The reclaim path R24b-delta:
- Conservative path (CLOSED via ARG-006): treats the reclaimed pair like a fresh InitialPartition derived from the snapshot — re-`split` from the reconstructed full net.
- Optimized path (CONDITIONAL on ARG-005): replays the deltas onto the surviving BorderGraph then participates as if continuing.

This test exercises the **conservative path** (default; closed via ARG-006).

## Assertions

| # | Assertion |
|---|-----------|
| A1 | The reclaim invokes `reconstruct(&bg_snapshot, surviving_partitions, vec![reclaimed_partition_from_snapshot])`. |
| A2 | The result is a complete `Net`; `split` is called on it for K_eff_new = K_eff - 1. |
| A3 | `metrics.retained_last_acked_reclaims_per_round` increments by 1. |
| A4 | `canonicalise(final) == canonicalise(reduce_all(input))`. |

## Edge / negative cases

- EC-1: optimized path enabled (ARG-005 supplied) → R24b-delta-optimized is exercised; assert the optimized path also yields the same final canonicalised result.
- EC-2: snapshot's BorderGraph differs from the live BorderGraph by N deltas (between snapshot and departure) — conservative path absorbs those into the fresh split; correctness preserved.
- EC-3: 0 successful rounds → falls through to EG-U7 (initial reclaim).

## Invariants asserted

- D3 (Border Completeness) via SPEC-19 R39.
- G1 CLOSED for conservative; CONDITIONAL on ARG-005 for optimized.

## ARG/DISC/REF citation

ARG-005 (delta border completeness — gates the optimized path) and ARG-006 (mixed-trace recoverability — gates the conservative path).

## Determinism notes

`#[tokio::test(flavor = "current_thread", start_paused = true)]`. Worker disconnect scripted.

## Cross-test dependencies

- TEST-SPEC-0412 (`reconstruct` 3-arg) is exercised here transitively.
- EG-I3-delta integration counterpart.
