# TASK-0420: `WorkerId` reservation + `partition_index` decoupling (D4-elastic)

**Spec:** SPEC-20 §3.1 R2 (K_eff = K + 1 slots), R2a (cross-mode `WorkerId 0` semantics, closes SC-016), R7 (self-partition stats), R7a (`WorkerId 0` permanent reservation), R11 (monotonic counter starting at 1, u32::MAX rejection), R11a (partition_index dense `[0, K_eff)` vs WorkerId sparse; D4-elastic, closes SC-006).
**Requirements:** R2, R2a, R7, R7a, R11, R11a.
**Priority:** P0 (blocker for hybrid FSM + joining; underpins all SPEC-20 ID-range logic).
**Status:** TODO
**Depends on:** TASK-0414 (new FSM enums for CoordinatorState/Event/Action), TASK-0418 (`JoinNack::WorkerIdSpaceExhausted`).
**Blocked by:** TASK-0414.
**Estimated complexity:** M (~100-150 LoC production + ~80 LoC tests)
**Bundle:** SPEC-20 Elastic Grid — Phase 2.1 Hybrid Coordinator (foundation).

## Context

SPEC-20 decouples the sparse `WorkerId` (monotonic, never-reused, sparse under churn) from the dense `partition_index` (position in the round's sorted active set, `[0, K_eff)`). SPEC-04's `compute_id_ranges(K_eff)` is indexed by `partition_index`, NOT `WorkerId` — this is the D4-elastic sub-invariant. `WorkerId = 0` is permanently reserved for the coordinator's self-partition role in hybrid mode (R7a); in non-hybrid mode `WorkerId = 0` may go to the first remote worker for v1 back-compat.

## Acceptance Criteria

- [ ] Add `next_worker_id: u32` to coordinator-side `W_active` bookkeeping; starts at `1` in hybrid mode, at `0` in non-hybrid mode (R7a + backwards-compat).
- [ ] On every worker `Register`/`JoinRequest` accept, allocate `worker_id = next_worker_id; next_worker_id += 1`.
- [ ] When `next_worker_id == u32::MAX` AND another `JoinRequest` arrives, reject with `JoinNack { reason: WorkerIdSpaceExhausted }` (R11 + SC-023).
- [ ] Do NOT reuse `WorkerId` values within the same run (R11). Retired ids stay retired.
- [ ] Compute `partition_index` for a round as: `W_active` sorted ascending by `WorkerId`, `[0, K_eff)` position. If hybrid, the self-partition is inserted at `partition_index = 0` corresponding to the reserved `WorkerId = 0`.
- [ ] Add helper `fn partition_index_of(worker_id: WorkerId, round: &RoundState) -> u32` that enumerates the sorted active set.
- [ ] Document D4-elastic sub-invariant in the partition module's Rustdoc (cross-ref §3.7).
- [ ] `is_coordinator_self: bool` field on `WorkerRoundStats` — R7 / R38b requirement; ensure log analyzers key on this field, NOT on `worker_id == 0`.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/protocol/coordinator.rs` | modify | Add `next_worker_id` counter; allocation on join; exhaustion rejection. |
| `relativist-core/src/net/worker_id.rs` *(or equivalent)* | modify | Rustdoc D4-elastic. |
| `relativist-core/src/partition/` | modify | Add `partition_index_of` helper. |
| `relativist-core/src/merge/round_stats.rs` *(WorkerRoundStats site)* | modify | Add `is_coordinator_self: bool` field per R38b. |

## Key Types / Signatures

```rust
pub fn partition_index_of(worker_id: WorkerId, active: &BTreeSet<WorkerId>) -> u32;

pub struct WorkerRoundStats {
    // ... existing ...
    pub is_coordinator_self: bool,  // R7 / R38b
}
```

## Test Expectations (forward-ref)

- EG-U1b `test_worker_id_zero_semantics_per_mode` (R2a, SC-016).
- EG-U12a `test_partition_index_vs_worker_id_decoupling` (R11a, SC-006) — ids {0, 1, 5, 7} with K_eff=4 yield dense ranges.
- EG-U14 `test_worker_id_exhaustion_join_nack` (R11, SC-023).

## Invariants Touched

- **D4-elastic** (new): `compute_id_ranges(K_eff)` indexed by `partition_index`, not `WorkerId`. PRESERVED via this task's helper.
- D4 (ID Uniqueness) — PRESERVED.

## Notes

- **`is_coordinator_self`**: log analyzers and benchmark tooling MUST key on this field rather than `worker_id == 0` alone (closes SC-027).
- **Non-hybrid backwards-compat**: R7a permits `WorkerId = 0` on the first remote worker when hybrid is off; this is non-conflicting because there is no self-partition role in that mode.

## DAG Links

- **Predecessors:** TASK-0414, TASK-0418.
- **Successors:** TASK-0421 (id-range computation), TASK-0430 (hybrid dispatch), TASK-0432 (JoinRequest flow).
