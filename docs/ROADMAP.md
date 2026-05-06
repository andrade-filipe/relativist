# Roadmap

> **NOTE:** For active implementation status, priorities, and milestones, see `next-steps.md`. For shipped-bundle history see `progress.md`. This document keeps **architectural rationale** for what is/was/will-be built, condensed for items that have shipped to make room for future planning.

This document lists features explicitly excluded from v1 (SPEC-13, R49-R50) and future architectural evolutions. Items marked with **(confluence-enabled)** are made possible specifically by the strong confluence property of Interaction Combinators — they would be incorrect or require complex consensus in systems without this guarantee.

Each section is tagged with one of:
- **[DONE — D-NNN / SPEC-NN]** — shipped and verified; per-bundle artefacts in `docs/{qa,reviews,spec-reviews}/archive/` and the closure narrative in `progress.md`
- **[PARTIAL — D-NNN]** — some scope shipped, follow-ups deferred (see `next-steps.md`)
- **[ARCHIVED]** — explicitly dropped or superseded
- (no tag) — pending; verbose body retained for design context

---

## v1 Scope (TCC)

v1 implements the minimum architecture that validates the TCC hypothesis: a single coordinator with K static workers in a star topology, using BSP (Bulk Synchronous Parallel) with barrier synchronization. This architecture is deliberately simple because the research question is about **correctness of distributed IC reduction**, not about grid infrastructure sophistication.

---

## v2 — Elastic Grid Architecture **(confluence-enabled)**

The central insight: strong confluence (Lafont 1997, Property P1 in ARG-001) guarantees that the result of reduction is identical regardless of **who** reduces **what** and in **what order**. This means work can be freely redistributed at any point without affecting correctness. v1 does not exploit this; v2 does.

### 2.1 Coordinator as Worker (hybrid node)  **[DONE — D-006 / SPEC-20 §3.1]**

Coordinator keeps one partition and reduces locally in parallel with workers. With K machines total, effective parallelism is K (not K-1). Confluence ensures correctness regardless of who reduces what. Shipped via D-006 Phase B+C+D+E (audit + REFACTOR; LLM-attempt archived at tag `v2-llm-experiment-archive`). Full narrative: `progress.md` D-006 entry.

### 2.2 Dynamic Worker Joining **(confluence-enabled)**  **[PARTIAL — D-006 / SPEC-20 §3.2]**

Workers can join mid-execution via windowed admission protocol (`--join-window-min/max-ms`, `--retain-partitions`). Coordinator tracks hybrid in-process + remote worker set. **Experimental outside `local` mode** — TCP join is exercised by tests but not by locked benchmarks. v1 limitation (worker count fixed at startup, SPEC-06 R24) addressed for the join side; mid-session catastrophic departure recovery deferred to v2.1.

### 2.3 Dynamic Worker Departure **(confluence-enabled)**  **[PARTIAL — D-006 §3.3 detection-only via Option A]**

`GridConfig.elastic_departure: bool = false` default; setting it true emits a one-time warning. Detection-only (release_worker on detected departure, AllWorkersDeparted error variant) shipped. Full reclaim path (R13-R18 reconstruct from retained partitions) was removed in D-006 Phase D and deferred to v2.1.

### 2.4 Distributed Partitioning and Merge **(confluence-enabled)**

**v1 limitation:** Partition is centralised in the coordinator; merge is centralised. This is the fundamental O(N) coordinator cost identified by §2.40 break-even analysis.

**v2 vision:** hierarchical merge (associative under confluence), distributed partition strategies. Item 2.28 (online streaming partitioning) is the first half (DONE in D-010). Hierarchical merge is **not yet shipped** (deferred until distributed partition + delta protocol expose its benefit on real LAN).

---

## v2 — Other Features

### 2.5 Coordinator as Worker (without being coordinator)
Pending. Decouples "I do reduction" from "I orchestrate" so a node can switch roles. Useful for federation; not on critical TCC path.

### 2.6 Work Stealing
Pending. Idle workers pull from busy workers' queues directly (peer-to-peer), bypassing coordinator round-robin. Confluence-enabled.

### 2.7 Intra-Worker Parallelism
Pending. SIMD or thread-pool inside a worker to exploit cores. Orthogonal to distributed work distribution. v1 exposes this via `--strict-bsp` reduction loop; serious implementation deferred.

### 2.8 Automatic Node Discovery
Pending. mDNS / Tailscale-aware discovery so coordinator + workers self-assemble on a LAN/VPN. Item 2.37 covers Tailscale specifically.

