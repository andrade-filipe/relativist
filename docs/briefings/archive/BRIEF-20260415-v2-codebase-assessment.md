# BRIEF: Relativist v1 Codebase Assessment for v2 Development

**Generated:** 2026-04-15
**Scope:** Assess current codebase state for v2 feature insertion

---

## Executive Summary

The v1 codebase (frozen at `v0.10.0-bench`, 690 tests, 4490 benchmarks, 0 correctness failures) is architecturally sound for the features it delivers: a stateless BSP loop where the coordinator holds the full merged net and ships entire partitions each round. The Core/Infrastructure layer separation (SPEC-13 R6-R8) is rigorously enforced -- `net/`, `reduction/`, `partition/`, `merge/`, `encoding/` have zero tokio dependency. The wire protocol is properly framed (8-byte header, CRC32C integrity), and `CompactSubnet` already decouples in-memory dense arena from wire-format sparse encoding.

However, v2's primary features (delta protocol 2.26, Transport abstraction 2.25, bincode v2 migration 2.23, dynamic workers 2.2/2.3) will stress three areas v1 treated as fixed: (1) the `Message` enum's discriminant stability under 5+ new variants, (2) the coordinator's assumption of holding full merged-net state between rounds, and (3) the hardcoded `TcpStream` transport in `coordinator.rs`/`worker.rs`. The total codebase is ~22,200 LoC across 43 Rust source files, with `protocol/` (2,246 LoC, 7 files) being the module with the highest refactoring exposure.

The invariants T1-T7 and D1-D5 are structural IC properties unaffected by v2. D6 (protocol termination) and G1 (fundamental property) require extended proofs for the delta protocol, where the coordinator no longer holds the full intermediate state. The break-even analysis (ROADMAP 2.40) shows c_o/c_r = 2.2, requiring a 77% overhead reduction (c_o/c_r < 0.50 for w=2) achievable only through the delta protocol (2.26) -- TCP tuning and wire compaction alone cannot close the gap.

---

## Invariant Impact Analysis

### Tier 1 (Break-even: items 2.22-2.26)

**G1 (Fundamental Property): Needs extended proof for delta protocol.**

In v1, G1 (`reduce_all(net) ~ extract_result(run_grid(net, n))`) is proved via the coordinator holding the full merged net after every round. Under the delta protocol (2.26), the full state is distributed: coordinator holds a `BorderGraph`, workers hold their partition interiors. The proof must show that `union(worker_partitions) + coordinator.border_graph` is recoverable to a net isomorphic to some sequential intermediate state. Strong confluence (T4) guarantees the decomposition exists, but the formal argument requires a fresh spec pass (ROADMAP 2.26 line 551: "the formal argument gets more subtle").

**D6 (Protocol Termination): Needs reformulation for delta protocol.**

Current D6 states R_lenient <= 1 and R_strict <= N. Under the delta protocol, border redexes are dispatched as deltas rather than resolved at the coordinator. The termination argument must be reformulated: each delta triggers at least one interaction at a worker, and the total interaction budget is finite and invariant (T7), so the round count is bounded. The proof structure is analogous but the formulation differs.

**D1-D5: No impact.** `split()` and `merge()` APIs are unchanged. The delta protocol changes *when* they are called (only initial partition and final merge), not *what* they do. D4 (ID uniqueness) needs minor attention for dynamic workers (2.2): `compute_id_ranges()` must be called with updated worker counts at re-partition time, but the function already takes `num_workers` dynamically.

**T1-T7: No impact.** All are structural IC properties independent of transport, protocol, or distribution architecture.

**I1-I7: No impact.** I3 (monotonic IDs) requires recomputed ranges for dynamic workers, but `compute_id_ranges(num_workers, next_id)` in `partition/helpers.rs` already supports this.

### Tier 2 (Elastic Grid: items 2.1-2.4)

**T1-T7: No impact.** Strong confluence (T4) is the enabler, not the target: dynamic joining (2.2), departure (2.3), and hierarchical merge (2.4) are safe precisely because T4 guarantees order-independence.

