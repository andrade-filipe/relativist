# BRIEF: Relativist Codebase Assessment for Tier 5 (Complex Features)

**Generated:** 2026-04-15
**Scope:** Assess current codebase state for Tier 5 feature insertion
**Features covered:** 2.15 (Compact Memory), 2.21 (WAN Deployment), 2.21.1 (Security Analysis), 2.29 (Recipe-Based Gen), 2.36 (Lazy Generation), 2.39 (GUI)
**Method:** Systematic reading of `src/net/types.rs`, `src/net/core.rs`, `src/protocol/coordinator.rs`, `src/protocol/frame.rs`, `src/protocol/types.rs`, `src/io/generators.rs`, `src/security/`, `src/lib.rs`, `Cargo.toml`, and all 6 existing Tier 1-3 specs (SPEC-17 through SPEC-22)

---

## Executive Summary

Tier 5 features touch four distinct areas of the codebase with varying levels of refactoring risk:

1. **net/ module (2.15 Compact Memory):** DEEP REFACTOR. The most invasive Tier 5 change. Every module that reads or writes `PortRef`, `Agent`, or `Net` is affected. The current `Vec<Option<Agent>>` + `Vec<PortRef>` representation with enum-based `PortRef` (6 bytes) would be replaced by bit-packed `u32` port references (4 bytes) and potentially dual-buffer `nodes: Vec<u64>` / `vars: Vec<u32>`. The public API (`create_agent`, `connect`, `get_target`) can remain stable — only internal representation changes. **Risk:** HIGH. Every assertion, every test, every debug print that pattern-matches on `PortRef::AgentPort(id, port)` must be updated or use accessor methods.

2. **protocol/ + security/ modules (2.21 WAN, 2.21.1 Security):** MODERATE REFACTOR. WAN deployment requires TLS integration (rustls is already an optional dependency), mTLS/OAuth2 auth (replaces SPEC-10 plaintext tokens), session-aware reconnect (new state in coordinator/worker FSMs), relay server (new crate), and adaptive timeouts. The existing `frame.rs` is already generic over `AsyncRead + AsyncWrite`, which makes Transport trait integration (SPEC-17) the natural insertion point. **Risk:** MEDIUM. The FSM changes (reconnect, session tracking) are the hardest part — they touch `coordinator.rs` and `worker.rs` control flow directly.

3. **io/generators.rs (2.29 Recipe Gen, 2.36 Lazy Gen):** LOW-MEDIUM REFACTOR. Recipe-based generation adds a `GenerationRecipe` type and per-generator `make_local_partition()` methods alongside the existing `generate()` function. The current generators are simple imperative functions (30-80 LoC each) that build a `Net` sequentially — adapting them to accept an `IdRange` and generate a partial net is straightforward for independent-pair benchmarks. Lazy generation (2.36) adds an orchestration layer in `merge/grid.rs` for pull-based dispatch. **Risk:** LOW. Existing code is not modified, only extended.

4. **Workspace restructure (2.39 GUI):** ARCHITECTURAL. The single-crate `relativist` must become a Cargo workspace with `relativist-core` (library), `relativist-cli` (binary), and `relativist-gui` (Tauri app). This is a structural change that does not modify any source file's semantics but changes every `use relativist::*` import path. **Risk:** MEDIUM. The restructure is mechanical but affects CI, release scripts, and the entire import graph.

**Cross-tier dependency:** Tier 5 features build on Tier 1 infrastructure. Specifically:
- 2.21 (WAN) requires SPEC-17 Transport trait — the `trait Transport` abstraction that decouples coordinator/worker from TCP specifics.
- 2.15 (Compact Memory) is designed to amend SPEC-02 (net/ types), which SPEC-22 (Arena Management) also amends.
- 2.36 (Lazy Gen) requires SPEC-21 (Streaming Generation) to be implemented first.

---

## Per-Feature Codebase Analysis

### 2.15 Compact Memory Representation

**Current state (net/ module, 1,865 LoC, 4 files):**

```
src/net/
  types.rs    — AgentId (u32), PortId (u8), PortRef (enum), Agent (struct), Symbol (enum)
  core.rs     — Net struct (agents: Vec<Option<Agent>>, ports: Vec<PortRef>, ...)
  debug.rs    — assert_all_invariants(), debug printing
  mod.rs      — re-exports
```

