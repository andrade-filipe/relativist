# Roadmap

This document lists features explicitly excluded from v1 (SPEC-13, R49-R50) and future architectural evolutions. Items marked with **(confluence-enabled)** are made possible specifically by the strong confluence property of Interaction Combinators -- they would be incorrect or require complex consensus in systems without this guarantee.

---

## v1 Scope (TCC)

v1 implements the minimum architecture that validates the TCC hypothesis: a single coordinator with K static workers in a star topology, using BSP (Bulk Synchronous Parallel) with barrier synchronization. This architecture is deliberately simple because the research question is about **correctness of distributed IC reduction**, not about grid infrastructure sophistication.

---

## v2 — Elastic Grid Architecture **(confluence-enabled)**

The central insight: strong confluence (Lafont 1997, Property P1 in ARG-001) guarantees that the result of reduction is identical regardless of **who** reduces **what** and in **what order**. This means work can be freely redistributed at any point without affecting correctness. v1 does not exploit this; v2 would.

### 2.1 Coordinator as Worker (hybrid node)

**v1 limitation:** The coordinator only orchestrates (partition, dispatch, collect, merge). It does not reduce. If only 1 machine is available, it waits for workers and does nothing.

**v2 change:** The coordinator keeps one partition for itself and reduces it locally while workers reduce theirs. When all workers return, the coordinator merges all partitions including its own.

**Why confluence makes this safe:** The coordinator's local reduction and the workers' reductions are independent. Strong confluence guarantees the merged result is identical to sequential `reduce_all` on the full net, regardless of which node reduced which partition.

**Complexity:** Low. The coordinator's BSP loop adds one local `reduce_all` call in parallel with the collect phase. No protocol changes needed -- the coordinator simply doesn't dispatch one partition and reduces it itself.

**Impact:** A single machine can start useful work immediately. With K machines total, the effective parallelism is K (not K-1 as in v1).

### 2.2 Dynamic Worker Joining **(confluence-enabled)**

**v1 limitation:** Worker count is fixed at startup (SPEC-06, R24). The coordinator waits for exactly K workers, then starts. No workers can join or leave during execution.

**v2 change:** Workers can join between BSP rounds (at the barrier synchronization point). When a new worker connects:

1. The current round completes normally with the existing workers.
2. At the next barrier, the coordinator accepts the new worker.
3. The net is re-partitioned with K+1 workers for the next round.

**Why confluence makes this safe:** Between rounds, the coordinator holds the complete merged net. Re-partitioning this net for K+1 workers is exactly the same operation as the initial partition -- strong confluence guarantees that reducing K+1 partitions produces the same result as reducing K partitions. The net's reduction history is irrelevant; only its current state matters.

**Practical scenario:** A user launches a large reduction on their local machine (coordinator + self-worker per 2.1). Later, 7 more machines become available and join. The work is automatically redistributed without restarting.

**Complexity:** Medium. Requires:
- New coordinator FSM state: `AcceptingWorkers` between rounds.
- Dynamic partition count in `split()`.
- Worker registration protocol that works mid-execution (not just at startup).
- Graceful handling of the K=0 case (coordinator reducing alone until someone joins).

Does NOT require:
- Consensus (single coordinator decides).
- State transfer to new workers (they receive a fresh partition like any other worker).
- Changes to the wire protocol messages (same `AssignPartition` / `PartitionResult`).

### 2.3 Dynamic Worker Departure **(confluence-enabled)**

**v1 limitation:** If a worker disconnects, the execution fails. No fault tolerance (OBJETIVO_TCC.md: out of scope).

**v2 change:** If a worker disconnects mid-round, the coordinator can:
1. Wait for timeout.
2. Reclaim the lost partition (the coordinator sent a copy, so it still has it).
3. Re-partition the lost work among remaining workers in the next round.

**Why confluence makes this safe:** The lost worker may have partially reduced its partition, but since the coordinator retains the original, it can simply re-dispatch the unreduced partition. Strong confluence guarantees that re-reducing from the original state produces the correct result. No rollback protocol needed.

**Complexity:** Medium-High. Adds timeout handling, partition retention, and re-dispatch logic. The "retain original partition" strategy trades memory for simplicity.

### 2.4 Distributed Partitioning and Merge **(confluence-enabled)**

