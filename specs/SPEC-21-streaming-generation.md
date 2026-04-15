# SPEC-21: Streaming Generation and Partitioning

**Status:** Draft
**Depends on:** SPEC-01 (Invariants), SPEC-02 (Net Representation), SPEC-04 (Partitioning), SPEC-05 (Merge and Grid Cycle), SPEC-13 (System Architecture)
**ROADMAP items:** 2.27 (Streaming Net Generation), 2.28 (Online/Streaming Graph Partitioning), 2.30 (Chunked Generation with Incremental Partitioning)
**References consumed:** REF-001 (Lafont 1990, p.96: locality), REF-002 (Lafont 1997, p.70-73: net structure, strong confluence), REF-005 (Mackie & Pinto 2002), REF-015 (Mackie & Sato 2015: batch vs streaming)
**Arguments consumed:** ARG-001 (central argument, P1-P6), ARG-002 (partitioning preserves structure, C1-C3)
**Briefings consumed:** BRIEF-20260415-v2-codebase-assessment (Section 4.3: partition module), BRIEF-20260415-v2-fundamentacao-teorica (Tier 3 Memory, locality principle, streaming partitioning)

---

## 1. Purpose

This spec defines how Relativist generates and partitions interaction combinator nets in a streaming (incremental) fashion, so that the coordinator never needs to hold the entire net in memory simultaneously. In v1, the pipeline is `generate full net -> partition globally -> dispatch partitions`: the coordinator holds the full net at peak, which is O(total_agents) memory. SPEC-21 replaces this with a chunked pipeline: `generate chunk -> partition chunk -> dispatch chunk -> repeat`, bounding coordinator peak memory to O(chunk_size + border_tracking_state) instead of O(total_agents).

Three tightly coupled features are specified together because they form a single pipeline:

1. **Streaming Partition Strategy (2.28):** A trait for partitioning strategies that assign agents to workers on-the-fly, one batch at a time, without requiring a global view of the net. Includes round-robin (MVP) and FENNEL/LDG-style heuristic (advanced) strategies.
2. **Streaming Net Generation (2.27):** A producer-consumer pattern where generators emit agents in bounded batches instead of constructing the full net upfront.
3. **Chunked Generation Pipeline (2.30):** The integration of 2.27 and 2.28 into a complete pipeline where chunks are generated, partitioned, and dispatched incrementally.

**Why they belong in one spec:** The three features share types (`AgentBatch`, `ForwardReference`), invariant extensions (D1 over batch unions, incremental C1-C3), and a single pipeline architecture. Specifying them separately would create circular cross-references.

---

## 2. Definitions

Terms defined in SPEC-00 (Glossary), SPEC-01, SPEC-02, SPEC-04, and SPEC-13 are used without redefinition. Terms introduced or refined in this spec:

| Term | Definition |
|------|-----------|
| **AgentBatch** | A bounded collection of agents with their connections, representing a single chunk of a net being generated incrementally. Contains agent definitions (id, symbol) and connection directives. Each batch has a bounded size controlled by the `chunk_size` configuration parameter. |
| **Chunk** | A single unit of work in the streaming pipeline: one `AgentBatch` produced by the generator, partitioned by the streaming strategy, and dispatched to workers. |
| **Chunk Size (C)** | The maximum number of agents in a single `AgentBatch`. Configurable. Controls the memory/quality trade-off: smaller C means less memory but potentially worse partition quality; larger C means more memory but better locality information for partitioning. |
| **Forward Reference** | A connection directive in an `AgentBatch` where the target agent has not yet been generated (it will appear in a future batch). Represented as a `PendingConnection` entry. Forward references are resolved when the target agent's batch is processed. |
| **Pending Connection** | A record `(source_agent_id, source_port, target_agent_id, target_port)` describing a wire whose target agent does not yet exist in any partition accumulator. Pending connections are buffered and resolved when the target agent is generated in a subsequent chunk. |
| **Partition Accumulator** | A per-worker structure that accumulates agents and connections as chunks are processed. After all chunks, each accumulator becomes a `Partition` (SPEC-04). |
| **Streaming Partition Strategy** | A partitioning strategy that assigns agents to workers incrementally, one batch at a time, using only information available up to the current batch (no global view). Contrast with `PartitionStrategy` (SPEC-04, R21) which requires the full net. |
| **Assignment Cache** | A mapping `AgentId -> WorkerId` maintained by stateful streaming partition strategies (e.g., FENNEL) to look up which worker owns a previously assigned agent. Required for neighbor-aware assignment. Grows to O(total_agents) but stores only 8 bytes per agent (vs. ~64 bytes for the full agent in the net). |
| **Border Accumulator** | An incrementally built border map that tracks cross-partition wires discovered as chunks are processed. Analogous to the `borders: HashMap<u32, (PortRef, PortRef)>` in `PartitionPlan` (SPEC-04, Section 4.1), but built incrementally. |

---

## 3. Requirements

### 3.1 Streaming Partition Strategy (ROADMAP 2.28)

**R1.** Relativist MUST define a trait `StreamingPartitionStrategy` that abstracts the per-batch allocation of agents to workers. This trait is the streaming counterpart of `PartitionStrategy` (SPEC-04, R21). **(MUST)**