**Key types to change:**

| Current | Proposed (HVM2-style) | Impact |
|---------|----------------------|--------|
| `PortRef` enum (AgentPort/FreePort), ~6 bytes | `u32` bit-packed: `(val << TAG_BITS) \| tag`, 4 bytes | Every pattern match on PortRef becomes a bit-mask operation |
| `Agent { symbol: Symbol, id: AgentId }`, 8 bytes | `u64` packed: symbol in high bits, id in low bits | Agent creation/inspection becomes bit ops |
| `Vec<Option<Agent>>` arena | `Vec<u64>` nodes (aux port pairs) + `Vec<u32>` vars (linking variables) | Dual address space, fundamentally different layout |
| `Vec<PortRef>` flat port array | Integrated into `nodes[]` and `vars[]` | Port indexing formula changes |
| `port_index(id, port) = id * 3 + port` | Different formula depending on symbol and port type | Must be encapsulated behind accessor |

**Files that reference PortRef directly (grep for `PortRef`):**

| File | Occurrences | Nature |
|------|-------------|--------|
| `net/types.rs` | Definition | Type definition, must change |
| `net/core.rs` | ~40 | Arena ops: `create_agent`, `connect`, `get_target`, `remove_agent` |
| `reduction/rules.rs` | ~30 | Interaction rules: pattern match to extract (id, port) for reconnection |
| `reduction/engine.rs` | ~10 | `reduce_step`: reads principal ports to detect active pairs |
| `partition/strategy.rs` | ~15 | `split()`: iterates agents, builds border map from FreePort |
| `partition/compact.rs` | ~20 | `CompactSubnet`: custom serde for wire encoding |
| `partition/helpers.rs` | ~10 | `compute_id_ranges`, `build_subnet` |
| `merge/core.rs` | ~25 | `merge()`: reconnects border FreePort → AgentPort |
| `merge/grid.rs` | ~5 | Grid loop: passes partitions around |
| `protocol/types.rs` | ~5 | Message enum contains Partition (contains Net) |
| `io/generators.rs` | ~40 | Every generator uses `PortRef::AgentPort(id, 0)` to connect |
| `encoding/church.rs` | ~20 | Church numeral encoding uses PortRef |
| `bench/suite.rs` | ~5 | Benchmark verification |
| **Total** | **~225** | |

**Migration strategy:**

The ROADMAP recommends: "Encapsulate PortRef in a newtype with conversion methods (SPEC-02 Section 5.4 already anticipates this). The public API (`connect`, `get_target`, `create_agent`) does not change; only the internal representation does."

Recommended approach:
1. **Phase A: Abstract the interface.** Introduce `PortRef::agent_id() -> AgentId`, `PortRef::port_id() -> PortId`, `PortRef::is_agent_port() -> bool`, `PortRef::is_free_port() -> bool` accessor methods. Migrate all 225 pattern-match sites to use accessors. This is a large but mechanical refactor that can be done on v1 without changing behavior.
2. **Phase B: Swap the representation.** Change `PortRef` from `enum { AgentPort(AgentId, PortId), FreePort(u32) }` to `struct PortRef(u32)` with bit-packed encoding. The accessor methods now decode bits instead of matching enum variants. All 225 call sites continue to work because they use accessors.
3. **Phase C: Dual buffer.** Replace `Net.agents: Vec<Option<Agent>>` + `Net.ports: Vec<PortRef>` with `nodes: Vec<u64>` + `vars: Vec<u32>`. This changes the internal layout of `Net` but the public API (`create_agent`, `connect`, `get_target`) is unchanged.

**SPEC-22 interaction:** SPEC-22 (Arena Management) adds arena recycling (2.33) and sparse representation (2.32) to the same `net/` module. If SPEC-23 (Compact Memory) is implemented first, SPEC-22's free-list must operate on the compact arena. If SPEC-22 is implemented first, SPEC-23 must rewrite the free-list for compact storage. **Recommendation:** Implement SPEC-23 AFTER SPEC-22, so the free-list is designed for the compact layout from the start.

**Test impact:** 690 tests. Approximately 150-200 tests in `net/`, `reduction/`, and `partition/` directly construct `PortRef` values. All must be updated to use the new constructor or the compact encoding.

---