### 2.9 Fault Tolerance
Pending. Worker-crash mid-round recovery. v1 surfaces failure as `Fatal(...)`; v2.x aims for partition reassignment. Confluence-enabled.

### 2.10 Multi-Tenancy and Job Queuing
Pending. SPEC-16 worker daemon mode (v0.11.0) gives the operational hook; full job queue API not designed.

### 2.11 Intelligent Partitioning
Pending. SPEC-21 streaming strategies (round-robin, hash, FENNEL) shipped via D-010 R10b/R10c — partial coverage of "intelligent" by topology-aware FENNEL. Per-graph partitioning (METIS-style) not implemented.

### 2.12 GPU Workers
Pending. Out of TCC scope; would require porting the reduction loop to CUDA/Vulkan. Future research.

### 2.13 Visualization
Pending. SPEC-26 GUI (Tauri v2, item 2.39) is the most likely vehicle.

### 2.14 WASM Target
Pending. Compile relativist-core to WASM for browser/edge demos. Workspace restructure (SPEC-26 R1-R7) makes this plausible; not implemented.

### 2.15 Compact Memory Representation (HVM2-style)  **[PARTIAL — D-009 / SPEC-22 SparseNet covers fallback case; bit-packing in SPEC-23 Draft]**

SparseNet (`HashMap`-backed alternative representation, D-009) covers the "save memory when allocation would overflow" case. True bit-packed ports (32-bit-word = 1 agent + 3 ports as in HVM2) is **SPEC-23 Draft**, not shipped. Touch surface is large (225 PortRef call sites identified in BRIEF-20260415-v2-tier5-codebase).

### 2.16 Streaming Reduction Mode **(confluence-enabled)**

**Related, already shipped in v0.10.0-bench:** the **strict BSP mode** (SPEC-05 R30a, opt-in via `--strict-bsp`) resolves the multi-round limitation (previously tracked as L2). This makes `rounds > 1` observable on cross-partition topologies (empirically validated: `cascade_cross(N) = N` rounds, `dual_tree(d) = d` rounds with `workers ≥ 2`). Strict mode is still a *batch* BSP — Section 2.16 below is the deeper asynchronous streaming redesign, orthogonal to strict BSP.

**v2 change (PENDING):** Replace barrier-synchronized BSP with asynchronous streaming — workers use `reduce_n(budget)` returning partial results; coordinator performs incremental merge as results arrive. **High complexity** (new `PartialPartitionResult` protocol message; ARG-001 P3 reformulation under weaker synchronization). Not shipped; on long-range research roadmap. NOTE: D-010 SPEC-21 shipped streaming **net generation** (different concept), not streaming reduction.

**Wall characterised by D-014 stress-curve campaign (PENDING run):** the campaign infrastructure (TASK-0700..0707) ships in this branch; the actual overnight run (TASK-0708) produces `results/locked/v2_stress_curve_<DATE>/` with the empirical characterisation per `(workload, env, W)` — the N at which each sequence aborts plus the failure mode (`StopReason::WallTimeExceeded` / `MemoryExceeded` / `Oom`). See `docs/benchmarks/campaigns/stress-curve.md` for methodology. Once the run lands, this paragraph will be amended with the observed `N_max ± noise` per cell.

### 2.17 Streaming Arithmetic Encoding
Pending. Streaming variants of Church arithmetic (`add(S(x), y) = S(add(x, y))` releases incrementally). Low-Medium complexity; benefit requires §2.16 to be useful in distributed mode.

### 2.18 Native Numeric Types (HVM2-style) — ARCHIVED
HVM2 packs small integers into agent slots. Out of scope for the formal IC the TCC studies. Not pursued.

### 2.19 Protocol Payload Chunking — ARCHIVED
Per-frame chunking to break large payloads. Superseded by §2.20 (CompactSubnet) and the 1 GiB cap raise; chunking would have added wire complexity for problems that no longer exist at the TCC's scale.

### 2.20 Compact Subnet Encoding (Sparse-to-Dense Conversion)  **[DONE — post-v0.9.0; further amended in D-009 + D-011]**

`CompactSubnet` in `relativist-core/src/partition/compact.rs` separates wire format (linear-in-live-agents list) from in-memory dense layout (O(1) reduction loop). Resolved L6 for every config in TCC matrix. SPEC-19 R35a (D-011 Phase A) extended wire encoding to include `free_list` (closes QA-D009-001).

### 2.21 WAN / Internet Deployment  **[Draft — SPEC-24]**

**v1 limitation:** plain TCP, no reconnect, no per-session strong auth, no time-sync requirement, no tolerance for transient packet loss. Adequate for LAN; immediately broken on internet.

