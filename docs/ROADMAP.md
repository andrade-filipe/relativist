# Roadmap

> **NOTE:** For active implementation status, priorities, and milestones, see `next-steps.md`. This document provides the architectural rationale and detailed theoretical descriptions.

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

**Related, already shipped in v0.10.0-bench:** the **strict BSP mode** (SPEC-05 R30a,
opt-in via `--strict-bsp`) resolves the multi-round limitation (previously tracked as
**L2**) at the *coordinator* level without touching the BSP contract: instead of
draining border cascades via `reduce_all` at merge time, the coordinator applies a
single `reduce_border_once` step and lets residual cascades be redistributed by the
next split/merge cycle. This makes `rounds > 1` observable on cross-partition topologies
(empirically validated in `v1_local_baseline`: `cascade_cross(N) = N` rounds,
`dual_tree(d) = d` rounds with `workers ≥ 2`), which is what Phase 3 LAN needs in
order to amortize per-round RTT cost. Strict mode is still a *batch* BSP — Section 2.16
below is the deeper asynchronous streaming redesign, orthogonal to strict BSP.

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

### 2.18 Native Numeric Types (HVM2-style) — ARCHIVED

**Status:** ARCHIVED. Relativist's purpose is net partitioning and distributed reduction — it is a general-purpose IC reducer, not a computation engine like HVM2. Domain-specific operations (arithmetic, logic, etc.) are defined by the encoder/decoder layer, not by native agent types in the network. Church encoding validates the theoretical contribution (universality); practical encoding is the encoder's responsibility.

**Original proposal:** Add native agent types (NUM, OPE, SWI) following HVM2's approach (REF-003) to bypass Church encoding. This would reduce `mul(1000, 1000)` from ~1M interactions to O(1).

**Why archived:** Different goal from HVM. HVM2 is a computation engine that needs efficient arithmetic; Relativist is a distributed reduction engine that needs efficient partitioning, transport, and merge. The encoder/decoder pattern (SPEC-14) already separates "what problem to solve" from "how to reduce the net."

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

### 2.21 WAN / Internet Deployment

**Scope.** Lift the "same-LAN" assumption of v1 so that Relativist workers can join a coordinator across the public Internet, over typical home/office NATs, with realistic security, connection stability, and performance guarantees. This is the feature that turns Relativist from a "LAN-bound distributed reducer" into an actual *grid computing* system in the volunteer-computing sense (e.g., BOINC-style), which is the underlying motivation of the TCC.

**v1 limitation.** v1 assumes a flat, cooperative LAN:
- Coordinator and workers must be mutually reachable by static IP / hostname (SPEC-09 R27: "TCP over LAN"; SPEC-07 §5.5: "manual discovery sufficient for 8 machines on LAN").
- `--coordinator HOST:PORT` is configured by hand (SPEC-07 R12; no discovery service).
- Authentication is a single shared token over unencrypted TCP (SPEC-10); no TLS, no identity, no revocation.
- There is no reconnect, no resume, no heartbeat timeout tuning for high-RTT links — a dropped connection mid-round is fatal to that round.
- The Phase 3 subtraction `t_network = t_lan − t_localhost` (USAGE_GUIDE §11.3.6) assumes stable, symmetric RTT and bandwidth, which WAN links do not provide.

Running v1 across the Internet is possible only with external scaffolding (VPN, SSH reverse tunnel, port forwarding), and even then the lack of reconnection and TLS makes it unsuitable for real deployment.

**v2 design — required pieces:**

1. **NAT traversal.** Workers behind home/office NATs cannot accept inbound TCP. The v2 design MUST invert the connection model so that the worker *dials out* to a stable endpoint, not the other way around. Two viable paths:
   - *(a) Rendezvous/relay server.* A small always-on service with a public IP and domain name that the coordinator and all workers connect to. Messages are relayed through it. Simplest to implement, highest latency overhead, single point of failure. Compatible with any NAT.
   - *(b) Hole-punching (STUN/TURN-style).* Coordinator and worker register with a signaling server, exchange candidate addresses, and establish a direct peer connection using UDP hole punching or TCP simultaneous open. Lower latency in the common case, falls back to relay when punching fails. Significantly more complex.
   - v1-v2 transition: start with (a) for simplicity, add (b) as an optimization once metrics prove the relay is the bottleneck.

