# TASK-0554: `generate_and_partition_chunked` main pipeline orchestrator

**Spec:** SPEC-21 §3.3 R17, R18 (steps 1-3); SPEC-21 §4.6 pipeline pseudocode; SPEC-21 §4.3 chunks_processed pipeline-owned note (closes SC-021).
**Requirements:** R17 (pipeline function signature), R18 (8-step processing sequence — this task covers steps 1-3, 7; step 4 = TASK-0553 install_connection; steps 5-6 = TASK-0555 pending store; step 7 stitch = this task; step 8 = TASK-0556).
**Priority:** P0 (the core entry point of SPEC-21).
**Status:** TODO
**Depends on:** TASK-0524 (StreamingPartitionStrategy trait), TASK-0521 (AgentBatch), TASK-0550-0553 (PartitionAccumulator + install_connection), TASK-0560 (border_id_counter init), TASK-0530 (RoundRobin default).
**Blocked by:** none
**Estimated complexity:** M (~150 LoC production + ~150 LoC tests).
**Bundle:** SPEC-21 Streaming Generation — Phase E (pipeline orchestrator).

## Context

Per SPEC-21 §3.3 R17:

```rust
fn generate_and_partition_chunked(
    stream: Box<dyn Iterator<Item = AgentBatch>>,
    num_workers: u32,
    strategy: &mut dyn StreamingPartitionStrategy,
) -> ChunkedPartitionResult
```

Per §4.6 pseudocode this task implements the main loop (steps 1-3, plus chunks_processed stitch step 7):

```
fn generate_and_partition_chunked(stream, num_workers, strategy):
    accumulators: Vec<PartitionAccumulator> = vec![new(); num_workers]
    border_id_counter: u32 = 0   // (or max_lafont_freeport_id + 1 if first-batch FreePorts — TASK-0560)
    border_map: HashMap<u32, (PortRef, PortRef)> = HashMap::new()
    pending: HashMap<AgentId, Vec<PendingConnection>> = HashMap::new()
    agent_owner: HashMap<AgentId, WorkerId> = HashMap::new()
    chunks_seen: u64 = 0  // PIPELINE-OWNED counter (closes SC-021)

    for batch in stream:
        chunks_seen += 1
        // Step 1
        assignments = strategy.allocate_batch(&batch, num_workers)
        for (agent_id, worker_id) in &assignments:
            agent_owner.insert(agent_id, worker_id)
            accumulators[worker_id].add_agent(agent_id, symbol_lookup(batch, agent_id))

        // Step 2: resolved connections via install_connection
        // Step 3: pending connections buffered (TASK-0555)
        // Step 6: previously pending → resolve (TASK-0555)

    // Step 4: assert pending.is_empty() (TASK-0555 + R19)

    // Step 5: finalize (TASK-0556)

    // Step 7: stitch chunks_processed (CLOSES SC-021)
    let mut stats = strategy.finalize();
    stats.chunks_processed = chunks_seen;

    return ChunkedPartitionResult { partitions, borders: border_map, stats }
```

**One-batch-in-flight (R22).** The pipeline MUST NOT buffer the full stream before partitioning. The `for batch in stream` loop processes one batch at a time and never collects.

## Acceptance Criteria

- [ ] Define `pub fn generate_and_partition_chunked(stream: Box<dyn Iterator<Item = AgentBatch>>, num_workers: u32, strategy: &mut dyn StreamingPartitionStrategy) -> Result<ChunkedPartitionResult, PartitionError>` in `relativist-core/src/partition/streaming.rs`.
- [ ] Initialize the 6 pipeline-local state structures: `accumulators` (Vec of PartitionAccumulator, one per worker), `border_id_counter`, `border_map`, `pending`, `agent_owner`, `chunks_seen`.
- [ ] First-batch FreePort scan (per TASK-0560): peek the first batch (or wrap stream in a peekable), scan for max Lafont FreePort id, initialize `border_id_counter` accordingly. If no Lafont FreePorts, init to 0.
- [ ] Main loop (per `for batch in stream`):
  1. Increment `chunks_seen`.
  2. Call `strategy.allocate_batch(&batch, num_workers)` (Step 1).
  3. For each (agent_id, worker_id), insert into `agent_owner` and call `accumulators[worker_id].add_agent`.
  4. For each `Resolved` directive in `batch.connections`, call `install_connection(...)` (Step 2 — TASK-0553).
  5. For each `Pending` directive, buffer into `pending` (Step 3 — TASK-0555 helper).
  6. After agent insertion, scan `pending` for entries whose target agent is now in `agent_owner`; resolve via `install_connection` (Step 6 — TASK-0555 helper).