**I1-I5: No impact.** The Net representation is unchanged regardless of which node reduces which partition.

**D4 (ID Uniqueness): Attention required.** Dynamic worker joining (2.2) means `compute_id_ranges()` must be re-invoked between rounds with updated worker count. The existing code handles this. Dynamic departure (2.3) means a departing worker's ID range may contain newly created agents; these must not collide when the partition is re-dispatched.

**D5 (Exclusive Ownership): Preserved.** Dynamic departure re-dispatches the *original* partition (coordinator retains a copy). No agent exists in two partitions simultaneously.

### Tier 3 (Memory: items 2.27-2.36)

**Arena invariants (I3, I6, I7):** Arena recycling (2.33) would allow reuse of `AgentId` values, directly violating I3 (monotonic IDs). Either I3 must be relaxed (replacing "never reused" with "unique within the current live set") or recycling must use a generation counter. I6 (ERA slot cleanliness) is unaffected. I7 (root port consistency) is unaffected.

**T1 (Port Linearity):** Sparse representation (2.32) would replace the dense `Vec<Option<Agent>>` + `Vec<PortRef>` with a `HashMap`-based layout. T1's verification (`port_index(agent_id, port_id) = id * 3 + port_id`) assumes dense indexing; sparse representation would need a new indexing function. The invariant's *statement* is unchanged but its *verification code* must be updated.

---

## Module Boundaries (SPEC-13)

### Current dependency direction (inviolable):

```
Core Layer (pure, no async):
  net/ <- reduction/ <- partition/ <- merge/ <- encoding/

Infrastructure Layer (async, tokio):
  protocol/ -> {net, partition, merge, reduction}
  coordinator.rs -> {protocol, merge, partition, reduction}
  worker.rs -> {protocol, merge, reduction}
```

### Where v2 features insert:

**protocol/** -- Transport abstraction inserts a new `transport.rs` defining `trait Transport` with TCP, UDS, and channel implementations. `frame.rs` is already generic over `AsyncWriteExt`/`AsyncReadExt` and needs no changes for transport abstraction. bincode v2 migration changes 6 call sites in `frame.rs` (`bincode::serialize` -> `bincode::serde::encode_to_vec`). Wire format v2 (SPEC-18) adds 5+ `Message` variants appended after `RegisterNack` (discriminant 6), preserving R5 discriminant stability. The `PROTOCOL_VERSION` constant (currently 1) must be bumped.

**merge/** -- Delta merge inserts a new `grid_delta.rs` (or extends `grid.rs`) containing `run_grid_delta()` as a parallel code path to the existing `run_grid()`. The v1 `run_grid` remains as the in-process simulation path. `merge()` in `core.rs` is fully reusable for the final convergence merge. `GridConfig` in `types.rs` needs `delta_mode: bool` and `GridMetrics` needs delta-specific counters. Coordinator-free rounds (2.34) would bypass `merge()` entirely when no border redexes exist -- the coordinator detects this from delta messages and skips the merge phase.

**partition/** -- Streaming partitioning (2.28) would add a `streaming_strategy.rs` implementing `PartitionStrategy` for online/streaming graphs. Delta awareness means `split()` is called only at round 0 and at re-partition events (dynamic workers). `CompactSubnet` continues to serve as the wire format for `InitialPartition` and `FinalStateResult` messages.

**net/** -- Sparse representation (2.32) would replace `Vec<Option<Agent>>` + `Vec<PortRef>` with `HashMap<AgentId, Agent>` + `HashMap<(AgentId, PortId), PortRef>`. This is a deep refactor touching every module. Arena recycling (2.33) would add a free-list to `Net` for reusing dead agent slots. Neither is planned for near-term v2 but both are on the ROADMAP.

---

## Wire Protocol State (SPEC-06)

### Current message types (7 variants):

| Disc. | Variant | Direction | Payload |
|------:|---------|-----------|---------|
| 0 | `AssignPartition` | C->W | `round: u32`, `partition: Partition` |
| 1 | `Shutdown` | C->W | (unit) |
| 2 | `PartitionResult` | W->C | `round: u32`, `partition: Partition`, `stats: WorkerRoundStats` |
| 3 | `Error` | W->C | `round: u32`, `worker_id: WorkerId`, `description: String` |
| 4 | `Register` | W->C | `RegisterPayload { protocol_version, auth_token }` |
| 5 | `RegisterAck` | C->W | `RegisterAckPayload { worker_id }` |
| 6 | `RegisterNack` | C->W | `RegisterNackPayload { reason }` |

### Serialization:

- **Codec:** bincode v1 (`bincode = "1"` in Cargo.toml), fixed-int encoding, little-endian.
- **Framing:** 8-byte header (4 bytes LE u32 payload length + 4 bytes LE u32 CRC32C checksum) + payload bytes.
- **Max payload:** 1 GiB (raised from 256 MiB by L6 fix via `CompactSubnet`).
- **Wire encoding for partitions:** `CompactSubnet` via custom serde `serialize_with`/`deserialize_with` on `Partition::subnet`. Only live agents and non-DISCONNECTED ports cross the wire.

### Impact of wire format v2 (SPEC-18):

New message variants appended after discriminant 6:

| Disc. | Variant | Direction | Purpose |
|------:|---------|-----------|---------|
| 7 | `InitialPartition` | C->W | One-time full partition at job start |
| 8 | `RoundStart` | C->W | Border deltas for stateful workers |
| 9 | `RoundResult` | W->C | Border deltas + stats (no full partition) |
| 10 | `FinalStateRequest` | C->W | Request full partition at convergence |
| 11 | `FinalStateResult` | W->C | Full partition for final merge |

### Impact of delta protocol (SPEC-19):

The delta protocol changes the payload economics: `RoundStart` and `RoundResult` carry only border changes (typically single-digit percent of partition size), not full partitions. For `ep_annihilation_con` 20M agents at w=2, this would reduce per-round wire cost from ~750 MB to potentially a few KB. The `InitialPartition` and `FinalStateResult` still carry full partitions and benefit from all wire optimizations (CompactSubnet, bincode v2 varint, optional LZ4).

### bincode v1 -> v2 migration:

Breaking wire change. bincode v1 uses 4-byte u32 fixed-width enum discriminants; bincode v2 uses varint (1 byte for variants 0-127). The `PROTOCOL_VERSION` constant (currently 1) must be bumped to 2. The `Register` handshake already rejects version mismatches.

---

## Per-Module Code State

### net/ (1,865 LoC, 4 files)

**Public API surface:**
- `Net` struct (agents arena, ports array, redex_queue, next_id, root, freeport_redirects) -- `core.rs`
- `PortRef` enum (`AgentPort(AgentId, PortId)` / `FreePort(u32)`) -- `types.rs`
- `Agent` struct (`symbol: Symbol`, `id: AgentId`) -- `types.rs`
- `Symbol` enum (`Con=0`, `Dup=1`, `Era=2`) -- `types.rs`
- `AgentId = u32`, `PortId = u8` -- `types.rs`
- `DISCONNECTED = FreePort(u32::MAX)`, `PORTS_PER_SLOT = 3` -- `types.rs`
- `port_index(AgentId, PortId) -> usize`, `arity()`, `total_ports()` -- `types.rs`
- `Net::new()`, `Net::with_capacity()`, `Net::create_agent()`, `Net::connect()`, `Net::get_target()`, `Net::count_live_agents()`, `Net::is_valid_redex()` -- `core.rs`
- `assert_all_invariants()` -- `debug.rs`

**Key types and traits:** `Net`, `PortRef`, `Agent`, `Symbol`. No traits -- all concrete types. `BorderMap = HashMap<u32, PortRef>` type alias in `types.rs`.

**Insertion points for v2:**
- Sparse representation (2.32): Replace `Vec<Option<Agent>>` + `Vec<PortRef>` in `Net`. Deep refactor.
- Arena recycling (2.33): Add free-list to `Net::create_agent()`.
- Custom PortRef serde (2.23): Manual `Serialize`/`Deserialize` impl on `PortRef` for varint encoding.

**Patterns to preserve:** `#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]` on all public types. `#[repr(u8)]` on `Symbol`. `DISCONNECTED` sentinel. Dense port array indexed by `port_index()`. Comprehensive unit tests for each type.

### reduction/ (2,913 LoC, 4 files)

**Public API surface:**
- `reduce_all(net: &mut Net) -> ReductionStats` -- `engine.rs`
- `reduce_n(net, budget) -> ReductionStats` -- `engine.rs`
- `reduce_step(net) -> StepResult` -- `engine.rs`
- `reduce_border_once(net) -> ReductionStats` -- `engine.rs`
- `interact_anni()`, `interact_comm()`, `interact_eras()`, `interact_void()` -- `rules.rs`
- `get_rule()`, `get_specific_rule()`, `normalize_pair()` -- `dispatch.rs`
- `Rule` enum (Anni/Comm/Eras/Void), `SpecificRule` enum (6 variants) -- `dispatch.rs`
- `ReductionStats` struct, `StepResult` enum -- `engine.rs`

**Key types and traits:** `ReductionStats { total_interactions, interactions_by_rule: [u64; 6] }`. No traits.

**Insertion points for v2:** None planned. The reduction engine is frozen and universal -- all v2 features operate at the distribution level, not the reduction level.

**Patterns to preserve:** Pure functions, no async, no I/O. Stale-redex tolerance (verify at dequeue, discard silently). Debug assertions after each reduction step.

### partition/ (1,983 LoC, 6 files)

**Public API surface:**
- `split(net: Net, num_workers: u32, strategy: &dyn PartitionStrategy) -> PartitionPlan` -- `split.rs`
- `PartitionStrategy` trait (single method: `allocate(&self, net: &Net, num_workers: u32) -> HashMap<AgentId, WorkerId>`) -- `strategy.rs`
- `ContiguousIdStrategy` (default implementation) -- `strategy.rs`
- `classify_wires()`, `compute_id_ranges()`, `max_freeport_id()` -- `helpers.rs`
- `Partition` struct (subnet, worker_id, free_port_index, id_range, border_id_start, border_id_end) -- `types.rs`
- `PartitionPlan` struct (partitions, borders) -- `types.rs`
- `IdRange` struct (start, end), `WorkerId = u32` -- `types.rs`
- `CompactSubnet` struct, `serialize_subnet_compact()`, `deserialize_subnet_compact()` -- `compact.rs`
- `WireClassification` struct -- `helpers.rs`

**Key types and traits:** `PartitionStrategy` trait is the primary extension point. `CompactSubnet` is the wire/memory separation layer.

**Insertion points for v2:**
- New `PartitionStrategy` implementations: `TopologyAwareStrategy` (2.11), `StreamingStrategy` (2.28).
- New file `topology_strategy.rs` or `streaming_strategy.rs`.
- `types.rs`: No changes to `Partition` or `PartitionPlan` needed for delta protocol -- they are used only at initial send and final receive.

**Patterns to preserve:** `split()` is a pure function consuming `Net` by value. C1/C3 debug assertions after split. `CompactSubnet` wire/memory separation via serde `serialize_with`/`deserialize_with`.

### merge/ (2,162 LoC, 5 files)

**Public API surface:**
- `merge(plan: PartitionPlan) -> (Net, u32)` -- `core.rs`
- `run_grid(net: Net, config: &GridConfig, strategy: &dyn PartitionStrategy) -> (Net, GridMetrics)` -- `grid.rs`
- `rebuild_free_port_index(subnet, border_id_start, border_id_end) -> HashMap<u32, PortRef>` -- `helpers.rs`
- `drain_stale_redexes(net: &mut Net)` -- `helpers.rs`
- `GridMetrics` struct (18 metric fields + helper methods) -- `types.rs`
- `WorkerRoundStats` struct (6 fields) -- `types.rs`
- `GridConfig` struct (num_workers, max_rounds, strict_bsp) -- `types.rs`

**Key types and traits:** `GridMetrics` is the primary data structure for benchmark analysis. `GridConfig` controls BSP mode.

**Insertion points for v2:**
- `grid.rs`: New function `run_grid_delta()` or parameterized `run_grid()` for delta protocol (2.26). The existing `run_grid()` remains as the in-process simulation path. ~170 lines of existing BSP loop logic.
- New file `grid_delta.rs`: Delta-protocol BSP loop (~400-500 LoC estimated).
- New file or struct `border_graph.rs`: `BorderGraph` for coordinator-side border connectivity tracking (~400 LoC).
- `types.rs`: Extend `GridConfig` with `delta_mode: bool`. Extend `GridMetrics` with `delta_bytes_per_round: Vec<usize>`.
- `helpers.rs`: `rebuild_free_port_index()` works in the delta model but border_id_start/end change per round as new borders arrive via `RoundStart` messages.

**Patterns to preserve:** `merge()` consumes `PartitionPlan` by value. `run_grid()` returns `(Net, GridMetrics)`. Pure core layer -- no async, no I/O. Debug assertions (`verify_no_redexes_full_scan`) after termination.

### protocol/ (2,246 LoC, 7 files)

**Public API surface:**
- `Message` enum (7 variants) -- `types.rs`
- `RegisterPayload`, `RegisterAckPayload`, `RegisterNackPayload` -- `types.rs`
- `ProtocolError` enum (8 variants) -- `error.rs`
- `NodeConfig` struct -- `config.rs`
- `FrameHeader` struct, `send_frame<W>()`, `recv_frame<R>()` -- `frame.rs`
- `accept_workers()`, `distribute_partitions()`, `collect_results()` -- `coordinator.rs`
- `connect_with_retry()`, `run_worker()`, `run_worker_daemon()` -- `worker.rs`
- `PROTOCOL_VERSION: u8 = 1` -- `coordinator.rs`

**Key types and traits:** No traits (Transport trait missing from v1). `send_frame`/`recv_frame` are generic over `AsyncWriteExt`/`AsyncReadExt` -- already transport-agnostic.

**Insertion points for v2:**
- New file `transport.rs`: `trait Transport`, `TcpTransport` (extracted from coordinator/worker).
- New file `channel.rs`: `ChannelTransport` for in-memory testing via tokio mpsc.
- New file `unix.rs`: `UnixTransport` for same-host fast path (2.25).
- `coordinator.rs` line 40: Replace `TcpListener::bind()` with `Transport::listen()`. ~80 lines affected.
- `worker.rs` line 47: Replace `TcpStream::connect()` with `Transport::connect()`. ~30 lines affected.
- `types.rs` line 73: Append new `Message` variants (5+ for delta protocol).
- `frame.rs`: bincode v2 migration: change `bincode::serialize` -> `bincode::serde::encode_to_vec`, `bincode::deserialize` -> `bincode::serde::decode_from_slice`. ~6 call sites.
- `config.rs`: Add `TransportTuning` and `TransportBackend` fields to `NodeConfig`.

**Patterns to preserve:** `send_frame`/`recv_frame` generics over async I/O traits. Message variants appended at end for discriminant stability. `ProtocolError` with `#[derive(Debug)]` and manual `Display` impl. CRC32C frame integrity.

### encoding/ (1,300 LoC, 3 files)

**Public API surface:**
- `encode_nat()`, `encode_church_into()`, `decode_nat()` -- `church.rs`
- `build_add()`, `build_mul()`, `build_exp()`, `build_sum_of_squares()`, `compute_arithmetic()`, `decode_nat_or_shared()`, `discover_root()` -- `arithmetic.rs`

**Insertion points for v2:** None planned. Encoding is stable and orthogonal to distribution features.

**Patterns to preserve:** Pure functions, IC-concept comments, thorough unit tests for each arithmetic operation.

---

## v1 Patterns to Preserve

### 1. Pure Core / Async Infrastructure Split (SPEC-13 R6-R8)

Core modules (`net/`, `reduction/`, `partition/`, `merge/`, `encoding/`) are synchronous pure Rust. No `async fn`, no tokio, no I/O. Infrastructure (`protocol/`, `coordinator.rs`, `worker.rs`) depends on Core, never the reverse. Any v2 feature (delta protocol, Transport trait) must live in Infrastructure. No Core module may gain an `async` dependency.

### 2. Error Types with thiserror

Per-module error enums: `NetError`, `ReductionError`, `PartitionError`, `MergeError`, `ProtocolError`. Top-level `RelativistError` unifies via `#[from]` conversions. All use `thiserror`. Invariant violation strings start with the invariant code (e.g., `"T1: port 42 is dangling"`).

### 3. Newtype IDs and Derive Macros

`AgentId = u32`, `PortId = u8`, `WorkerId = u32` (type aliases). `IdRange`, `Agent`, `PortRef`, `Symbol` are proper types with `#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]`. `Symbol` has `#[repr(u8)]`.

### 4. Debug Assertions for Invariants

`#[cfg(debug_assertions)]` guards on: C1/C3 assertions in `split.rs`, stale-redex assertions in `merge/core.rs`, `verify_no_redexes_full_scan` in `grid.rs`, `assert_all_invariants()` in `debug.rs`. Zero-cost in release builds.

### 5. CompactSubnet Wire/Memory Separation

In-memory: dense `Vec<Option<Agent>>` + `Vec<PortRef>` (O(1) access). On wire: `CompactSubnet` with only live agents and non-DISCONNECTED ports (O(live_agents) size). Achieved via serde `serialize_with`/`deserialize_with` on `Partition::subnet`.

### 6. Redex Queue with Stale Tolerance

`redex_queue: VecDeque<(AgentId, AgentId)>` may contain stale entries. The engine verifies validity at dequeue time via `is_valid_redex()` and silently discards stale entries (SPEC-01 I4).

### 7. freeport_redirects Map

`Net::freeport_redirects: HashMap<u32, PortRef>` tracks FreePort-to-FreePort redirections during partition reduction. Skipped in serde (`#[serde(skip)]`). Used by `rebuild_free_port_index()`.

---

## Dependencies (Cargo.toml)

### Current always-on dependencies:

| Crate | Version | Purpose |
|-------|---------|---------|
| `serde` | 1.x (with `derive`) | Serialization framework |
| `serde_json` | 1.x | JSON output (benchmarks, config) |
| `bincode` | **1.x** | Binary wire encoding (v1 fixed-int) |
| `clap` | 4.x (with `derive`) | CLI parsing |
| `clap_complete` | 4.x | Shell completion generation |
| `thiserror` | 2.x | Error type derivation |
| `tracing` | 0.1 | Structured logging |
| `tracing-subscriber` | 0.3 (env-filter, json, registry) | Log formatting + filtering |
| `tokio` | 1.x (rt-multi-thread, net, macros, io-util, time, signal) | Async runtime |
| `crc32fast` | 1.x | CRC32C frame checksums |
| `futures` | 0.3 | Concurrent partition sends |
| `rayon` | 1.x | Local parallelism (fair comparison) |
| `rand` | 0.8 | Token generation, test data |
| `base64` | 0.22 | Token base64 encoding |
| `subtle` | 2.x | Constant-time token comparison |

### Feature-gated dependencies:

| Crate | Feature | Purpose |
|-------|---------|---------|
| `rustls` + `tokio-rustls` + `rustls-pemfile` | `tls` | TLS 1.3 |
| `prometheus-client` + `axum` | `metrics` | Prometheus /metrics endpoint |
| `opentelemetry` + `opentelemetry_sdk` | `otel` | OpenTelemetry tracing |

### Dev dependencies:

`proptest`, `criterion`, `tokio-test`, `tower`.

### What v2 needs to add:

| Crate | Purpose | ROADMAP Item |
|-------|---------|-------------|
| `bincode` **2.x** | Varint encoding, smaller wire frames | 2.23 |
| `lz4_flex` | LZ4 compression for large frames | 2.23 |
| `rkyv` | Zero-copy deserialization on hot path | 2.24 |
| `socket2` | TCP tuning (SO_SNDBUF, TCP_NODELAY) | 2.22 |

**Note:** `bincode` 1.x -> 2.x is a breaking API change (`bincode::serialize` -> `bincode::serde::encode_to_vec`). Both versions cannot coexist in the same crate without renaming.

---

## Feature Dependencies at Code Level

### Tier 1 Feature: 2.22 TCP Transport Tuning

**Exact files/functions to modify:**
- `src/protocol/config.rs`: Add `TransportTuning` struct with `tcp_nodelay: bool`, `send_buffer_size: Option<usize>`, `recv_buffer_size: Option<usize>`.
- `src/protocol/coordinator.rs::accept_workers()` (line 40): Apply `set_nodelay(true)` and buffer sizes to each accepted `TcpStream`.
- `src/protocol/worker.rs::connect_with_retry()` (line 47): Apply tuning to connected `TcpStream`.
- `Cargo.toml`: Add `socket2` dependency for buffer size configuration.

**Estimated LoC:** ~100. No correctness risk. Ship first.

### Tier 1 Feature: 2.23 Wire Format Compaction (bincode v2 + LZ4)

**Exact files/functions to modify:**
- `Cargo.toml`: Change `bincode = "1"` to `bincode = "2"`, add `lz4_flex`.
- `src/protocol/frame.rs::send_frame()` (line 56): Change `bincode::serialize` to `bincode::serde::encode_to_vec`.
- `src/protocol/frame.rs::recv_frame()` (line ~90): Change `bincode::deserialize` to `bincode::serde::decode_from_slice`.
- `src/protocol/coordinator.rs`: Bump `PROTOCOL_VERSION` from 1 to 2.
- Optionally `src/net/types.rs`: Custom serde impl on `PortRef` for compact varint encoding.
- Optionally `src/partition/compact.rs`: Verify `CompactSubnet` round-trip under bincode v2.
- All files using `bincode::serialize`/`bincode::deserialize` in tests (~20 call sites across `net/types.rs`, `partition/types.rs`, `protocol/types.rs`).

**Estimated LoC:** ~350. Breaks wire compatibility (PROTOCOL_VERSION bump).

### Tier 1 Feature: 2.25 Transport Abstraction

**Exact files/functions to modify:**
- New file `src/protocol/transport.rs`: Define `trait Transport { async fn listen(...); async fn connect(...); }`.
- New file `src/protocol/tcp.rs`: Extract TCP logic from `coordinator.rs`/`worker.rs`.
- New file `src/protocol/channel.rs`: In-memory transport for testing.
- `src/protocol/coordinator.rs::accept_workers()`: Replace `TcpListener::bind()` with `Transport::listen()`.
- `src/protocol/worker.rs::connect_with_retry()`: Replace `TcpStream::connect()` with `Transport::connect()`.
- `src/protocol/mod.rs`: Add module declarations.

**Estimated LoC:** ~200 (150 new + 50 refactored). Zero correctness risk -- `frame.rs` is already generic.

### Tier 1 Feature: 2.26 Delta-Only Protocol (centrepiece)

**Exact files/functions to modify:**
- New file `src/merge/grid_delta.rs`: `run_grid_delta()` function (~400-500 LoC).
- New struct/file for `BorderGraph` (~400 LoC): coordinator-side border connectivity tracking.
- `src/protocol/types.rs`: 5 new `Message` variants appended after line 73.
- `src/protocol/coordinator.rs`: New FSM states (`HoldingPartition`, `AwaitingDelta`), delta distribution logic.
- `src/protocol/worker.rs`: Stateful partition loop (worker holds partition across rounds).
- `src/merge/types.rs`: Extend `GridConfig` with `delta_mode: bool`. Extend `GridMetrics` with delta-specific counters.
- `src/merge/helpers.rs::rebuild_free_port_index()`: Called per-round with updated border ranges from `RoundStart` messages.
- Spec updates: SPEC-01 D6/G1 reproof, new SPEC-17/18 for `BorderGraph`.

**Estimated LoC:** ~1,500-1,800. Highest complexity. 2-4 weeks of focused work.

### Dependency graph:

```
2.22 (TCP Tuning) ----------> independent, ship first
                               |
2.25 (Transport Trait) ------> independent, ship early
                               |
2.23 (bincode v2 + LZ4) ----> independent of 2.25, breaks wire compat
                               |
                               v
2.26 (Delta Protocol) -------> depends on 2.25 (for testing with ChannelTransport)
      |                        requires SPEC-01 D6/G1 reproof
      |                        requires new SPEC-17/18
      v
2.2 (Dynamic Joining) -------> depends on 2.26 optional (interacts with delta state)
      |
      v
2.3 (Dynamic Departure) -----> depends on 2.2
```

---

## Primary Sources

| # | File | Relevance |
|---|------|-----------|
| 1 | `specs/SPEC-01-invariantes.md` | All invariants T1-T7, D1-D6, I1-I7, G1. Impact analysis basis for every v2 feature tier. |
| 2 | `specs/SPEC-04-partition.md` | Split API R1-R5, C1-C3 correctness conditions R6-R10, FreePort/border mechanism R11-R15, ID space partitioning R16-R20. |
| 3 | `specs/SPEC-05-merge.md` | Merge R1-R11, border detection R12-R14, grid loop R24-R30a, strict/lenient BSP. |
| 4 | `specs/SPEC-06-wire-protocol.md` | Message variants R1-R5, framing R6-R10, bincode serialization R11-R15, TCP transport R16-R22. |
| 5 | `specs/SPEC-13-system-architecture.md` | Module boundaries R5-R14, dependency direction R6-R8, Transport trait spec, FSM definitions R19-R25, error handling R15-R18. |
| 6 | `src/protocol/types.rs` | Message enum (7 variants), discriminant order, serde derives. |
| 7 | `src/protocol/frame.rs` | FrameHeader, send_frame/recv_frame generic signatures, bincode v1 call sites. |
| 8 | `src/protocol/coordinator.rs` | accept_workers(), TcpListener hardcode, Register handshake, PROTOCOL_VERSION. |
| 9 | `src/protocol/worker.rs` | connect_with_retry(), TcpStream hardcode, daemon mode, retry backoff. |
| 10 | `src/protocol/error.rs` | ProtocolError enum (8 variants), manual Display impl. |
| 11 | `src/merge/grid.rs` | run_grid() BSP loop, lenient/strict mode, termination check. |
| 12 | `src/merge/core.rs` | merge() function, PartitionPlan consumption, border restoration. |
| 13 | `src/merge/helpers.rs` | rebuild_free_port_index(), drain_stale_redexes(). |
| 14 | `src/merge/types.rs` | GridMetrics (18 fields), WorkerRoundStats (6 fields), GridConfig (3 fields). |
| 15 | `src/partition/split.rs` | split() 7-step algorithm, trivial plan, root propagation. |
| 16 | `src/partition/types.rs` | Partition struct (CompactSubnet serde), PartitionPlan, IdRange. |
| 17 | `src/partition/compact.rs` | CompactSubnet, serialize/deserialize_subnet_compact. |
| 18 | `src/partition/strategy.rs` | PartitionStrategy trait, ContiguousIdStrategy. |
| 19 | `src/net/types.rs` | PortRef, Agent, Symbol, AgentId, port_index(), DISCONNECTED. |
| 20 | `src/net/core.rs` | Net struct, create_agent, connect, get_target, freeport_redirects. |
| 21 | `src/reduction/mod.rs` | Re-exports: reduce_all, reduce_n, reduce_step, rules. |
| 22 | `src/reduction/engine.rs` | reduce_all(), reduce_step(), ReductionStats, StepResult. |
| 23 | `Cargo.toml` | bincode = "1", feature flags, 15 always-on + 8 optional deps. |
| 24 | `docs/ROADMAP.md` (section 2.40) | Break-even model: c_o/c_r=2.2, need <0.50, 77% reduction required. |
| 25 | `docs/ROADMAP.md` (section 2.26) | Delta protocol design: stateful workers, BorderGraph, new messages. |