**R2.** The `StreamingPartitionStrategy` trait MUST support stateful operation (`&mut self`) because strategies may track per-worker load, assignment history, or neighbor counts across batches. **(MUST)**

**R3.** The `StreamingPartitionStrategy` trait MUST provide:
- `allocate_batch(&mut self, batch: &AgentBatch, num_workers: u32) -> Vec<(AgentId, WorkerId)>`: assigns each agent in the batch to a worker.
- `finalize(&self) -> StreamingPartitionStats`: returns statistics about the partitioning (total agents assigned, per-worker counts, estimated border wire count).
**(MUST)**

**R4.** A `RoundRobinStreamingStrategy` MUST be provided as the default (MVP) implementation. It assigns agent `i` (in batch order) to worker `i % num_workers`. This strategy requires zero state beyond a counter, produces O(1) per-agent assignment, and matches the partition quality of `ContiguousIdStrategy` (SPEC-04, R22) for sequential generators. **(MUST)**

**R5.** A `FennelStreamingStrategy` SHOULD be provided as an advanced implementation. It maintains per-worker degree counters and an assignment cache (`HashMap<AgentId, WorkerId>`), and assigns each agent to the worker where it has the most already-assigned neighbors, with a capacity penalty: `argmax_w(neighbors_w - alpha * degree_w)`. The parameter `alpha` SHOULD be configurable. **(SHOULD)**

**R6.** The `FennelStreamingStrategy` assignment cache MUST grow to at most O(total_agents) entries, storing only the `(AgentId, WorkerId)` mapping (~8 bytes per agent). This is an 8x memory reduction compared to holding the full net (~64 bytes per agent). **(MUST)**

**R7.** Every `StreamingPartitionStrategy` implementation MUST guarantee that after all batches have been processed, the union of all assignments satisfies C1 (SPEC-04, R6): every agent that appeared in any batch is assigned to exactly one worker, with no duplicates and no omissions. **(MUST)**

**R8.** The allocation function produced by a streaming strategy MUST be deterministic: given the same sequence of batches and the same `num_workers`, the assignment MUST be identical across invocations. **(MUST)**

**R9.** `StreamingPartitionStrategy` implementations MUST be pure Core-layer code: no async, no tokio, no I/O. They reside in `src/partition/`. **(MUST)**

### 3.2 Streaming Net Generation (ROADMAP 2.27)

**R10.** The `Benchmark` trait (in `src/bench/mod.rs`) MUST gain a new method:
```rust
fn make_net_stream(&self, size: u32, chunk_size: usize) -> Box<dyn Iterator<Item = AgentBatch>>
```
that emits the net as a sequence of `AgentBatch` values, each containing at most `chunk_size` agents. **(MUST)**

**R11.** The existing `make_net(&self, size: u32) -> Net` method MUST remain as a convenience wrapper that collects the stream into a full net. Backward compatibility with callers that need the full net (sequential baseline, verification) MUST be preserved. **(MUST)**

**R12.** Each generator in `src/io/generators.rs` MUST gain a streaming variant that returns a `Box<dyn Iterator<Item = AgentBatch>>`. At minimum, the `ep_annihilation` generator MUST support streaming for the MVP. **(MUST for ep_annihilation; SHOULD for other generators)**

**R13.** For `ep_annihilation`, streaming is trivial: each batch emits independent ERA-ERA pairs. No cross-batch wires exist, so no forward references are needed. **(informative)**

**R14.** For generators with cross-batch dependencies (e.g., `dual_tree`), the `AgentBatch` MUST support forward references via `PendingConnection` entries. The batch carries both resolved connections (both endpoints in the current or previous batches) and pending connections (target agent will appear in a future batch). **(MUST)**

**R15.** The generator MUST assign `AgentId` values to agents in a globally unique, monotonically increasing sequence across all batches. This preserves SPEC-01, I3 (monotonicity) at the stream level: the maximum `AgentId` in batch `k` MUST be less than the minimum `AgentId` in batch `k+1`. **(MUST)**

**R16.** Generator streaming variants MUST be pure Core-layer code: no async, no tokio, no I/O. The iterator is synchronous. Integration with async channels (e.g., `tokio::sync::mpsc`) for backpressure is an Infrastructure-layer concern handled by the coordinator, not by the generator. **(MUST)**

### 3.3 Chunked Generation Pipeline (ROADMAP 2.30)

**R17.** Relativist MUST provide a function `generate_and_partition_chunked()` (in `src/partition/streaming.rs` or `src/merge/grid.rs`) that integrates streaming generation and streaming partitioning into a single pipeline:
```rust
fn generate_and_partition_chunked(
    stream: Box<dyn Iterator<Item = AgentBatch>>,
    num_workers: u32,
    strategy: &mut dyn StreamingPartitionStrategy,
) -> ChunkedPartitionResult
```
**(MUST)**