### 2.21 WAN / Internet Deployment + 2.21.1 Security Analysis

**Current state (protocol/ + security/, ~3,300 LoC combined):**

```
src/protocol/
  types.rs      — Message enum (7 variants), payload structs
  frame.rs      — send_frame/recv_frame over AsyncRead+AsyncWrite (547 LoC)
  config.rs     — NodeConfig (bind, timeouts)
  coordinator.rs — accept_workers, distribute, collect (hardcoded TcpStream)
  worker.rs     — connect_to_coordinator, reduce_loop (hardcoded TcpStream)
  error.rs      — ProtocolError
  mod.rs        — re-exports

src/security/
  token.rs      — AuthToken generation and validation
  tls.rs        — TLS config (optional, rustls)
  error.rs      — SecurityError
  mod.rs        — re-exports
```

**Key observations:**

1. **frame.rs is already generic.** `send_frame` and `recv_frame` take `impl AsyncWriteExt + Unpin` and `impl AsyncReadExt + Unpin` respectively. This means the Transport trait (SPEC-17) can be introduced without modifying frame.rs at all. TLS streams (`tokio_rustls::TlsStream<TcpStream>`) already implement `AsyncRead + AsyncWrite` and can be used directly.

2. **coordinator.rs and worker.rs are hardcoded to TcpStream.** `accept_workers()` returns `Vec<TcpStream>`. `connect_to_coordinator()` returns `TcpStream`. These functions are the insertion point for the Transport trait (SPEC-17), which SPEC-24 builds upon.

3. **Security module has TLS scaffolding.** `src/security/tls.rs` exists with `rustls` support (optional feature). The current implementation provides certificate loading and TLS config. WAN deployment extends this with: mTLS (client certificates), certificate chain validation, trust-on-first-use pinning.

4. **No session tracking.** The coordinator tracks workers by index (0..num_workers). There is no session ID, no reconnect capability, no worker identity beyond the index. WAN reconnect requires:
   - `SessionId` type (UUID or random token) assigned at registration.
   - Session state in coordinator: `HashMap<SessionId, WorkerSession>`.
   - New FSM states: `Reconnecting`, `SessionExpired`.
   - New Message variants: `Reconnect { session_id }`, `ReconnectAck`, `ReconnectNack`.

5. **No relay server.** WAN deployment with NAT traversal requires a relay/rendezvous server. This would be a new crate (`relativist-relay`) with its own binary and minimal dependencies (tokio, rustls). The relay forwards framed messages between coordinator and workers, adding one network hop.

**Insertion points for SPEC-24:**

| Component | Where | What to add |
|-----------|-------|-------------|
| TLS everywhere | `security/tls.rs` | Mandatory TLS 1.3 for WAN mode, certificate generation, fingerprint verification |
| mTLS auth | `security/` new file | Client certificate issuance, CA management, revocation |
| Session tracking | `coordinator.rs` | `SessionId`, `WorkerSession`, reconnect handling |
| Reconnect | `protocol/types.rs` | 3+ new Message variants (Reconnect, ReconnectAck, ReconnectNack) |
| Adaptive timeouts | `protocol/config.rs` | RTT tracking, dynamic timeout computation |
| Discovery | New file or crate | Rendezvous URL, bootstrap list |
| Relay server | New crate | `relativist-relay` binary |
| Abuse mitigation | `coordinator.rs` | Rate limiting, auth failure logging, frame size pre-check |

**SPEC-17 dependency:** WAN deployment MUST be implemented after SPEC-17 (Transport Abstraction). The Transport trait provides the generic interface over which TLS, relay, and UDS operate. Without it, WAN changes would be hardcoded to a specific TLS-over-TCP path, making future transport additions (QUIC, relay) require further refactoring.

**2.21.1 (Security Analysis) codebase needs:**
- No code changes. The deliverable is a document.
- Requires reading: `src/security/` (current auth), `src/protocol/coordinator.rs` (accept_workers handshake), `src/protocol/types.rs` (Message enum, payload validation), `src/protocol/frame.rs` (framing, CRC32C, max_payload_size), `src/observability/` (logging, metrics).
- The analysis should catalog every `recv_frame` call site (attack surface: what happens if the frame content is adversarial?) and every `send_frame` call site (information disclosure: what data crosses the wire?).

---