**v2 vision (SPEC-24):** session-tracked reconnect on TCP drop, strong auth replacing 3-tier token (TLS mandatory), explicit RTT/jitter measurement and adaptive batch sizing, integration with WireGuard / Tailscale (§2.37).

**Status:** SPEC-24 Draft. Not implemented; depends on SPEC-17 (transport abstraction, DONE) + SPEC-10 hardening.

### 2.21.1 End-to-End Security Analysis
Threat model + concrete control list for §2.21 deployment. Draft inside SPEC-24. Will be load-bearing for any "Relativist on the open internet" claim; out of TCC scope.

---

## v2 — Network Overhead Reduction (items 2.22-2.26)

Empirical c_o/c_r = 2.20 (item 2.40 Phase 1+2 calibration) decomposes ~80 % distribution / ~20 % transport. Items 2.22-2.25 attack transport; 2.26 attacks distribution structurally.

### 2.22 TCP Transport Tuning  **[DONE — SPEC-17 transport abstraction]**

`TransportConfig { send_buffer_bytes, recv_buffer_bytes, keepalive, nodelay }` in `relativist-core/src/protocol/tcp.rs` applied via `socket2::SockRef`. Validated by `test_buffer_sizes_applied`, `test_keepalive_applied`, `test_tcp_nodelay_applied`. Note: kernel may cap `SO_SNDBUF/RCVBUF` at `net.core.{wmem,rmem}_max` (admin policy, see `cargo test test_buffer_sizes_applied` for details).

### 2.23 Wire Format Compaction (bincode v2 varint + enum shrink + optional LZ4)  **[PARTIAL — SPEC-18 wire v2 + delta protocol]**

bincode v2 with varint shipped via wire format v2 (PROTOCOL_VERSION 5, bumped through D-009/D-010). Custom `PortRef` packed encoding NOT shipped. LZ4 NOT shipped (deferred — delta protocol §2.26 already delivered the larger wire saving: ~30 % bytes/round vs v1).

### 2.24 Zero-Copy Archive (rkyv) on the Hot Path  **[DONE (opt-in) — SPEC-18]**

`cargo build --release --features zero-copy` enables rkyv-archived payloads alongside bincode. Activated at runtime via `--use-zero-copy`. Per SPEC-18 R20 NOT default. Reserved for Phase 3 LAN axis 2 measurement. See `docs/guides/07-zero-copy.md`.

### 2.25 Same-Host Fast Path (Unix Domain Sockets / Shared Memory)  **[DONE (Unix) — SPEC-17]**

`UnixTransport` shipped as one of the three SPEC-17 transports (TCP, Unix, Channel). Tested via `protocol::unix::tests` (cfg(unix) only — no Windows native UDS). Shared memory NOT shipped.

### 2.26 Delta-Only Protocol (Stateful Workers) **(confluence-enabled)**  **[DONE — D-005 / SPEC-19]**

Workers retain partition state; coordinator sends only border deltas (`PendingCommutation`, `LocalDeltaDispatch`, etc.). PROTOCOL_VERSION bumped 2→3. **Empirical signature:** ~30 % byte reduction per round vs v1 on `ep_annihilation_con 500k w=1`. Closes the §2.40 break-even strategy "Delta partition 90% / Stateful workers" — but full c_o/c_r reduction requires real LAN where bandwidth scarcity makes the saving dominate over CPU framing cost.

---

## v2 — Memory-Bounded Distributed Reduction (items 2.27–2.36) **(confluence-enabled)**

Theme: keep coordinator memory bounded (independent of total net size) by streaming generation + partitioning + merging. v1's coordinator must hold the full net once before partitioning; v2 generates and partitions in chunks.

### 2.27 Streaming Net Generation (Producer-Consumer Pipeline)  **[DONE — D-010 / SPEC-21]**

`--chunk-size N` produces chunks of N agents through a producer-consumer pipeline; `bench-tcp` docker-compose profile exercises it. Streaming generators implemented for `ep_annihilation`, `dual_tree`, `condup_expansion`. R10b/c free-list recycle integrated under streaming (Strategy A streaming gate, Strategy B `BorderClean`). See `docs/guides/09-streaming-generation.md`.

### 2.28 Online/Streaming Graph Partitioning  **[DONE — D-010 R10b strategies]**

Round-robin (default), hash-based, and FENNEL (heuristic streaming) strategies shipped. `--streaming-strategy` flag selects. FENNEL `f64::total_cmp` + non-finite alpha rejection hardening shipped via D-010 QA REFACTOR.

### 2.29 Distributed/Parallel Net Generation (Recipe-Based)  **[Draft — SPEC-25]**