**R18.** The pipeline MUST process one chunk at a time in the following sequence:
1. Receive the next `AgentBatch` from the stream.
2. Invoke `strategy.allocate_batch(&batch, num_workers)` to assign each agent to a worker.
3. For each agent in the batch: insert it into the corresponding partition accumulator.
4. For each resolved connection in the batch: if both endpoints are assigned to the same worker, add as an internal wire. If assigned to different workers, generate a border wire (FreePort pair + border map entry).
5. For each `PendingConnection` in the batch: buffer it in the pending connection store.
6. For each previously pending connection whose target agent now exists: resolve it (create the wire or border as in step 4), and remove from the pending store.
7. Repeat from step 1 until the stream is exhausted.
8. After the last chunk: verify that no unresolved pending connections remain (assert empty pending store). Finalize partition accumulators into `Partition` values.
**(MUST)**

**R19.** At the end of the pipeline (step 8), the pending connection store MUST be empty. A non-empty pending store indicates a generator bug (an agent was referenced but never generated). The pipeline MUST return an error if unresolved pending connections remain. **(MUST)**

**R20.** The `ChunkedPartitionResult` MUST contain:
- `partitions: Vec<Partition>` — one per worker, fully formed as per SPEC-04 `Partition` type.
- `borders: HashMap<u32, (PortRef, PortRef)>` — the complete border map.
- `stats: StreamingPartitionStats` — partitioning statistics from the strategy.
**(MUST)**

**R21.** The output `ChunkedPartitionResult` MUST be directly usable by the existing merge protocol (SPEC-05). The `Partition` values MUST satisfy the same structural requirements as those produced by SPEC-04's `split()` function: each partition carries a `subnet: Net`, `free_port_index`, `id_range`, `border_id_start`, `border_id_end`, and `worker_id`. **(MUST)**

**R22.** At no point during the pipeline MUST the coordinator hold more than one `AgentBatch` in memory simultaneously, plus the partition accumulators and the border/pending tracking structures. The pipeline MUST NOT buffer the full stream before partitioning. **(MUST)**

**R23.** Partition accumulators grow incrementally as chunks are processed. Each accumulator is a `Net` that receives agents and connections chunk by chunk. The accumulator's `agents` Vec and `ports` Vec MUST be sized to `max_agent_id_in_this_worker + 1` (and `* PORTS_PER_SLOT` for ports), NOT to the global `max_agent_id`. This is a key memory optimization: each worker's accumulator only covers the ID range relevant to that worker. **(MUST)**

### 3.4 Configuration

**R24.** The `chunk_size` parameter MUST be configurable via `GridConfig` (SPEC-07). The default value SHOULD be 10,000 agents. **(MUST for configurability; SHOULD for default)**

**R25.** The streaming partition strategy MUST be selectable via `GridConfig`. Two options MUST be available: `round_robin` (default) and `fennel` (if implemented). **(MUST)**

**R26.** When `chunk_size` is set to `u32::MAX` (or a sentinel value indicating "no chunking"), the pipeline MUST degenerate to the v1 behavior: generate the full net, then partition globally using SPEC-04's `split()`. This ensures backward compatibility. **(MUST)**

### 3.5 Invariant Preservation

**R27.** The streaming pipeline MUST preserve all invariants from SPEC-01 that apply to the output partitions. Specifically:

- **T1 (Port Linearity):** Every port in every output partition MUST be connected to exactly one other port (or to a `FreePort(bid)` for border wires). The incremental construction MUST maintain bidirectionality at every step. **(MUST)**

- **I3 (Monotonic IDs):** `AgentId` values MUST be monotonically increasing within the stream (R15) and within each partition accumulator. Each partition's `next_id` MUST be set correctly at finalization. **(MUST)**

- **D1 (Split/Merge Identity) — extended for streaming:**
  The union of all output partitions MUST be isomorphic to the net that would have been produced by collecting the entire stream into a single net. Formally:
  ```
  merge(generate_and_partition_chunked(stream, n, strategy))
    ~ collect_and_split(stream, n, equivalent_global_strategy)
  ```
  where `~` denotes isomorphism (SPEC-00, Section 6.12). This ensures that the incremental construction produces the same logical result as the batch construction. **(MUST)**

- **C1 (Complete Agent Coverage):** Over the union of ALL batches, every agent MUST be assigned to exactly one worker. No agent is lost or duplicated across the chunked pipeline. **(MUST)**

- **C2 (Complete Wire Coverage):** Over the union of ALL batches, every wire (including cross-chunk wires resolved via forward references) MUST be classified as internal, interface, or border. No wire is lost. **(MUST)**

- **C3 (FreePort Bijectivity):** Every `borderId` generated during the pipeline MUST appear in exactly two distinct partitions. **(MUST)**

**R28.** In debug mode (`#[cfg(debug_assertions)]`), the finalized output MUST pass the same C1-C3 assertions defined in SPEC-04, Section 4.8 (`assert_coverage_and_disjunction`, `assert_border_consistency`). The assertions operate on the finalized `Vec<Partition>`, not on intermediate states. **(MUST)**

**R29.** ID ranges (SPEC-04, R16-R18) MUST be computed identically to SPEC-04: `chunk_size_id = u32::MAX / num_workers`, worker `i` receives `[i * chunk_size_id, (i+1) * chunk_size_id)`. ID ranges do not depend on the partitioning mode (streaming vs. batch) and MUST be identical in both modes. **(MUST)**

---

## 4. Design

### 4.1 Types