### 2.29 Recipe-Based Distributed Generation

**Current state (io/generators.rs, ~350 LoC):**

```rust
pub fn generate(example: ExampleNet, size: u32) -> Net {
    match example {
        ExampleNet::EpAnnihilation => ep_annihilation(size),
        ExampleNet::EpAnnihilationCon => ep_annihilation_con(size),
        // ... 7 more variants
    }
}
```

Each generator is a standalone function that creates a `Net`, calls `create_agent()` and `connect()` in a loop, and returns the complete net. The generators are imperative and simple (30-80 LoC each).

**What recipe generation adds:**

1. **New types (protocol/types.rs or io/recipe.rs):**
   ```rust
   pub struct GenerationRecipe {
       pub benchmark: ExampleNet,
       pub global_size: u32,
       pub num_workers: u32,
       pub worker_id: WorkerId,
       pub id_range: IdRange,
       pub borders: Vec<(PortRef, u32)>,  // (local_port, border_id)
   }
   ```

2. **New per-generator methods (io/generators.rs):**
   ```rust
   pub fn make_local_partition(recipe: &GenerationRecipe) -> Partition
   ```
   For `ep_annihilation(N)` with K workers: worker `w` generates pairs `w*chunk..(w+1)*chunk`. Each worker pre-sets `net.next_id = recipe.id_range.start` so IDs fall in the assigned range.

3. **New coordinator dispatch path (coordinator.rs or merge/grid.rs):**
   Instead of `split()` → `AssignPartition`, the coordinator sends `AssignRecipe`. New Message variant:
   ```rust
   AssignRecipe { round: u32, recipe: GenerationRecipe }
   ```

4. **New worker response path (worker.rs):**
   Worker receives `AssignRecipe`, calls `make_local_partition()`, reduces locally, returns `PartitionResult`.

**Generator decomposability analysis:**

| Generator | Decomposable? | Strategy | Borders |
|-----------|--------------|----------|---------|
| `ep_annihilation` | YES | Independent pairs, trivial split | None |
| `ep_annihilation_con` | YES | Independent pairs, trivial split | None |
| `ep_annihilation_dup` | YES | Independent pairs, trivial split | None |
| `con_dup_expansion` | YES | Independent pairs, trivial split | None |
| `mixed_rules` | YES | Composition of independent pairs | None |
| `dual_tree` | PARTIAL | Level-based split, single root border | 1 border wire |
| `erasure_propagation` | PARTIAL | Chain segments, border at cut points | K-1 borders |
| `tree_sum` | NO | Sequential DUP chain | Falls back to centralized |
| `sum_of_squares` | NO | Church encoding, sequential | Falls back to centralized |

**IdRange infrastructure:** Already exists in `src/partition/helpers.rs`:
```rust
pub fn compute_id_ranges(num_workers: u32, next_id: AgentId) -> Vec<(AgentId, AgentId)>
```
This function computes disjoint `(start, end)` ranges for each worker. Recipe generation reuses this directly.

**Test impact:** Low. New tests for `make_local_partition()` per generator. Existing generator tests unchanged — `generate()` still works for centralized mode.

---

### 2.36 Lazy/Demand-Driven Generation (Pull Model)

**Current state (merge/grid.rs, ~400 LoC):**

The grid loop in `run_grid()` follows a push model:
1. Generate full net
2. Split into K partitions
3. Dispatch all K partitions simultaneously
4. Wait for all K results
5. Merge → check convergence → repeat or finish

**What lazy generation changes:**

The new `run_grid_lazy()` (or amendment to `run_grid()`) follows a pull model:
1. Coordinator holds a generator (streaming or recipe-based)
2. Workers send `RequestWork` messages
3. Coordinator generates/dispatches one chunk per request
4. Workers reduce locally and request more (or send `PartitionResult` at convergence)

**Insertion point:** `src/merge/grid.rs` — new function `run_grid_lazy()` or a mode flag in `GridConfig`:
```rust
pub struct GridConfig {
    // ... existing fields ...
    pub generation_mode: GenerationMode,  // Push | Pull
}
```

**New Message variants (protocol/types.rs):**
```rust
RequestWork { worker_id: WorkerId },
WorkComplete { worker_id: WorkerId },
```