**v1 limitation:** The coordinator is the sole point of aggregation (SPEC-13, R35). ALL partitions return to the coordinator for merge. For very large nets, this is a bottleneck: O(A) data flows through a single node every round.

**v2 change:** Hierarchical coordination. Instead of a flat star topology:

```
v1 (star):           v2 (tree):
    C                     C
   /|\                   / \
  W W W               SC    SC
                      / \   / \
                     W   W W   W
```

Sub-coordinators (SC) merge their local group's partitions and send the partial result up. The root coordinator merges the sub-results.

**Why confluence makes this safe:** Merge is associative under strong confluence. `merge(merge(P1, P2), merge(P3, P4))` produces the same net as `merge(P1, P2, P3, P4)`. The order and grouping of merges does not affect the final result.

**Complexity:** High. Requires:
- Tree topology construction and management.
- Sub-coordinator role (new node type or hybrid mode).
- Distributed border resolution (border redexes between sub-groups must be resolved at the appropriate tree level).
- Distributed termination detection (the root must know when ALL sub-trees have reached normal form).
- O(n^2) potential border interactions in the worst case (SPEC-05, Section 5.1: peer-to-peer assessment).

**Assessment:** This is a research project in its own right. The TCC should mention it as future work enabled by confluence, but implementing it would require a separate effort.

---

## v2 — Other Features

### 2.5 Coordinator as Worker (without being coordinator)

Flip the model: any machine can submit a net for reduction, and the grid itself decides who coordinates. This requires leader election (Raft, Paxos, or simpler alternatives for a known-membership grid).

**v1 exclusion source:** SPEC-13 R49 (Consensus protocols).

### 2.6 Work Stealing

Workers that finish early steal work from slower workers. Incompatible with BSP barrier synchronization (SPEC-13 R49), but could work in an asynchronous model.

**v1 exclusion source:** SPEC-13 R49 (Work stealing), PESQ-011 L1.

### 2.7 Intra-Worker Parallelism

Workers use `rayon` to reduce their partition with multiple threads. Each worker already has a disjoint ID range (SPEC-04 R16-R20), which could be further subdivided for thread-local allocation.

**v1 exclusion source:** SPEC-13 R49 (rayon intra-worker), PESQ-011 L2.

### 2.8 Automatic Node Discovery

Replace manual `--coordinator HOST:PORT` with multicast/DNS-SD/Consul-based discovery.

**v1 exclusion source:** SPEC-07, Section 5.5 (manual discovery sufficient for 8 machines on LAN).

### 2.9 Fault Tolerance

Checkpointing, re-dispatch on failure, worker health monitoring. Builds on 2.3 (dynamic departure) with persistence.

**v1 exclusion source:** OBJETIVO_TCC.md (out of scope), SPEC-13 R49 (Byzantine fault tolerance).

### 2.10 Multi-Tenancy and Job Queuing

Multiple users submit nets for reduction. The grid schedules and executes them, possibly concurrently on different subsets of workers.

### 2.11 Intelligent Partitioning

Replace round-robin with redex-aware or graph-aware partitioning strategies that minimize border redexes.

**v1 exclusion source:** SPEC-09 R29 (MAY for alternative strategies).

### 2.12 GPU Workers