```rust
/// A connection directive within an AgentBatch.
///
/// Represents a wire between two ports. Both endpoints may be in the
/// current batch, or the target may be in a future batch (forward ref).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ConnectionDirective {
    /// Both endpoints exist in the current or previous batches.
    /// The wire can be immediately installed in the partition accumulator.
    Resolved {
        source: (AgentId, PortId),
        target: (AgentId, PortId),
    },
    /// The target agent has not been generated yet. This connection
    /// will be resolved when the target agent appears in a future batch.
    /// This is used by generators with cross-batch dependencies (e.g.,
    /// dual_tree: leaf-to-parent wires are forward references resolved
    /// when the parent batch arrives).
    Pending {
        source: (AgentId, PortId),
        target_agent_id: AgentId,
        target_port: PortId,
    },
}
```

```rust
/// A bounded batch of agents produced by a streaming generator.
///
/// Each batch contains agent definitions and connection directives.
/// The batch is the unit of work in the streaming pipeline: the
/// generator produces one batch, the partitioner assigns its agents,
/// and the pipeline installs agents and connections incrementally.
///
/// IC concept: An AgentBatch represents a fragment of an interaction
/// net (SPEC-02). The fragment may have dangling ports (forward
/// references to agents in future batches). These dangling ports are
/// temporary and are resolved when the referenced agents are generated.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentBatch {
    /// Agent definitions: (id, symbol) pairs.
    /// IDs MUST be globally unique and monotonically increasing
    /// across batches (SPEC-01, I3).
    pub agents: Vec<(AgentId, Symbol)>,

    /// Connection directives for this batch.
    /// Each directive describes a wire between two ports.
    /// Resolved directives have both endpoints available;
    /// Pending directives reference agents in future batches.
    pub connections: Vec<ConnectionDirective>,
}
```

```rust
/// Statistics about a streaming partitioning run.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StreamingPartitionStats {
    /// Total agents assigned across all batches.
    pub total_agents: u64,
    /// Number of agents assigned to each worker.
    pub per_worker_counts: Vec<u64>,
    /// Number of border wires created.
    pub border_wire_count: u64,
    /// Number of chunks processed.
    pub chunks_processed: u64,
}
```

```rust
/// The result of the chunked generation + partitioning pipeline.
///
/// This type is structurally equivalent to PartitionPlan (SPEC-04)
/// but produced incrementally. It is directly consumable by the
/// merge protocol (SPEC-05).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChunkedPartitionResult {
    /// One partition per worker, fully formed.
    pub partitions: Vec<Partition>,
    /// Border map: borderId -> (original_endpoint_A, original_endpoint_B).
    pub borders: HashMap<u32, (PortRef, PortRef)>,
    /// Statistics from the streaming partitioning strategy.
    pub stats: StreamingPartitionStats,
}
```

### 4.2 Trait StreamingPartitionStrategy

```rust
/// Trait for streaming (online) partition strategies.
///
/// Unlike PartitionStrategy (SPEC-04, R21) which requires the full
/// net to compute the allocation function sigma, this trait assigns
/// agents to workers incrementally, one batch at a time.
///
/// IC concept: Correctness of distributed IC reduction does not depend
/// on partition quality (DISC-004 v2, Section 1.6; ARG-002, Passo 10).
/// Any allocation function sigma that satisfies C1-C3 produces a
/// correct result. Streaming strategies trade partition quality for
/// bounded memory usage: the strategy never sees the full net.
///
/// Implementations are stateful (&mut self) because they may track
/// per-worker load or assignment history across batches.
pub trait StreamingPartitionStrategy {
    /// Assigns each agent in the batch to a worker.
    ///
    /// Input: reference to the batch and number of workers.
    /// Output: list of (AgentId, WorkerId) assignments.
    ///
    /// Post-conditions:
    /// - Every agent in the batch has exactly one assignment.
    /// - Every WorkerId is in range [0, num_workers).
    /// - No agent is assigned twice (within this batch or across batches).
    fn allocate_batch(
        &mut self,
        batch: &AgentBatch,
        num_workers: u32,
    ) -> Vec<(AgentId, WorkerId)>;

    /// Returns statistics about the partitioning so far.
    fn finalize(&self) -> StreamingPartitionStats;
}
```

### 4.3 RoundRobinStreamingStrategy (MVP)

```rust
/// Simplest streaming partition strategy: assigns agents in
/// round-robin order across workers.
///
/// Properties:
/// - O(1) per agent, O(B) per batch where B is the batch size.
/// - Zero state beyond a counter (no assignment cache).
/// - Deterministic: same sequence of batches = same result.
/// - Same partition quality as ContiguousIdStrategy (SPEC-04, R22)
///   for sequential generators with contiguous IDs.
/// - Ignores graph topology entirely.
///
/// This is the MVP strategy and the default for v2.
pub struct RoundRobinStreamingStrategy {
    /// Running counter for round-robin assignment.
    counter: u64,
    /// Per-worker agent counts for statistics.
    per_worker_counts: Vec<u64>,
}

impl StreamingPartitionStrategy for RoundRobinStreamingStrategy {
    fn allocate_batch(
        &mut self,
        batch: &AgentBatch,
        num_workers: u32,
    ) -> Vec<(AgentId, WorkerId)> {
        batch.agents.iter().map(|(id, _symbol)| {
            let worker = (self.counter % num_workers as u64) as WorkerId;
            self.counter += 1;
            self.per_worker_counts[worker as usize] += 1;
            (*id, worker)
        }).collect()
    }

    fn finalize(&self) -> StreamingPartitionStats {
        StreamingPartitionStats {
            total_agents: self.counter,
            per_worker_counts: self.per_worker_counts.clone(),
            border_wire_count: 0, // not tracked by round-robin
            chunks_processed: 0,  // tracked externally by pipeline
        }
    }
}
```

