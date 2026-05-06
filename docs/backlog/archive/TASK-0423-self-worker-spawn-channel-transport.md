# TASK-0423: Spawn in-process self-worker via `ChannelTransport` (R3 bridge)

**Spec:** SPEC-20 §3.1 R1 (hybrid reduction), R3 (bridge via ChannelTransport, SPEC-17 R15), R3a (panic → Error), R4-v1 (self-partition treated identically to remote in merge).
**Requirements:** R1, R3, R3a, R4-v1 (self flows through merge path).
**Priority:** P0 (makes the self-partition actually run).
**Status:** TODO
**Depends on:** TASK-0422 (main loop), TASK-0420 (WorkerId 0 reservation), TASK-0421 (id-range for index 0).
**Blocked by:** TASK-0422.
**Estimated complexity:** M (~120-180 LoC production + ~80 LoC tests)
**Bundle:** SPEC-20 Elastic Grid — Phase 2.1 Hybrid Coordinator.

## Context

When hybrid mode is on, the coordinator spawns a local in-process worker that speaks the same wire protocol as remote workers via `ChannelTransport` (SPEC-17 R15). The self-worker's `PartitionResult` (v1) / `RoundResult` (delta) flows back through the channel, is consumed by `workers.next_message()` in the main select (TASK-0422), and enters the FSM identically to a remote worker's result.

This task lands the v1-mode path only; delta-mode (R4-delta-self-symmetry) is TASK-0437.

## Acceptance Criteria

- [ ] Implement `fn spawn_self_partition(partition: Partition, cfg: &GridConfig) -> SelfWorkerHandle` that:
  - Creates a `ChannelTransport` pair (SPEC-17 R15).
  - Spawns a `tokio::task::spawn_blocking` task that runs the standard v1 worker loop: `reduce_all(partition) -> PartitionResult`.
  - Returns a handle exposing: (i) the coordinator-side channel half for sending `AssignPartition` / receiving `PartitionResult`, (ii) a `panic_signal()` oneshot for arm (d) of the select.
- [ ] The self-worker's `WorkerId` is always `0` (R7a).
- [ ] On successful reduction, the self-worker sends `PartitionResult(partition')` through the channel.
- [ ] On panic in the spawn_blocking task, the oneshot channel receives `Err(panic_reason)`, which arm (d) of TASK-0422 translates to `CoordinatorEvent::SelfPartitionPanic(String)` per R3a.
- [ ] FSM transition on `WaitingForResults × SelfPartitionPanic → Error` (TASK-0436 wires this).
- [ ] v1-mode merge (R4-v1): self-partition's `PartitionResult` flows through the same merge path as remote — no special branch.
- [ ] `WorkerRoundStats` emitted with `is_coordinator_self: true` (TASK-0420).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/protocol/self_worker.rs` *(created in TASK-0422)* | modify | Add `spawn_self_partition` + `SelfWorkerHandle` struct. |
| `relativist-core/src/protocol/coordinator.rs` | modify | Call `spawn_self_partition` at Dispatching state when `hybrid_coordinator = true`. |

## Key Types / Signatures

```rust
pub struct SelfWorkerHandle {
    pub transport: ChannelTransport,
    pub join: tokio::task::JoinHandle<()>,
    pub panic_rx: tokio::sync::oneshot::Receiver<String>,
}

pub fn spawn_self_partition(
    partition: Partition,
    cfg: &GridConfig,
) -> SelfWorkerHandle;
```

## Test Expectations (forward-ref)

- EG-U1 `test_hybrid_coordinator_single_machine` (R1, R5).
- EG-U4 `test_hybrid_merge_includes_self` (R4-v1).
- EG-U16 `test_self_partition_panic_to_error` (R3a).

## Invariants Touched

- D2 (Local Reduction Equivalence) — preserved because self-worker uses the same `reduce_all`.
- D6 (Protocol Termination) — preserved via panic-to-Error path.

## Notes

- **Self-partition is NOT eligible for elastic departure recovery** (R3a): it is the coordinator's own process; panic is fatal.
- **Delta-mode symmetry**: the R4-delta-self-symmetry clause (TASK-0437) REQUIRES the self-worker to execute the full worker-side delta loop, not a short-circuit `reduce_all` on the full partition. That work is in a separate task.

## DAG Links

- **Predecessors:** TASK-0422, TASK-0420, TASK-0421.
- **Successors:** TASK-0424 (R3c strict_bsp uniformity), TASK-0430 (hybrid dispatch orchestration), TASK-0437 (delta self-worker loop).