**SPEC-21 dependency:** Lazy generation is an orchestration layer on top of streaming generation (2.27+2.28). The `StreamingPartitionStrategy` trait from SPEC-21 provides the chunk-to-worker allocation. Lazy generation wraps this with demand-driven dispatch instead of eager dispatch.

**Risk:** LOW. The existing `run_grid()` is not modified. A new code path is added alongside it.

---

### 2.39 GUI Desktop Application (Tauri v2)

**Current state (Cargo.toml, single crate):**

```toml
[package]
name = "relativist"
# ... single binary crate with lib.rs + main.rs
```

All functionality lives in a single crate. `src/lib.rs` re-exports all modules. `src/main.rs` delegates to `src/commands.rs` which dispatches CLI subcommands.

**Workspace restructure required:**

```
relativist/                     (workspace root)
├── Cargo.toml                  (workspace manifest)
├── relativist-core/            (library crate)
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs              (current src/lib.rs)
│       ├── net/                (unchanged)
│       ├── reduction/          (unchanged)
│       ├── partition/          (unchanged)
│       ├── merge/              (unchanged)
│       ├── protocol/           (unchanged)
│       ├── coordinator.rs      (unchanged)
│       ├── worker.rs           (unchanged)
│       ├── config.rs           (unchanged)
│       ├── commands.rs         (unchanged)
│       ├── security/           (unchanged)
│       ├── observability/      (unchanged)
│       ├── io/                 (unchanged)
│       ├── encoding/           (unchanged)
│       ├── bench/              (unchanged)
│       └── error.rs            (unchanged)
├── relativist-cli/             (binary crate)
│   ├── Cargo.toml
│   └── src/
│       └── main.rs             (current src/main.rs, imports relativist-core)
├── relativist-gui/             (Tauri app)
│   ├── Cargo.toml              (depends on relativist-core + tauri)
│   ├── src-tauri/
│   │   └── src/
│   │       ├── main.rs         (Tauri entry point)
│   │       └── commands.rs     (Tauri command wrappers)
│   └── ui/                     (Svelte frontend)
│       ├── package.json
│       ├── src/
│       │   ├── App.svelte
│       │   ├── routes/         (per-screen components)
│       │   └── lib/            (shared utilities)
│       └── vite.config.ts
└── relativist-relay/           (optional, for 2.21 WAN)
    ├── Cargo.toml
    └── src/
        └── main.rs
```

**Migration steps:**

1. **Create workspace Cargo.toml:**
   ```toml
   [workspace]
   members = ["relativist-core", "relativist-cli", "relativist-gui"]
   ```

2. **Move src/ to relativist-core/src/.** All source files move unchanged. `Cargo.toml` dependencies move to `relativist-core/Cargo.toml`.

3. **Create relativist-cli/:** Thin binary with `fn main()` that calls `relativist_core::commands::run()`. Dependencies: `relativist-core`, `clap`.

4. **Create relativist-gui/:** Tauri v2 project scaffold. `tauri::command` functions wrap `relativist_core` APIs.

5. **Update CI/CD:** Build both `relativist-cli` and `relativist-gui`. Tests run against `relativist-core`.

**Impact on existing code:** Zero semantic changes to any source file. All `use crate::*` imports within `relativist-core` remain valid. Only `use relativist::*` in external contexts (integration tests, benchmarks) becomes `use relativist_core::*`.

**Impact on tests:** The 690 tests are all in `relativist-core` (they test the library, not the CLI). The `[[bench]]` section moves to `relativist-core/Cargo.toml`. No test modifications needed.

**Tauri command layer (relativist-gui/):**

The GUI wraps 8-10 CLI commands as Tauri commands:

| CLI Command | Tauri Command | Core Function |
|------------|---------------|---------------|
| `generate` | `generate_net()` | `commands::run_generate_command()` |
| `inspect` | `inspect_net()` | `commands::run_inspect_command()` |
| `reduce` | `reduce_net()` | `commands::run_reduce_command()` |
| `local` | `run_local_grid()` | `commands::run_local_grid_command()` |
| `coordinator` | `start_coordinator()` | `commands::run_coordinator_command()` |
| `worker` | `start_worker()` | `commands::run_worker_command()` |
| `compute` | `compute_arithmetic()` | `commands::run_compute_command()` |
| `bench` | `run_benchmarks()` | `commands::run_bench_command()` |