### 4.4 FennelStreamingStrategy (Advanced)

```rust
/// FENNEL/LDG-style streaming partition strategy.
///
/// Assigns each agent to the worker where it has the most
/// already-assigned neighbors, with a capacity penalty.
///
/// Score for assigning agent A to worker w:
///   score(w) = neighbors_w(A) - alpha * degree(w)
///
/// where:
///   neighbors_w(A) = number of A's ports connected to agents
///                     already assigned to worker w
///   degree(w) = total agents assigned to w so far
///   alpha = configurable balance parameter (default: 1.0)
///
/// Memory: O(total_agents) for the assignment cache,
///         O(num_workers) for per-worker counters.
///
/// References:
/// - Tsourakakis et al., KDD 2014 (FENNEL)
/// - Stanton & Kliot, KDD 2012 (LDG)
pub struct FennelStreamingStrategy {
    /// Maps AgentId -> WorkerId for all previously assigned agents.
    /// Required for neighbor lookup when assigning new agents.
    assignment_cache: HashMap<AgentId, WorkerId>,
    /// Per-worker agent counts.
    per_worker_counts: Vec<u64>,
    /// Balance parameter: higher alpha penalizes imbalanced partitions more.
    alpha: f64,
}
```

### 4.5 Streaming Generator Variant (ep_annihilation example)

```rust
/// Streaming variant of the ep_annihilation generator.
///
/// Emits ERA-ERA pairs in batches. Each batch contains at most
/// chunk_size agents (pairs are emitted in full — a batch may
/// contain chunk_size - 1 agents if the last pair would exceed
/// the limit, to avoid splitting a pair across batches).
///
/// IC concept: ERA-ERA pairs are completely independent — no wires
/// connect different pairs. Therefore no forward references are
/// needed, and each batch is self-contained.
fn ep_annihilation_stream(
    size: u32,
    chunk_size: usize,
) -> Box<dyn Iterator<Item = AgentBatch>> {
    // Each pair: 2 agents (ERA, ERA), 1 connection (p0 <-> p0).
    // Agents per pair: 2. Pairs per batch: chunk_size / 2.
    // Total batches: ceil(size / (chunk_size / 2)).
    // ...
    todo!()
}
```

### 4.6 Chunked Pipeline Architecture

The pipeline processes the net as a sequence of chunks in a single pass:

```
Generator (Iterator<AgentBatch>)
    |
    | batch_k
    v
StreamingPartitionStrategy
    |
    | Vec<(AgentId, WorkerId)>
    v
Partition Accumulators [W0, W1, ..., W_{n-1}]
    |
    | (resolved connections -> install as wires)
    | (pending connections -> buffer)
    | (previously pending -> resolve if target now exists)
    v
Border Accumulator (HashMap<u32, (PortRef, PortRef)>)
    |
    | (after last chunk)
    v
ChunkedPartitionResult { partitions, borders, stats }
```

**Pseudocode for `generate_and_partition_chunked`:**

```
fn generate_and_partition_chunked(stream, num_workers, strategy):
    // Initialize per-worker partition accumulators
    accumulators: Vec<PartitionAccumulator> = vec![new(); num_workers]

    // Border tracking
    border_id_counter: u32 = 0   // no pre-existing FreePorts in a fresh gen
    border_map: HashMap<u32, (PortRef, PortRef)> = HashMap::new()

    // Pending connections (forward references)
    pending: HashMap<AgentId, Vec<PendingConnection>> = HashMap::new()

    // Assignment lookup (which worker owns which agent)
    agent_owner: HashMap<AgentId, WorkerId> = HashMap::new()

    // Process each chunk
    for batch in stream:
        // Step 1: Assign agents to workers
        assignments = strategy.allocate_batch(&batch, num_workers)
        for (agent_id, worker_id) in &assignments:
            agent_owner.insert(agent_id, worker_id)
            accumulators[worker_id].add_agent(agent_id, symbol)

        // Step 2: Process resolved connections
        for conn in batch.connections:
            match conn:
                Resolved { source, target }:
                    install_connection(
                        source, target, &agent_owner,
                        &mut accumulators, &mut border_map,
                        &mut border_id_counter
                    )
                Pending { source, target_agent_id, target_port }:
                    pending.entry(target_agent_id)
                        .or_default()
                        .push(PendingConnection { source, target_port })

        // Step 3: Resolve any previously pending connections
        //         whose target agent appeared in this batch
        for (agent_id, _) in &assignments:
            if let Some(pending_conns) = pending.remove(agent_id):
                for pc in pending_conns:
                    install_connection(
                        pc.source, (agent_id, pc.target_port),
                        &agent_owner, &mut accumulators,
                        &mut border_map, &mut border_id_counter
                    )

    // Step 4: Verify no unresolved forward references remain
    assert!(pending.is_empty(),
        "Generator bug: {} unresolved forward references",
        pending.len())

    // Step 5: Finalize accumulators into Partition values
    id_ranges = compute_id_ranges(num_workers)  // same as SPEC-04, R18
    partitions = finalize_accumulators(
        accumulators, id_ranges, border_id_counter
    )

    // Step 6: Debug assertions (C1-C3)
    debug_assert!(verify_c1_c2_c3_from_stats(strategy, &partitions, &border_map))

    return ChunkedPartitionResult { partitions, borders: border_map,
                                     stats: strategy.finalize() }


fn install_connection(source, target, agent_owner, accumulators,
                      border_map, border_id_counter):
    let src_worker = agent_owner[source.0]
    let tgt_worker = agent_owner[target.0]
    if src_worker == tgt_worker:
        // Internal wire: install directly in the accumulator
        accumulators[src_worker].connect(
            AgentPort(source.0, source.1),
            AgentPort(target.0, target.1)
        )
    else:
        // Border wire: generate FreePort pair
        let bid = *border_id_counter
        *border_id_counter += 1
        border_map.insert(bid,
            (AgentPort(source.0, source.1), AgentPort(target.0, target.1)))
        accumulators[src_worker].connect(
            AgentPort(source.0, source.1), FreePort(bid))
        accumulators[tgt_worker].connect(
            AgentPort(target.0, target.1), FreePort(bid))
```