Heterogeneous compute: some workers reduce on GPU (following HVM2's approach).

### 2.13 Visualization

Graphviz export of net state, live progress dashboard, reduction animation.

### 2.14 WASM Target

Browser-based IC reduction for education and demonstration.

### 2.15 Compact Memory Representation (HVM2-style)

**v1 representation:** Semantic Rust types prioritizing clarity (SPEC-02):
- Agent arena: `Vec<Option<Agent>>` with named fields (`symbol: Symbol`, `id: AgentId`).
- Port array: `Vec<PortRef>` where `PortRef` is an enum (`AgentPort(AgentId, PortId) | FreePort(u32)`), ~6 bytes per entry.
- Single flat array indexed by `agent_id * 3 + port_id`.

**v2 change:** Adopt the HVM2 compact encoding (AC-006, AC-015 CC-1):
- Two separate buffers: `nodes: Vec<u64>` for auxiliary port pairs, `vars: Vec<u32>` for linking variables. Each worker has its own arena (a contiguous slice of the global ID space, as in HVM4 AC-011).
- Compact `u32` port references with bit-packing: `(val << TAG_BITS) | tag`, where tag distinguishes CON/DUP/ERA/VAR. Fits in a register, 4 bytes per reference (vs ~6 bytes for enum).
- Bump allocation within each worker's slice. For communication between workers, serialize as direct buffer copy (zero-copy when possible).
- Optional: pool allocator with free list (inspired by Optiscope AC-012) if fragmentation becomes a problem after many reduction rounds.

**Why v1 chose simplicity:**
- Enum PortRef provides exhaustive pattern matching and readable debug output (SPEC-02, Section 5.4).
- Single flat port array avoids dual address spaces and LSB tag manipulation (SPEC-02, Section 5.2).
- The space overhead (~50% more per PortRef) is acceptable for a research prototype.

**Why v2 would switch:**
- ~33% memory reduction per PortRef (4 bytes vs 6 bytes).
- Better cache locality (dense `u32` arrays instead of enum with padding).
- Faster serialization for wire protocol (memcpy-style, as recommended in AC-015 CC-7).
- Less waste for ERA agents (ERA doesn't need slots in `nodes[]`).

**Migration path:** Encapsulate PortRef in a newtype with conversion methods (SPEC-02, Section 5.4 already anticipates this). The public API (`connect`, `get_target`, `create_agent`) does not change; only the internal representation does.

**Complexity:** Medium. Requires bit manipulation for every port access, dual address space management, and updated serialization. No correctness implications (same semantics, different encoding).

**v1 exclusion source:** SPEC-02 Sections 5.2, 5.4 (clarity over micro-optimization); SPEC-02 Resolved Question 1 (uniform allocation accepted for v1).

### 2.16 Streaming Reduction Mode **(confluence-enabled)**

**v1 limitation:** The BSP cycle is strictly batch: workers reduce their partition to Normal Form (`reduce_all`), return the complete result, and the coordinator waits for ALL workers before merging (SPEC-13 R2). No partial results are exchanged.

**v2 change:** Replace the barrier-synchronized BSP cycle with an asynchronous streaming model:

1. Workers use `reduce_n(budget)` instead of `reduce_all`, returning partially-reduced partitions after a fixed interaction budget.
2. The coordinator performs incremental merge as results arrive, without waiting for all workers.
3. New border redexes are dispatched immediately as they emerge from partial merges.
4. The cycle repeats with finer granularity, allowing faster workers to contribute more.

**Why confluence makes this safe:** Strong confluence guarantees that ANY subsequence of reductions applied in ANY order produces the same final result. Partial reductions are just shorter subsequences. The coordinator can merge partial results in any order because the merge of partially-reduced partitions is still a valid intermediate state.

**Motivation (Mackie and Sato, REF-015):** Mackie and Sato demonstrated that streaming operations (which release partial results incrementally) enable dramatically more parallelism than batch operations. For Fibonacci, the complexity drops from exponential (batch) to quadratic (streaming). Applying this insight to distributed IC reduction would allow workers to generate and exchange partial results continuously instead of waiting for full Normal Form.

**Complexity:** High. Requires:
- New protocol messages: `PartialPartitionResult` with intermediate state.
- Reformulation of the merge algorithm to support incremental merge without all partitions present.
- Reformulation of premise P3 (completeness) in the formal argument (ARG-001) to accommodate partial merges.
- Coordinator state machine redesign: event-driven instead of round-based.
- Careful handling of border redex resolution with incomplete partition state.

**Assessment:** This is a significant research direction that could transform Relativist's performance characteristics for Profile B and C workloads. However, it requires re-establishing the formal correctness argument under weaker synchronization assumptions. Best pursued after v1 validates the batch approach.

**v1 exclusion source:** SPEC-13 R2 (mandatory barrier synchronization), DISC-009 (analysis of batch vs streaming trade-offs).

### 2.17 Streaming Arithmetic Encoding

**v1 limitation:** Church arithmetic combinators are batch: `add(S(x), y) = add(x, S(y))` accumulates all computation before returning. This limits parallelism because intermediate redexes are not exposed until the entire operation completes.

**v2 change:** Implement streaming variants of arithmetic combinators following Mackie and Sato (REF-015): `add(S(x), y) = S(add(x, y))` releases partial results immediately, creating new redexes that other agents (or workers) can consume before the full computation finishes.

**Impact:** Combined with streaming reduction (2.16), this would enable pipelined distributed arithmetic where workers process intermediate results as they emerge, instead of waiting for complete Normal Form.

**Complexity:** Low-Medium (encoding changes are straightforward; benefit requires 2.15 to be useful in distributed mode).

**v1 exclusion source:** SPEC-14 Open Question 0, DISC-009 Section 4.

### 2.18 Native Numeric Types (HVM2-style)

**v1 limitation:** Arithmetic uses Church numerals (unary encoding): church(n) requires O(n) agents. Multiplication requires O(a*b) interactions. This is theoretically correct but practically inefficient for large numbers.

**v2 change:** Add native agent types for numbers and operations, following HVM2's approach (REF-003): NUM (numeric literal), OPE (binary operator), SWI (conditional switch). These bypass Church encoding entirely, reducing `mul(1000, 1000)` from ~1M interactions to O(1).

**Why this is separate from universality:** Lafont's universality (REF-002, Theorem 1) guarantees that Church encoding works. Native types are a performance optimization that does not affect the theoretical contribution. The TCC validates the formal property with pure ICs; native types would be an engineering improvement for post-TCC practical use.

**Complexity:** Medium. Requires extending the Symbol enum, adding reduction rules for native types, and updating serialization and partitioning to handle new agent kinds.

**v1 exclusion source:** SPEC-14 Section 5.1 (Church encoding chosen for simplicity and theoretical alignment).

### 2.19 Protocol Payload Chunking (Streaming Large Frames) — ARCHIVED (alternative not chosen)

**Status:** Not implemented. Superseded by the two-part L6 fix described in item 2.20 and `docs/PHASE2-FINDINGS.md` Section 6. The chunking alternative is preserved here for future reference: if a future workload produces a fully-dense subnet larger than the new 1 GiB cap, chunking becomes relevant again.

**v1 limitation:** Each protocol frame is capped at 256 MiB (`DEFAULT_MAX_PAYLOAD_SIZE` in `src/protocol/frame.rs`). `AssignPartition` and `PartitionResult` serialize the entire partition into a single frame via bincode. The frame size of the last worker's partition is `~250 MB + k * (total_agents / num_workers)` under the current subnet encoding (see item 2.20 for the root cause of the fixed term), so the cap is reached not only at `workers=1` but also at intermediate worker counts on very large nets: `ep_annihilation_con=5M` fails at `workers=1`, `workers=2` (~315 MB), and `workers=4` (282 MB), and only passes at `workers=8` (~267 MB). `dual_tree=22` fails only at `workers=1` (~293 MB). Documented as limitation L6 in `docs/PHASE2-FINDINGS.md`. Item 2.19 addresses the per-frame cap; item 2.20 addresses the fixed overhead and is orthogonal.

**v2 change:** Support chunked transmission of large payloads across multiple frames:

1. Append new `Message` variants (preserving bincode discriminant stability per SPEC-06 R5): `AssignPartitionChunk { round, chunk_id, total_chunks, data }` and `PartitionResultChunk { round, worker_id, chunk_id, total_chunks, data, stats_opt }`.
2. Add helper functions `send_partition_chunked()` and `recv_partition_chunked()` in `src/protocol/frame.rs` (or a new `src/protocol/chunking.rs`): serialize the partition once with bincode, split the raw bytes into chunks below a configurable chunk size, send frames sequentially per worker.
3. Add FSM state to the worker loop in `src/protocol/worker.rs`: an optional `(round, Vec<u8>, expected_chunks)` accumulator that buffers chunks until the last one arrives, then concatenates and deserializes.
4. Refactor `distribute_partitions()` in `src/protocol/coordinator.rs` to use the chunked sender.
5. Mirror the logic on the return path (worker to coordinator) so `PartitionResult` can also exceed the cap.
6. Per-chunk CRC32C already comes from the existing frame header. Add a `full_partition_crc: u32` field in the final chunk to detect corruption after reassembly.
7. Add per-chunk and per-partition timeouts; a stalled transfer should fail the round gracefully rather than hang indefinitely.
8. Tests: chunk roundtrip, CRC integrity, out-of-order rejection, per-chunk timeout, full-partition reassembly.
9. Update SPEC-06 (protocol) and SPEC-07 (framing) to describe the chunk message flow and the relationship to the atomic-frame invariants.

**Why v1 chose atomic frames:** Simplicity. SPEC-06 and SPEC-07 treat each `Message` as one atomic frame, which matches bincode's all-or-nothing serialization and keeps the worker FSM stateless between messages. For the benchmark sizes the TCC targets (up to 5 million agents, distributed across 2 or more workers), the 256 MiB cap is not a limiting factor.

**Why v2 would switch:** Unblocks `workers=1` on nets above 256 MiB, enables real-world grid workloads with arbitrarily large inputs, and removes a DoS-style cap that has no theoretical justification in the IC model itself. Complements 2.16 (Streaming Reduction Mode) by handling the transport layer rather than the reduction layer.

**Limitation (does NOT solve):** bincode is atomic, so reassembly still requires the full buffer in memory on both sides. Chunking circumvents the **frame-level** cap, not the memory cost of serialization. True streaming (incremental parsing, constant-memory deserialization) would require replacing bincode with a codec that supports progressive parsing -- a separate, larger refactor and a new item if it proves necessary.

**Complexity:** Medium. Approximately 560 lines of Rust (around 200 for code, 100 for tests, 100 for coordinator/worker wiring, 80 for SPEC updates, 80 for spec review cycles). Estimated effort: 1-2 days of focused work, plus the debatedor and especialista-specs review pipeline mandated by the TCC workflow.

**v1 exclusion source:** SPEC-06 Section on framing (atomic messages), `DEFAULT_MAX_PAYLOAD_SIZE` constant in `src/protocol/frame.rs`, and the frozen v0.9 spec baseline.

### 2.20 Compact Subnet Encoding (Sparse-to-Dense Conversion) — DONE

**Status:** Implemented post-v0.9.0. Shipped as `CompactSubnet` in `src/partition/compact.rs`, wired via `#[serde(serialize_with = "serialize_subnet_compact", deserialize_with = "deserialize_subnet_compact")]` on `Partition::subnet`. Round-trip unit tests cover empty net, single agent, connected pair, tombstone slot, bincode roundtrip, redex queue preservation, root preservation, and sparse compaction. Combined with a raise of `DEFAULT_MAX_PAYLOAD_SIZE` from 256 MiB to 1 GiB, this resolves L6 for every configuration in the TCC benchmark matrix. See `docs/PHASE2-FINDINGS.md` Section 6 for the validation data and `results/post_fix/B3_comparison.md` for the pre/post local-mode speedup comparison. The historical description of the limitation is preserved below for traceability.

**v1 limitation (resolved):** The partition subnet built by `src/partition/helpers.rs::build_subnet` creates a `Vec<Option<Agent>>` and `Vec<PortRef>` both sized to `(max_agent_id_of_worker + 1) * PORTS_PER_SLOT`, where `max_agent_id_of_worker` is the maximum live agent ID assigned to the worker under the allocation strategy. Because `ContiguousIdStrategy` gives the highest-ID agents to the last worker, that worker's subnet is always a full-length dense-indexed array of the entire net, regardless of how many live agents it actually owns. Under bincode 1.x with fixed-int encoding, the port Vec alone contributes roughly `8.5 * 3 * total_agents` bytes of serialized output -- about 240 MB for a 10 million-agent net -- even when the last worker owns only a fraction of those slots. This fixed term was the dominant component of the L6 frame-size failures and is independent of the framing cap addressed by 2.19.

**Why the dense-index layout exists in v1:** O(1) lookup by agent ID. The reduction loop in `src/reduction/reduce.rs` indexes directly into `net.agents[id]` and `net.ports[id * PORTS_PER_SLOT + port]` without any map translation, which keeps the hot path branch-free and cache-friendly. Changing the in-memory representation would touch every reduction rule.

**v2 change:** Split the in-memory representation from the wire representation. Keep the dense layout for `Net` in memory, but introduce a compact serialization path that emits only the live agents of a partition as an ordered list of `(agent_id, Agent, [PortRef; 3])` entries, plus the small auxiliary structures (`free_port_index`, `id_range`, borders, redex queue):

1. Add a `CompactSubnet` intermediate struct in `src/partition/types.rs` that represents a partition in list form: `Vec<(AgentId, Agent, [PortRef; 3])>` plus the Partition's existing small fields.
2. Add a `Partition::to_compact(&self) -> CompactSubnet` method that walks the dense arrays and emits only the live slots.
3. Add a `CompactSubnet::into_partition(self) -> Partition` method that rebuilds a dense `Net` on the receiving side by allocating a Vec of size `max_id_in_list + 1` and filling only the entries present in the list.
4. Change `Message::AssignPartition` to carry `CompactSubnet` instead of `Partition` (or add a new `AssignCompactPartition` variant appended to the enum to preserve R5 discriminant stability).
5. Do the same for `Message::PartitionResult` on the return path.
6. Update `distribute_partitions()` in `src/protocol/coordinator.rs` to call `to_compact()` before sending.
7. Update the worker's FSM in `src/protocol/worker.rs` to call `into_partition()` before handing the Partition to the reduction loop.
8. Benchmark the compact encoding on the L6 failure cases (ep_annihilation_con=5M w=1,2,4 and dual_tree=22 w=1); confirm the wire size drops to ~6 bytes per live agent (~60 MB for a 10M-agent net with w=1, under 20 MB with w=4).
9. Tests: compact roundtrip preserves all agent data and port connections, compact size is strictly linear in live agents, and the reconstructed Partition produces the same reduction output as the original.
10. Update SPEC-04 (partitioning) and SPEC-06 (protocol) to describe the compact wire format and the in-memory/wire separation.

**Why v1 did not do this:** The dense layout is both simpler (no serialization translation) and matches the internal reduction loop directly. For the benchmark sizes the TCC targets on a typical development machine, the fixed 250 MB overhead is hidden because the total frame never exceeds the cap except at the extreme configurations documented in L6.

**Why v2 would switch:** Linear serialization cost in live agents rather than in `max_id`. This unblocks every configuration in L6 without touching the framing layer, and it reduces TCP transfer time on Phase 3 (real network) where every byte saved is a real latency reduction. It also removes a surprising scaling behavior where adding workers does not reduce the per-worker frame size as much as expected because the last worker always pays the fixed cost.

**Relationship to 2.19:** Independent. 2.19 lifts the per-frame cap; 2.20 shrinks the payloads that would hit the cap. 2.20 was implemented as the primary fix because it benefits every run (not just large-net runs) and removes the structural artifact of dense-arena padding without modifying the wire protocol envelope. The cap raise from 256 MiB to 1 GiB then handled the four fully-dense configurations (`dual_tree=22 w=1`, `ep_annihilation_con=5M w={1,2,4}`) where `CompactSubnet` had nothing to strip. 2.19 stays archived as a fallback for future workloads where even a 1 GiB frame is insufficient.

**Limitation (does NOT solve):** Memory pressure on the reduction loop itself. If the reduction loop is the bottleneck (rather than transport), switching the wire format does nothing. Also, the compact-to-dense reconstruction on the worker side still allocates a full-length Vec before reduction starts, so peak memory on the worker is unchanged; only the bytes-on-the-wire shrink.

**Complexity:** Medium. Approximately 400-500 lines of Rust: 100 for `CompactSubnet` and conversion methods, 80 for the new protocol variant and serialization, 100 for coordinator/worker wiring, 80 for tests, 80 for SPEC updates. Estimated effort: 1-2 days of focused work plus the standard review pipeline.

**v1 exclusion source:** `src/partition/helpers.rs::build_subnet` (dense-index allocation), SPEC-04 Section 4.5 (subnet construction), and the frozen v0.9 spec baseline.

---

## The Confluence Argument for the Paper

The features in Section 2 (2.1-2.4) share a common theoretical foundation that should be presented in the TCC's Discussion section (Section 5):

> Strong confluence does not merely guarantee correctness for a fixed distributed configuration. It guarantees correctness under **any** redistribution of work at **any** point during reduction. This means:
>
> - A coordinator can participate in reduction without affecting the result (2.1).
> - Workers can join mid-execution and receive fresh partitions without invalidating prior work (2.2).
> - Workers can depart and their unreduced partitions can be re-dispatched without rollback (2.3).
> - Merge can be performed hierarchically because it is associative under confluence (2.4).
>
> These properties are unique to Interaction Combinators among distributed reduction models. Systems based on lambda calculus or term rewriting require explicit confluence checks or deterministic scheduling to achieve the same guarantees.
>
> v1 validates the fundamental property (distributed reduction = sequential reduction) with the simplest architecture. The architectural evolutions above are not speculative -- they are direct corollaries of the same property, requiring only engineering effort, not new theoretical results.

This argument strengthens the TCC's contribution: the prototype validates the foundation, and the roadmap demonstrates the breadth of what that foundation enables.