Coordinator emits compact `Recipe` (bytes-to-KB); each worker materialises its partition locally without touching the full net. Critical for distributed M+ agent benchmarks. SPEC-25 Draft, not implemented; M7 milestone in §"Recommended Implementation Order" below.

### 2.30 Chunked Generation with Incremental Partitioning  **[DONE — D-010]**

Combines §2.27 + §2.28. Glued via `generate_and_partition_chunked_with_lifetime` in production code. `max_pending_lifetime` budget enforced.

### 2.31 Memory-Mapped / Out-of-Core Net Representation
Pending. mmap-based `Net` for single-node giant-net scenarios. Platform-specific complexity may not justify effort if streaming approaches (DONE) cover the same use case. Low priority.

### 2.32 Sparse Net Representation  **[DONE — D-009 / SPEC-22 SparseNet]**

`HashMap<AgentId, Agent>` + `HashMap<(AgentId, PortId), PortRef>` representation for partitions where dense allocation would overflow. Routing decision via SPEC-22 v2.4 `effective_arena_size > 4 × live_agent_count` metric (D-011 fix for the partition-perf BLOCKER). See `docs/guides/10-arena-management.md`.

### 2.33 Arena Recycling / GC During Reduction  **[DONE — D-009 / SPEC-22 free-list]**

`Net.free_list: Vec<AgentId>` with LIFO push/pop in `create_agent`/`remove_agent`. Recycling policy via `--recycle-policy {enable,disable-under-delta,disable}` (default `disable-under-delta`). Free-list `validate_free_list` post-condition + `count_live_agents` excludes recycled slots.

### 2.34 Coordinator-Free Round (Merge Avoidance) **(confluence-enabled)**

