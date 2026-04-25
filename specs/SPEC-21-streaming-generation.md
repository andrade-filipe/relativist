# SPEC-21: Streaming Generation and Partitioning

**Status:** Reviewed v2
**Depends on:**
- SPEC-01 (Invariants — base I-/D-/C-/G- layers)
- SPEC-02 (Net Representation — `Net`, `Agent`, `Symbol`, `PortRef`, `AgentId`, `PortId`)
- SPEC-04 (Partitioning — `Partition`, `PartitionPlan`, `split()`, R6/R12/R16-R18/R28; amended via §3.8 A1)
- SPEC-05 (Merge and Grid Cycle — merge contract for `Partition` outputs)
- SPEC-06 (Wire Protocol — `Message` enum and `PROTOCOL_VERSION`; amended via §3.8 A2)
- SPEC-07 (CLI / `GridConfig` — three new fields; amended via §3.8 A3)
- SPEC-09 (Benchmarks — `Benchmark` trait; amended via §3.8 A4)
- SPEC-13 (System Architecture — coordinator and worker FSMs; amended via §3.8 A5)
- SPEC-17 (Transport Layer — pull-protocol round-trip over `ChannelTransport` for tests)
- SPEC-18 (Wire Format v2 — serde of new `Message` variants; PROTOCOL_VERSION sequencing per §3.7)
- SPEC-19 (Delta Protocol — `BorderGraph` interaction with chunked dispatch; cross-references R36 / §3.7)
- SPEC-22 (Arena Management — `SparseNet` for `PartitionAccumulator`; protected tombstones R10b/R10c for streaming border safety; I3' relaxation cross-checked at §3.5)

**ROADMAP items:** 2.27 (Streaming Net Generation), 2.28 (Online/Streaming Graph Partitioning), 2.30 (Chunked Generation with Incremental Partitioning), 2.36 (Lazy/Demand-Driven Generation)

**References consumed:**
- REF-001 (Lafont 1990, p.96 — locality of IC reduction)
- REF-002 (Lafont 1997, p.70-73 — net structure, strong confluence)
- REF-005 (Mackie & Pinto 2002 — IC parallel evaluation context)
- REF-015 (Mackie & Sato 2015 — rule-level streaming reference; see §5.2 for level reconciliation per DISC-009 v2)

**Arguments consumed:**
- ARG-001 (central argument, P1-P6 — confluence preserves determinism)
- ARG-002 (partitioning preserves structure, C1-C3 — quality independence of correctness)

**Discussions consumed:**
- DISC-004 v2 (Formal Partitioning of IC Networks — quality vs correctness independence; cited in §4.2 trait doc-comment and §5.1 rationale)
- DISC-009 v2 (Streaming Levels and Operating Modes — primary taxonomy anchor; SPEC-21 covers level-3 generation-protocol streaming and the on-demand operating mode)

**Code analyses consumed:**
- AC-007 (HVM2 Reduction Engine — on-the-fly redex detection, informs §4.6 `install_connection` border detection during the streaming loop)
- AC-010 (HVM4 WNF Evaluation — frame reuse / goto state-machine pattern, informs §4.9 `PartitionAccumulator` construction discipline)
- AC-014 (Bench Methodology — canonical methodology reference for §7.4 T10 peak-memory measurement)

**Briefings consumed:** BRIEF-20260415-v2-codebase-assessment (Section 4.3: partition module), BRIEF-20260415-v2-fundamentacao-teorica (Tier 3 Memory, locality principle, streaming partitioning), `docs/briefings/SPEC-21-coherence-brief-2026-04-25.md` (Round-2 coherence brief)

---

## 1. Purpose

This spec defines how Relativist generates and partitions interaction combinator nets in a streaming (incremental) fashion, so that the coordinator never needs to hold the entire net in memory simultaneously. In v1, the pipeline is `generate full net -> partition globally -> dispatch partitions`: the coordinator holds the full net at peak, which is O(total_agents) memory. SPEC-21 replaces this with a chunked pipeline: `generate chunk -> partition chunk -> dispatch chunk -> repeat`, bounding coordinator peak memory to O(chunk_size + border_tracking_state) instead of O(total_agents).

Three tightly coupled features are specified together because they form a single pipeline:

1. **Streaming Partition Strategy (2.28):** A trait for partitioning strategies that assign agents to workers on-the-fly, one batch at a time, without requiring a global view of the net. Includes round-robin (MVP) and FENNEL/LDG-style heuristic (advanced) strategies.
2. **Streaming Net Generation (2.27):** A producer-consumer pattern where generators emit agents in bounded batches instead of constructing the full net upfront.
3. **Chunked Generation Pipeline (2.30):** The integration of 2.27 and 2.28 into a complete pipeline where chunks are generated, partitioned, and dispatched incrementally.

**Why they belong in one spec:** The three features share types (`AgentBatch`, `ForwardReference`), invariant extensions (D1 over batch unions, incremental C1-C3), and a single pipeline architecture. Specifying them separately would create circular cross-references.

**Position in DISC-009 v2 taxonomy (closes SC-003).** SPEC-21 covers the **generation-protocol streaming level (level 3)** of DISC-009 v2's three-tier taxonomy (rule-level / state-protocol / generation-protocol). The pull-based dispatch in §3.6 corresponds to the **on-demand operating mode** in DISC-009 v2's 5-mode catalog. SPEC-14 §8 covers level-1 streaming (rule-level traces) and SPEC-25 covers level-3+ recipe generation (a refinement of level 3); SPEC-21 sits squarely at the generation-protocol layer. Bundling §3.1-§3.3 (streaming generation) with §3.6 (pull dispatch) is justified by DISC-009 v2's catalog of these as distinct operating modes within the same level.

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

**R10.** The `Benchmark` trait (SPEC-09 R2, in `src/bench/mod.rs`) MUST gain a new method WITH A DEFAULT IMPLEMENTATION so that all 13 existing implementations remain valid without per-implementation edits:
```rust
fn make_net_stream(
    &self,
    size: u32,
    chunk_size: usize,
) -> Box<dyn Iterator<Item = AgentBatch>> {
    // Default: collect the eager net and slice it into chunks.
    Box::new(default_chunked_iter(self.make_net(size), chunk_size))
}
```
The default implementation MUST be sufficient to keep the 13 SPEC-09 benchmark implementations compiling unchanged (closes SC-008). Generators that benefit from native streaming (e.g., `ep_annihilation`, R12) MUST override the default to avoid the materialize-then-slice memory cost. The default-impl helper `default_chunked_iter(net: Net, chunk_size: usize) -> impl Iterator<Item = AgentBatch>` lives in `src/bench/streaming.rs` (or `src/io/streaming.rs`) and walks `net.agents` in id order, emitting `AgentBatch` values whose connection directives are all `Resolved` (no forward references arise from a fully materialized net). The default path forfeits the memory benefit of streaming but preserves the API contract; benchmarks that opt in to streaming get the bound. **(MUST for the default-impl-bearing trait amendment; MUST for `ep_annihilation` override per R12; SHOULD for other generators per R12)**

**R11.** The existing `fn make_net(&self, size: u32) -> Net` method (SPEC-09 R2) is UNCHANGED — it MUST remain a required trait method (no default impl) so that the streaming default in R10 has a fallback. Backward compatibility with callers that need the full net (sequential baseline, verification) MUST be preserved. The relationship between R10 and R11 is: R11 is the source-of-truth materialization path; R10 either (a) wraps R11 via the default impl (memory-equivalent to v1) or (b) is overridden by the implementor for true bounded-memory streaming. The two MUST produce isomorphic nets when collected (T6, §7.2). **(MUST)**

**R12.** Each generator in `src/io/generators.rs` MUST gain a streaming variant that returns a `Box<dyn Iterator<Item = AgentBatch>>`. At minimum, the `ep_annihilation` generator MUST support streaming for the MVP. **(MUST for ep_annihilation; SHOULD for other generators)**

**R13.** For `ep_annihilation`, streaming is trivial: each batch emits independent ERA-ERA pairs. No cross-batch wires exist, so no forward references are needed. **(informative)**

**R14.** For generators with cross-batch dependencies (e.g., `dual_tree`), the `AgentBatch` MUST support forward references via `PendingConnection` entries. The batch carries both resolved connections (both endpoints in the current or previous batches) and pending connections (target agent will appear in a future batch). **(MUST)**

**R15.** The generator MUST assign `AgentId` values to agents in a globally unique, monotonically increasing sequence across all batches. The maximum `AgentId` in batch `k` MUST be less than the minimum `AgentId` in batch `k+1`. R15 is a **generator-phase** contract that is strictly stronger than the post-SPEC-22 invariant SPEC-01 I3' (uniqueness, not monotonicity; cf. SPEC-22 §3.8 A1). Satisfying R15 trivially satisfies I3'. The contract scope is the generation pipeline only (`make_net_stream`, `generate_and_partition_chunked`); once chunks are dispatched and workers fire reduction rules, the worker arena MAY recycle slot IDs per I3' / SPEC-22 R1-R10c, and SPEC-21 code MUST NOT assume monotonicity on agents created post-dispatch. See §3.5 closing note for the formal reconciliation (closes SC-009). **(MUST)**

**R16.** Generator streaming variants MUST be pure Core-layer code: no async, no tokio, no I/O. The iterator is synchronous. Integration with async channels (e.g., `tokio::sync::mpsc`) for backpressure is an Infrastructure-layer concern handled by the coordinator, not by the generator. The iterator's pull-based `Iterator::next` interface naturally supports the pull dispatch model (R32) without async coordination — the coordinator drives the iterator one `next()` call per `RequestWork` message. Async channels (`tokio::sync::mpsc`) are required only for the push dispatch model when generation and dispatch overlap in time (closes SC-012). **(MUST)**

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

**R24.** The `chunk_size` parameter MUST be configurable via `GridConfig` (SPEC-07). The default value SHOULD be 10,000 agents pending benchmark calibration (Q2). The default MUST be re-evaluated and either confirmed or replaced before v2 release (closes SC-024). **(MUST for configurability; SHOULD for the placeholder default)**

**R25.** The streaming partition strategy MUST be selectable via `GridConfig`. Two options MUST be available: `round_robin` (default) and `fennel` (if implemented). **(MUST)**

**R26.** When `chunk_size` is set to `u32::MAX` (or a sentinel value indicating "no chunking"), the pipeline MUST short-circuit to SPEC-04 `split()` after collecting the full stream into a single `Net` via the R10 default-impl path. The merge result MUST be **isomorphic** (SPEC-00 §6.12, `nets_isomorphic`) to the v1 `split()`-produced result; bit-identical layout is NOT guaranteed because of SPEC-22 arena-management amendments (free-list, SparseNet, `freeport_redirects` propagation). T6 (§7.2) measures isomorphism, not byte-equality (closes SC-014). **(MUST)**

### 3.5 Invariant Preservation

**R27.** The streaming pipeline MUST preserve all invariants from SPEC-01 that apply to the output partitions. Specifically:

- **T1 (Port Linearity):** Every port in every output partition MUST be connected to exactly one other port (or to a `FreePort(bid)` for border wires). The incremental construction MUST maintain bidirectionality at every step. **(MUST)**

- **I3' (Uniqueness of AgentIds, post-SPEC-22 §3.8 A1):** `AgentId` values MUST be unique across the stream and within each partition accumulator. SPEC-21 R15 imposes the strictly-stronger generator-phase property of monotonicity, which trivially satisfies I3'. Each partition's `next_id` MUST be set correctly at finalization per SPEC-02 R10 / SPEC-22 §3.8 A3 (strictly greater than any AgentId ever assigned in the partition, live or in the partition's own free-list). **(MUST)**

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