### 4.7 Forward Reference Resolution

Cross-chunk wires arise when a generator produces agents in topological order where a child references a parent not yet emitted (e.g., `dual_tree` generates leaves before the root).

**Resolution protocol:**

1. When the generator emits a `Pending` connection directive, the pipeline stores it in `pending: HashMap<AgentId, Vec<PendingConnection>>`, keyed by the target agent ID.

2. When a new batch arrives, after agent assignment (Step 1), the pipeline checks `pending` for any entries matching the newly generated agent IDs (Step 3).

3. Each resolved pending connection is installed as either an internal wire or a border wire, exactly as resolved connections are (via `install_connection`).

4. After the final batch, `pending` MUST be empty. A non-empty map means the generator promised an agent that was never delivered -- this is a generator bug, not a pipeline bug.

**Memory overhead:** The pending store holds at most O(forward_refs_in_flight) entries. For `dual_tree`, this is O(width_of_current_layer) -- bounded by the tree width, not the tree size.

### 4.8 Border ID Assignment for Streaming

In SPEC-04, border IDs start at `max_existing_freeport_id(net) + 1` (R12). In the streaming pipeline, there is no pre-existing net to scan. Border ID assignment follows these rules:

1. For freshly generated nets (no pre-existing Lafont FreePorts): border IDs start at 0 and increment monotonically.

2. If the generator produces agents with pre-existing Lafont FreePorts (interface wires), the pipeline MUST scan the first batch for the maximum FreePort ID and start border IDs above that. Generators SHOULD emit all Lafont FreePorts in the first batch to simplify this check.

3. The `border_id_start` and `border_id_end` fields of each output `Partition` (SPEC-04, R15a) MUST be set to the global range `[0, border_id_counter)` (or `[max_lafont_freeport_id + 1, border_id_counter + max_lafont_freeport_id + 1)` if Lafont FreePorts exist).

### 4.9 Partition Accumulator Design

Each `PartitionAccumulator` wraps a `Net` (SPEC-02) and a `HashMap<u32, PortRef>` (free port index):

```rust
/// Accumulates agents and connections for a single worker
/// during chunked generation.
struct PartitionAccumulator {
    /// The sub-net being built incrementally.
    subnet: Net,
    /// Reverse index of boundary FreePorts.
    free_port_index: HashMap<u32, PortRef>,
    /// Worker ID.
    worker_id: WorkerId,
}

impl PartitionAccumulator {
    fn add_agent(&mut self, id: AgentId, symbol: Symbol) {
        // Expand subnet.agents Vec if needed to fit id.
        // Insert Agent { id, symbol } at position id.
        // Ensure subnet.ports Vec is sized to (id + 1) * PORTS_PER_SLOT.
    }

    fn connect(&mut self, a: PortRef, b: PortRef) {
        // If b is FreePort(bid): update free_port_index[bid] = a.
        // If a is FreePort(bid): update free_port_index[bid] = b.
        // Delegate to subnet.connect(a, b) for port array update.
    }

    fn finalize(self, id_range: IdRange, border_id_start: u32,
                border_id_end: u32) -> Partition {
        Partition {
            subnet: self.subnet,
            worker_id: self.worker_id,
            free_port_index: self.free_port_index,
            id_range,
            border_id_start,
            border_id_end,
        }
    }
}
```

**Key property:** The accumulator's `subnet.agents` Vec is sized to accommodate only the agents assigned to this worker. If worker 0 receives agents with IDs {0, 3, 7}, the Vec is sized to 8 (index 0..=7), with slots 1, 2, 4, 5, 6 as `None`. This is the same sparse layout as SPEC-04, Section 4.5 Step 5, but built incrementally.

### 4.10 Diagram: Streaming vs. Batch Pipeline