**Pending.** When NO workers report border activity in a round, the coordinator can skip merge entirely (the next round dispatch is the previous round's partitions verbatim). Per §2.40 projection, with 2.34 + 2.26 the c_o/c_r drops to ~0.10 → speedups of 1.67/2.86/4.44 at w=2/4/8. SPEC-19 §3.1 R3+R4 documents the all_no_border_activity short-circuit; production implementation triggered via `every_border_has_inert_remote` helper (D-005 commit `434a242`).

**Status:** Helper exists in `merge/grid.rs`; full short-circuit pathway not yet uniformly invoked across coordinator FSM. Pending verification.

### 2.35 Delta-Based Merge (Lightweight Border Resolution) **(confluence-enabled)**

Pending. Replace O(N) `merge` with O(border_changes) merge that only reconciles border state. Companion to §2.26 (deltas on dispatch) — together they get c_o/c_r toward the §2.40 stateful-workers projection. Not implemented as a distinct module yet; partial benefit subsumed by D-005's delta protocol.

### 2.36 Lazy / Demand-Driven Generation (Pull Model)  **[PARTIAL — D-010 pull-dispatch FSM]**

D-010 shipped pull-dispatch FSM (`RequestWork`/`NoMoreWork` wire variants, PROTOCOL_VERSION 5→6). Workers pull chunks rather than coordinator push. Full lazy generation (compute-as-you-go per agent) not implemented; D-010's chunk-pull is the practical middle-ground.

### Recommended Implementation Order

The techniques form a dependency graph with clear critical paths. After D-006..D-012 the picture is:

- **Phase A — Quick wins (DONE)**: §2.33 Arena Recycling (D-009), §2.34 Coordinator-Free Round (helper shipped, full integration pending)
- **Phase B — Streaming foundation (DONE)**: §2.28, §2.27, §2.30 (all in D-010)
- **Phase C — Full coordinator memory elimination (PARTIAL)**: §2.35 Delta Merge (subsumed in D-005); §2.29 Recipe-Based Distributed Generation (Draft SPEC-25); §2.36 Lazy Generation (partial via D-010 pull-dispatch)
- **Phase D — Supplementary**: §2.32 Sparse Net (DONE D-009), §2.31 Memory-Mapped Net (low priority)

### Minimum Viable Pipeline (MVP)

The smallest set of changes that would let the coordinator handle nets larger than available RAM — **all DONE in D-010**:

1. §2.28 round-robin streaming partitioner (~80 lines)
2. §2.27 streaming generator for `ep_annihilation_con` (~100 lines)
3. §2.30 chunked generation loop (~150 lines)
4. §2.35 lightweight border-only merge (~200 lines) — partial via D-005

The MVP target `ep_annihilation_con=100M` on coordinator-with-1-GB-RAM + 4 workers each with 2 GB is now structurally possible; not yet validated empirically (Phase 3 LAN scope).

---

## v2 — User Experience and Deployment (items 2.37–2.39)

### 2.37 Tailscale Mesh VPN Integration
Pending. Auto-discovery + zero-config WAN deployment via Tailscale. Companion to §2.21 WAN.

### 2.38 Installation and Distribution UX
Partially shipped via SPEC-15 (DONE in v0.9.0): GitHub Releases (Windows .exe, Linux .tar.gz/.deb), Docker GHCR, install script, self-update command, shell completions. Future polish (e.g., curl|sh installers, Homebrew tap) pending.

### 2.39 Graphical User Interface (Tauri v2)  **[Draft — SPEC-26]**

Tauri v2 desktop app for non-CLI users. Workspace restructure (SPEC-26 R1-R7) shipped — `relativist-core` now embeddable as library — but UI itself is Draft. Out of TCC scope.

---

## v2 — Theoretical Break-Even Analysis (item 2.40)

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

**v2 empirical update (post-D-012, 2026-05-05).** The canonical v2 baseline `results/locked/v2_post_d012_baseline_2026-05-05/` (32 distributed slots × 10 reps, all `all_correct=true`) shows v2 sends **30 % fewer bytes per round** (28.7 MB vs 41 MB on `ep_500k w=1`) with delta protocol active — confirming §2.26's structural saving. However, on TCP-localhost the v2 wall-clock is 1.4–2.0× v1: the bytes-saved-on-wire don't pay for the CPU cost of the abstraction layers (SPEC-17/18/19/21/22). Per-round decomposition (D-012 instrumentation): `wall ≈ 47 % network + 8 % compute + 6 % merge + 38 % framing/orchestration`. The break-even where v2 wins is at LAN bandwidth ≤ 156 MB/s (≈1.2 Gbps) — every real LAN/WAN qualifies. **Phase 3 LAN measurement is required to publish the break-even crossover empirically** (`docs/benchmarks/phase-3-lan.md` Axis 1 + Axis 2).

### 2.41 Encoder/Decoder API and Problem Registry  **[Draft — SPEC-27, partial Layer 3 in TASK-0340]**

The only end-to-end encode→reduce→decode pipeline in v1 is Church numeral arithmetic. Layer 1 (LambdaEncoder), Layer 2 (EncoderRegistry), Layer 3 (RecipeEncoder generalisation) outlined in DISC-012 v2. **Layer 3 partial** — `Codec` registry trait + non-coupling guarantee landed via TASK-0340; R26-R28 (refactor SPEC-25, generalize AssignRecipe wire, worker-side dispatch) deferred until SPEC-25 itself is implemented (M7). REST API / FFI / WASM / HVM-label support all explicitly out of scope.

### 2.42 Label Support for Extended Interaction Combinators (Decision Pending)

**Motivation.** HVM2/Bend uses labels (u16) to differentiate CON/DUP agents of different "scopes". Without labels, CON+CON is always annihilation; with labels, same-label pairs annihilate while different-label pairs commute. Required for non-trivial Bend programs.

**This is a fundamental architectural decision, not an incremental feature.** Touches Symbol representation, all 6 interaction rules, redex identification (T3), wire protocol, serialization, ~690 existing tests + new label-specific tests, formal argument ARG-001 (must cite confluence for IC with labels, e.g., Mazza 2006). Estimated ~1000+ LoC + formal invariant revision.

**Status:** Decision pending. If HVM/Bend compatibility is strategically prioritized, labels enable `LambdaEncoder` to work for non-trivial programs. If Relativist remains pure Lafont IC (3 symbols, no labels), this item is not needed but the HVM bridge is limited.

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

**v2 empirical reinforcement (post-D-011, 2026-05-04).** The D-011 BLOCKER investigation produced an additional scientific finding for the TCC: the apparent v1→v2 perf regression masked **2 latent correctness bugs** (Bug 1: `freeport_redirects` not propagated; Bug 2: `next_id = 0` causing cross-partition AgentId collision) that v1's empirical test suite never detected. The formal invariant framework (SPEC-01 I1, SPEC-22 R10/D3) detected what 4490 v1 benchmark executions did not. **v2 is now strictly more correct than v1 ever was**, at a residual ~10 % wall-clock cost on the canonical workload. This strengthens the Confluence Argument: not only does formal IC theory enable the architectural evolutions above, it also produces empirical correctness guarantees that empirical testing alone cannot.

This argument strengthens the TCC's contribution: the prototype validates the foundation, the roadmap demonstrates the breadth of what that foundation enables, and the v2 instrumentation (D-011 + D-012) produces direct empirical signatures for §2.40's break-even claim.