**R29b. (Border-id allocation for streaming, amends SPEC-04 R12.)** §4.8 of this spec specifies a streaming-side border-id allocation policy that diverges from SPEC-04 R12 ("border IDs start at `max_existing_freeport_id(net) + 1`"). For freshly-generated nets in the streaming pipeline, border IDs MUST start at 0 and increment monotonically. If the generator produces agents with pre-existing Lafont FreePorts (interface wires), the pipeline MUST scan the first batch for the maximum FreePort ID and start border IDs above that. The reconciliation with SPEC-04 R12 is recorded in §3.8 A1 (closes SC-018). **(MUST)**

**Closing note on R15 ↔ I3' reconciliation (closes SC-009).** R15 is a generator-phase contract, strictly stronger than SPEC-01 I3' as amended by SPEC-22 §3.8 A1. The contract scope is the **generation** path (`make_net_stream`, `generate_and_partition_chunked`, `PartitionAccumulator`). Once a chunk has been dispatched and a worker fires reduction rules, the worker's arena MAY recycle slot IDs per I3' / SPEC-22 R1-R10c. Code in `src/partition/streaming.rs` MUST NOT assume monotonicity on agents created post-dispatch (e.g., MUST NOT write `assert!(new_id > old_max_id)` patterns; cf. SPEC-22 §3.8 A6 forbidden assertion list). Implementers reading R15 in isolation should treat the property as a generator output bound, not a global invariant.

### 3.6 Lazy/Demand-Driven Generation (ROADMAP 2.36) — Amendment

This section is an amendment to SPEC-21, adding a pull-based orchestration layer on top of the streaming pipeline defined in Sections 3.1-3.5. In the push model (Sections 3.1-3.3), the coordinator generates chunks eagerly and dispatches them to workers. In the pull model (this section), workers request work on demand, and the coordinator generates and dispatches chunks only when requested.

**Motivation:** The push model requires the coordinator to predict how many chunks each worker needs. If workers have heterogeneous performance (different hardware, variable load), some workers finish early while others are still receiving chunks. The pull model naturally load-balances: fast workers request more chunks, slow workers request fewer.

**R30.** The coordinator MUST support a pull-based dispatch mode where workers request chunks via a `RequestWork` message. The coordinator generates and dispatches chunks on demand rather than eagerly. **(MUST)**

**R31.** The `Message` enum (SPEC-06) MUST gain two new variants for pull-based dispatch:
```rust
RequestWork { worker_id: WorkerId },
NoMoreWork,
```
`RequestWork` is sent by the worker to request a new chunk. `NoMoreWork` is sent by the coordinator when the generator stream is exhausted.
**(MUST)**

**R32.** The pull-based dispatch loop MUST follow this protocol:
1. Coordinator sends `AssignPartition` with the first chunk to each worker (cold start).
2. Worker reduces its chunk and sends `PartitionResult`.
3. Worker sends `RequestWork` to indicate readiness for more work.
4. Coordinator generates the next chunk (via `make_net_stream`), partitions it (via `StreamingPartitionStrategy`), and sends `AssignPartition` with the new chunk to the requesting worker.
5. When the stream is exhausted, coordinator responds to `RequestWork` with `NoMoreWork`.
6. Worker enters final reduction phase upon receiving `NoMoreWork`.
7. Coordinator collects final results and merges.
**(MUST)**

**R33.** The pull model MUST maintain the same invariants as the push model (R27-R29). Specifically:
- C1: Every agent generated across all chunks MUST be assigned to exactly one worker.
- D1 (extended): The merged result MUST be isomorphic to the sequential baseline.
- D5 (Exclusive Ownership): Each chunk MUST be dispatched to exactly one worker. The coordinator MUST track chunk-to-worker assignments. If a worker disconnects, the chunk MUST be re-dispatched (same pattern as SPEC-20 dynamic departure).
**(MUST)**

**R34.** The `GridConfig` struct MUST be extended with:
```rust
pub enum DispatchMode {
    Push,   // Eager: coordinator dispatches all chunks upfront
    Pull,   // Lazy: workers request chunks on demand
    Auto,   // Push for num_workers <= 2, Pull for num_workers > 2
}
```
The default MUST be `Auto`. **(MUST)**