Each Tauri command is ~10-30 LoC: parse frontend input → construct core args → call core function → serialize result.

**Frontend technology:** Svelte recommended (ROADMAP 2.39). Tauri v2 has official Svelte template. Bundle size: ~5 KB vs React's ~40 KB. 11 screens mapped in ROADMAP.

**New dependencies:**
- `tauri = "2"` (relativist-gui only)
- `tauri-build = "2"` (build dependency)
- `serde_json = "1"` (already in workspace)
- Node.js + npm for frontend build (dev dependency)

---

## Code Gaps Summary

| # | Gap | Feature | Files | Est. LoC | Risk |
|---|-----|---------|-------|----------|------|
| 1 | Compact PortRef (u32 bit-packed) | 2.15 | net/types.rs, 225 call sites | ~400 | HIGH |
| 2 | Session tracking + reconnect FSM | 2.21 | coordinator.rs, worker.rs, protocol/types.rs | ~300 | MEDIUM |
| 3 | mTLS auth + certificate management | 2.21 | security/ (new files) | ~200 | MEDIUM |
| 4 | Relay server | 2.21 | new crate | ~400 | MEDIUM |
| 5 | Adaptive timeouts | 2.21 | protocol/config.rs | ~150 | LOW |
| 6 | GenerationRecipe + local partition gen | 2.29 | io/recipe.rs (new), io/generators.rs | ~700 | LOW |
| 7 | Pull-based dispatch loop | 2.36 | merge/grid.rs | ~350 | LOW |
| 8 | Workspace restructure | 2.39 | Cargo.toml (root + 3 crates) | ~50 config | MEDIUM |
| 9 | Tauri command layer | 2.39 | relativist-gui/src-tauri/ | ~300 | LOW |
| 10 | Svelte frontend (11 screens) | 2.39 | relativist-gui/ui/ | ~2500+ | LOW (isolated) |

**Total estimated new code:** ~5,350 LoC across Tier 5.

---

## Implementation Order Recommendations

```
Phase 1 (parallel with Tier 1-3)     Phase 2 (after M1-M5)        Phase 3 (independent)
─────────────────────────────        ─────────────────────        ──────────────────────
2.39 workspace restructure            2.15 compact memory           2.39 GUI screens
  (needed early, no code change)      (after SPEC-22 arena)         (after workspace done)
                                     2.29 recipe generation
                                       (after generators stable)
                                     2.36 lazy generation
                                       (after SPEC-21 streaming)
                                     2.21 WAN deployment
                                       (after SPEC-17 transport)
                                     2.21.1 security analysis
                                       (parallel with 2.21)
```

**Key sequencing constraints:**
- Workspace restructure (2.39) should happen FIRST — it is a prerequisite for the GUI but also improves CI (parallel crate builds) and enables `relativist-relay` for WAN.
- Compact memory (2.15) should happen AFTER arena management (SPEC-22) — the free-list should be designed for compact layout, not retrofitted.
- WAN (2.21) MUST happen after Transport trait (SPEC-17) — TLS and relay need the generic transport interface.
- Security analysis (2.21.1) can run in PARALLEL with SPEC-24 drafting — findings feed back as amendments.

---

## Patterns to Preserve

1. **Core/Infrastructure separation (SPEC-13 R6-R8).** `net/`, `reduction/`, `partition/`, `merge/` remain pure (no async, no tokio). Compact memory (2.15) is a core-layer change and MUST NOT introduce any async dependency.

2. **Feature gating (SPEC-13 R12, R37).** TLS, metrics, and otel are optional features. WAN-specific code (relay, mTLS) should be behind a new `wan` feature flag.

3. **Discriminant stability (SPEC-06 R5).** New Message variants MUST be appended after the last discriminant (currently 6: RegisterNack). Recipe and lazy generation add 2-3 variants. WAN adds 3+ variants. Coordinate numbering across SPEC-24 and SPEC-25 to avoid conflicts.

4. **Newtype IDs.** `AgentId`, `PortId`, `WorkerId` are all type aliases. Compact memory should consider promoting `PortRef` to a proper newtype struct, consistent with the existing pattern.

5. **Test count floor.** 690 tests MUST remain green. Compact memory (2.15) will require updating ~150-200 test assertions but MUST NOT reduce the count.
