# TASK-0553: `install_connection` helper — on-the-fly border detection

**Spec:** SPEC-21 §4.6 install_connection pseudocode; SPEC-21 §3.3 R18 step 4; AC-007 cross-reference (HVM2 atomic-link-with-ownership pattern).
**Requirements:** §4.6 install_connection — classify wire as internal vs border based on `agent_owner`; on border, allocate a borderId and emit FreePort pair into both partitions.
**Priority:** P0 (called per resolved/pending-resolution connection in the pipeline).
**Status:** TODO
**Depends on:** TASK-0551 (PartitionAccumulator::connect), TASK-0560 (border_id_counter init for first-batch FreePort scan).
**Blocked by:** none
**Estimated complexity:** S (~70 LoC production + ~120 LoC tests).
**Bundle:** SPEC-21 Streaming Generation — Phase E (pipeline orchestrator).

## Context

Per SPEC-21 §4.6 install_connection pseudocode:

```
fn install_connection(source, target, agent_owner, accumulators,
                      border_map, border_id_counter):
    let src_worker = agent_owner[source.0]
    let tgt_worker = agent_owner[target.0]
    if src_worker == tgt_worker:
        // Internal wire
        accumulators[src_worker].connect(
            AgentPort(source.0, source.1),
            AgentPort(target.0, target.1)
        )
    else:
        // Border wire: generate FreePort pair
        let bid = *border_id_counter
        *border_id_counter += 1
        border_map.insert(bid, (AgentPort(source.0, source.1), AgentPort(target.0, target.1)))
        accumulators[src_worker].connect(AgentPort(source.0, source.1), FreePort(bid))
        accumulators[tgt_worker].connect(AgentPort(target.0, target.1), FreePort(bid))
```

**AC-007 pattern (per §4.6 intro paragraph):** Detect cross-partition pairs at the moment of connection, NOT in a separate pass. AC-007's atomic-link-with-ownership discipline maps to the streaming pipeline's per-chunk install loop — `agent_owner` is the streaming analog of HVM2's per-thread ownership mask.

**Border-id allocation interaction (per TASK-0560 / R29b):** `border_id_counter` is a `&mut u32` initialized to 0 (or to `max_lafont_freeport_id_in_first_batch + 1` if Lafont FreePorts exist) by the pipeline orchestrator (TASK-0554). This helper increments it on each border-wire emission.

**C3 (FreePort Bijectivity).** Each border wire produces ONE entry in `border_map` and TWO `FreePort(bid)` insertions (one per side) in their respective accumulators. The bijection contract is satisfied by construction.

## Acceptance Criteria

- [ ] Define `pub(crate) fn install_connection(source: (AgentId, PortId), target: (AgentId, PortId), agent_owner: &HashMap<AgentId, WorkerId>, accumulators: &mut [PartitionAccumulator], border_map: &mut HashMap<u32, (PortRef, PortRef)>, border_id_counter: &mut u32)` in `relativist-core/src/partition/streaming.rs`.
- [ ] Resolve `src_worker` / `tgt_worker` via `agent_owner` lookup; panic with a clear diagnostic if either is missing (this is a pipeline invariant — every agent in `assignments` is inserted into `agent_owner` BEFORE `install_connection` is called).
- [ ] Internal-wire path: `src_worker == tgt_worker` → call `accumulators[src_worker].connect(AgentPort(...), AgentPort(...))`.
- [ ] Border-wire path: `src_worker != tgt_worker` → allocate `bid = *border_id_counter`, increment counter, insert into `border_map`, call `connect` on BOTH accumulators with the symmetric FreePort.
- [ ] No `unwrap()` on `agent_owner.get()` — use `expect` with a diagnostic message that includes the missing AgentId.
- [ ] T1 (port linearity) maintained: every wire has exactly two endpoints, bidirectionally registered (delegated via PartitionAccumulator::connect TASK-0551).
- [ ] C3 maintained: each border id appears in exactly two distinct accumulators (verified by TASK-0570 finalized assertions).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/partition/streaming.rs` | modify | Add `install_connection` helper function. |

## Key Types / Signatures

```rust
pub(crate) fn install_connection(
    source: (AgentId, PortId),
    target: (AgentId, PortId),
    agent_owner: &HashMap<AgentId, WorkerId>,
    accumulators: &mut [PartitionAccumulator],
    border_map: &mut HashMap<u32, (PortRef, PortRef)>,
    border_id_counter: &mut u32,
);
```

## Test Expectations (forward-ref)

TEST-SPEC-0553:
- Internal wire: agents 0 and 1 both owned by worker 0; install_connection inserts wire into accumulators[0] only; border_map unchanged.
- Border wire: agent 0 owned by worker 0, agent 1 owned by worker 1; install_connection allocates bid 0, inserts into border_map, calls connect on BOTH accumulators with `FreePort(0)`.
- Sequential border allocation: 5 cross-partition wires in sequence → border ids 0..4.
- T5 partial (C3 bijectivity verified post-finalize via TASK-0570).
- AC-007 pattern verification: connection-time classification (no separate scan pass) — structural property verified at code review.

## Invariants Touched

- T1 (Port Linearity) — preserved via delegated connect.
- C2 (Complete Wire Coverage) — every connection becomes either an internal wire or a border-wire pair.
- C3 (FreePort Bijectivity) — preserved by exactly-2 FreePort insertions per border.

## Notes

- The function takes mutable slice / HashMap / u32 references — the borrow checker concerns are non-trivial. DEVELOPER may need to refactor signatures slightly (e.g., split accumulators access if Rust complains about double-mutable borrows) but the contract semantics are fixed.
- The `agent_owner` lookup MUST succeed — if it doesn't, it's a pipeline bug (assignments must precede installs). The diagnostic message MUST help debug which agent was missing.
- Consumed by TASK-0554 (main pipeline loop) and TASK-0555 (pending-store resolution path).

## DAG Links

- **Predecessors:** TASK-0551, TASK-0560.
- **Successors:** TASK-0554, TASK-0555.