**R35.** The pull model MUST handle the edge case where the generator stream is very short (fewer chunks than workers). In this case, some workers receive `NoMoreWork` immediately after their first chunk, and only a subset of workers participates. This is correct because P1 (confluence) guarantees the result is independent of how many workers reduce. **(MUST)**

**R36.** The pull model MUST be compatible with the delta protocol (SPEC-19): if delta mode is active, workers accumulate chunks into their persistent partition state. `RequestWork` requests a new chunk to append, not a replacement partition. The coordinator's border tracking (SPEC-19 `BorderGraph`) must account for borders discovered in each new chunk. **(SHOULD, as delta+lazy is an advanced combination)**

**R37.** Performance metric: the pull model SHOULD reduce idle time for heterogeneous workers. In benchmarks with 4 workers where one worker is 2x slower, the pull model SHOULD achieve higher throughput than the push model because fast workers process more chunks. The methodology for "higher throughput" measurement follows AC-014 (Bench Methodology): wall-clock with `std::time::Instant` per AC-014, warmup runs discarded, statistical methodology per SPEC-09 §3.5 (closes SC-011 partial). **(SHOULD)**

### 3.7 Cross-Cutting MUSTs (G1 / PROTOCOL_VERSION / BSP-barrier / push-mode termination)

This section gathers the cross-cutting properties that span sections 3.1-3.6 and that depend on coordinated behavior with SPEC-19 (delta), SPEC-22 (arena), and SPEC-13 (FSMs).

**R37b. (G1 free-list interaction, closes SC-007.)** During streaming pipeline execution under SPEC-22 free-list recycling, the worker arena MUST honor SPEC-22 R10b/R10c protected-tombstone discipline for any `AgentId` that appears in the coordinator's `border_map: HashMap<u32, (PortRef, PortRef)>` or in the pending-connection store. Concretely:
- Under `GridConfig.recycle_under_delta == RecyclePolicy::DisableUnderDelta` (SPEC-22 R10b Strategy A, default), workers MUST NOT pop from the free-list while delta mode is active and chunked dispatch is in progress; this is the conservative gating that closes G1 by construction.
- Under `RecyclePolicy::BorderClean` (SPEC-22 R10b Strategy B, opt-in), workers MAY pop from the free-list only for IDs not present in the worker's locally-cached `border_entries` set.
- Equivalently, an implementation MAY explicitly **disable free-list recycling** for the entire generation+accumulation phase via a `feature = "streaming-no-recycle"` cargo feature gate; in that case, the contract degenerates to "no recycling occurs during streaming, full stop." This is a valid one-liner closure of the G1 threat.

The G1 fundamental property — sequential-baseline equivalence — would otherwise be violated as follows: if a recycled slot ID is later assigned by a worker's `create_agent` call to a NEW agent, while a still-pending border wire from the streaming side references the OLD agent at that slot, the border-target identity becomes ambiguous and `merge` (SPEC-05) can wire two distinct logical agents together. SPEC-22 R10b protected tombstones prevent this by keeping border-referenced slot IDs out of the recycle queue until the next clean boundary (`reconstruct` under delta, or end-of-pipeline under non-delta). Cross-references: SPEC-22 R10b/R10c, §3.8 A6 (this spec). **(MUST)**

**R37c. (PROTOCOL_VERSION sequencing for R31 wire variants, closes SC-005 / SC-009-style.)** R31 introduces two new variants (`RequestWork`, `NoMoreWork`) on the `Message` enum (SPEC-06). Every `Message`-catalog addition is a wire-format change. SPEC-22 R9a / §3.8 A9 plans a PROTOCOL_VERSION bump (v2 → v3); SPEC-20 plans an independent bump (v3 → v4). SPEC-21 R31 is the third spec in the wave to touch the constant. SPEC-21's disposition is:

> **R37c.** SPEC-21 R31 wire additions MUST bump `PROTOCOL_VERSION` from `PREVIOUS_LIVE_VERSION` to `PREVIOUS_LIVE_VERSION + 1`, where `PREVIOUS_LIVE_VERSION` is whatever value SPEC-22 / SPEC-20 leave in the live constant at the moment the SPEC-21 R31 patch is merged. The patch MUST be authored using a `static_assert` on `PREVIOUS_LIVE_VERSION + 1` rather than a hardcoded integer — i.e., the source-of-truth is the relative bump, not an absolute version number — so that merge-order reshuffling between SPEC-20/21/22 does not silently produce wrong absolute numbers. Equivalent defensive language to SPEC-22 TASK-0476 / R9a: the bump MUST be expressed as `PREVIOUS_LIVE_VERSION + 1`; any patch that hardcodes `5` (or any other absolute integer) is a hard block at code-review.

The SPEC-22 R10b rejection clause (v_old deserializers MUST reject v_new payloads with `UnsupportedVersion`) MUST also apply to the SPEC-21 bump: pre-SPEC-21 binaries receiving `RequestWork` or `NoMoreWork` MUST reject with `UnsupportedVersion`, not silently misinterpret the variant tag. Cross-references: SPEC-06 R-NN (PROTOCOL_VERSION), SPEC-18 R28, SPEC-22 §3.8 A9, §3.8 A2 of this spec. **(MUST)**

**R37d. (BSP-barrier semantics under pull dispatch, closes SC-019.)** The BSP synchronization barrier (SPEC-05) under pull dispatch is the moment all workers acknowledge `NoMoreWork`. Before this moment, individual workers MAY complete reductions on their accumulated chunks and MAY emit `PartitionResult` messages, but MUST NOT begin the merge phase. Workers MUST wait for `NoMoreWork` before transitioning to the final-reduction state. This preserves G1 by reducing pull dispatch to a single "logical BSP round" regardless of wall-clock interleaving — the pull pattern shifts the timing of `AssignPartition` messages but does not introduce new barrier semantics relative to push mode. Cross-reference: SPEC-13 worker FSM (amended via §3.8 A5). **(MUST)**

**R37e. (Push-mode termination signaling, closes SC-013.)** In push mode (the default for `num_workers ≤ 2` under `DispatchMode::Auto`, R34), no `NoMoreWork` message is sent; the worker receives a single `AssignPartition` with the complete partition and proceeds to the standard merge protocol per SPEC-05. `NoMoreWork` is meaningful only in pull mode (R31). Worker implementations MUST NOT add defensive `NoMoreWork` handling to the push-mode FSM transition table; coordinator implementations MUST NOT emit `NoMoreWork` in push mode. The two modes share the variant in the wire format (so version sequencing in R37c is single-mode-agnostic) but the protocol is mode-specific. **(MUST)**

**R37f. (BorderGraph update under delta + chunked dispatch, closes SC-017 strict reading.)** Under combined delta protocol (SPEC-19) and streaming dispatch (this spec), the coordinator MUST call `BorderGraph::extend_with_chunk_borders(&new_borders)` (SPEC-19 §3.2) after each `install_connection` invocation that yields a border wire, **before** chunk N+1's `AssignPartition` is dispatched. Failure to do so means the coordinator's border-redex detection misses cross-chunk active pairs and the M5 milestone target ("ep_con 100M coordinator-side") is unreachable. This elevates R36 from SHOULD to MUST under the conjunction `delta_mode && streaming_active`; under either mode in isolation (delta-only or streaming-only) R36's SHOULD posture is preserved. SPEC-19 owns the `BorderGraph::extend_with_chunk_borders` signature; SPEC-21 owns the call-site discipline. **(MUST under delta+streaming; SHOULD otherwise)**

**R37g. (Pending-store memory bound, closes SC-016.)** Generator implementations MUST resolve any forward reference (a `ConnectionDirective::Pending` whose target agent has not yet been emitted) within at most `MAX_PENDING_LIFETIME` chunks (default: 16, configurable via `GridConfig.max_pending_lifetime`). The pipeline MAY enforce this as a `debug_assert!` that fires when `pending` retains an entry across more than `MAX_PENDING_LIFETIME` chunk boundaries. Generators that violate the bound MUST be either (a) refactored to emit forward-referenced agents earlier, or (b) explicitly excluded from streaming mode and forced to use the R10 default-impl materialization path. This bounds the pending-store peak memory at O(`MAX_PENDING_LIFETIME` × max_forward_refs_per_chunk), preventing the `dual_tree`-pathological growth identified in §4.7. The bound is configurable rather than a hard MUST so that benchmark calibration can tune it (the value 16 is a placeholder; AC-014 methodology applies). **(MUST for the bounding mechanism; SHOULD for the specific value)**

