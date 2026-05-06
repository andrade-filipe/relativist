# TASK-0439: Retained-state bookkeeping — `retained_initial` + `retained_last_acked` with atomic refresh

**Spec:** SPEC-20 §3.3.3 R23 (state retention enabled-by), R23a (release policy incl. reclaim consumption, NF-011), R23b-v1/R23b-delta (retained_initial contents), R23c-v1/R23c-delta (retained_last_acked contents), R23d (priority order), R31 (atomic release on successful round boundary, NF-011 memory bounds).
**Requirements:** R23, R23a, R23b-v1, R23b-delta, R23c-v1, R23c-delta, R23d, R31.
**Priority:** P0 (departure recovery cannot exist without retained state).
**Status:** TODO
**Depends on:** TASK-0415 (`retain_partitions`, `checkpoint_partitions` config).
**Blocked by:** TASK-0415.
**Estimated complexity:** M (~150-200 LoC production + ~120 LoC tests)
**Bundle:** SPEC-20 Elastic Grid — Phase 2.3 Dynamic Departure.

## Context

R23 mandates two retained-state slots per worker when `retain_partitions = true`:
- `retained_initial[w]`: round-0 dispatch state, allocated once, released on graceful completion / `Shutdown` / reclaim consumption (R23a clause c).
- `retained_last_acked[w]`: most recent committed state, refreshed atomically at successful round boundaries.

Per-mode contents (R23b/c):
- v1: both are `Partition` structures.
- delta: `retained_initial[w] = Partition` (from `InitialPartition`); `retained_last_acked[w] = (border_graph_snapshot, last_round_result)` OR (if `checkpoint_partitions = true`) a full `Partition`.

R31 atomic refresh: release `retained_last_acked[w]_round_n` only after `round_n+1` dispatch has been fully transmitted to all surviving members of `W_active` AND `w` has not departed between (a) and the dispatch start.

R31 memory bounds (NF-011 corrected):
- `retained_initial` memory bound: `O(sum_{w in W_currently_active ∪ W_pending_reclaim} |partition_w|)` — bounded by `2 · K_eff` at any instant.
- `retained_last_acked` memory bound: `O(sum_{w in W_currently_active} |partition_w|)`.

## Acceptance Criteria

- [ ] Coordinator holds `HashMap<WorkerId, RetainedInitial>` + `HashMap<WorkerId, RetainedLastAcked>` state.
- [ ] `RetainedInitial::V1(Partition)` / `RetainedInitial::Delta(Partition)` — store Partition in both modes (R23b).
- [ ] `RetainedLastAcked` enum:
  - V1(Partition) — `PartitionResult.partition`.
  - DeltaLight(BorderGraphSnapshot, RoundResult) — (border_graph, last_deltas).
  - DeltaCheckpoint(Partition) — full snapshot when `checkpoint_partitions = true`.
- [ ] `retained_initial[w]` allocated on `AssignPartition` (v1) / `InitialPartition` (delta) at worker w's first round.
- [ ] `retained_last_acked[w]` refreshed atomically at the "round N+1 dispatch transmitted" boundary (R31).
- [ ] Release policy R23a:
  - (a) graceful `LeaveRequest{AfterResult}` → release both slots.
  - (b) coordinator `Shutdown` → release both.
  - (c) **reclaim consumption** (NF-011 case c) → slot is freed after re-`split` + re-introduction; `w` exits `W_active`; WorkerId permanently retired (R11 no-reuse).
- [ ] Debug assertions enforcing the memory bounds: `retained_initial.len() <= 2 * K_eff` and `retained_last_acked.len() <= K_eff` at any steady state.
- [ ] `retained_last_acked` supports checkpoint mode toggling via `cfg.checkpoint_partitions`.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/protocol/coordinator.rs` | modify | Add retained-state fields to coordinator struct + atomic-refresh logic. |
| `relativist-core/src/protocol/retained.rs` *(new)* | create | `RetainedInitial` / `RetainedLastAcked` enums + helpers. |

## Key Types / Signatures

```rust
pub enum RetainedInitial {
    V1(Partition),
    Delta(Partition),  // InitialPartition.partition
}

pub enum RetainedLastAcked {
    V1(Partition),
    DeltaLight { snapshot: BorderGraphSnapshot, last: RoundResult },
    DeltaCheckpoint(Partition),
}
```

## Test Expectations (forward-ref)

- EG-U7b `test_departure_reclaim_last_acked_v1` (R23c-v1, R24b).
- EG-U7c `test_departure_reclaim_last_acked_delta` (R23c-delta, R24b).
- EG-U13 `test_retained_partition_atomic_release` (R31 MUST, SC-013).

## Invariants Touched

- D5 (Exclusive Ownership) — R31 atomic refresh prevents transient double-ownership.

## Notes

- **Memory bound correction (NF-011)**: the Round-2 wording `O(sum_{w in W_ever_active})` was unbounded under churn; the corrected bound is `2 · K_eff` instant cap.

## DAG Links

- **Predecessors:** TASK-0415.
- **Successors:** TASK-0443 (reclaim + re-split), TASK-0445 (memory tests EG-U13).