2. **TLS everywhere.** The public Internet is hostile. All transport MUST be TLS 1.3 (rustls). The coordinator presents a certificate (self-signed trust-on-first-use, or Let's Encrypt if it has a DNS name). Workers verify the coordinator's certificate fingerprint on connect. Supersedes the plaintext TCP assumption of SPEC-06 and SPEC-10 R3.

3. **Strong authentication.** The shared-token scheme in SPEC-10 is insufficient for Internet deployment:
   - Tokens leak through process listings, shell history, and logs.
   - There is no way to revoke a compromised token without restarting the coordinator.
   - There is no per-worker identity, so abuse cannot be traced or rate-limited.
   - v2 MUST replace it with either (i) mTLS where every worker holds a per-identity certificate signed by the coordinator's CA, or (ii) short-lived bearer tokens issued by an OAuth2/OIDC flow against the TCC's institutional identity provider. Revocation and rotation become first-class.

4. **Persistent reconnection and resume.** A worker whose TCP connection drops mid-round MUST be able to reconnect within a configurable window (default: 60 s) and resume without losing its current partition assignment. This requires:
   - Session IDs that survive TCP connection loss (carried in the first frame of the reconnect).
   - Idempotent `AssignPartition` and `PartitionResult` messages (the coordinator tolerates a duplicate result for a known session).
   - Coordinator-side state that tracks "partition P is out to session S, not yet returned" and only reassigns after a timeout.
   - This is the natural extension of ROADMAP 2.3 (Dynamic Worker Departure) to the common WAN failure mode of flaky connections rather than clean exits.

5. **High-RTT tolerance.** WAN RTT is 20-300 ms vs. LAN 0.1-1 ms. The BSP grid loop already amortizes latency per round, but three tunables MUST become WAN-aware:
   - `worker_connect_timeout`: raise default from 120 s (fine for LAN) and make it explicit that WAN workers may take longer to handshake through a relay.
   - `distribute_timeout` / `collect_timeout`: make per-round deadlines adaptive to observed RTT rather than fixed constants (SPEC-06 R30).
   - BSP strict mode (SPEC-05 R30a): the per-round RTT cost is *exactly* the metric Phase 3 measures, so nothing to change there — but the Phase 3 subtraction methodology needs a WAN variant where `t_wan − t_lan − t_localhost` is the additional per-hop internet cost. This would be Phase 4, not Phase 3.

6. **Automatic discovery for Internet peers.** ROADMAP 2.8 lists LAN-scoped discovery (multicast, DNS-SD, Consul). v2 Internet deployment needs either a well-known rendezvous URL (the relay server) or a bootstrap list baked into config. This is a strict superset of 2.8, not a replacement: LAN discovery and WAN rendezvous can coexist.

7. **Abuse mitigation.** Any Internet-facing coordinator is a target for resource exhaustion attacks:
   - Unauthenticated peers MUST be dropped at the TLS handshake (not inside the protocol FSM).
   - Partition assignment MUST rate-limit per worker identity.
   - Result frames MUST honor the R9 `max_payload_size` cap (SPEC-06) before any deserialization work.
   - The coordinator MUST log auth failures and unusual frame sizes (SPEC-11) so that operators can spot probing.

**Relationship to other roadmap items.**
- **2.2 Dynamic Worker Joining** — prerequisite pattern. v2 Internet deployment is 2.2 extended with NAT traversal, TLS, auth, and reconnect.
- **2.3 Dynamic Worker Departure** — prerequisite pattern. Reconnect-with-resume (point 4 above) is a specific form of 2.3 where departure is unplanned and the partition re-dispatch window is bounded.
- **2.8 Automatic Node Discovery** — complementary. LAN discovery stays as-is for intranet clusters; WAN discovery uses rendezvous URLs.
- **2.9 Fault Tolerance** — complementary. Checkpointing (2.9) is orthogonal: reconnect-with-resume handles short drops; checkpointing handles long outages and process restarts.

**Why v1 did not do this.** The TCC's scientific question — "does the IC confluence guarantee hold when reduction is distributed?" — is answered by any deterministic distributed baseline, including LAN. Adding NAT traversal, TLS, and Internet-grade auth would have quintupled the implementation surface area and shifted the bottleneck from the IC model to the network engineering, at the cost of the central experiment. v1 therefore scopes down to the cleanest possible distributed setting (SPEC-09 R27: TCP over LAN) and leaves the generalization to Internet deployment for v2.

**Why v2 should do this.** Grid computing in the real sense — the title of the TCC — is about aggregating *heterogeneous, geographically distributed, opportunistically available* compute. A LAN-bound reducer demonstrates the theoretical claim but does not realize the vision. v2 Internet deployment is what makes Relativist deployable as a volunteer-computing platform, which is the long-term direction stated in the TCC's motivation. It is also the minimum prerequisite for any cross-institution collaboration that would extend the work beyond a single lab.

**Limitation (does NOT solve).** Byzantine fault tolerance (malicious workers returning fabricated results) is still out of scope and belongs with 2.9 fault tolerance. WAN deployment closes the *connectivity* gap, not the *trust* gap. Production volunteer computing systems (BOINC, Folding@home) layer redundant execution + result voting on top; that layer would be a separate v3 item.

**Complexity.** High. Rough breakdown: ~600 lines for TLS and certificate handling (rustls integration in SPEC-06), ~400 lines for the rendezvous/relay server (a new crate, `relativist-relay`), ~300 lines for session-aware reconnect in coordinator and worker FSMs, ~200 lines for mTLS or OAuth2 auth, ~150 lines for adaptive timeouts, ~200 lines of tests, plus 4-5 SPEC rewrites (SPEC-06, SPEC-07, SPEC-10, SPEC-11 metrics) and one new SPEC (SPEC-16: WAN Deployment). Estimated effort: 3-4 weeks of focused work plus the standard spec/debate review pipeline. Not a weekend project.

**v1 exclusion source:** SPEC-07 §5.5 (LAN-only discovery), SPEC-09 R27 ("TCP over LAN"), SPEC-10 R3 (plaintext token auth), SPEC-06 (no TLS, no reconnect), OBJETIVO_TCC.md (scope bounded to distributed baseline on LAN).

### 2.21.1 End-to-End Security Analysis

**Scope.** A systematic security analysis of the entire Relativist system when exposed to the public Internet via WAN deployment (2.21). This is not a code feature but a formal deliverable: a threat model document, attack surface enumeration, and mitigation mapping that validates the security design of SPEC-24 before implementation begins.

**Motivation.** WAN deployment (2.21) introduces 7 sub-components (NAT traversal, TLS, strong auth, reconnect, adaptive timeouts, discovery, abuse mitigation), each with its own attack surface. Without a structured threat analysis, security decisions are ad-hoc and gaps are discovered in production. The TCC's claim that IC reduction can be distributed over a grid is only credible if the grid is deployable — and deployability over the Internet requires a defensible security posture, not just TLS.

**Deliverables:**

1. **Threat model.** STRIDE-based analysis of the coordinator, worker, and relay components. Enumerate: spoofing (identity), tampering (results, partitions), repudiation (who did what), information disclosure (net contents in transit/at rest), denial of service (resource exhaustion), elevation of privilege (worker→coordinator). Map each threat to the specific SPEC-24 requirement that mitigates it.

2. **Attack surface enumeration.** Catalog every network-facing endpoint, protocol message, and trust boundary. For each: what input does it accept, what validation does it perform, what happens if validation is bypassed. Cover at minimum: TLS handshake, mTLS certificate validation, session token issuance/refresh, relay message forwarding, coordinator FSM transitions triggered by network input, worker result frame acceptance.

3. **Mitigation mapping.** For each identified threat: (a) which SPEC-24 requirement addresses it, (b) the specific code-level mechanism (e.g., "rustls rejects non-TLS 1.3 handshakes at R-SEC-03"), (c) residual risk after mitigation. Flag any threats where SPEC-24 has no corresponding requirement — these become spec amendments.

4. **Trust boundary diagram.** Visual (TikZ) diagram of the system's trust boundaries in WAN mode, showing: Internet ↔ relay ↔ coordinator ↔ worker, with annotations for each security control at each boundary crossing.

5. **Residual risk register.** Explicit list of what is NOT mitigated by v2 (e.g., Byzantine workers, result integrity without redundant execution, relay compromise). Each entry states why it is deferred and what future work (v3+) would address it.

**Relationship to 2.21.** This is a sub-item of WAN deployment, not a standalone feature. It runs in parallel with SPEC-24 creation and should be completed before SPEC-24 implementation begins. Any gaps found in the security analysis feed back as SPEC-24 amendments.

**Relationship to SPEC-10 (Security).** SPEC-10 defines v1 security (shared token, plaintext TCP). The security analysis evaluates the delta between SPEC-10 and SPEC-24, ensuring that every v1 security assumption that is broken by WAN exposure is explicitly replaced by a stronger mechanism.

**Complexity.** Medium. ~300 lines of analysis document (Markdown + TikZ diagram). No code changes, but may generate 5-10 SPEC-24 requirement amendments. Estimated effort: 1-2 weeks, primarily research and writing. Requires familiarity with STRIDE, TLS 1.3, mTLS, and common distributed system attack patterns.

**Prerequisites.** SPEC-24 draft must exist (at least the requirements section) so that the analysis can map threats to specific requirements. If SPEC-24 is incomplete, the analysis flags unmapped threats as "spec gap — amendment required."

---

## v2 — Network Overhead Reduction (items 2.22-2.26)

The v1 local baseline (`v0.10.0-bench`) and the 2026-04-11 stress smoke test on `ep_annihilation_con` at 20 M agents exposed a scaling asymmetry: the tcp_localhost mode runs 2-3x slower than the in-process sequential baseline, and the ratio *worsens* with input size (2.02x at 5 M agents, 3.48x at 20 M). This is not a theoretical inevitability — it is the sum of several concrete, addressable sources of overhead in the current transport layer. Items 2.22-2.26 are the five-front attack plan on that overhead. They stack: applied together, the conservative estimate is that the tcp_localhost / sequential ratio drops from 2-3.5x to <1.2x on the same workload, which is the minimum necessary for the Phase 3 LAN subtraction (`t_network = t_lan − t_localhost`) to isolate genuine RTT cost rather than transport-layer waste.

Each item below is written so it can be shipped independently. The ordering below reflects increasing implementation cost, so 2.22 and 2.23 are the quick-wins to pursue first.

### 2.22 TCP Transport Tuning

**Scope.** Apply the well-known TCP socket options that v1 left at OS defaults. This is the cheapest item in the group and should be the first step on any overhead-reduction campaign.

**v1 limitation.** `src/protocol/coordinator.rs` and `src/protocol/worker.rs` create `tokio::net::TcpStream` connections with no explicit socket tuning. In particular:
- `TCP_NODELAY` is **not** set. Nagle's algorithm is on by default, which batches small writes for up to 40 ms. This does not matter for the big `AssignPartition` frame (a single `write_all` of ~1 GB is not a small write), but it *does* matter for the Register/RegisterAck handshake, for any small control message, and for the final `flush()` call that waits on the kernel to push the last TCP segment.
- `SO_SNDBUF` / `SO_RCVBUF` are left at the Linux defaults (typically ~208 KB). For frames that reach the 1 GiB cap (already observed in v1 L6 and confirmed in the 20 M stress smoke), the kernel forces thousands of context switches and `writev`/`readv` cycles during a single `write_all`. Raising the buffers to 2-8 MB amortises this dramatically without costing memory in the common case.
- There is no `SO_KEEPALIVE`, `TCP_KEEPIDLE` / `TCP_KEEPINTVL` / `TCP_KEEPCNT` configuration, so a stalled connection takes the full Linux default (~2h) to surface as an error. For Phase 3 LAN this is already inadequate; for 2.21 WAN deployment it is actively dangerous.

**v2 change:** In `src/protocol/config.rs` add a new `TransportTuning` struct with fields `nodelay: bool` (default true), `send_buffer_bytes: Option<usize>` (default 4 MiB), `recv_buffer_bytes: Option<usize>` (default 4 MiB), `keepalive: Option<Duration>` (default 30 s). Apply the tuning inside `accept_workers()` and `connect_to_coordinator()` via `TcpSocket::set_nodelay()`, `TcpSocket::set_send_buffer_size()`, `TcpSocket::set_recv_buffer_size()`, and `socket2::SockRef::set_tcp_keepalive()` where `tokio::TcpSocket` does not expose the option directly. Expose the struct in `NodeConfig` so it is configurable via CLI flags and persisted in `manifest.md` for reproducibility.

Test plan: a pair of integration tests in `src/protocol/frame.rs` tests that spin up a listener and a connector, assert `tcp_stream.nodelay()` returns `true`, and assert `recv_buffer_size()` matches the configured value (subject to the kernel's doubling behaviour, which should be documented in the test assertion).

**Why v1 did not do this.** SPEC-06 treats the TCP layer as "plain TCP" without tuning, which is the minimum viable baseline. The L6 fix (`CompactSubnet` + 1 GiB cap) was what unblocked the failing benchmarks; Transport Tuning was not on the critical path for correctness and was deferred to the post-v1 overhead analysis.

**Why v2 should do this.** It is the lowest-cost item in the overhead roadmap and provides measurable gains with zero correctness risk. The observable effect is a reduction in `distribute_time + collect_time` per round at large frame sizes, which is exactly the slowest phase measured in `phase2_rounds.csv`. Expected impact: 5-15 % wall-clock reduction on the large-frame configs (`ep_annihilation_con=5 M/20 M w=1,2`, `dual_tree=22/25 w=1`), more on LAN than on localhost.

**Relationship to other items.** Independent and compatible with everything else. 2.22 should be shipped first because 2.23-2.26 build on the same socket setup code path.

**Complexity.** Trivial. Approximately 100 lines of Rust (50 for the `TransportTuning` struct and application, 30 for the integration test, 20 for CLI wiring). Estimated effort: 1-2 hours.

**v1 exclusion source:** SPEC-06 (plain TCP, no tuning), `src/protocol/coordinator.rs` and `src/protocol/worker.rs` (no socket options set).

**References:**
- [tokio TcpSocket docs](https://docs.rs/tokio/latest/tokio/net/struct.TcpSocket.html) — exposes `set_nodelay`, `set_send_buffer_size`, `set_recv_buffer_size`.
- [Red Hat RT tuning guide, TCP_NODELAY section](https://docs.redhat.com/en/documentation/red_hat_enterprise_linux_for_real_time/7/html/tuning_guide/tcp_nodelay_and_small_buffer_writes) — canonical description of Nagle's algorithm and when to disable it.
- [Linux cyberciti TCP tuning](https://www.cyberciti.biz/faq/linux-tcp-tuning/) — send/recv buffer sizing on Linux.
- Hacker News discussion [*It's always TCP_NODELAY*](https://news.ycombinator.com/item?id=40310896) — operational stories on forgetting to set this option.

### 2.23 Wire Format Compaction (bincode v2 varint + enum shrink + optional LZ4)

**Scope.** Shrink the *number of bytes* that cross the wire per partition, without changing the protocol semantics. Three independent techniques stacked together: migrate bincode v1 → v2 with varint int encoding, replace the enum discriminant encoding of `PortRef` with a 1-byte tag, and add an opt-in LZ4 compression wrapper on payloads above a configurable threshold.

**v1 limitation.** `Cargo.toml` pins `bincode = "1"`, and every `Serialize`/`Deserialize` in `src/net/types.rs`, `src/partition/compact.rs`, and `src/protocol/types.rs` goes through bincode v1. v1 uses **fixed-int encoding** for every integer and **4-byte u32 discriminants** for every enum variant. The cost is visible in `PortRef`:
- `AgentPort(AgentId, PortId)` is a `u32` + `u8`. Under bincode v1 this serialises as `4 (enum tag) + 4 (AgentId) + 1 (PortId) = 9 bytes`. Under bincode v2 varint with a u8 enum repr, the same value encodes as `1 (tag) + 1-2 (AgentId varint) + 1 (PortId) = 3-4 bytes` for typical agent IDs below 2^14.
- `FreePort(u32)` is `4 + 4 = 8 bytes` in v1, `1 + 1-5 bytes` in v2 depending on the value. The special `DISCONNECTED = FreePort(u32::MAX)` sentinel unfortunately encodes large in varint (5 bytes for the value), but that is still 6 bytes total instead of 8.
- `CompactSubnet::live` stores `Vec<(AgentId, Agent, [PortRef; 3])>`. With three port entries per live agent under v1, the port array alone is ~25 bytes per live agent; under v2 it drops to ~9-12 bytes per live agent. Over 40 M live agents (the 20 M `ep_annihilation_con` case) that is a ~500 MB saving on the wire, well above the difference between a 1 GiB cap hit and a comfortably-sized frame.

On top of the encoding cost, there is a *redundancy* cost: IC nets are full of repeated patterns — `DISCONNECTED` sentinels, entire auxiliary-port triples of the same shape, long runs of identical `Symbol` tags. Generic compression algorithms extract this redundancy cheaply. v1 does not compress the wire at all.

**v2 change:**

1. **bincode v2 migration.** Bump the dependency in `Cargo.toml` to `bincode = "2"`, switch to the new `bincode::serde::encode_to_vec`/`decode_from_slice` API with a `Configuration` whose `IntEncoding = Varint`. Bincode v2 with varint is the default, so the only additional work is reviewing every call site for the new signature. No wire format can be read by v1 readers after this change, so bump the `PROTOCOL_VERSION` constant in `src/protocol/coordinator.rs` from 1 to 2 and update the Register handshake rejection message.

2. **Custom `PortRef` serde impl.** Bincode v2's default enum discriminant is a single `u32` varint, which already collapses to 1 byte for `PortRef`. But we can do better by writing a manual `Serialize`/`Deserialize` impl that uses a 2-bit tag packed into the first byte plus the payload in the remaining 6 bits (for small agent IDs) or spilled into a varint tail (for larger ones). Concretely, reserve `tag = 0b00` for `AgentPort`, `0b01` for `FreePort`, `0b11` for the `DISCONNECTED` sentinel, and read the remaining 6 bits as the high bits of the payload. This pushes the common case (an `AgentPort` with an ID below 2^14 and port 0-2) to a single byte on the wire. The impl lives in `src/net/types.rs` behind `#[serde(with = "portref_compact")]`.

3. **Optional LZ4 compression wrapper.** Add a new `CompressedFrame` type alongside the existing frame in `src/protocol/frame.rs`:
   ```
   fn send_frame_compressed(w, msg, threshold, algo) -> Result<usize, ProtocolError>
   ```
   `threshold` is a per-config byte cut-off (default 1 MiB): below the threshold the frame is sent uncompressed, above it the payload is LZ4-compressed first and the frame header is marked with a single bit in the CRC field (or, cleaner, a new one-byte header field). On receive, `recv_frame_compressed` checks the flag and LZ4-decompresses before passing to bincode. LZ4 sustains ~500-3500 MB/s on a single core, so for a 1 GB frame the CPU cost is <500 ms and typical ratios on IC nets (lots of `DISCONNECTED` runs, repeated ports) are 3-10x. This alone dwarfs the bincode-level savings.

4. **Tests:** roundtrip each encoding independently (bincode v2, manual `PortRef`, LZ4 wrapper), roundtrip the full stack against a realistic `Partition` built from `build_partition_for_tests()`, measure serialised size before/after on the L6 test cases and assert the improvement.

5. **SPEC-06 update:** record the new wire version, the optional compression flag, and the compatibility matrix against old clients.

**Why v1 did not do this.** SPEC-06 R5 explicitly requires bincode v1 discriminant stability — appending new `Message` variants must not break existing serialised data. The v1 authors pinned the bincode major version deliberately, and wire compaction was correctly categorised as an optimisation that the scientific question did not hinge on. The three L6 benchmarks that failed at the 256 MiB cap were addressed by raising the cap to 1 GiB (item 2.20), not by shrinking the payload.

**Why v2 should do this.** Wire compaction is pure upside on the Phase 3 LAN path: every byte saved is a real latency reduction on a 1 Gbps link (roughly 8 ns/byte). On localhost it saves bincode CPU time (the encoding/decoding is a big chunk of the current ~25 % TCP/seq overhead that has nothing to do with the network itself). Combined with 2.22, 2.23 should take the tcp_localhost / sequential ratio from 3.48x (20 M smoke) down into the 1.5-2.0x range even before considering the more invasive items 2.24-2.26.

**Relationship to other items.** Stacks cleanly with 2.22, 2.24, 2.25, 2.26. Supersedes the archived 2.19 chunking idea (if the wire is small enough, chunking becomes unnecessary even for larger nets). Orthogonal to 2.20 (`CompactSubnet` already addresses the *structural* padding; 2.23 addresses the *encoding* padding).

**Complexity.** Medium. Rough breakdown: 80 lines for the bincode v2 migration (mostly signature updates), 120 lines for the manual `PortRef` compact impl + tests, 150 lines for the LZ4 wrapper + tests, 50 lines for `TransportTuning` / compression config wiring, 100 lines of SPEC-06 updates. Estimated effort: 1-2 days plus the standard spec/debate review pipeline.

**v1 exclusion source:** `Cargo.toml` (`bincode = "1"`), SPEC-06 R5 (discriminant stability), `src/net/types.rs` (derived `serde::Serialize` on `PortRef`), `src/protocol/frame.rs` (no compression layer).

**References:**
- [bincode v2 spec](https://docs.rs/bincode/latest/bincode/spec/index.html) — varint encoding details.
- [rust_serialization_benchmark on GitHub](https://github.com/djkoloski/rust_serialization_benchmark) — comparative size/speed of bincode, rkyv, postcard, flatbuffers, capnp.
- [rkyv is faster than {bincode, ...}](https://david.kolo.ski/blog/rkyv-is-faster-than/) — concrete benchmark numbers (context for 2.24).
- [lz4_flex crate docs](https://docs.rs/lz4_flex) — pure-Rust LZ4 with explicit block API; sustained 500 MB/s-3.5 GB/s depending on cache fit.
- [Facebook zstd site](http://facebook.github.io/zstd/) — reference compression ratios and speeds; zstd-1 is the alternative for WAN deployments.

### 2.24 Zero-Copy Archive (rkyv) on the Hot Path

**Scope.** Replace bincode entirely on the `AssignPartition` / `PartitionResult` messages with [rkyv](https://rkyv.org), a zero-copy serialisation framework. The wire payload becomes a binary image of the partition that the receiver can *access directly* via safe accessors without running a `deserialize` pass at all. Frames that are 1 GB today spend hundreds of ms in bincode deserialisation alone on the receiving side; rkyv removes that cost.

**v1 limitation.** `src/protocol/frame.rs::recv_frame` does `vec![0u8; header.length as usize]` (one large allocation), reads the payload into that buffer, and then calls `bincode::deserialize::<Message>(&payload)`. The deserialise step walks every byte, reconstructing a full `Partition` with new `Vec`s for agents, ports, redex queue, and border map — another full-size allocation. For a 1 GB frame this is two 1 GB allocations plus a full O(A) byte-walk per `recv`, executed on every round by both the coordinator and the workers. Empirically this is a substantial fraction of the per-round time when the frame is large.

**v2 change:** Introduce an alternative wire payload format on `Partition` via rkyv archives:

1. Add `rkyv = "0.7"` to `Cargo.toml`. Derive `Archive`, `Serialize`, `Deserialize` (rkyv's traits, not serde's) on `Net`, `Partition`, `Agent`, `Symbol`, `PortRef`, and `WorkerRoundStats`, alongside the existing serde derives. Bincode v2 coexists for the small control messages (`Register`, `Shutdown`, `Error`) where the cost of rkyv's alignment padding is not worth the savings.

2. Add a new message variant `AssignPartitionArchived { round: u32, archive: Vec<u8> }` and the symmetric `PartitionResultArchived { round, archive, stats }`. These carry an rkyv-serialised `Partition` inside the `archive` field. The frame header already supports arbitrary byte payloads, so the framing layer does not need to change.

3. In the coordinator (`src/protocol/coordinator.rs::distribute_partitions`), serialise each partition with `rkyv::to_bytes`, then wrap in the archived variant. On the worker side (`src/protocol/worker.rs`), receive the archived variant and call `rkyv::access::<ArchivedPartition>(&archive)` to obtain a `&ArchivedPartition` view into the received buffer. The reduction loop then reads from the archive directly, without materialising a full `Partition` in the old sense. When the worker finishes, it allocates a fresh buffer and serialises the result back.

4. **Key trade-off:** rkyv requires alignment constraints. The `recv_frame` path must hand the worker an aligned buffer. One implementation path is to allocate the recv buffer via `aligned_vec::AVec<u8, 16>` instead of `vec![0u8; len]`. This adds one dependency and costs a handful of lines.

5. **Security posture:** rkyv's `access` is safe *iff* the archive is validated first (`rkyv::access` vs `rkyv::access_unchecked`). The safe path runs a structural validation pass before returning the reference. This costs some CPU but not nearly as much as a full deserialise, and defends against malicious WAN peers in 2.21's threat model.

6. **Tests:** roundtrip a realistic `Partition`, assert the archived size is within 20 % of the bincode+varint size, measure the recv-side CPU cost on a 1 GB frame and assert it is <50 ms (rkyv's structural validation).

7. **SPEC-06 update:** the new message variants are appended to preserve bincode discriminant stability; SPEC-06 R5 continues to hold. Document the rkyv path in a new section on the archive format.

**Why v1 did not do this.** Two reasons: (a) rkyv has stricter derive requirements than serde and does not compose with `#[serde(with = "...")]` adapters, so the existing `serialize_subnet_compact` adapter in `src/partition/compact.rs` would need to be rewritten; (b) the rkyv archive format is not human-inspectable the way bincode + varint is, which makes debugging a wire-level bug harder. v1 chose readable + debuggable over fast.

**Why v2 should do this.** The `rust_serialization_benchmark` public results put rkyv at 3-10x faster than bincode on deserialise for large nested structures, and the zero-copy property is a *direct* CPU saving, not a wire saving. Phase 3 LAN will spend proportionally more time in recv (because bandwidth is the bottleneck there), so the saving is amplified. Stacks with 2.23: archived payloads can also be LZ4-compressed, though the gain is smaller because rkyv is already compact.

**Relationship to other items.** Complementary to 2.23 (wire compaction) and 2.22 (TCP tuning). Superseded by 2.26 (delta protocol) on the happy path — if we only send border deltas, the per-frame cost of rkyv vs bincode is less impactful because frames are small. But 2.26 is much more invasive, so 2.24 is worth shipping first as a standalone improvement.

**Complexity.** Medium-High. Approximately 600 lines: 250 for rkyv derives and the archive variant wiring, 120 for the aligned buffer in `recv_frame`, 80 for the coordinator/worker archive path, 80 for tests, 70 for SPEC-06 updates. Estimated effort: 2-3 days plus the standard review pipeline.

**v1 exclusion source:** absence of rkyv derives in `src/net/types.rs` / `src/partition/types.rs`, `recv_frame` uses `vec![0u8; len]` without alignment guarantees, SPEC-06 R5.

**References:**
- [rkyv docs.rs](https://docs.rs/rkyv) — official documentation, includes zero-copy access patterns and validation APIs.
- [rkyv: zero-copy deserialization](https://rkyv.org/zero-copy-deserialization.html) — rationale and benchmark context.
- [rust_serialization_benchmark](https://github.com/djkoloski/rust_serialization_benchmark) — direct bincode-vs-rkyv numbers on deserialise.
- [Manish Goregaokar, Zero-Copy All the Things](https://manishearth.github.io/blog/2022/08/03/zero-copy-2-zero-copy-all-the-things/) — tutorial on applying zero-copy techniques in Rust.

### 2.25 Same-Host Fast Path (Unix Domain Sockets / Shared Memory)

**Scope.** When coordinator and workers are on the same host — which is always the case in the v1 Phase 2 Docker Compose setup, and is the norm for "try it on my laptop" local mode — bypass the TCP/IP stack entirely and move data through a faster channel.

**v1 limitation.** v1 uses `tokio::net::TcpStream` unconditionally. On Linux TCP loopback adds a full pass through the kernel's network stack: softirq, netfilter, routing, checksumming (even though the kernel elides the actual CRC for loopback, the code path is still walked), and at least one scheduler wake-up per packet. Published benchmarks consistently show Unix domain sockets delivering 2-3x lower latency and up to 7x higher throughput for same-host communication, and `memfd_create` + shared mmap delivering another 10-30x on top for very large buffers. The v1 Phase 2 "tcp_localhost" mode measures the cost of going through that stack on every round; that cost contaminates the Phase 3 subtraction, because the localhost reference used to subtract network cost is itself network-taxed.

The `tcp_localhost` mode name is technically accurate but strategically misleading: it is not a "zero-network" reference point, it is a "loopback-network" reference point, which is slower than true in-process by a measurable and load-dependent amount.

**v2 change:** Introduce a transport abstraction over the existing `TcpStream` path:

1. Add an enum `TransportBackend { Tcp, UnixSocket, SharedMemory }` in `src/protocol/config.rs`. Each variant carries its own config struct (bind address for TCP, socket path for UDS, shm segment name + size for SHM).

2. Introduce a `Transport` trait with `accept` / `connect` methods returning a `Pin<Box<dyn AsyncRead + AsyncWrite + Unpin + Send>>`. Implement it for TCP (wraps the existing logic), Unix sockets (`tokio::net::UnixListener` / `UnixStream`), and shared memory (a ring-buffer over `memfd_create` with futex-based wakeups, or the `raw-sync` / `shared_memory` crate for a ready-made primitive).

3. The protocol layer (`frame.rs`, `coordinator.rs`, `worker.rs`) stays unchanged — it is already generic over `AsyncRead + AsyncWrite`. Only the *connection setup* in `accept_workers` and `connect_to_coordinator` switches on `TransportBackend`.

4. **Auto-selection heuristic:** if `config.bind` is `127.0.0.1:*` or the coordinator and worker are in the same network namespace (Docker Compose bridge detection), log a warning and recommend switching to UDS. An explicit CLI flag `--transport={tcp,unix,shm}` keeps it manual for now.

5. **Shared memory option (advanced):** for the same-host case with 1 GB frames, mmap a ring buffer once at startup and exchange only offsets/lengths through a small Unix socket. This eliminates every kernel-side memory copy: the coordinator writes the serialised partition into the shared buffer, sends the offset, the worker reads from the shared buffer into its local `Partition`, and the inverse on the return path. Requires careful synchronisation but, once it works, approaches the theoretical minimum of "the memcpy plus the wake-up latency".

6. **Docker Compose wiring:** replace the TCP port publish with a bind mount `./run:/run` and switch to `--transport=unix --socket-path=/run/relativist.sock`. The docker-compose.yml becomes slightly more complex but the coordinator and workers see each other with zero TCP overhead.

7. **Tests:** protocol integration tests run against all three transports in CI, and the test helpers gain a `with_transport(backend)` combinator.

8. **SPEC-06 + SPEC-07 updates:** describe the transport abstraction and the auto-selection heuristic, while keeping SPEC-06 R5 discriminant stability intact.

**Why v1 did not do this.** The TCC's research question is about the *distributed* case, and TCP is the transport used in Phase 3 LAN. Switching to UDS / SHM for Phase 2 would have made the cross-phase comparison ambiguous: Phase 2 would measure "same-host optimal transport", Phase 3 would measure "LAN TCP", and the subtraction would conflate the transport switch with the network switch. v1 chose to pay the tcp_localhost tax uniformly so that Phase 3 subtraction has one well-defined term.

**Why v2 should do this.** The Phase 3 subtraction methodology can be updated to compare (a) same-host optimal (UDS or SHM) vs (b) LAN TCP — this gives a much cleaner isolation of the "going over a network" cost than the current (a) same-host TCP vs (b) LAN TCP. And for ordinary users running Relativist on a laptop or a single Docker host, the overhead drop is substantial (often 3-10x on IO-bound phases). This item is also the prerequisite for a "volunteer-on-a-laptop" UX that does not require opening TCP ports.

**Relationship to other items.** Stacks with 2.22, 2.23, 2.24. Orthogonal to 2.26. Makes 2.21 slightly harder (the transport abstraction must handle TLS for the TCP path but not for UDS/SHM, which is fine — UDS/SHM do not need application-level encryption on the same host).

**Complexity.** Medium. Approximately 500 lines: 150 for the `Transport` trait and TCP/UDS backends, 200 for the SHM ring-buffer backend (the complex one), 80 for the transport config and CLI flag, 70 for tests. Estimated effort: 3-5 days plus the review pipeline. SHM can be deferred to a later pass; the TCP + UDS step is a few hours of work.

**v1 exclusion source:** `src/protocol/coordinator.rs` and `src/protocol/worker.rs` hardcode `TcpListener`/`TcpStream`, no `Transport` trait exists, SPEC-06 §4.2 treats the socket as "a TCP stream".

**References:**
- [IPC performance comparison (Baeldung)](https://www.baeldung.com/linux/ipc-performance-comparison) — latency/throughput numbers for named pipes, UDS, TCP loopback, and SHM.
- [Yanxu Rui: benchmark TCP/UDS/named pipe](https://www.yanxurui.cc/posts/server/2023-11-28-benchmark-tcp-uds-namedpipe/) — recent reproducible Linux numbers.
- [redhat-performance/rusty-comms](https://github.com/redhat-performance/rusty-comms) — Rust-native IPC benchmark suite that covers UDS, SHM, TCP, POSIX MQ; useful reference for the auto-selection heuristic and for the test harness design.
- [3tilley: IPC in Rust, a ping-pong comparison](https://3tilley.github.io/posts/simple-ipc-ping-pong/) — Rust ping-pong benchmark patterns for UDS vs TCP loopback vs SHM.

### 2.26 Delta-Only Protocol (Stateful Workers) **(confluence-enabled)**

**Scope.** The single most impactful item in this group. Stop sending the entire partition every round and instead send only the *changes* — the new border redexes the coordinator discovered in the merge phase, and the per-worker delta (updated border ports + stats) on the return path. The partition state lives *on the worker* between rounds, not at the coordinator.

This is the architectural pattern that Pregel, Giraph, and essentially every modern BSP graph processing framework uses. v1 ships the whole partition every round because the coordinator is the sole holder of the merged-net state, which simplifies the formal argument (G1: `reduce_all ≡ run_grid`) but pays an enormous wire cost that scales with *partition size* instead of with *change size*.

**v1 limitation.** The current BSP loop in `src/merge/grid.rs::run_grid` is stateless on the worker side. Every round:
1. The coordinator calls `split` to produce K partitions, each containing the full live agents of that worker's ID range.
2. The coordinator sends `AssignPartition { round, partition }` to each worker. For 20 M `ep_annihilation_con` at `w=2`, each partition is ~750 MB of `CompactSubnet` under bincode v1 — and even after 2.20 (`CompactSubnet`) and 2.23 (varint + LZ4), that is still several hundred MB in the worst case.
3. Each worker runs `reduce_all` on its partition, then returns `PartitionResult { round, partition, stats }` carrying the full reduced partition back.
4. The coordinator merges the K partitions, runs the lenient `reduce_all` on border cascades, and starts the next round.

Step 2 is `O(total_live_agents)` per round. Step 3 is `O(total_live_agents − local_annihilations)` per round. Over R rounds, the total wire cost is `O(R × total_live_agents × bytes_per_agent)`. For `ep_annihilation_con` this is trivially dominated by step 2 because `ep_annihilation_con` annihilates fast and the residual partitions are small — the problem is the *initial* send. For `dual_tree` and `cascade_cross` under strict BSP, each round after the first carries a partition that has *barely changed* from the previous round; 95 %+ of the bytes on the wire are retransmitted identical data.

**v2 change:** Workers become stateful. The protocol carries deltas.

1. **Partition lifecycle:** at the start of execution, the coordinator sends `InitialPartition { worker_id, partition }` *once* per worker. The worker stores this partition in its state and never receives a full partition again for the remainder of the run.

2. **Per-round control messages:** each round the coordinator sends `RoundStart { round, new_border_redexes: Vec<(BorderId, PortRef)>, freed_border_ids: Vec<BorderId> }`. `new_border_redexes` are the principal-port pairs that landed at partition boundaries during the previous round's merge and now need to be activated in the appropriate worker. `freed_border_ids` are borders resolved at the coordinator. The payload is *tiny* — typically single-digit percent of the partition size — because most of the state did not change between rounds.

3. **Per-round worker reply:** the worker runs `reduce_all` on its *stateful* partition (applying the new border redexes first), then sends `RoundResult { round, border_deltas: Vec<(BorderId, PortRef)>, interactions: u64, stats: WorkerRoundStats }`. `border_deltas` are the ports whose identities have changed on border positions during this round's reduction; the coordinator uses these to construct the next round's border redex list. The full interior of the partition is never transmitted.

4. **Coordinator-side state:** the coordinator stops holding the full merged net between rounds. Instead, it holds a `BorderGraph` describing the connectivity between worker partitions at the border level, plus a per-worker checksum of the partition interior (for integrity checks). When the BSP loop converges (all workers report zero border deltas and zero pending redexes), the coordinator *requests the full state back* via `FinalStateRequest { worker_id }` and the workers reply with `FinalStateResult { worker_id, partition }`. This final exchange is the only time the full state crosses the wire after round 0. At that point the coordinator merges, writes the output, and terminates.

5. **Impact on G1 (correctness).** The existing proof of `reduce_all ≡ run_grid` relies on the invariant that after every round the coordinator holds a full merged net isomorphic to some intermediate state of the sequential reduction. Under the delta protocol, the coordinator holds only the border graph; the full isomorphism invariant is *distributed* across the coordinator and the workers. The proof needs a new decomposition: "the sequential intermediate state is recoverable from the coordinator's border graph plus the union of all worker partition states". Strong confluence (ARG-001 P1) guarantees that this decomposition exists and is unique up to the confluence equivalence. G1 continues to hold in both lenient and strict BSP modes (SPEC-05 R30a), but the formal argument gets more subtle and needs a fresh pass in the specs.

6. **Impact on dynamic workers (2.2, 2.3).** The delta protocol interacts with dynamic worker joining and departure. When a new worker joins mid-execution, the coordinator has to hand it an `InitialPartition` derived from the *current* merged state, which means the coordinator must be able to *request current state* from existing workers (via the same `FinalStateRequest` mechanism, but mid-run). Similarly, a departing worker's partition is effectively lost and must be re-derived by asking the coordinator to re-partition from some earlier snapshot. The delta protocol is strictly harder than v1 in this respect but still benefits from confluence — any recovery strategy is correct because any order of reductions converges to the same normal form.

7. **Tests.** TDD RED-first: a test `t_delta_1` that runs a single-round net through `run_grid_delta` and asserts the final net matches `reduce_all`. A test `t_delta_multi_round` that runs `cascade_cross(10)` under strict BSP through `run_grid_delta` and asserts both `rounds == 10` and `g1_equivalence`. A test `t_delta_vs_full` that runs `dual_tree=20 w=4` through both the old and the new protocol and asserts byte-identical outputs. An integration test that measures the *wire size* per round under the delta protocol vs the current protocol on the 20 M smoke workload and asserts at least a 10x reduction.

8. **SPEC updates:** SPEC-05 gets a new section on delta protocol; SPEC-06 gets the new message variants; SPEC-01 D6 gets a delta-aware formulation; a new SPEC-17 or SPEC-18 may be warranted for the border graph structure.

**Why v1 did not do this.** Two reasons: (a) the stateful-worker design complicates G1 proof sketches, and the TCC's central claim is about G1, so v1 chose the simplest design that preserves the "coordinator holds merged state" invariant; (b) shipping the delta protocol before the strict-BSP mode (2026-04-11 v0.10.0-bench) would have made the multi-round question un-testable, because there was no way to force rounds > 1 to exercise the delta path. With strict BSP shipped and `cascade_cross` / `dual_tree` strict data in `v1_local_baseline`, the delta protocol now has a well-defined correctness target to aim at.

**Why v2 should do this.** This is the item that makes Relativist *viable as a grid computing system* rather than a distributed reducer. The ep_con 20 M stress smoke (wall-clock 27.88 s Docker TCP w=2 vs 8.02 s sequential, 3.48x overhead) would likely drop to a 1.3-1.6x overhead range once the delta protocol is combined with 2.22 and 2.23. That is the range where Phase 3 LAN measurements genuinely reflect network RTT rather than transport-layer waste. Without 2.26, Relativist's distributed mode has a wire cost that scales with partition size regardless of how much work was actually done in the round, which is a structural limitation.

The confluence-enabled tag is literal: this design is *only correct* because strong confluence guarantees that the distributed state (coordinator border graph + worker partition interiors) is a well-defined decomposition of the sequential intermediate state. A term-rewriting system without confluence could not use this pattern — it would need either consensus (to agree on what the current state "is") or a rollback protocol.

**Relationship to other items.** This is the biggest and hardest item in the group, but it is also the one that would change Relativist's architecture most deeply. It should be shipped *after* 2.22, 2.23, 2.24 provide their incremental wins, so that the delta protocol can be A/B benchmarked against a well-tuned baseline. It supersedes a significant portion of 2.23 and 2.24 on steady-state wire cost (because per-round frames are small), but does not supersede them on the initial `InitialPartition` send and the `FinalStateResult` exchange, where the full partition still crosses the wire and benefits from every other wire optimisation.

Complements 2.16 (streaming reduction) and 2.17 (streaming arithmetic) — a delta protocol is the natural wire-level companion to a streaming reduction loop, because both exchange partial results instead of full normal forms.

**Complexity.** High. Approximately 1400-1800 lines: 400 for the `BorderGraph` and worker partition state management, 300 for the new message variants and FSM transitions, 250 for the coordinator's delta planner, 300 for the correctness proof in SPEC-01/05 + new SPEC-17/18, 250 for tests, 200-300 for migration and deprecation of the old protocol. Estimated effort: 2-4 weeks of focused work plus the standard spec/debate review pipeline. Not a weekend project; this is the centrepiece of a v2.x milestone.

**v1 exclusion source:** `src/merge/grid.rs::run_grid` (full-partition ship per round), SPEC-06 R5 (message stability), SPEC-13 R2 (BSP barrier on full partitions), OBJETIVO_TCC.md (G1 proved against the simpler architecture).

**References:**
- [Pregel: A System for Large-Scale Graph Processing — the morning paper](https://blog.acolyer.org/2015/05/26/pregel-a-system-for-large-scale-graph-processing/) — Google's original BSP graph processing paper that introduces the "vertices send messages to neighbours" delta pattern.
- [Giraph Unchained: Barrierless Asynchronous Parallel Execution in Pregel-like Graph Processing Systems](http://www.vldb.org/pvldb/vol8/p950-han.pdf) — VLDB paper on the evolution of Giraph's messaging model.
- [Giraphx: Parallel Yet Serializable Large-Scale Graph Processing](https://www.researchgate.net/publication/262351791_Giraphx_Parallel_Yet_Serializable_Large-Scale_Graph_Processing) — introduces the border-vertex / internal-vertex split that this item adapts to interaction nets.
- [A communication-reduced and computation-balanced framework for fast graph computation (Frontiers of Computer Science)](https://link.springer.com/article/10.1007/s11704-018-6400-1) — LCC-BSP model, directly motivates the delta approach.
- [Round- and Message-Optimal Distributed Graph Algorithms (Haeupler et al., CMU)](https://www.cs.cmu.edu/~dwajc/pdfs/haeupler18.pdf) — theoretical bounds on how much communication can be reduced under BSP.

#### 2.26 Invariant Amendments (SPEC-19 §3.5 — TASK-0392)

This is narrative documentation of the SPEC-19 amendments. The formal text lives in SPEC-19 §3.5; the formal proofs are TCC work items (OQ-1, ARG-005, DISC-011). No SPEC-01 edit is required — SPEC-19 §3.5 is the canonical amendment location per the spec-critic ruling on AMB-D-3.

**G1 (Fundamental Property) — R38. *Proof pending.***
The delta protocol keeps the merged net distributed across workers between rounds. G1 is reformulated from `reduce_all(net) ~ extract_result(run_grid(net, n))` to `reduce_all(net) ~ extract_result(run_grid_delta(net, n))`, where `extract_result` performs Final State Collection (R27-R29) followed by `merge()`. SPEC-19 §3.5 R38 states this reformulation as MUST but defers the full formal proof of the recoverability property to Section 8 (OQ-1 → DISC-011 → ARG-005); this spec defines the design, and the theoretical proof is a separate TCC deliverable. **Operational guarantee in the implementation:** sub-bundle 2.26-C's convergence test exercises a canonical workload end-to-end with `delta_mode = true` and compares the decoded result to the `reduce_all` reference — this is the engineering discharge while the formal argument is outstanding.

**D3 (Border Completeness) — R39. *Proof pending.***
Border-redex detection is incremental via `BorderGraph.detect_border_redexes()` (shipped in bundle 2.35, TASK-0374..0388). The v1 exhaustive scan in `merge::find_border_redexes` is retained for the `delta_mode = false` path; the two oracles return the same set of redexes for any reachable coordinator state. Equivalence between the incremental and exhaustive oracles is the core correctness claim of the delta protocol and is pending formal proof (SPEC-19 §3.5 R39; see Section 8). The bundle 2.35 probes QA-A..QA-H (all passing; see the bundle 2.35 closure entry in `docs/progress.md`) are the empirical discharge until the proof lands.

**D6 (Protocol Termination) — R40. *Operationally complete after 2.26-C.***
Termination is anchored on **Global Normal Form**: (a) every worker reports `local_redexes == 0` AND `has_border_activity == false` (TASK-0348), AND (b) `BorderGraph.detect_border_redexes()` returns empty (bundle 2.35). The joint check is performed by the delta BSP loop in bundle 2.26-C (`check_delta_convergence`, TASK-0386). The termination argument is operational and self-contained in the spec text: each round consumes ≥ 1 interaction from the finite T7 budget, so the protocol terminates in ≤ N rounds strict / 1 round lenient (`R_delta_lenient = 1` in the absence of cross-partition cascades; `R_delta_strict ≤ N` matches the v1 strict-mode bound). No Section 8 deferral applies to R40. **Status:** PARTIAL after 2.26-D (this sub-bundle ships only the `delta_mode` flag plumbing via TASK-0389/0390 and the narrative notes here); **COMPLETE after 2.26-C** (the delta BSP loop with the Global Normal Form termination check ships there).

**Configuration mechanism.** The `GridConfig.delta_mode: bool` field (TASK-0389) and the `--delta-mode` CLI flag (TASK-0390) toggle between the v1 path and the amended path. Default is `false` (SPEC-19 §3.6 R42 — backwards compatibility). The regression test `r42_default_delta_mode_preserves_v1_smoke_output` (TASK-0391) asserts that the default branch produces bit-identical behaviour to the pre-bundle baseline on a canonical smoke workload.

---

## v2 — Memory-Bounded Distributed Reduction (items 2.27–2.36) **(confluence-enabled)**

The v1 architecture is completely eager: `bench.make_net(size)` constructs ALL agents in coordinator memory, `split(net, num_workers, strategy)` requires the FULL net to compute the allocation function σ, the coordinator holds the full net plus all K partitions during dispatch, and after each BSP round `merge()` reconstructs the full net on the coordinator. Memory per agent is ~64 bytes (8 bytes `Agent` + 3 × ~8 bytes `PortRef` indexed + padding). A 100 M agent net ≈ 6.4 GB. A 1 B agent net ≈ 64 GB — exceeding the RAM of any single node in a realistic volunteer grid.

The coordinator should be capable of GENERATING the net and DISTRIBUTING work simultaneously. Instead of build-all-then-split, the coordinator should pipeline generation with partitioning and dispatch. The net should never need to fully exist in memory at once.

**Theoretical foundation.** Strong confluence (Lafont 1997, Property P1 in ARG-001) guarantees that the result of reduction is identical regardless of who reduces what and in what order. This means partial nets can be dispatched safely: if worker W receives agents A1..A500 and worker V receives agents A501..A1000, the union of their reductions converges to the same normal form as reducing the full net sequentially. Mackie and Sato (REF-015) demonstrated that streaming operations enable dramatically more parallelism than batch — the same principle applies to net generation and dispatch, not just reduction.

**The four memory peaks in v1.** Each is a separate bottleneck requiring a different technique:

1. **Generation peak:** `make_net(size)` allocates the full `Net` (in `src/io/generators.rs`, `src/encoding/church.rs`). **Addressed by 2.27, 2.29, 2.36.**
2. **Partitioning peak:** `split()` in `src/partition/split.rs` requires the full net to compute σ via `strategy.allocate(&net, num_workers)`, classify all wires, and build all K subnets simultaneously. **Addressed by 2.28, 2.30.**
3. **Dispatch peak:** The coordinator holds the full net AND all K partitions between `split()` return and the last `AssignPartition` send. **Addressed by 2.27, 2.30.**
4. **Merge peak:** `merge()` in `src/merge/core.rs` allocates a result `Net` sized to `max_agents_len` and copies all partition data into it. **Addressed by 2.33, 2.34, 2.35.** (Also partially by 2.26 Delta-Only Protocol.)

### 2.27 Streaming Net Generation (Producer-Consumer Pipeline)

Replace the `Benchmark::make_net(&self, size: u32) -> Net` signature with a streaming variant that emits agents in bounded batches instead of constructing the full net upfront.

**How it would work in Relativist.**

1. Add a new trait method to `Benchmark` (in `src/bench/mod.rs`):
   ```rust
   fn make_net_stream(&self, size: u32) -> Box<dyn Iterator<Item = AgentBatch>>
   ```
   where `AgentBatch` contains a `Vec<(AgentId, Symbol, Vec<(PortId, PortRef)>)>` — a batch of agents with their connections. The batch size is configurable (default: 10,000 agents), bounding memory to `batch_size × 64 bytes` per batch in flight.

2. The existing `make_net()` becomes a convenience wrapper: `fn make_net(&self, size: u32) -> Net { collect_stream(self.make_net_stream(size)) }`. Backward compatibility preserved for callers that need the full net (sequential baseline, verification).

3. Each generator in `src/io/generators.rs` gains a streaming variant. For `ep_annihilation`, the stream emits batches of ERA-ERA pairs — trivial because pairs are independent. For `dual_tree`, the stream emits tree layers bottom-up, which is more complex because parent nodes need to reference child ports from previous batches.

4. The coordinator's BSP loop (`src/merge/grid.rs`) gains a new entry point `run_grid_streaming()` that reads batches from the stream, assigns each batch to a worker via the partitioning strategy, and dispatches immediately. The coordinator never holds more than one batch in memory plus the border tracking structure.

5. **Channel-based backpressure.** When integrated with tokio (Phase 3), the stream becomes an async `Stream` over a bounded `tokio::sync::mpsc` channel. If the coordinator cannot dispatch fast enough, the channel fills up and the generator blocks — natural bounded memory.

**Challenge: cross-batch wires.** When an agent in batch B1 has a port connected to an agent in batch B2 (not yet emitted), the connection target does not exist at emission time. Two resolution strategies:

- **(a) Forward references.** The batch carries `PendingConnection(source_agent_id, source_port, target_agent_id, target_port)` entries. The coordinator (or the receiving worker) buffers these and resolves them when the target agent arrives. This is how `dual_tree` would work: leaf-to-parent connections are forward references resolved when the parent batch arrives.
- **(b) Two-pass emission.** The generator emits all agents in pass 1 (DISCONNECTED ports), then all connections in pass 2. Simpler but wastes the opportunity to dispatch agents while connections are still being computed.

Strategy (a) is preferred because it allows pipelining.

**What it solves.** Eliminates memory peak #1 (generation) and #3 (dispatch, when combined with 2.30). Coordinator peak memory drops from `O(total_agents)` to `O(batch_size + border_tracking_state)`.

**What it does NOT solve.** Partitioning peak (#2) — requires a streaming partitioning strategy (2.28). Merge peak (#4). Does not help with the sequential baseline, which still needs the full net for `reduce_all`.

**Complexity:** Medium. ~500 lines: 80 for `AgentBatch` type and trait extension, 200 for streaming generator variants, 150 for `run_grid_streaming()` skeleton, 70 for tests.

**Dependencies:** 2.28 (streaming partitioning) for full benefit. Can be implemented independently but only useful if the partitioning strategy can also operate on batches.

### 2.28 Online/Streaming Graph Partitioning

Replace `ContiguousIdStrategy` (which requires a full scan of all live agents in `src/partition/strategy.rs`) with partitioning strategies that can assign agents to workers on-the-fly, one batch at a time, without a global view of the net.

**How it would work in Relativist.**

1. Add a new trait alongside `PartitionStrategy`:
   ```rust
   trait StreamingPartitionStrategy {
       fn allocate_batch(&mut self, batch: &AgentBatch, num_workers: u32) -> Vec<(AgentId, WorkerId)>;
       fn finalize(&self) -> StreamingPartitionStats;
   }
   ```
   The strategy is stateful (`&mut self`) because it may track per-worker load for balancing.

2. **Hash-based strategy (simplest, stateless):**
   ```rust
   fn allocate_batch(&mut self, batch, num_workers) -> ... {
       batch.agents.iter().map(|(id, ..)| (*id, id % num_workers)).collect()
   }
   ```
   No global view needed. O(1) per agent. Partition quality is poor — hash assignment ignores topology entirely. But it *works* and requires zero memory beyond the current batch.

3. **FENNEL/LDG streaming partitioning (better quality):** The FENNEL algorithm (Tsourakakis et al., KDD 2014) and LDG (Stanton & Kliot, 2012) maintain per-worker degree counters and assign each vertex to the worker where it has the most existing neighbors, with a capacity penalty. For Relativist: the strategy tracks `worker_degree[w]` (total agents assigned to w) and `worker_neighbors[w]` (connections from the current agent to agents already assigned to w). The state is O(num_workers) per agent — tiny. When assigning agent A, look at its ports; for each port connected to an already-assigned agent B, increment `worker_neighbors[σ(B)]`. Assign A to `argmax(neighbors[w] - α × degree[w])`. This requires a lookup table `sigma_cache: HashMap<AgentId, WorkerId>` that grows to O(total_agents) — but only the ID-to-worker mapping (~8 bytes per agent vs. ~64 bytes per agent for the full net). For 100 M agents: ~800 MB vs. ~6.4 GB — an 8× reduction.

4. **Round-robin strategy (middle ground):** Assign agents `agent_index % num_workers`. Same partition quality as `ContiguousIdStrategy` for sequential generators. O(1) per agent, no state. Likely the default for v2 because it preserves existing partition quality while enabling streaming.

**What it solves.** Eliminates memory peak #2 (partitioning). The partitioner processes one batch at a time.

**What it does NOT solve.** Wire classification in `src/partition/helpers.rs::classify_wires` currently scans all agents to identify border wires. Under streaming partitioning, border wires are discovered incrementally — a wire becomes a border when its endpoints land on different workers. The border map must be built as an accumulator, changing the `WireClassification` return type.

**Trade-off: partition quality vs. memory.** `ContiguousIdStrategy` produces near-optimal quality for the TCC benchmark suite because agent IDs correlate with topology. Hash/round-robin strategies lose this correlation. The trade-off is acceptable: for nets too large to fit in memory, ANY partitioning that *works* is infinitely better than one that requires the full net.

**Complexity:** Medium. ~400 lines: 60 for the trait, 80 for hash/round-robin strategies, 150 for FENNEL adaptation, 110 for tests.

**Dependencies:** 2.27 (streaming generation) provides the batches. Can be tested independently with a synthetic batch iterator.

**References:**

- [FENNEL: Streaming Graph Partitioning for Massive Scale Graphs (Tsourakakis et al., KDD 2014)](https://doi.org/10.1145/2556195.2556213) — the balanced streaming partitioner that trades global optimality for single-pass, bounded-memory operation.
- [Streaming Graph Partitioning for Large Distributed Graphs (Stanton & Kliot, KDD 2012)](https://doi.org/10.1145/2339530.2339722) — LDG heuristic, predecessor to FENNEL.

### 2.29 Distributed/Parallel Net Generation (Recipe-Based)

The most memory-efficient approach: the coordinator never holds the net at all. Instead of generating the net centrally and distributing partitions, the coordinator sends a *generation recipe* to each worker. Each worker generates its own portion of the net deterministically, skipping the coordinator entirely.

**How it would work in Relativist.**

1. Add a `GenerationRecipe` struct (in `src/bench/mod.rs` or a new `src/generation/mod.rs`):
   ```rust
   struct GenerationRecipe {
       benchmark_id: BenchmarkId,
       size: u32,
       worker_id: WorkerId,
       num_workers: u32,
       id_range: IdRange,
   }
   ```

2. Add a new method to `Benchmark`:
   ```rust
   fn make_local_partition(&self, recipe: &GenerationRecipe) -> Partition
   ```
   The worker calls this to generate only its portion. For `ep_annihilation(N)` with K workers: worker `w` generates pairs `w*chunk..min((w+1)*chunk, N)` where `chunk = ceil(N/K)`. Each worker creates agents with IDs in its assigned `id_range`, ensuring disjoint ID spaces without coordinator involvement.

3. The coordinator FSM gains a new dispatch path: instead of `SplitComplete(Vec<Partition>)` followed by `AssignPartition`, the coordinator sends `AssignRecipe { recipe }`. The worker generates its partition locally and immediately begins reduction.

4. **Border wire handling.** For `ep_annihilation_con` (independent pairs), no cross-pair wires → no borders between workers (assuming each worker generates complete pairs). For `dual_tree(D)` with a single root wire connecting two mirrored trees, the recipe must specify border endpoints: `borders: Vec<(PortRef, borderId)>`. The coordinator computes this small border specification (O(borders), not O(agents)) and includes it in the recipe.

5. **Deterministic generation.** Each worker must produce exactly the same net fragment that centralized generation + partitioning would yield. Generators accept an `id_range` parameter for agent ID allocation. Currently `Net::create_agent` uses `next_id` and increments; with recipe-based generation, the worker pre-sets `next_id = recipe.id_range.start` and IDs fall naturally into the assigned range. The `IdRange` infrastructure already exists in `src/partition/helpers.rs::compute_id_ranges`.

**What it solves.** Eliminates ALL four memory peaks on the coordinator side. Coordinator memory is O(K × recipe_size) — typically a few hundred bytes. Workers each hold only their local partition — O(total_agents / K) per worker. This is the theoretical minimum.

**What it does NOT solve.** Not all topologies can be generated distributedly. Church encoding (`encode_church_into` in `src/encoding/church.rs`) builds a DUP chain that must be contiguous. For the TCC benchmark suite, all generators except `tree_sum` and `church_*` are parallelizable. Church numeral generation is inherently sequential because the DUP-chain structure does not decompose cleanly across workers.

**Complexity:** Medium-High. ~700 lines: 80 for `GenerationRecipe` and protocol, 400 for distributed generator variants (one per benchmark), 120 for border specification logic, 100 for tests.

**Dependencies:** None (fully independent of 2.27 and 2.28). Can coexist: use recipe-based generation for simple benchmarks and streaming generation for complex ones.

### 2.30 Chunked Generation with Incremental Partitioning

A hybrid of 2.27 and 2.28: generate the net in chunks of C agents, partition each chunk independently using a streaming strategy, and dispatch chunks to workers as they are generated.

**How it would work in Relativist.**

1. A new function `generate_and_dispatch_chunked()` in `src/merge/grid.rs`:
   ```rust
   fn generate_and_dispatch_chunked(
       bench: &dyn Benchmark,
       size: u32,
       chunk_size: usize,
       num_workers: u32,
       strategy: &mut dyn StreamingPartitionStrategy,
   ) -> Vec<Partition>
   ```
   Internally: generate `chunk_size` agents, assign each to a worker via `strategy.allocate_batch()`, append agents/ports to per-worker `Partition` accumulators, repeat until all agents generated.

2. **Cross-chunk connections.** When agent A (chunk C1, worker W1) connects to agent B (chunk C2, worker W2 ≠ W1), this wire becomes a border. The border map accumulates incrementally. When A connects to B but B has not been assigned yet, the connection is stored as a *pending border* and resolved when B's chunk is processed.

3. **Memory profile.** At any point, the coordinator holds: (a) current chunk (~64C bytes), (b) per-worker partition accumulators (growing, but each sized to local max_id, not global), (c) border map + pending-border buffer. Peak memory is O(total_agents) in the worst case (agents live in accumulators), but with an important improvement: accumulators use dense `Net` layout sized to `max_agent_id_in_this_worker` rather than `max_agent_id_globally`.

4. **Early dispatch.** If workers support receiving multiple partial partitions (cf. 2.19 chunking on the protocol level), the coordinator can dispatch partial partitions as soon as a chunk is partitioned. Workers accumulate locally. This pipeline keeps coordinator peak memory at `O(chunk_size + border_map_size)` instead of `O(total_agents)`.

**What it solves.** Reduces peaks #1 (generation — one chunk at a time), #2 (partitioning — incremental), and #3 (dispatch — if early dispatch enabled).

**What it does NOT solve.** Merge peak (#4). Sequential baseline. Workers accumulate the full local partition before reduction begins (unless combined with 2.36).

**Complexity:** Medium. ~600 lines: 150 for chunked generation loop, 100 for partition accumulator logic, 80 for pending-border resolution, 120 for early dispatch protocol changes, 150 for tests.

**Dependencies:** 2.28 (streaming partitioning strategy) for chunk assignment. Can use simple round-robin without 2.28.

### 2.31 Memory-Mapped / Out-of-Core Net Representation

Replace the in-memory `Vec<Option<Agent>>` and `Vec<PortRef>` backing the `Net` struct with memory-mapped file-backed storage. The OS virtual memory manager handles paging — only active pages reside in physical RAM.

**How it would work in Relativist.**

1. Add a new `MmapNet` type (in `src/net/mmap.rs`) that mirrors the `Net` API but backs `agents` and `ports` with an mmap-ed anonymous file (Linux: `memfd_create` + `mmap`; Windows: `CreateFileMapping` + `MapViewOfFile`). The `memmap2` crate provides a cross-platform abstraction.

2. `MmapNet::create_agent()`, `connect()`, `get_target()` work through raw pointer arithmetic into the mapped region, with the same `agent_id * PORTS_PER_SLOT + port_id` indexing. The OS pages in/out 4 KB pages as needed.

3. The reduction engine in `src/reduction/engine.rs` would need to be made generic over a `NetStorage` trait (or `MmapNet` would implement `Deref` to slice types that the engine already consumes).

4. **Arena size.** For 100 M agents: `agents` = 800 MB, `ports` = 2.4 GB. Total mapped: ~3.2 GB. On a 4 GB RAM machine, the OS pages most to disk, with only actively-accessed pages in memory.

**What it solves.** Allows any single node to handle nets larger than physical RAM, transparently. Existing code barely changes.

**What it does NOT solve.** I/O latency. Disk-backed pages are ~1000× slower than RAM. The reduction hot path (`reduce_step` in `src/reduction/engine.rs`) indexes into the port array on every interaction — if the accessed page is on disk, that interaction takes ~0.1 ms instead of ~1 ns. This makes mmap impractical for the reduction loop itself; it is only viable for the generation and partitioning phases where sequential access patterns allow prefetching.

**Platform differences.** Windows and Linux have different mmap semantics, page sizes, and eviction policies. The `memmap2` crate abstracts the API but not the performance characteristics.

**Complexity:** Medium-High. ~500 lines: 200 for `MmapNet` and storage abstraction, 100 for platform-specific setup, 100 for integration with generation path, 100 for tests. Reduction engine generification may require 300+ additional lines.

**Dependencies:** None. Fully independent. Most useful as a stopgap before the more invasive streaming approaches.

### 2.32 Sparse Net Representation

Replace the dense `Vec<Option<Agent>>` with `HashMap<AgentId, Agent>` and the flat `Vec<PortRef>` with `HashMap<(AgentId, PortId), PortRef>`. Memory becomes proportional to live agents only, with no tombstone slots or padding.

**How it would work in Relativist.**

1. Add a `SparseNet` type (in `src/net/sparse.rs`):
   ```rust
   struct SparseNet {
       agents: HashMap<AgentId, Agent>,
       ports: HashMap<(AgentId, PortId), PortRef>,
       redex_queue: VecDeque<(AgentId, AgentId)>,
       next_id: AgentId,
       root: Option<PortRef>,
   }
   ```

2. `build_subnet()` in `src/partition/helpers.rs` currently allocates `vec![None; max_id + 1]` — under `ContiguousIdStrategy`, the last worker's subnet has `max_id = total_agents - 1`. With `SparseNet`, the subnet contains only the worker's actual agents. For `ep_annihilation_con=10M w=4`: each worker owns 2.5 M agents ≈ 160 MB sparse, vs. ~640 MB for the last worker's dense arena.

3. **Hybrid approach.** Use `SparseNet` for construction and partitioning (where proportionality matters), then convert to dense `Net` before the reduction loop (where O(1) indexed access matters). `reduce_step()` in `src/reduction/engine.rs` accesses `net.agents[id]` and `net.ports[id * 3 + port]` — O(1) lookups that would become O(1) amortized but with 5-10× worse constant factor due to hashing and cache misses. The hybrid avoids this: `SparseNet::to_dense() -> Net` is O(live_agents) and runs once per partition.

**What it solves.** Reduces memory for construction and partitioning phases (#1, #2). Effective when the dense arena has high tombstone ratios.

**What it does NOT solve.** Reduction hot path (dense indexing is fundamental). Converting sparse-to-dense before reduction means peak memory during reduction is unchanged. Merge peak (#4) unchanged.

**Complexity:** Medium. ~600 lines: 200 for `SparseNet` struct and API, 100 for hybrid conversion, 150 for integration with `build_subnet()`, 150 for tests.

**Dependencies:** None. Can be applied independently as an optimization to `build_subnet()`.

### 2.33 Arena Recycling / GC During Reduction

Add periodic compaction to the `Net` struct: copy live agents to a new, smaller arena, reclaiming tombstoned slots. This bounds memory for workloads where agents are created then destroyed.

**How it would work in Relativist.**

1. Add a `Net::compact()` method (in `src/net/core.rs`):
   - Allocate new `agents` and `ports` Vecs sized to `live_count + headroom`.
   - Build a remapping table `old_id → new_id` for live agents.
   - Copy agents with new IDs; update all `PortRef::AgentPort(old_id, port)` references.
   - Update `redex_queue`, `root`, `freeport_redirects`.

2. Trigger compaction in `reduce_all` (or `reduce_n`) when tombstone ratio exceeds a threshold: `tombstones / arena_len > 0.5` and `arena_len > 100_000`. Amortizes the O(live_agents) copy cost.

3. **ID renumbering is the hard part.** Every `PortRef::AgentPort(id, port)` in the port array, redex queue, `freeport_redirects`, and `root` must be updated. If any external reference to old IDs exists (e.g., the border map in a partition), compaction invalidates it. Compaction must be restricted to contexts with no external references: during `reduce_all` on a standalone net or after all external structures are internalized.

4. **Alternative: free-list recycling without renumbering.** Maintain a free list of tombstoned slots and reuse them for new agents. `create_agent()` pops from the free list instead of incrementing `next_id`. Avoids renumbering entirely but does not shrink the arena — only prevents growth when new agents replace destroyed ones. For `con_dup_expansion`: significant benefit (each commutation destroys 2 and creates 4 — the 2 destroyed slots are immediately reused).

**What it solves.** Bounds memory growth for workloads with high agent turnover. After compaction, memory proportional to live agents, not high-water mark.

**What it does NOT solve.** Generation peak (#1), partitioning peak (#2), merge peak (#4). Only helps during reduction.

**Complexity:** High (full compaction with renumbering) / Low (free-list only). Full: ~700 lines. Free-list: ~150 lines (strictly local change to `Net`).

**Dependencies:** None. Independent of all other items.

### 2.34 Coordinator-Free Round (Merge Avoidance) **(confluence-enabled)**

In strict BSP mode, when a worker's partition has no border redexes after local reduction, the worker can proceed to the next round without returning to the coordinator. The coordinator only involves when border redexes require resolution.

**How it would work in Relativist.**

1. After `reduce_all` (or `reduce_n`), each worker inspects its `free_port_index` to determine whether any border ports are involved in active pairs. If no border agent's principal port is connected to another agent's principal port → partition is "internally stable."

2. The worker reports `RoundResult { has_border_redexes: false, stats }` — a lightweight message without the full partition payload.

3. The coordinator tracks per-worker border status. When ALL workers report `has_border_redexes: false`, the net is in Normal Form (internal redexes resolved by each worker, no border redexes). The coordinator requests final state and terminates.

4. When SOME workers report border redexes, the coordinator merges only partitions with border involvement. Workers without border redexes skip the merge entirely and retain their partition state.

**What it solves.** Reduces coordinator memory peaks during rounds where no merge is needed. Reduces network traffic. Effective for `ep_annihilation` workloads where most agents annihilate internally.

**What it does NOT solve.** Initial generation/partitioning/dispatch peaks.

**Complexity:** Low-Medium. ~300 lines: 100 for border-redex detection on worker side, 80 for coordinator per-worker tracking, 70 for skip-merge logic, 50 for tests.

**Dependencies:** Benefits from 2.26 (Delta Protocol) but can be implemented independently. Requires strict BSP mode (already shipped in v0.10.0-bench).

### 2.35 Delta-Based Merge (Lightweight Border Resolution) **(confluence-enabled)**

Replace the full-net reconstruction in `merge()` with a protocol where the coordinator holds only the border graph and applies delta updates from workers. The full net is never reconstructed until final output.

**How it would work in Relativist.**

1. Add a `BorderGraph` struct (in `src/merge/types.rs`):
   ```rust
   struct BorderGraph {
       borders: HashMap<u32, BorderState>,   // borderId → current state
       worker_borders: Vec<Vec<u32>>,        // per-worker list of borderIds
   }
   ```
   where `BorderState` stores the two endpoints and whether they form an active pair.

2. After each round, workers send only border port changes: `Vec<(borderId, PortRef)>` — a list of border ports whose targets changed during local reduction. Coordinator updates `BorderGraph`.

3. When a border becomes an active pair (both endpoints are principal ports), the coordinator resolves it. Two options:
   - **(a)** Coordinator requests the two involved agents from respective workers, performs the interaction locally, sends results back as deltas. Stays within the star topology.
   - **(b)** Coordinator sends `ResolveBorder` to one worker, which fetches remote agent data from the other worker (peer-to-peer). Requires worker-to-worker communication.

   Option (a) is simpler and compatible with v1's star topology.

4. **Convergence check.** The BSP loop converges when all workers report zero local redexes AND `BorderGraph` has zero active pairs. The coordinator requests full partitions from workers for final output.

**What it solves.** Eliminates memory peak #4 (merge). Coordinator steady-state memory: `O(num_borders × const)` instead of `O(total_agents)`. For `ep_annihilation_con=10M w=4`: borders ≈ 1.25 M, `BorderGraph` ≈ 10 MB vs. ~640 MB for the full merged net.

**What it does NOT solve.** Generation and partitioning peaks (#1, #2). Final output still requires collecting full state.

**Relationship to 2.26.** This is the merge-side counterpart of 2.26's dispatch-side delta protocol. They share the `BorderGraph` concept and should be designed together.

**Complexity:** High. ~800 lines: 200 for `BorderGraph`, 150 for delta application, 150 for border resolution (option a), 100 for convergence check, 200 for tests.

**Dependencies:** Benefits greatly from 2.26 and 2.34. Can be implemented independently.

### 2.36 Lazy / Demand-Driven Generation (Pull Model)

Workers request work when idle (pull model) instead of the coordinator pushing partitions (push model). The coordinator generates the next portion of the net on-demand. Natural backpressure: slow workers do not accumulate unbounded work.

**How it would work in Relativist.**

1. Add new protocol messages: `RequestWork { worker_id }` and `WorkAssignment { agents: AgentBatch, borders: Vec<BorderEntry> }` (or `NoMoreWork` when generation is complete).

2. The coordinator maintains a lazy generator (an `Iterator<Item = AgentBatch>` from 2.27) and a streaming partitioner (from 2.28). When a worker sends `RequestWork`, the coordinator advances the generator by one batch, partitions it, and sends the worker's portion. Peak memory bounded by the number of in-flight batches — at most K batches (one per worker).

3. **Scheduling fairness.** If all workers request simultaneously, the coordinator generates K batches in succession. With a bounded channel (from 2.27), this is naturally rate-limited.

4. **End-of-generation signal.** When the generator is exhausted, coordinator sends `NoMoreWork`. Workers that have received all portions begin reduction.

**What it solves.** Natural backpressure and bounded memory without pre-planning the generation schedule. Particularly useful for heterogeneous grids (WAN deployment per 2.21) where worker speeds vary.

**What it does NOT solve.** Merge phase. Generation order may not be arbitrary: `dual_tree` requires children before parents, `church_add` requires numerals before the combinator. The generator must produce agents in valid topological order, limiting pull flexibility.

**Complexity:** Low-Medium. ~350 lines: 80 for protocol messages, 100 for on-demand dispatch loop, 80 for worker pull logic, 90 for tests.

**Dependencies:** 2.27 (streaming generation) and 2.28 (streaming partitioning). The pull model is an orchestration layer on top of these.

### Recommended Implementation Order

The techniques form a dependency graph with clear critical paths:

**Phase A — Quick wins, independent (1–2 weeks each)**

1. **2.33 Arena Recycling (free-list variant only)** — No renumbering, no dependencies, reduces memory during reduction. ~150 lines. Ship independently.
2. **2.34 Coordinator-Free Round** — Low complexity, direct benefit for strict BSP mode. ~300 lines. Ship independently.

**Phase B — Streaming foundation (3–4 weeks together)**

3. **2.28 Online/Streaming Graph Partitioning** — Implement hash-based and round-robin strategies first. Prerequisite for every streaming approach.
4. **2.27 Streaming Net Generation** — Implement streaming variants for `ep_annihilation`, `ep_annihilation_con`, `con_dup_expansion` (independent-pair generators). Skip tree/Church in first pass.
5. **2.30 Chunked Generation with Incremental Partitioning** — Combine 2.27 and 2.28 into the first working streaming pipeline.

**Phase C — Full coordinator memory elimination (4–6 weeks)**

6. **2.35 Delta-Based Merge** — Eliminate merge peak. Design together with 2.26.
7. **2.29 Distributed/Parallel Net Generation** — For benchmarks with independent-pair structure. Highest impact for practical deployment.
8. **2.36 Lazy/Demand-Driven Generation** — Orchestration layer on top of 2.27 + 2.28.

**Phase D — Supplementary (optional, independent)**

9. **2.32 Sparse Net Representation** — Optimization for `build_subnet()`. Useful but not on the critical path.
10. **2.31 Memory-Mapped Net** — Stopgap for single-node scenarios. Platform-specific complexity may not justify the effort if streaming approaches are available.

### Minimum Viable Pipeline (MVP)

The smallest set of changes that would let the coordinator handle nets larger than available RAM:

1. **2.28 (round-robin streaming partitioner)** — ~80 lines. A trivial `agent_id % num_workers` strategy.
2. **2.27 (streaming generator for `ep_annihilation_con`)** — ~100 lines for one benchmark. Simplest generator (independent pairs).
3. **2.30 (chunked generation loop)** — ~150 lines. Wire generator to partitioner and dispatch chunks.
4. **2.35 (lightweight border-only merge)** — ~200 lines for the minimal version tracking only border state.

**Total: ~530 lines of new code.** This pipeline would allow `ep_annihilation_con=100M` to run on a coordinator with 1 GB RAM and 4 workers each with 2 GB RAM. Coordinator peak memory drops from ~12.8 GB (100 M agents × 2 × 64 bytes) to ~10 MB (batch buffer + border map). Each worker holds ~25 M agents × 64 bytes ≈ 1.6 GB.

The MVP targets `ep_annihilation_con` because it has the simplest topology (independent pairs, trivial borders). Extending to `dual_tree` and `church_*` requires the forward-reference mechanism (2.27) and the border specification (2.29), which are Phase C items.

---

## v2 — User Experience and Deployment (items 2.37–2.39)

The v1 architecture validates the core hypothesis (correct distributed IC reduction) but the user experience of installing, configuring, and running Relativist is rough. SmartScreen warns that the binary is dangerous, there is no installer, PATH must be set manually, connecting machines requires firewall configuration, and the only interface is a CLI with 40+ flags. These barriers prevent adoption beyond the author's own machines and make demonstrations (TCC defense, university labs) unnecessarily difficult.

This section documents three complementary improvements: mesh VPN for effortless networking (2.37), streamlined installation and distribution (2.38), and a graphical user interface (2.39). Together, they transform Relativist from a research prototype into a tool that anyone can install and use — mirroring the CLI + GUI + networking model that makes Docker Desktop one of the most accessible developer tools.

### 2.37 Tailscale Mesh VPN Integration

Use Tailscale (a WireGuard-based mesh VPN) as a recommended companion tool for Relativist networking. Instead of requiring users to configure firewalls, set up port forwarding, or manage VPNs, Relativist would detect and leverage an existing Tailscale installation to connect coordinator and workers across any network.

**v1 limitation.** The coordinator binds to a `SocketAddr` (default `127.0.0.1:9000`, per SPEC-10 R5). Workers connect via `--coordinator HOST:PORT`. Both machines must be on the same network with no firewall blocking port 9000. NAT, corporate VPNs, and Wi-Fi client isolation all prevent connection. Even on a home LAN, the user must configure Windows Firewall rules manually. At a university lab, firewall changes may be prohibited entirely.

**Why NOT embed Tailscale.** Tailscale's core is a Go userspace networking stack (`tsnet`). There is no official Rust binding. Embedding would require either (a) calling the Tailscale daemon via its local API, which requires the user to have Tailscale installed anyway, or (b) linking a Go library via CGO/FFI, which adds massive build complexity and defeats the single-binary Rust philosophy. The `tsconnect` library is WASM-targeted and not suitable for CLI embedding. Conclusion: recommend Tailscale as a companion, do not embed it.

**v2 change — Tailscale as companion with thin CLI wrappers.**

1. **Auto-detect Tailscale interface.** Add a `--bind tailscale` shorthand to the coordinator CLI. When specified, Relativist calls `tailscale ip -4` (or reads the `tailscale0`/`utun` interface) to discover the machine's Tailscale IP and binds to it. No manual IP entry needed.

2. **Coordinator DNS advertisement.** Add `--advertise-dns <name>` to the coordinator. When specified, Relativist calls `tailscale set --hostname <name>` or writes a MagicDNS record so that workers can connect by name (e.g., `relativist worker --coordinator my-coordinator:9000`) instead of IP address. This subsumes ROADMAP item 2.8 (Automatic Node Discovery) for Tailscale-connected machines.

3. **Worker auto-discovery.** Add `--discover tailscale` to the worker CLI. When specified, Relativist calls `tailscale status --json`, parses the peer list, and looks for peers with a Relativist-specific tag or DNS name. If exactly one coordinator is found, the worker connects automatically. If multiple are found, it lists them and asks the user to choose.

4. **Documentation.** Add a "Quick Start: Connecting Two Machines" section to USAGE_GUIDE.md that recommends installing Tailscale on both machines, joining the same tailnet, and running `relativist coordinator --bind tailscale` / `relativist worker --coordinator <tailscale-name>:9000`. This replaces the current multi-step firewall+IP setup.

**Benefits for Relativist.**

- **NAT traversal.** Every device on a tailnet is reachable regardless of NAT, firewall, or network topology. No port forwarding needed.
- **Encryption.** WireGuard encryption is built into every Tailscale connection. For intra-tailnet traffic, this replaces the need for the `tls` feature gate (`rustls`, `tokio-rustls`). The existing TLS feature remains available for non-Tailscale deployments.
- **Stable addresses.** Tailscale assigns stable IPs (100.x.x.x) that don't change with DHCP leases or network switches. A coordinator's address stays constant across sessions.
- **ACL-based auth.** Tailscale ACLs can restrict which devices can connect to the coordinator, replacing or supplementing the token-based authentication in SPEC-10.
- **Zero infrastructure.** No relay server, no VPN server, no certificate authority. Just install Tailscale, join the tailnet, done.

**For the TCC.** Tailscale demonstrates that Relativist can operate as a practical grid computing tool across heterogeneous networks (home, office, university) without infrastructure complexity. This is closer to the BOINC-style volunteer computing vision than a LAN-only deployment. Document in the TCC as a deployment strategy with a single line in the manifest: "transport: Tailscale WireGuard mesh, overhead ≈ 1-2 ms RTT over direct link."

**Relationship to other items.**

- **2.21 (WAN Deployment):** Tailscale is the fastest path to achieving 2.21's goals. 2.21 builds a custom WAN stack (NAT traversal, TLS, relay); 2.37 delegates to a battle-tested existing stack. Complementary: 2.37 for practical deployment now, 2.21 for eventual self-contained operation.
- **2.8 (Automatic Node Discovery):** Tailscale MagicDNS subsumes 2.8 for tailnet-connected machines.
- **2.39 (GUI):** The Network screen in the GUI would surface Tailscale status and peer discovery.

**Complexity:** Low for documentation + `--bind tailscale` shorthand (~100 lines). Medium for auto-discovery (~200 lines + `tailscale status` JSON parsing). Total: ~300 lines plus documentation.

**Dependencies:** Tailscale must be installed on the host (free, cross-platform). No changes to Relativist's protocol layer.

### 2.38 Installation and Distribution UX

Streamline the first-time installation experience across all platforms. The current state: Linux has an install script, Windows has a bare `.exe` that triggers SmartScreen with no installer or PATH setup, and macOS has no pre-built binary at all.

**v1 limitation.** Users who download `relativist.exe` from GitHub Releases face: (1) SmartScreen warning ("Windows protected your PC"), requiring right-click → Properties → Unblock or "More info" → "Run anyway"; (2) no automatic installation — the user must create a directory, move the binary, and add it to PATH manually; (3) no Start Menu entry, no uninstall option, no integration with Windows Add/Remove Programs. On macOS, no pre-built binary exists — users must compile from source.

**v2 change — four improvements.**

**1. Code signing (Windows SmartScreen).**

Apply to **SignPath Foundation** (signpath.org), which provides free Authenticode code signing certificates for open-source projects with OSI-approved licenses. Relativist qualifies (MIT license, public GitHub repository).

Process:
1. Apply at signpath.org with the GitHub repository URL.
2. Enable MFA on the GitHub account (required).
3. Approval takes days to weeks.
4. Once approved, add a signing step to `.github/workflows/release.yml` using the SignPath GitHub Action, after `cargo build --release` for the Windows target.
5. The signed `.exe` and `.msi` suppress SmartScreen on first run.

Alternative: SmartScreen reputation builds over time with enough downloads, but this is unreliable and provides no guarantee. Not recommended as the primary strategy.

Interim measure (already mandated by SPEC-15 R16): document the bypass in USAGE_GUIDE.md and README.md.

**2. Windows installer (MSI).**

Generate a proper MSI installer using **`cargo-wix`** (WiX Toolset v4 integration for Cargo):

1. `cargo install cargo-wix && cargo wix init` generates WiX manifests in the repo.
2. `cargo wix` produces a `.msi` file.
3. Add to `release.yml`: after cross-compilation for Windows, run `cargo wix` and upload the `.msi` as a release artifact alongside the bare `.exe` and `.zip`.

The MSI should:
- Install to `%LOCALAPPDATA%\relativist\bin\relativist.exe`.
- Add the install directory to the user's PATH (no admin required for per-user install).
- Create a Start Menu entry ("Relativist" → opens a terminal with `relativist --help`, or the GUI once 2.39 ships).
- Support silent install via `msiexec /i relativist.msi /quiet`.
- Register in Add/Remove Programs for clean uninstallation.

Alternative: If 2.39 (GUI via Tauri) is pursued, the Tauri bundler generates MSI automatically as part of the GUI build. The `cargo-wix` approach covers the CLI-only distribution path that remains important for servers, Docker, CI, and headless environments.

**3. Package manager submissions.**

| Package Manager | Platform | Effort | User command |
|---|---|---|---|
| **winget** | Windows | Low — submit YAML manifest to `microsoft/winget-pkgs` repo. Automate with `winget-releaser` GitHub Action. | `winget install relativist` |
| **Homebrew** | macOS | Low — create a Formula in a tap repo (`homebrew-relativist`). Requires adding macOS targets (`x86_64-apple-darwin`, `aarch64-apple-darwin`) to `release.yml`. | `brew install andrade-filipe/tap/relativist` |
| **AUR** | Arch Linux | Low — PKGBUILD that downloads the release tarball. | `yay -S relativist` |
| **chocolatey** | Windows | Medium — `.nuspec` package, moderated submission. | `choco install relativist` |
| **cargo install** | Any | Already works | `cargo install --git https://github.com/andrade-filipe/relativist` |

Priority: winget (Windows is the primary pain point), then Homebrew (macOS has no binary), then AUR.

**4. Expanded build matrix.**

Add to `release.yml`:
- `aarch64-unknown-linux-gnu` — ARM Linux (Raspberry Pi, cloud ARM instances)
- `x86_64-apple-darwin` — macOS Intel
- `aarch64-apple-darwin` — macOS Apple Silicon

This unblocks Homebrew and ARM deployments. Cross-compilation via the `cross` tool or GitHub's macOS runners.

**5. First-run experience.**

When `relativist` is invoked with no subcommand, currently clap prints a generic "missing subcommand" error. Change to: print a welcome message with version, a 3-line quick start, and a pointer to `relativist --help` and USAGE_GUIDE.md. Detect first run by checking for the absence of a config/state file.

**Complexity:** Low-Medium per item. SignPath application is a process, not code. MSI via cargo-wix is ~50 lines of CI config plus WiX manifests. Package manager submissions are per-platform maintenance (~1 hour each). Build matrix expansion is ~30 lines in `release.yml`.

**Dependencies:** SignPath requires MFA on the GitHub account. Homebrew requires macOS release targets. winget requires the repo to be public (it is).

**Relationship to 2.39:** Tauri's bundler solves installer + signing + auto-update for the GUI binary. 2.38 covers the CLI-only distribution path that remains critical for servers, Docker, and CI.

### 2.39 Graphical User Interface (Tauri v2)

Build a native desktop application using **Tauri v2** (Rust backend + system webview frontend) that exposes all Relativist CLI functionality through a clean, minimalist GUI. Inspired by Docker Desktop: the CLI remains the primary interface for power users and automation, while the GUI provides an accessible entry point for everyone else.

**v1 limitation.** Relativist is CLI-only. All 11 commands and 40+ flags must be memorized or looked up. Connecting a coordinator and worker requires typing IP addresses, tokens, and port numbers correctly in two separate terminals. Benchmark results are CSV files that require external tools to visualize. There is no visual feedback during reduction — just log lines scrolling in a terminal. For a TCC defense demo, the presenter must switch between terminal windows while explaining what each command does.

**Why Tauri v2.**

Four approaches were evaluated:

| Approach | Binary size | Rust integration | Installer | Code signing | Verdict |
|---|---|---|---|---|---|
| **A: Tauri v2** | 5-15 MB | Native (import crate) | MSI, .dmg, .deb, AppImage | Built-in | **Recommended** |
| B: Web UI (axum) | 0 (uses browser) | Native | None | N/A | Good complement, not primary |
| C: egui | ~10 MB | Native | Manual | Manual | Hard to make beautiful |
| D: Electron | 150+ MB | FFI/IPC bridge | Yes | Yes | Wrong for Rust project |

Tauri v2 is the clear winner because it solves problems across all three sections simultaneously:
- **GUI** — web frontend with React/Svelte/Vue for beautiful, responsive UI
- **Installer** — Tauri bundler generates MSI (Windows), .dmg (macOS), .deb and AppImage (Linux) automatically
- **Code signing** — supports Windows Authenticode and macOS notarization in the build pipeline
- **Auto-update** — built-in updater replaces the current `relativist update` command for GUI users
- **System tray** — coordinator and worker can run as background processes with tray icon status
- **Small binary** — uses the system webview (WebView2 on Windows, WebKit on macOS/Linux), not bundled Chromium

**Architecture.**

```
┌──────────────────────────────────────────────┐
│                  Tauri App                    │
│  ┌────────────────┐  ┌────────────────────┐  │
│  │  Web Frontend   │  │   Rust Backend     │  │
│  │  (Svelte)       │←→│  (Tauri Commands)  │  │
│  │                 │  │        ↓            │  │
│  │  - Dashboard    │  │  relativist crate  │  │
│  │  - Generate     │  │  (existing code)   │  │
│  │  - Reduce       │  │        ↓            │  │
│  │  - Grid         │  │  - net/core.rs     │  │
│  │  - Coordinator  │  │  - reduction/      │  │
│  │  - Worker       │  │  - partition/      │  │
│  │  - Bench        │  │  - protocol/       │  │
│  │  - Calculator   │  │  - commands.rs     │  │
│  │  - Network      │  │  - bench/suite.rs  │  │
│  │  - Settings     │  │                    │  │
│  └────────────────┘  └────────────────────┘  │
└──────────────────────────────────────────────┘
```

The Tauri backend imports the existing `relativist` crate as a library dependency and exposes `#[tauri::command]` functions that wrap the existing command handlers. The web frontend calls these via Tauri's IPC bridge. No code duplication — the GUI and CLI share the same core logic.

Key architectural decision: the project becomes a **Cargo workspace**:
- `relativist-core` — the library crate (renamed from the current single crate), all existing functionality
- `relativist-cli` — thin binary, does what `src/main.rs` does today, depends on `relativist-core`
- `relativist-gui` — Tauri app, depends on `relativist-core`, contains `src-tauri/` (Rust) and `ui/` (web frontend)

**Tauri command layer.**

```rust
#[tauri::command]
async fn generate_net(example: String, size: u32, output: PathBuf) -> Result<NetSummary, String> {
    // Construct GenerateArgs, call the same logic as run_generate_command()
}

#[tauri::command]
async fn start_coordinator(config: CoordinatorConfig) -> Result<CoordinatorHandle, String> {
    // Spawn coordinator as background tokio task, return handle for status polling
}

#[tauri::command]
async fn run_benchmarks(config: BenchConfig) -> Result<BenchResults, String> {
    // Call run_benchmark_suite(), emit progress events via tauri::Emitter
}
```

Progress reporting uses Tauri's event system: the backend emits events (`tauri::Emitter::emit`) for round completion, worker status changes, and benchmark progress. The frontend subscribes via `listen()` and updates in real-time.

**GUI screen map.**

| CLI Command | GUI Screen | Key UI Elements |
|---|---|---|
| — | **Dashboard** | Overview cards (version, recent runs), quick actions (generate, reduce, connect) |
| `generate` | **Generate** | Net type dropdown, size slider/input, output path picker, preview of agent/redex count |
| `inspect` | **Inspect** | Drag-and-drop .bin file, stats cards (agents, redexes, normal form), symbol breakdown |
| `reduce` | **Reduce** | Input file picker, progress indicator, before/after comparison |
| `local` | **Local Grid** | Workers slider (1-16), input picker, round-by-round progress table, speedup chart |
| `coordinator` | **Coordinator** | Bind address, worker count, token display/copy, real-time connected workers panel |
| `worker` | **Worker** | Coordinator address input (or Tailscale auto-discover), connection status, reduction progress |
| `compute` | **Calculator** | Operation picker (add/mul/exp), number inputs, animated reduction, result display |
| `bench` | **Benchmarks** | Config panel (select benchmarks, sizes, workers), live progress bars, results table, charts, CSV export |
| — | **Network** | Tailscale status, connected peers, coordinator discovery (integrates with 2.37) |
| `update` | **Settings** | Current/latest version, one-click update, paths config, log level, TLS toggle |

**Frontend framework recommendation: Svelte.**
- Smallest bundle size among major frameworks (~5 KB vs React's ~40 KB)
- No virtual DOM — direct DOM manipulation, faster rendering
- Simple component model — `.svelte` files with HTML/CSS/JS in one file
- Growing ecosystem with excellent Tauri integration (official template exists)
- Alternative: React if the team prefers a larger ecosystem. Both work well with Tauri.

**Implementation phases.**

- **Phase 1 — Skeleton (1-2 weeks):** Cargo workspace restructure, Tauri project scaffold, Dashboard + Generate + Inspect + Reduce screens. Proves the architecture.
- **Phase 2 — Grid (2-3 weeks):** Local Grid + Coordinator + Worker screens with real-time status via Tauri events. System tray for background coordinator.
- **Phase 3 — Benchmarks (1-2 weeks):** Benchmark suite UI with chart library (Chart.js or Recharts), CSV export, results comparison.
- **Phase 4 — Network (1 week):** Tailscale status screen, peer discovery, coordinator auto-detect.
- **Phase 5 — Polish (2-3 weeks):** Installer generation (MSI/.dmg/.deb), code signing integration, auto-updater, theming, accessibility.

**Complexity:** High overall. Workspace restructure: Medium (~1-2 days). Tauri scaffold: Low (~1 day). Each GUI screen: Low-Medium (~1-2 days). Total for functional MVP (Phase 1-2): 3-5 weeks. Total for all phases: 8-12 weeks.

**Dependencies:** Tauri v2 (stable, released). Node.js toolchain for frontend build. Web framework (Svelte recommended). `relativist-core` library extraction.

**Relationship to other items.**

- **2.37 (Tailscale):** The Network screen surfaces Tailscale status and peer discovery.
- **2.38 (Distribution):** Tauri bundler generates MSI/dmg/deb installers. Tauri's code signing integration solves SmartScreen. Tauri's auto-updater replaces `relativist update` for GUI users. The CLI distribution path (cargo-wix, winget, homebrew) remains independent for headless environments.
- **Approach B complement:** The existing axum HTTP server (`src/observability/http.rs`) can serve a lightweight web dashboard for monitoring coordinator status remotely, independent of the Tauri app. This is a natural complement, not a replacement.

---

## v2 — Theoretical Break-Even Analysis (items 2.40)

The v1 benchmark campaigns (4490 executions, 0 correctness failures) revealed a structural performance limitation: **no configuration with workers >= 2 achieved speedup > 1.0**. This section derives the theoretical break-even model from v1 empirical data and quantifies the overhead reduction required for v2.

### 2.40 Break-Even Model and Required Overhead Reduction

**The model.** For workloads where both reduction and partition/merge are O(N) in the number of agents:

```
T_seq(N)    = c_r × N                     (sequential reduction)
T_dist(N,w) = c_o × N + c_r × N / w       (partition + merge + parallel reduction)
Speedup     = 1 / (c_o/c_r + 1/w)
```

where `c_r` is the per-agent reduction cost and `c_o = c_partition + c_merge` is the per-agent overhead cost. **N cancels in the speedup formula** — speedup is structurally constant, independent of problem size.

**Empirical calibration.** From `ep_annihilation_con` (Phase 1 local, w=2) across 4 orders of magnitude:

| N (agents) | c_r (µs/agent) | c_o (µs/agent) | c_o/c_r | S_max (w→∞) |
|------------|---------------:|---------------:|--------:|------------:|
| 1,000      | 0.137          | 0.202          | 1.47    | 0.68        |
| 10,000     | 0.126          | 0.225          | 1.79    | 0.56        |
| 100,000    | 0.128          | 0.278          | 2.17    | 0.46        |
| 10,000,000 | 0.347          | 0.791          | 2.28    | 0.44        |
| 50,000,000 | 0.409          | 0.885          | 2.17    | 0.46        |

The overhead per agent is **2.0–2.3× the reduction cost per agent** at large scales. Even with infinite workers, the theoretical maximum speedup is 0.44–0.46. The ratio worsens at scale due to cache effects (larger nets exceed L3 cache).

**Why no break-even exists in v1.** The condition for speedup > 1 with w workers is:

```
c_o/c_r < (w-1)/w
```

| Workers | Required c_o/c_r | Current c_o/c_r | Overhead reduction needed |
|--------:|-----------------:|----------------:|--------------------------:|
| 2       | < 0.50           | 2.2             | 77%                       |
| 4       | < 0.75           | 2.2             | 66%                       |
| 8       | < 0.88           | 2.2             | 60%                       |

The root cause is **architectural**: the coordinator rebuilds the entire net every BSP round (`split` + `merge` are O(N)), and the reduction itself is also O(N) with very small constant factors (IC interactions are fast pointer swaps). Scaling N does not help because both overhead and computation grow linearly.

**Overhead decomposition (Phase 1 vs Phase 2).** Paired measurements at identical scales isolate the two overhead components:

| Component | Factor (ep_con, w=2) | Source |
|-----------|---------------------:|--------|
| Distribution (partition+merge) | 2.78–2.80× | Phase 1 local / sequential |
| Transport (serde+TCP) | 1.16–1.20× | Phase 2 TCP / Phase 1 local |
| **Total** | **3.22–3.35×** | Phase 2 TCP / sequential |

Distribution is responsible for ~80% of total overhead. TCP transport adds only ~20%. **Optimizing the wire protocol alone cannot achieve break-even**; the partition/merge architecture must change.

**Break-even under v2 optimizations.** Projected speedups for v2 items that reduce `c_o`:

| Scenario | c_o/c_r | w=2 | w=4 | w=8 | Key items |
|----------|--------:|----:|----:|----:|-----------|
| **v1 current** | 2.20 | 0.37 | 0.41 | 0.43 | — |
| Delta partition 50% | 1.10 | 0.63 | 0.74 | 0.82 | 2.26 partial |
| Delta partition 90% | 0.22 | **1.39** | **2.13** | **2.90** | 2.26 + 2.35 |
| Stateful workers | 0.10 | **1.67** | **2.86** | **4.44** | 2.26 + 2.34 + 2.35 |
| Break-even threshold | 0.49 | **1.01** | **1.35** | **1.63** | minimum viable |

The **minimum viable change** for break-even requires reducing `c_o` by 77% (to c_o/c_r < 0.50). This maps directly to item 2.26 (Delta-Only Protocol): if workers retain partition state and the coordinator sends only border deltas, the per-round overhead drops from O(N) to O(border_changes), which for EP workloads (zero border changes per round) approaches zero.

**Note on SPEC-16 (Worker Daemon Mode).** SPEC-16 (implemented in v0.11.0) keeps workers alive **between jobs** (between coordinator invocations) for operational convenience during benchmark campaigns. It does NOT keep worker state **between BSP rounds within a single job** — each round still receives a full `AssignPartition`. SPEC-16 addresses a different problem (campaign automation) than the break-even gap (per-round overhead). The break-even gap requires item 2.26.

**Implication for the TCC.** This analysis produces a **clean negative result**: the v1 architecture validates correctness (0 failures in 4490 executions) but demonstrates that the O(N) partition/merge cost structurally exceeds the O(N) reduction cost, making speedup impossible at any scale. The negative result is scientifically valuable because it (a) identifies the exact bottleneck (c_o/c_r = 2.2, needing < 0.5), (b) proves the bottleneck is architectural rather than scale-dependent (N cancels), and (c) quantifies the engineering target for v2 (77% overhead reduction via 2.26 + 2.34 + 2.35).

### 2.41 Encoder/Decoder API and Problem Registry

**Motivation.** The only end-to-end encode→reduce→decode pipeline in v1 is Church numeral arithmetic (`relativist compute add 3 5`). Any other problem requires forking the codebase and writing Rust. This makes the Relativist a demonstration prototype, not an extensible library. DISC-012 (v2) analyzed this gap systematically and recommended a 4-layer approach.

**Scope (Layers 1-3 of DISC-012).** Layer 0 (workspace restructure) is covered by SPEC-26 R1-R7.

**Layer 1 — Traits + LambdaEncoder (~400 LoC).**
- Define `Encoder` and `Decoder` traits (or unified `Codec`) in `relativist-core::encoding`.
- Refactor existing Church numeral code to implement the trait.
- Implement `LambdaEncoder`: encode/decode pure lambda-calculus terms via the Mackie/Pinto pipeline (REF-005, Theorems 5.2 and 6.2). Subconjunto: Lambda, Application, Variable, Erasure (no numerals, no types).
- Decode via port-directed readback (simplified from Bend's `net_to_term`, documented in AC-013).
- CLI integration: `relativist compute --encoder lambda --input '<term>'`.

**Layer 2 — Encoder Registry (~300 LoC).**
- `EncoderRegistry`: HashMap of named encoders, discoverable at runtime.
- CLI: `relativist encoders list` to show available encoders.
- CLI: `relativist compute --encoder <name> --input '<json>'` (generic dispatch).
- Validation: every encoder output is checked against invariants T1-T7 (SPEC-01) before reduction.

**Layer 3 — RecipeEncoder Generalization (~200 LoC).**
- `RecipeEncoder` trait extending `Encoder` with `make_recipe()` and `generate_partition()`.
- Generalizes SPEC-25 (Recipe-Based Generation) beyond the 9 built-in generators to any registered encoder.
- Protocol: `AssignRecipe` message variant accepts recipes from any encoder in the registry.
- Coordinator sends compact recipe (bytes to kilobytes); each worker materializes its partition locally.

**Status (2026-04-16):** Layer 3 is **partially shipped**. R24+R25 (trait
definition + non-coupling guarantee with the `Codec` registry) landed via
TASK-0340 in Phase 6 mínimo. R26 (refactor SPEC-25 `GenerationRecipe`),
R27 (generalize `AssignRecipe` wire message), and R28 (worker-side registry
dispatch) are **deferred until SPEC-25 itself is implemented** (item 2.29,
milestone M7). See `docs/DEFERRED-WORK.md` row D-001 for the unblock
checklist and the files to revisit when M7 starts.

**What stays out of scope (v2.x/v3).**
- REST API (Layer 4 of DISC-012): ~800-1000 LoC, deferred.
- FFI/Python bindings (PyO3): ~800 LoC, deferred.
- WASM plugins: ~1500 LoC, deferred.
- HVM/Bend compatibility (label support): ~1000+ LoC + formal revision, deferred. See 2.42.
- DSL: descartada (reinvents Bend).

**Estimated total:** ~900 LoC (Layers 1-3, excluding Layer 0 which is SPEC-26).

**Depends on:** SPEC-26 R1-R7 (workspace restructure), SPEC-25 (recipe generation), SPEC-06 (wire protocol for AssignRecipe).

**DISC reference:** DISC-012 v2 (Job Submission, Encoding, and Decoding), Sections 2, 6, 7, 11, 12.

### 2.42 Label Support for Extended Interaction Combinators (Decision Pending)

**Motivation.** HVM2/Bend uses labels (u16) to differentiate CON/DUP agents of different "scopes". Without labels, CON+CON is always annihilation in the Relativist; with labels, same-label pairs annihilate while different-label pairs commute. This is required for correct reduction of any non-trivial Bend program.

**This is a fundamental architectural decision, not an incremental feature.** It changes:
- Symbol representation (~30 LoC)
- All 6 interaction rules (conditional on label match) (~200 LoC)
- Redex identification (T3 invariant changes)
- Wire protocol frame format (~150 LoC)
- Serialization formats (binary, IC text, JSON) (~150 LoC)
- Tests (690 existing need review + new label-specific tests) (~400 LoC)
- Formal argument ARG-001 (must cite confluence for IC with labels, e.g., Mazza 2006)

**Estimated effort:** ~1000+ LoC + formal invariant revision.

**Status:** Decision pending. If HVM/Bend compatibility is strategically prioritized, labels enable the `LambdaEncoder` to work correctly for non-trivial programs. If the Relativist remains pure Lafont IC (3 symbols, no labels), this item is not needed but the HVM bridge is limited to a narrow subset.

**Depends on:** Architectural decision on IC puro vs IC estendido.

**DISC reference:** DISC-012 v2, Section 8 (HVM as Encoder), R-C5 corrected estimate.

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