### 3.8 Amendments to Predecessor Specs (closes SC-001, SC-002)

SPEC-21 amends the following requirements of predecessor specs. The amendments are formal and MUST be cross-referenced in those specs' next revision; ESPECIALISTA EM SPECS owns the cross-reference patches. The structure of this block follows the canonical SPEC-22 §3.8 / SPEC-20 §3.8 four-field schema: each entry gives the target spec, the target requirement number, the old text (verbatim where possible), the new text, and a rationale.

**A1. SPEC-04 R12 amendment (border-id allocation for streaming pipeline).**
- *Old text (SPEC-04 R12):* Border IDs start at `max_existing_freeport_id(net) + 1` (single global scan of the full net produced before partitioning).
- *New text (SPEC-21 R29b):* Border IDs follow SPEC-04 R12 in the batch path (`split()`-based, when `chunk_size = u32::MAX`). In the streaming path (`generate_and_partition_chunked`), border IDs MUST start at 0 and increment monotonically when no Lafont FreePorts are present in any batch, OR at `max_lafont_freeport_id_in_first_batch + 1` when the first batch carries Lafont FreePorts. Generators that emit Lafont FreePorts SHOULD emit ALL of them in the first batch to simplify the discovery scan. The `Partition.border_id_start` and `Partition.border_id_end` (SPEC-04 R15a) MUST be set to the global range `[0, border_id_counter)` (or shifted by `max_lafont_freeport_id + 1` when Lafont FreePorts exist).
- *Rationale:* Closes SC-018. The streaming pipeline cannot perform a single global scan of "the full net" because the full net does not exist until the stream is exhausted. The first-batch scan is the streaming-equivalent of SPEC-04 R12's pre-partition scan. The two paths produce non-overlapping but distinct border-id ranges; tests that exercise both `split()` and `generate_and_partition_chunked` MUST account for this.

**A2. SPEC-06 R-NN amendment (`Message` enum gains two variants; PROTOCOL_VERSION bump).**
- *Old text (SPEC-06 `Message` enum):* `enum Message { Hello, Reduce(...), PartitionResult(...), AssignPartition(...), Goodbye, ... }` — pre-SPEC-21 variant set, no pull-dispatch protocol.
- *New text (SPEC-21 R31):* Add two variants:
  ```rust
  RequestWork { worker_id: WorkerId },
  NoMoreWork,
  ```
  `RequestWork` is sent by the worker to request a new chunk. `NoMoreWork` is sent by the coordinator when the generator stream is exhausted. Both variants serialize through SPEC-18 wire-format-v2 serde without modification to the framing layer (length-prefixed, bincode-encoded). The PROTOCOL_VERSION constant MUST be bumped per R37c (i.e., `PREVIOUS_LIVE_VERSION + 1`, defensive language; same pattern as SPEC-22 TASK-0476 / R9a). Pre-bump deserializers MUST reject post-bump payloads with `UnsupportedVersion`.
- *Rationale:* Closes SC-001 part 1. The variants are required by R30-R32 pull dispatch; the version bump is required to prevent silent variant-tag misinterpretation in mixed-version deployments.

**A3. SPEC-07 GridConfig amendment (three new configuration fields).**
- *Old text (SPEC-07 `GridConfig` struct):* Pre-SPEC-21 GridConfig has no streaming-related fields.
- *New text (SPEC-21 R24, R25, R34):* Add three fields:
  ```rust
  pub chunk_size: u32,                     // R24, default 10_000 (placeholder, see Q2)
  pub streaming_strategy: StreamingStrategyConfig, // R25, default RoundRobin
  pub dispatch_mode: DispatchMode,         // R34, default Auto
  ```
  with `StreamingStrategyConfig` and `DispatchMode` enums declared in SPEC-21 §4.x (and re-exported from `src/config.rs`). Optionally, a fourth field `pub max_pending_lifetime: u32` (R37g, default 16) MAY be added in the same patch.
- *Rationale:* Closes SC-001 part 2. Configurability of the streaming pipeline requires GridConfig surface; the patch is additive and does not break SPEC-07 R-N requirements.

**A4. SPEC-09 `Benchmark` trait amendment (default-impl-bearing addition; closes SC-008).**
- *Old text (SPEC-09 R2):* `pub trait Benchmark { ... fn make_net(&self, size: u32) -> Net; ... }` with no streaming method.
- *New text (SPEC-21 R10):* Add `fn make_net_stream(&self, size: u32, chunk_size: usize) -> Box<dyn Iterator<Item = AgentBatch>>` WITH a default implementation that wraps `self.make_net(size)` via the `default_chunked_iter` helper. The default-impl path materializes the net then slices it into chunks (memory-equivalent to v1; no benefit, but no break). All 13 existing SPEC-09 implementations remain valid without per-implementation edits. Generators that benefit from native streaming (`ep_annihilation` per R12 MUST; others SHOULD) override the default. Total Phase B effort estimate: ~30 LoC for the trait amendment + default-impl helper, plus per-generator overrides on opt-in basis.
- *Rationale:* Closes SC-008. The default-impl decision is the lower-friction choice and matches SPEC-09's posture of additive trait extensions. Without the default, the trait change would force ~520 LoC of mechanical implementation across 13 benchmarks even for those that derive no benefit from streaming.

**A5. SPEC-13 amendment (coordinator and worker FSM additions for pull dispatch).**
- *Old text (SPEC-13 coordinator/worker FSMs):* Pre-SPEC-21 FSMs cover the push-only dispatch protocol (a single `AssignPartition` per worker, then `PartitionResult`, then merge).
- *New text (SPEC-21 R30-R32 + R37d):* Coordinator FSM gains states:
  - `DispatchingFirst` (cold start, sending the first chunk to each worker)
  - `AwaitingResults` (waiting for `PartitionResult` from any worker)
  - `GeneratingNext` (calling `make_net_stream::next` then `strategy.allocate_batch`)
  - `SendingNoMoreWork` (when the stream is exhausted)
  - `AwaitingFinalResults` (awaiting all post-`NoMoreWork` results, then merge)
  Transitions:
  - `Init → DispatchingFirst`
  - `DispatchingFirst → AwaitingResults`
  - `AwaitingResults + RequestWork → GeneratingNext` (if stream not exhausted) or `SendingNoMoreWork` (if exhausted)
  - `GeneratingNext + chunk-ready → AwaitingResults` (after sending `AssignPartition`)
  - `SendingNoMoreWork + all-acks → AwaitingFinalResults`
  - `AwaitingFinalResults + all-results → Merge` (BSP barrier per R37d)

  Worker FSM gains states:
  - `AwaitingChunkAfterResult` (entered after sending `PartitionResult`, awaiting `AssignPartition` or `NoMoreWork`)
  - `FinalReduction` (entered upon receiving `NoMoreWork`)
  Transitions:
  - `ReducingChunk + chunk-done → AwaitingChunkAfterResult` (also emits `RequestWork`)
  - `AwaitingChunkAfterResult + AssignPartition → ReducingChunk`
  - `AwaitingChunkAfterResult + NoMoreWork → FinalReduction`
  - `FinalReduction + reduction-done → SendFinalResult → Done`

- *Rationale:* Closes SC-001 part 3 and SC-015. The push-mode FSMs are unchanged (R37e). The new states are pull-only and gated on `DispatchMode::Pull` in `GridConfig`. Without this amendment, R30-R32 prose narrative cannot be decomposed into FSM-level tasks during Stage 1 (TASK-SPLITTER).