- [ ] After the loop:
  - Assert `pending.is_empty()` (R19); on non-empty, return `Err(PartitionError::UnresolvedForwardReferences)` (TASK-0555 owns the error variant definition).
  - Compute id_ranges per SPEC-04 R16-R18 / SPEC-21 R29 (TASK-0556).
  - Finalize accumulators (TASK-0552 per worker → TASK-0556 assembly).
  - Stitch `stats.chunks_processed = chunks_seen` per §4.6 Step 7 (closes SC-021).
  - Return `Ok(ChunkedPartitionResult { partitions, borders: border_map, stats })`.
- [ ] R22 structural verification: code review confirms NO `stream.collect()` or equivalent buffer-the-stream operation.
- [ ] T5 end-to-end: `generate_and_partition_chunked(ep_annihilation_stream(100, 20), 4, &mut RoundRobin::new(4))` produces 200 agents across 4 partitions, 100 wires, no pending refs, isomorphic to `split(make_net(ep_annihilation(100)), 4, ContiguousIdStrategy)`.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/partition/streaming.rs` | modify | Add `generate_and_partition_chunked` function. |

## Key Types / Signatures

```rust
pub fn generate_and_partition_chunked(
    stream: Box<dyn Iterator<Item = AgentBatch>>,
    num_workers: u32,
    strategy: &mut dyn StreamingPartitionStrategy,
) -> Result<ChunkedPartitionResult, PartitionError>;
```

## Test Expectations (forward-ref)

TEST-SPEC-0554:
- T5 (streaming pipeline produces valid partitions): full integration on `ep_annihilation(100)`, chunk_size=20, num_workers=4. C1/C2/C3 verified.
- T8 (chunk size independence): vary chunk_size from 1 to size; merged result MUST be isomorphic to sequential baseline (covered jointly with TASK-0567).
- T6 (streaming vs batch equivalence): `generate_and_partition_chunked` output ↔ `split()` output isomorphic post-merge (joint with TASK-0567).
- chunks_processed stitch verified: assert `result.stats.chunks_processed == ceil(total_agents / chunk_size)` (SC-021 closure validation).
- R22 one-batch-in-flight: instrument peak memory; verify peak is bounded by O(chunk_size + accumulators + borders + pending) — covered by TASK-0584 T10.

## Invariants Touched

- D1 (Split/Merge Identity, extended) — preserved by R7 + R18 + R19.
- C1 / C2 / C3 — established per SPEC-04 §4.8 assertions invoked at finalize (TASK-0570).
- I3' — preserved via accumulator constraints.
- R22 (one-batch-in-flight) — enforced structurally.

## Notes

- The orchestrator delegates step 4 to TASK-0553 (install_connection), steps 5/6 to TASK-0555 (pending store), step 8 to TASK-0556 (final assembly + id_range computation). This task is the integrator.
- DEVELOPER MUST coordinate with TASK-0560 on the first-batch peek-scan-for-Lafont-FreePort discipline; the most ergonomic approach is to wrap the stream in `std::iter::Peekable` or do a one-shot pre-scan and use `std::iter::once(first_batch).chain(rest)`.
- The `symbol_lookup(batch, agent_id)` call in the pseudocode is a clarity stand-in; in production, `batch.agents` is `Vec<(AgentId, Symbol)>`, so the symbol is co-located with the id and the lookup is direct: `for (agent_id, symbol) in &batch.agents { ... }`.
- Consumed by TASK-0556 (final assembly), TASK-0567 (R26 short-circuit), TASK-0570 (C1-C3 assertions), TASK-0577/0579 (push/pull dispatch orchestration), TASK-0600 (regression).

## DAG Links

- **Predecessors:** TASK-0524, TASK-0521, TASK-0550, TASK-0551, TASK-0552, TASK-0553, TASK-0560.
- **Successors:** TASK-0555, TASK-0556, TASK-0557, TASK-0567, TASK-0570, TASK-0577.