```
=== v1 BATCH PIPELINE (SPEC-04) ===

Generator ──(full net)──> split(net, n, strategy) ──> PartitionPlan
             O(N) mem      O(N) mem                   O(N) mem

Peak coordinator memory: O(N) throughout


=== v2 STREAMING PIPELINE (SPEC-21) ===

Generator ──(batch_1)──> strategy.allocate_batch ──> accumulators
         ──(batch_2)──> strategy.allocate_batch ──> accumulators
         ──  ...    ──>        ...               ──>    ...
         ──(batch_k)──> strategy.allocate_batch ──> accumulators
                                                         |
                                                         v
                                                 ChunkedPartitionResult

Peak coordinator memory: O(C + sum(accumulators) + borders + pending)
  where C = chunk_size (bounded)
  accumulators grow to O(N) total (unavoidable: workers need the agents)
  borders = O(B) where B = number of border wires
  pending = O(max forward refs in flight)

Memory improvement: The coordinator never holds the full dense Net
with its port array. Accumulators are sparse per-worker Nets. The
generator's internal state for each chunk is bounded by C.
```

---

## 5. Rationale

### 5.1 Why Streaming Partitioning is Correct

The correctness of distributed IC reduction depends on conditions C1-C3 (SPEC-04, R6-R8) being satisfied by the output partitions, and on the strong confluence property (SPEC-01, T4) guaranteeing that any valid partition produces the correct result regardless of partition quality (DISC-004 v2, Section 1.6; ARG-002, Passo 10).

The key insight: C1-C3 are conditions on the **output** partitions, not on the **process** that creates them. Whether the partitions are created by a single atomic `split()` call or by incremental accumulation across multiple chunks, the only thing that matters is that the final partitions satisfy C1-C3. The streaming pipeline guarantees this:

- **C1 (Complete coverage):** Every agent emitted by the generator is assigned exactly once by `allocate_batch` and inserted into exactly one accumulator (R7). The pipeline tracks all agents via `agent_owner`.
- **C2 (Complete wire coverage):** Every connection directive is either installed immediately (resolved) or buffered and installed later (pending). The empty-pending-store assertion (R19) guarantees no wire is lost.
- **C3 (FreePort bijectivity):** Each border wire generates exactly one `borderId` with entries in exactly two accumulators (via `install_connection`). The border map records both endpoints.

### 5.2 Locality Enables Streaming

Agents in an IC net can be generated and partitioned incrementally because reduction depends only on immediate neighborhoods (REF-001, p.96; REF-002, p.70). An agent's reducibility is determined by its principal port connection, not by the global structure of the net. Therefore, assigning an agent to a worker based on local information (its immediate neighbors) is sufficient for correctness. Partition quality may suffer compared to global strategies, but correctness is guaranteed by strong confluence.

### 5.3 Memory Trade-off

The streaming pipeline does not eliminate O(N) memory entirely -- the partition accumulators still grow to O(N) total because the agents must physically reside somewhere before dispatch. The improvement is:

1. **No full dense Net:** The v1 pipeline allocates a dense `Net` with `agents.len() * 3` port array entries, where `agents.len()` covers the full ID range. Accumulators are per-worker and sized to each worker's max ID, which is smaller.
2. **No simultaneous full net + full partition plan:** In v1, both the input `Net` and the output `PartitionPlan` coexist in memory during `split()`. In v2, the generator's state for each chunk is bounded by `chunk_size`.
3. **Early dispatch (future):** If combined with protocol support for partial partition dispatch (ROADMAP 2.19), the coordinator could send completed chunks to workers as they are partitioned, keeping only the border tracking state.

---

## 6. Migration Path

### 6.1 From v1 to Streaming

The streaming pipeline is additive: it introduces new types and functions without modifying the existing `split()` function or `PartitionStrategy` trait.

**Phase 1 (MVP):**
1. Implement `AgentBatch`, `ConnectionDirective`, `StreamingPartitionStats` types.
2. Implement `StreamingPartitionStrategy` trait and `RoundRobinStreamingStrategy`.
3. Implement `ep_annihilation_stream()` generator variant.
4. Implement `generate_and_partition_chunked()` pipeline.
5. Wire into `run_grid` as an alternative path when `config.streaming` is enabled.
6. Estimated: ~400 LoC (types: 80, strategy: 80, generator: 60, pipeline: 150, tests: 30 in main code).

**Phase 2 (Enhanced):**
1. Implement `FennelStreamingStrategy`.
2. Add streaming variants for `dual_tree` and other generators (requires forward references).
3. Add configuration UI (CLI flags for `chunk_size`, strategy selection).
4. Estimated: ~300 additional LoC.

**Phase 3 (Integration):**
1. Integrate with early dispatch (ROADMAP 2.19) for full memory reduction.
2. Integrate with async channels (`tokio::sync::mpsc`) for backpressure.
3. Estimated: ~200 additional LoC (Infrastructure-layer).

### 6.2 Coexistence with SPEC-04

The streaming pipeline produces output (`ChunkedPartitionResult`) that is structurally identical to `PartitionPlan`. The coordinator can use either path transparently:

```rust
let plan = if config.streaming {
    let stream = benchmark.make_net_stream(size, config.chunk_size);
    let result = generate_and_partition_chunked(
        stream, num_workers, &mut streaming_strategy
    );
    PartitionPlan {
        partitions: result.partitions,
        borders: result.borders,
    }
} else {
    let net = benchmark.make_net(size);
    split(&net, num_workers, &batch_strategy)
};
```