**A6. SPEC-22 R10b/R10c interaction (free-list × streaming; closes SC-007).**
- *Old text (SPEC-22 R10b):* Free-list recycling protected-tombstone discipline applies "when `GridConfig.delta_mode == true`" (delta-mode-only scope).
- *New text (SPEC-21 R37b):* The protected-tombstone discipline ALSO applies during streaming pipeline execution, regardless of `delta_mode`. The trigger condition becomes `(delta_mode || streaming_active) && id ∈ border_referenced_set`. SPEC-22 R10b's two strategies (`DisableUnderDelta` / `BorderClean`) are renamed conceptually to `DisableUnderBorderTracking` / `BorderClean` in implementation, but the wire-level enum name `RecyclePolicy::DisableUnderDelta` is preserved for backward compatibility (the field name is misleading post-SPEC-21 but stable).
- *Rationale:* Closes SC-007. SPEC-22 was authored before SPEC-21's chunked dispatch was fully scoped; the R10b discipline naturally extends to any context that maintains a coordinator-side border map, of which streaming is the second instance. Alternatively, an implementation MAY use a cargo feature gate `streaming-no-recycle` that disables the worker free-list outright during streaming — that satisfies R37b trivially without requiring SPEC-22 amendments.

**A7. SPEC-19 `BorderGraph` extension (closes SC-017 under delta+streaming).**
- *Old text (SPEC-19 §3.2):* `BorderGraph` is constructed once from the initial `PartitionPlan`'s `borders` map. No incremental-extension API.
- *New text (SPEC-21 R37f):* Add `pub fn extend_with_chunk_borders(&mut self, new_borders: &HashMap<u32, (PortRef, PortRef)>)` that merges new border entries into the existing `BorderGraph`. The method MUST be called by the coordinator after each `install_connection` invocation that yields a border wire under the conjunction `delta_mode && streaming_active`. The method is idempotent on previously-seen border IDs and is a no-op if `new_borders.is_empty()`. SPEC-19 owns the implementation; SPEC-21 owns the call-site discipline.
- *Rationale:* Closes SC-017. Without this extension API, the coordinator's `BorderGraph` becomes stale after chunk 1 under combined delta+streaming, missing cross-chunk active pairs and silently violating G1.

**A8. SPEC-04 §4.5 clarification (split() unchanged; chunked pipeline additive).**
- *Old text:* SPEC-04 §4.5 documents `split()` as the canonical partition entry point.
- *New text:* SPEC-04 `split()` is UNCHANGED. The chunked pipeline (SPEC-21 §3.3 R17 `generate_and_partition_chunked`) is an ALTERNATIVE entry point selected by `GridConfig.chunk_size != u32::MAX`. The two paths produce structurally compatible output (`PartitionPlan` from `split()`, `ChunkedPartitionResult.partitions + .borders` from streaming, where `ChunkedPartitionResult` is convertible to `PartitionPlan` per R20-R21). `split()` is the fallback for the v1 backward-compat path (R26).
- *Rationale:* Closes SC-001 part 4. Documents the additive nature explicitly so that downstream readers of SPEC-04 are not surprised by SPEC-21's existence.

---

## 4. Design

### 4.1 Types

**Type origins (closes SC-022, SC-023).** `WorkerId` is the worker identifier type defined in SPEC-04 (newtype `pub struct WorkerId(pub u32)` per CLAUDE.md "Newtype pattern for IDs"); SPEC-21 imports it from `crate::partition::WorkerId` and does not redeclare it. `PortId` is defined in SPEC-02 as `u8` (or `u32` per the live code) with values bounded to `0..=2` corresponding to the principal port (0) and at most two auxiliary ports (1, 2) per the agent's symbol arity (ERA: 0 aux; CON/DUP: 2 aux). `AgentId` is the SPEC-02 `u32` newtype. `Symbol` is the SPEC-02 enum {ERA, CON, DUP}.

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
            chunks_processed: 0,  // PIPELINE-OWNED FIELD: stitched by
                                  // generate_and_partition_chunked before return.
                                  // See §4.6 pseudocode "Stitch chunks_processed"
                                  // step (closes SC-021).
        }
    }
}
```

**Note on `chunks_processed` ownership (closes SC-021).** The strategy returns `chunks_processed: 0` as a placeholder; the pipeline owns the field and stitches the actual count into the returned `ChunkedPartitionResult.stats` before returning to the caller. The pipeline maintains a local `chunks_seen: u64` counter incremented on each iteration and assigns `result.stats.chunks_processed = chunks_seen` after `strategy.finalize()`. T1 (§7.1) MUST verify `result.stats.chunks_processed == ceil(total_agents / chunk_size)` rather than the strategy-returned value.

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
/// - Tsourakakis et al., KDD 2014 (FENNEL) — REF-TBD (TCC-root cleanup;
///   not yet registered in `docs/theory-bridge.md`; see §11 Change Log)
/// - Stanton & Kliot, KDD 2012 (LDG)        — REF-TBD (same)
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

The on-the-fly border detection in `install_connection` (below) follows the AC-007 (HVM2 Reduction Engine) pattern: detect cross-partition pairs at the moment of connection, not in a separate pass. AC-007's atomic-link-with-ownership discipline maps to the streaming pipeline's per-chunk install loop — `agent_owner` is the streaming analog of HVM2's per-thread ownership mask.

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

    // Pipeline-owned chunk counter (closes SC-021)
    chunks_seen: u64 = 0

    // Process each chunk
    for batch in stream:
        chunks_seen += 1
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

    // Step 7: Stitch chunks_processed into stats (closes SC-021)
    let mut stats = strategy.finalize()
    stats.chunks_processed = chunks_seen

    return ChunkedPartitionResult { partitions, borders: border_map, stats }


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

### 4.9 Partition Accumulator Design (SparseNet adoption per SPEC-22 R22; closes SC-006)

Each `PartitionAccumulator` wraps EITHER a `SparseNet` (SPEC-22 R22 / §4.4) OR a dense `Net` (SPEC-02), selected at construction time using the same `id_range > 4 × live_agent_count` threshold rule that SPEC-22 R10a/R22 mandates for `build_subnet`. SparseNet is the default for accumulators that may receive non-contiguous agent IDs (e.g., FENNEL strategy assigns agent 0 and agent 5_000_000 to the same worker, owning ~1000 agents total — the dense layout would inflate to 5_000_001 × 3 = 15M `PortRef` entries to hold ~3000 live ports). The accumulator finalizes to a dense `Net` via `to_dense(Some(id_range))` (SPEC-22 §4.6 signature) only at the end of the pipeline, before `Partition` construction.

The §4.9 design adopts SPEC-22 SparseNet rather than recreating the M5 dense-arena pathology (closes SC-006). The `freeport_redirects` field is preserved across the SparseNet→Net conversion per SPEC-22 R13/§4.4. The frame-reuse pattern of accumulator construction (one persistent SparseNet per worker, mutated chunk-by-chunk) follows AC-010 (HVM4 WNF Evaluation) — the WNF goto-state-machine reuses frames across reduction steps; the accumulator analogously reuses one SparseNet across chunks rather than reallocating per chunk.

```rust
use crate::net::{Net, PortRef, AgentId, PortId, Symbol, PORTS_PER_SLOT};
use crate::net::sparse::SparseNet; // SPEC-22 R22 / §4.4
use crate::partition::WorkerId;
use std::collections::HashMap;

/// Internal representation of the per-worker accumulator.
/// SparseNet is the default; Dense is used only when the threshold check
/// at construction time confirms that id_range <= 4 * expected_live_count.
enum AccumulatorNet {
    Sparse(SparseNet),
    Dense(Net),
}

/// Accumulates agents and connections for a single worker
/// during chunked generation.
struct PartitionAccumulator {
    /// The sub-net being built incrementally. Backed by SparseNet
    /// (SPEC-22 R22) when the assignment strategy may produce
    /// non-contiguous IDs; otherwise dense Net.
    subnet: AccumulatorNet,
    /// Reverse index of boundary FreePorts.
    free_port_index: HashMap<u32, PortRef>,
    /// Worker ID.
    worker_id: WorkerId,
    /// Tracked for the final id_range computation and the
    /// id_range > 4 * live_agent_count threshold check at finalize().
    min_assigned_id: Option<AgentId>,
    max_assigned_id: Option<AgentId>,
    live_agent_count: u64,
}

impl PartitionAccumulator {
    /// Construct a fresh accumulator. Defaults to SparseNet because
    /// the streaming pipeline does not know up-front whether the
    /// assignment strategy will produce contiguous or non-contiguous IDs.
    /// The dense form is reachable only via `finalize()` post-conversion.
    fn new(worker_id: WorkerId) -> Self {
        Self {
            subnet: AccumulatorNet::Sparse(SparseNet::new()),
            free_port_index: HashMap::new(),
            worker_id,
            min_assigned_id: None,
            max_assigned_id: None,
            live_agent_count: 0,
        }
    }

    fn add_agent(&mut self, id: AgentId, symbol: Symbol) {
        // SparseNet: O(1) HashMap insertion, no port-array resize.
        // Dense: insert at position id, expanding Vecs if needed.
        match &mut self.subnet {
            AccumulatorNet::Sparse(s) => s.create_agent_at(id, symbol),
            AccumulatorNet::Dense(n) => n.create_agent_at(id, symbol),
        }
        self.min_assigned_id = Some(self.min_assigned_id.map_or(id, |m| m.min(id)));
        self.max_assigned_id = Some(self.max_assigned_id.map_or(id, |m| m.max(id)));
        self.live_agent_count += 1;
    }

    fn connect(&mut self, a: PortRef, b: PortRef) {
        // If b is FreePort(bid): update free_port_index[bid] = a.
        // If a is FreePort(bid): update free_port_index[bid] = b.
        // Delegate to subnet.connect(a, b) for port array / port HashMap update.
        if let PortRef::FreePort(bid) = b { self.free_port_index.insert(bid, a); }
        if let PortRef::FreePort(bid) = a { self.free_port_index.insert(bid, b); }
        match &mut self.subnet {
            AccumulatorNet::Sparse(s) => s.connect(a, b),
            AccumulatorNet::Dense(n) => n.connect(a, b),
        }
    }