---

## 7. Test Strategy

### 7.1 Unit Tests

**T1. RoundRobinStreamingStrategy assignment correctness.**
- Generate 100 agents in 5 batches of 20.
- Verify each agent is assigned to exactly one worker.
- Verify round-robin order: agent 0 -> worker 0, agent 1 -> worker 1, ..., agent n -> worker n % K.
- Verify `finalize()` statistics match expectations.

**T2. AgentBatch construction.**
- Create batches with known agents and connections.
- Verify agent IDs are monotonically increasing across batches.
- Verify connection directives are correctly classified (resolved vs. pending).

**T3. Forward reference resolution.**
- Generate a batch with a `Pending` connection to agent ID 50.
- Generate a second batch containing agent 50.
- Verify the pending connection is resolved after the second batch.
- Verify the wire (internal or border) is correctly installed.

**T4. Empty pending store assertion.**
- Generate batches with a `Pending` connection to an agent that is never generated.
- Verify the pipeline returns an error.

**T5. Streaming pipeline produces valid partitions.**
- Run `generate_and_partition_chunked` with `ep_annihilation(100)`, `chunk_size=20`, `num_workers=4`.
- Verify C1: all 200 agents are present across partitions.
- Verify C2: all 100 wires are present (internal or border).
- Verify C3: each border ID appears in exactly 2 partitions.

### 7.2 Equivalence Tests

**T6. Streaming vs. batch equivalence.**
- Generate the same net via both `make_net` (batch) and `make_net_stream` (streaming).
- Partition via both `split()` and `generate_and_partition_chunked()`.
- Merge both results via SPEC-05's `merge()`.
- Verify the two merged nets are isomorphic.

**T7. End-to-end reduction equivalence.**
- Run `reduce_all(make_net(size))` (sequential baseline).
- Run `run_grid` with streaming pipeline.
- Verify the results are isomorphic.
- Verify interaction counts are identical (SPEC-01, T7).

### 7.3 Property-Based Tests

**T8. Chunk size independence.**
- For a fixed `(benchmark, size, num_workers)` triple, vary `chunk_size` from 1 to `size`.
- Verify that the merged result is always isomorphic to the sequential baseline.
- This tests that the streaming pipeline is correct for all chunk sizes.

**T9. Strategy independence.**
- For a fixed `(benchmark, size, chunk_size)` triple, test with `RoundRobinStreamingStrategy` and (if implemented) `FennelStreamingStrategy`.
- Verify that the merged results are isomorphic to each other and to the sequential baseline.
- This tests that partition quality affects performance but never correctness.

### 7.4 Performance Tests

**T10. Peak memory measurement.**
- Instrument the pipeline to track peak allocation size.
- Verify that peak memory during streaming is bounded by O(chunk_size + accumulator_sizes) and does not scale with total net size (beyond accumulator growth).

---

## 8. Open Questions

**Q1. Accumulator memory for large nets.** The partition accumulators grow to O(N/K) per worker (total O(N)). For nets too large to fit in coordinator memory, the accumulators themselves will exceed memory. This is addressed by early dispatch (ROADMAP 2.19) or recipe-based generation (ROADMAP 2.29), both of which are out of scope for SPEC-21.

**Q2. Optimal chunk size.** The default of 10,000 agents is a guess. Benchmarking is needed to determine the optimal chunk size for different generator types and network topologies. Smaller chunks mean more overhead (per-chunk strategy calls, pending connection lookups); larger chunks mean less streaming benefit.

**Q3. FennelStreamingStrategy parameter tuning.** The `alpha` parameter in the FENNEL scoring function affects the load-balance vs. edge-cut trade-off. The optimal value depends on the net topology and the number of workers. Empirical calibration is needed. The literature suggests `alpha = sqrt(K) * |E| / |V|^2` (Tsourakakis et al., 2014) but this requires knowing total edges/vertices upfront, which contradicts the streaming model. An adaptive alpha that adjusts as batches arrive is a potential enhancement.

**Q4. Interaction with delta protocol (ROADMAP 2.26).** Under the delta protocol, the coordinator does not hold the full merged net between rounds. If the initial partition is done via streaming, the coordinator only needs the border map and the assignment cache -- significantly less state. The interaction between SPEC-21 and the delta protocol spec needs formalization.

**Q5. Root port handling in streaming.** SPEC-04, R28 requires the root port to be propagated to the partition containing the root agent. In the streaming pipeline, the root agent may appear in any batch. The pipeline MUST defer root assignment until the batch containing the root agent is processed. If the generator does not specify a root, or if root is determined post-generation, this is a non-issue.

**Q6. Port array sizing in accumulators.** When agents are assigned to workers non-contiguously (e.g., FENNEL assigns agent 0 and agent 1000 to worker 0), the accumulator's port array must be sized to `(1000 + 1) * 3` despite having only 2 agents. This sparse representation wastes memory. A more compact representation (e.g., HashMap-based or offset-indexed) could improve memory efficiency but would diverge from the SPEC-02 port array layout. This trade-off should be evaluated during implementation.