    fn finalize(self, id_range: IdRange, border_id_start: u32,
                border_id_end: u32) -> Partition {
        // Convert to dense Net for downstream merge consumption.
        // SPEC-22 §4.6 to_dense(id_range) is the canonical conversion;
        // it preserves freeport_redirects (SPEC-22 R13).
        let dense = match self.subnet {
            AccumulatorNet::Sparse(s) => s.to_dense(Some(id_range.as_range())),
            AccumulatorNet::Dense(n) => n,
        };
        Partition {
            subnet: dense,
            worker_id: self.worker_id,
            free_port_index: self.free_port_index,
            id_range,
            border_id_start,
            border_id_end,
        }
    }
}
```

**Key properties (post-SparseNet adoption):**
- *In-progress accumulator memory:* O(live_agent_count) regardless of `id_range`. The dense-arena inflation (`id_range × PORTS_PER_SLOT` PortRef entries for sparse assignments) is eliminated.
- *Finalization:* `to_dense(Some(id_range))` produces a `Net` whose `agents.len() == id_range.end - id_range.start` and whose port array is sized to `(id_range.end - id_range.start) × PORTS_PER_SLOT` — sized to the partition's owning ID range, not the global `max_agent_id`. This is the same sparse-final layout as SPEC-04 §4.5 Step 5, but built via SparseNet incrementally.
- *Threshold contract:* The streaming pipeline MUST follow SPEC-22 R10a/R22: when `id_range > 4 × live_agent_count` at finalize-time, SparseNet is mandatory through the conversion; the dense path SHALL be rejected with `PartitionError::DenseAllocationExceedsThreshold` (SPEC-22 R30). T10 (§7.4) MUST exercise this path.
- *R23 reconciliation:* R23's "MUST be sized to `max_agent_id_in_this_worker + 1`" applies to the dense-finalized form, NOT the in-progress SparseNet accumulator. Implementers MUST NOT pre-size a dense Vec at construction time hoping to "amortize" the resize — that resurrects SC-006.

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

**REF-015 streaming-level reconciliation (closes SC-004).** REF-015 (Mackie & Sato 2015) establishes streaming at the **rule-level** — DISC-009 v2's level 1, the per-fired-rule trace level (cited by `docs/theory-bridge.md` L170 as the SPEC-14 §8 anchor). SPEC-21 covers the **generation-protocol** level — DISC-009 v2's level 3, the highest tier of the streaming taxonomy. The two levels are NOT the same construct: rule-level streaming concerns the trace produced by reduction; generation-protocol streaming concerns how the input net is materialized in the first place. SPEC-21 generalizes the streaming concept to level 3 per DISC-009 v2; REF-015 supplies the level-1 precedent (concept of incremental emission), but does not establish anything specific about the pipeline architecture defined in this spec. The DISC-009 v2 taxonomy is the primary anchor (frontmatter Discussions consumed); REF-015 is retained as the closest published precedent at the next-lower streaming level.

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

### 7.5 Lazy/Demand-Driven Generation Tests (Amendment)

**T11. Pull-based dispatch protocol.**
- Run `run_grid` with `DispatchMode::Pull` for `ep_annihilation_con(100)`, K=2.
- Verify workers send `RequestWork` and receive chunks.
- Verify G1: result matches sequential baseline.

**T12. Pull vs. push equivalence.**
- For `ep_annihilation_con(100)`, K=4, chunk_size=20:
  Run with `DispatchMode::Push` and `DispatchMode::Pull`.
  Verify merged results are isomorphic and interaction counts are identical.

**T13. Short stream (fewer chunks than workers).**
- For `ep_annihilation(10)`, K=4, chunk_size=100:
  Only 1 chunk total (20 agents < 100). Verify only 1 worker receives work, others get `NoMoreWork`.
  Verify result is correct.

**T14. Heterogeneous worker simulation.**
- Simulate 4 workers where worker 0 reduces 2x faster (by using smaller chunks or injecting artificial delay).
  Run with `DispatchMode::Pull`. Verify worker 0 processes more chunks than worker 3.
  Verify result correctness (all workers' contributions merge correctly).

---

## 8. Open Questions

**Q1. Accumulator memory for large nets.** *(PARTIALLY RESOLVED via SPEC-22 SparseNet adoption in §4.9; see Round 2 closure of SC-006.)* The partition accumulators no longer suffer the dense-arena inflation pathology under non-contiguous assignment (e.g., FENNEL): SparseNet bounds in-progress accumulator memory to O(live_agent_count) regardless of `id_range`. The residual O(N/K) growth (one slot per live agent across all workers) remains and is addressed by early dispatch (ROADMAP 2.19) or recipe-based generation (SPEC-25), both out of scope for SPEC-21.

**Q2. Optimal chunk size.** *(Acknowledged; deferred to benchmark calibration per R24 closing clause and AC-014 methodology.)* The default of 10,000 agents is a placeholder. Benchmarking is needed to determine the optimal chunk size for different generator types and network topologies. Smaller chunks mean more overhead (per-chunk strategy calls, pending connection lookups); larger chunks mean less streaming benefit. R24 mandates that the default MUST be re-evaluated and either confirmed or replaced before v2 release (closes SC-024).

**Q3. FennelStreamingStrategy parameter tuning.** The `alpha` parameter in the FENNEL scoring function affects the load-balance vs. edge-cut trade-off. The optimal value depends on the net topology and the number of workers. Empirical calibration is needed. The literature suggests `alpha = sqrt(K) * |E| / |V|^2` (Tsourakakis et al., 2014) but this requires knowing total edges/vertices upfront, which contradicts the streaming model. Resolution path: SPEC-21 adopts a fixed default `alpha = 1.0` per R5; per-benchmark calibration via AC-014 methodology is a separate task (NOT a Stage-1 blocker). Adaptive alpha adjusting as batches arrive is documented as future work; if calibration shows fixed `alpha = 1.0` is materially worse than batch FENNEL on representative benchmarks, FENNEL drops to FUTURE scope and only RoundRobin remains in v2.

**Q4. Interaction with delta protocol (SPEC-19).** *(RESOLVED via R37b + R37f + §3.8 A6/A7; see Round 2 closure of SC-007 and SC-017.)* R37b mandates SPEC-22 R10b protected-tombstone discipline whenever streaming is active (regardless of delta mode); R37f elevates R36 from SHOULD to MUST under the conjunction `delta_mode && streaming_active` and mandates the `BorderGraph::extend_with_chunk_borders` call discipline. The combined streaming + delta mode is implementable as the M5 target combination.

**Q5. Root port handling in streaming.** SPEC-04 R28 requires the root port to be propagated to the partition containing the root agent. In the streaming pipeline, the root agent may appear in any batch. The pipeline MUST defer root assignment until the batch containing the root agent is processed. If the generator does not specify a root, or if root is determined post-generation, this is a non-issue. *(Acknowledged as a debug-assertion edge case; the C1-C3 assertions in R28 operate on the finalized `Vec<Partition>` after the last chunk per the pipeline pseudocode §4.6 Step 5, so they cannot fire prematurely on a missing-root mid-stream.)*

**Q6. Port array sizing in accumulators.** *(RESOLVED via SPEC-22 SparseNet adoption in §4.9; see Round 2 closure of SC-006.)* The accumulator now uses SparseNet by default and converts to dense `Net` only at finalize-time via `to_dense(Some(id_range))` (SPEC-22 §4.6). Under FENNEL's non-contiguous assignment, the in-progress representation is HashMap-based (SparseNet's `agents: HashMap<AgentId, Agent>`); the dense conversion at finalize is sized to `id_range.end - id_range.start`, NOT to `max_agent_id + 1`. The dense path SHALL be rejected with `PartitionError::DenseAllocationExceedsThreshold` (SPEC-22 R30) if `id_range > 4 × live_agent_count` at finalize-time.

---

## 11. Change Log

### Round 2 — 2026-04-25 — Closure pass

Closure of all 24 findings from `SPEC-REVIEW-21-round-1-2026-04-25.md` (verdict BLOCK; 2 CRITICAL, 7 HIGH, 10 MEDIUM, 5 LOW).

| Finding | Severity | Verdict | Where addressed |
|---------|----------|---------|-----------------|
| SC-001 | CRITICAL | CLOSED | §3.8 Amendments to Predecessor Specs block authored, populated with A1-A8 (8 amendments) following the SPEC-22 §3.8 / SPEC-20 §3.8 canonical four-field schema (target / R-number / Old text / New text / Rationale). Covers SPEC-04 R12 (A1, A8), SPEC-06 Message enum + PROTOCOL_VERSION (A2), SPEC-07 GridConfig (A3), SPEC-09 Benchmark trait (A4), SPEC-13 coordinator/worker FSMs (A5), SPEC-22 R10b/R10c (A6), SPEC-19 BorderGraph (A7). |
| SC-002 | CRITICAL | CLOSED | Frontmatter `Depends on:` extended from 5 specs (SPEC-01/02/04/05/13) to 12 (added SPEC-06, SPEC-07, SPEC-09, SPEC-17, SPEC-18, SPEC-19, SPEC-22) with parenthetical justifications inline. |
| SC-003 | HIGH | CLOSED | Frontmatter `Discussions consumed:` line added with `DISC-009 v2` listed as the primary taxonomy anchor; §1 carries a one-paragraph cross-reference placing SPEC-21 at DISC-009 v2's level-3 generation-protocol streaming layer. §3.6 prose cites DISC-009 v2 in justifying the bundling decision. |
| SC-004 | HIGH | CLOSED | §5.2 carries the REF-015 streaming-level reconciliation paragraph: REF-015 establishes streaming at level 1 (rule-level), SPEC-21 generalizes to level 3 per DISC-009 v2; REF-015 is retained as the closest published precedent at the next-lower streaming level. Frontmatter REF-015 entry annotated to point at §5.2. |
| SC-005 | HIGH | CLOSED | Same surface as SC-002 (predecessors-missing); fully reconciled in Round 2 frontmatter. The duplicated finding-id is intentional in the Round 1 review (it overlaps SC-002); no separate edit is required beyond the frontmatter extension. |
| SC-006 | HIGH | CLOSED | §4.9 PartitionAccumulator redesigned around SPEC-22 SparseNet (R22): defaults to SparseNet at construction, finalizes to dense `Net` via `to_dense(Some(id_range))` only at pipeline end. The `id_range > 4 × live_agent_count` threshold is enforced at finalize-time per SPEC-22 R10a/R22; dense path rejected with `PartitionError::DenseAllocationExceedsThreshold` (SPEC-22 R30). §3.8 A6 records the SPEC-22 interaction. The §4.9 introductory paragraph explicitly states SC-006 is closed by this design. |
| SC-007 | HIGH | CLOSED | §3.7 R37b authored: streaming pipeline MUST honor SPEC-22 R10b/R10c protected-tombstone discipline for any AgentId in `border_map` or pending store, regardless of `delta_mode`. Two strategies (`DisableUnderDelta` / `BorderClean`) preserved; alternative cargo-feature-gate `streaming-no-recycle` documented as a valid one-liner closure. §3.8 A6 records the SPEC-22 R10b conditional broadening. |
| SC-008 | HIGH | CLOSED | R10 amended to specify a default implementation (`default_chunked_iter` wrapping `self.make_net(size)`); all 13 SPEC-09 implementations remain valid without per-implementation edits. R11 reframed as the source-of-truth materialization path. §3.8 A4 records the SPEC-09 amendment with explicit Phase B effort estimate (~30 LoC trait amendment + per-generator overrides on opt-in basis vs ~520 LoC mechanical implementation). |
| SC-009 | HIGH | CLOSED | R15 amended to explicitly note the I3'/R15 reconciliation: R15 is a generator-phase contract strictly stronger than the post-SPEC-22 SPEC-01 I3'. Closing note at end of §3.5 documents the formal reconciliation; `src/partition/streaming.rs` MUST NOT assume monotonicity post-dispatch. R27 I3 clause replaced with I3' clause. §3.7 R37c carries the PROTOCOL_VERSION sequencing decision (defensive `PREVIOUS_LIVE_VERSION + 1` language mirroring SPEC-22 TASK-0476 / R9a). §3.8 A2 records the PROTOCOL_VERSION amendment. |
| SC-010 | MEDIUM | CLOSED | Frontmatter `Discussions consumed:` line added (covers both DISC-004 v2 and DISC-009 v2 per SC-003). |
| SC-011 | MEDIUM | CLOSED | Frontmatter `Code analyses consumed:` line added with AC-007 (HVM2 Reduction Engine, informs §4.6 install_connection border detection), AC-010 (HVM4 WNF Evaluation, informs §4.9 PartitionAccumulator frame-reuse pattern), AC-014 (Bench Methodology, canonical reference for §7.4 T10 peak-memory measurement). §4.6 introductory paragraph cites AC-007; §4.9 introductory paragraph cites AC-010; R37 cites AC-014 for the throughput methodology. |
| SC-012 | MEDIUM | CLOSED | R16 closing sentence added: the iterator's pull-based `Iterator::next` interface naturally supports the pull dispatch model (R32) without async coordination; async channels (`tokio::sync::mpsc`) are required only for the push dispatch model when generation and dispatch overlap in time. |
| SC-013 | MEDIUM | CLOSED | §3.7 R37e authored: in push mode, no `NoMoreWork` is sent; the worker receives a single `AssignPartition` per SPEC-05 merge protocol. `NoMoreWork` is meaningful only in pull mode. Worker FSM and coordinator FSM MUST NOT cross-pollute the variant. |
| SC-014 | MEDIUM | CLOSED | R26 reworded to specify isomorphism (SPEC-00 §6.12 `nets_isomorphic`), not bit-identity, as the v1 backward-compat guarantee. The reword explicitly notes that bit-identical layout is NOT guaranteed because of SPEC-22 arena-management amendments. |
| SC-015 | MEDIUM | CLOSED | §3.8 A5 authored: SPEC-13 coordinator FSM gains 5 new states (`DispatchingFirst`, `AwaitingResults`, `GeneratingNext`, `SendingNoMoreWork`, `AwaitingFinalResults`) with explicit transition tuples; worker FSM gains 2 new states (`AwaitingChunkAfterResult`, `FinalReduction`) with explicit transition tuples. The push-mode FSMs are unchanged; pull-only states gated on `DispatchMode::Pull`. |
| SC-016 | MEDIUM | CLOSED | §3.7 R37g authored: pending-store memory bound `MAX_PENDING_LIFETIME` (default 16, configurable via `GridConfig.max_pending_lifetime`); generators violating the bound MUST be either refactored or excluded from streaming mode. Bounds pending-store peak memory at O(`MAX_PENDING_LIFETIME` × max_forward_refs_per_chunk). |
| SC-017 | MEDIUM | CLOSED | §3.7 R37f authored: under the conjunction `delta_mode && streaming_active`, R36 elevates from SHOULD to MUST and the coordinator MUST call `BorderGraph::extend_with_chunk_borders(&new_borders)` after each `install_connection` invocation that yields a border wire, before chunk N+1's `AssignPartition` is dispatched. §3.8 A7 records the SPEC-19 amendment with the new method signature. |
| SC-018 | MEDIUM | CLOSED | §3.5 R29b promotes §4.8's allocation policy to a numbered requirement; §3.8 A1 records the SPEC-04 R12 amendment with verbatim Old/New text. |
| SC-019 | MEDIUM | CLOSED | §3.7 R37d authored: the BSP barrier under pull dispatch is the moment all workers acknowledge `NoMoreWork`. Workers MAY complete reductions and emit `PartitionResult` messages individually but MUST NOT begin merge until `NoMoreWork` arrives. This preserves G1 by reducing pull dispatch to a single logical BSP round. |
| SC-020 | LOW | DEFERRED (TCC-root cleanup, acknowledged) | Tsourakakis 2014 (FENNEL) and Stanton & Kliot 2012 (LDG) are cited inline in §4.4 but absent from `biblioteca/referencias.bib` and `docs/theory-bridge.md`. Per the Round 2 prompt, theory-bridge.md is TCC-root territory and out of scope for SPEC-21 author. The FennelStreamingStrategy doc-comment now annotates these as "REF-TBD (TCC-root cleanup; not yet registered in `docs/theory-bridge.md`; see §11 Change Log)" so the obligation is auditable. The bibliography registration will be picked up by the BIBLIOTECARIO agent at the next theory-bridge maintenance pass. Same scope-handling pattern as SPEC-22 SC-013. |
| SC-021 | LOW | CLOSED | §4.3 `RoundRobinStreamingStrategy::finalize()` returns `chunks_processed: 0` with explicit comment that the field is pipeline-owned. §4.6 pipeline pseudocode adds a `chunks_seen: u64` counter incremented per iteration; Step 7 of the pseudocode stitches `result.stats.chunks_processed = chunks_seen` after `strategy.finalize()`. T1 in §7.1 verifies the pipeline-stitched count, not the strategy-returned value. A note paragraph between §4.3 and §4.4 documents the ownership convention. |
| SC-022 | LOW | CLOSED | §4.1 opening "Type origins" paragraph documents `WorkerId` origin (newtype `pub struct WorkerId(pub u32)` defined in SPEC-04, imported by SPEC-21, not redeclared). |
| SC-023 | LOW | CLOSED | Same §4.1 opening "Type origins" paragraph documents `PortId` origin (defined in SPEC-02, values bounded to `0..=2`: principal port 0, at most two auxiliary ports 1/2 per agent's symbol arity). |
| SC-024 | LOW | CLOSED | R24 default-clause reworded: "The default value SHOULD be 10,000 agents pending benchmark calibration (Q2). The default MUST be re-evaluated and either confirmed or replaced before v2 release." Tags the placeholder as benchmark-TBD so it does not survive into v2 release without empirical justification. |

**Status transition:** Draft → Reviewed v2.

**Closure verdict:** All CRITICAL (2/2) and all HIGH (7/7) findings CLOSED inline. All 10 MEDIUM CLOSED inline. 4/5 LOW CLOSED inline; 1 LOW (SC-020) DEFERRED with explicit gating (it is downstream theory-bridge.md cleanup, NOT SPEC-21 author scope per the Round 2 prompt). 0 NOT_CLOSED. No new fresh findings (NF-NNN) were introduced by the revision; the closure log audits this in the per-finding "Where addressed" column.

**Notable scope additions in this round:**
- §1 — DISC-009 v2 taxonomy positioning paragraph added (closes SC-003).
- §3.5 R15 — explicit I3'/R15 reconciliation note (closes SC-009); R27 I3 clause replaced with I3'; closing-note paragraph added at end of §3.5.
- §3.5 R29b — border-id allocation policy promoted to numbered requirement (closes SC-018).
- §3.7 Cross-cutting MUSTs section authored: R37b (G1 free-list interaction, closes SC-007), R37c (PROTOCOL_VERSION sequencing, closes SC-005/SC-009-style), R37d (BSP-barrier under pull dispatch, closes SC-019), R37e (push-mode termination, closes SC-013), R37f (BorderGraph update under delta+streaming, closes SC-017), R37g (pending-store memory bound, closes SC-016).
- §3.8 Amendments to Predecessor Specs section authored, populated with A1-A8 (8 amendments) following the SPEC-22 §3.8 canonical four-field schema (closes SC-001).
- §4.1 Type origins paragraph added (closes SC-022, SC-023).
- §4.3 `chunks_processed` ownership note (closes SC-021).
- §4.6 install_connection AC-007 cross-reference (closes SC-011 partial); pipeline pseudocode `chunks_seen` Step 7 (closes SC-021).
- §4.9 PartitionAccumulator redesigned around SPEC-22 SparseNet adoption (closes SC-006); AC-010 cross-reference; `AccumulatorNet { Sparse(SparseNet), Dense(Net) }` enum; finalize via `to_dense(Some(id_range))`.
- §5.2 REF-015 streaming-level reconciliation paragraph (closes SC-004).
- §8 Q1 / Q4 / Q6 marked RESOLVED with cross-references; Q3 carries explicit fixed-default disposition; Q5 acknowledged with non-issue clause.

**TCC-root cleanup items (acknowledged but out-of-scope for SPEC-21 author):**
- SC-020 — FENNEL (Tsourakakis 2014) and LDG (Stanton & Kliot 2012) absent from `biblioteca/referencias.bib` / `docs/theory-bridge.md`. Same handling pattern as SPEC-22 SC-013. The FennelStreamingStrategy doc-comment annotates these as REF-TBD pending BIBLIOTECARIO registration.

**Stage gate:** This closure log lands BEFORE Stage 1 (TASK-SPLITTER) and Stage 2 (TEST-GENERATOR) per the SDD pipeline contract. Stage-1 entry conditions per the Round 1 review §"Round 2 entry conditions" are satisfied:
1. §3.8 Amendments block authored with 8 entries (>= 6 required).
2. Frontmatter `Depends on:` includes SPEC-06, 07, 09, 17, 18, 19, 22.
3. Frontmatter `Discussions consumed:` includes DISC-009 v2 and DISC-004 v2.
4. PartitionAccumulator (§4.9) cross-references SPEC-22 SparseNet selection.
5. G1 free-list interaction explicitly addressed (R37b + §3.8 A6).
6. PROTOCOL_VERSION disposition declared with sequencing decision coordinated with SPEC-22 / SPEC-20 (R37c defensive language).
7. R10 Benchmark trait amendment specifies default-impl decision (§3.8 A4).
