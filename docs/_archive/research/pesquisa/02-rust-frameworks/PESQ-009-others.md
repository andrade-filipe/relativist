---
pesq_id: PESQ-009
title: "Other Rust Distributed Computing Crates (2025 Landscape)"
category: Rust Distributed Frameworks
date: 2026-03-26
status: Complete
cross_references:
  specs: [SPEC-06, SPEC-13]
  pesqs: [PESQ-006, PESQ-007, PESQ-008]
  discs: [DISC-006, DISC-008]
---

# PESQ-009: Other Rust Distributed Computing Crates (2025 Landscape)

**Category:** Rust Distributed Frameworks
**Status:** Complete
**Cross-references:**
- Specs: SPEC-06 (wire protocol), SPEC-13 (system architecture)
- PESQs: PESQ-006 (Hydro), PESQ-007 (Paladin), PESQ-008 (Constellation)
- Discussions: DISC-006 v2 (communication overhead), DISC-008 (shared memory → distributed)

---

## 1. Subject Overview

Beyond the three frameworks analyzed in depth (Hydro, Paladin, Constellation), the Rust distributed computing ecosystem in 2025 includes several other notable crates. This document surveys the landscape to identify any additional patterns, libraries, or tools relevant to Relativist's architecture decisions.

The ecosystem has matured significantly: as noted by practitioners, "developers can spend 90% of their time on protocol instead of plumbing" (Disant Upadhyay, 2025). This validates Relativist's approach of building a focused, custom system rather than adopting a general framework.

---

## 2. Crate Survey

### 2.1 d-engine — Distributed Coordination Engine

**Repository:** https://github.com/deventlab/d-engine
**Purpose:** Lightweight Raft consensus implementation for Rust applications
**License:** MIT / Apache 2.0

d-engine provides:
- **Raft consensus** with a single-threaded event loop
- **CompareAndSwap** (CAS) for distributed locks and leader election
- **Flexible read consistency:** Linearizable / Lease-Based / Eventual
- **Watch API** for real-time change notifications
- **Pluggable storage:** RocksDB, Sled, raw file
- **Embedded + standalone modes**

Performance (v0.2.3, 3-node AWS EC2):
- 64K writes/sec, 181K linearizable reads/sec
- Sub-millisecond latency in embedded mode

**Relevance to Relativist:** LOW. d-engine solves consensus (agreement among replicas), which Relativist does not need. Relativist has a single coordinator — no leader election, no replicated state. However, if Relativist ever needs coordinator high-availability (future roadmap), d-engine's embedded Raft could be a candidate.

**Assessment:** REJECT for v1. Note for future HA coordinator.

### 2.2 Ractor — Rust Actor Framework

**Repository:** https://github.com/slawlor/ractor
**Purpose:** Actor framework inspired by Erlang/OTP
**Features:**
- Typed actors with message handling
- Supervision trees (unlike Constellation)
- Remote actors via `ractor_cluster`
- Built on tokio

**Relevance to Relativist:** LOW. Actor model is more suitable for heterogeneous, message-driven systems. Relativist's workers are homogeneous reducers in a BSP pattern — actors add abstraction without benefit. Same conclusion as Constellation (PESQ-008 L1).

**Assessment:** REJECT. Actor model doesn't fit BSP coordinator-worker.

### 2.3 Coerce-rs — Actor Runtime & Distributed Systems

**Repository:** https://github.com/LeonHartley/Coerce-rs
**Purpose:** Actor runtime with clustering, persistence, and distributed state
**Features:**
- Actor clustering with node discovery
- Persistent actors (journal + snapshot)
- CQRS/Event Sourcing support
- Remote messaging

**Relevance to Relativist:** LOW. Same reasoning as Ractor. The persistence and CQRS features are designed for stateful services, not computational pipelines.

**Assessment:** REJECT. Stateful actor patterns don't map to IC reduction.

### 2.4 Kameo — Lightweight Actor Library

**Repository:** https://crates.io/crates/kameo
**Purpose:** Fault-tolerant async actors with distributed support
**Features:**
- Kademlia DHT for distributed actor lookup (via libp2p)
- Typed message handling
- Actor lifecycle management
- Async-first design on tokio

**Relevance to Relativist:** LOW for the actor model, but INTERESTING for the DHT-based discovery pattern. If Relativist needed peer-to-peer worker discovery (no coordinator), Kameo's libp2p approach would be relevant. Not applicable to v1's star topology.

**Assessment:** REJECT. DHT discovery noted for future peer-to-peer mode.

### 2.5 Node Crunch — Simple Distributed Computing

**Repository:** Available on crates.io
**Purpose:** Minimal framework for distributing computation across nodes
**Features:**
- Simple client-server model
- Task distribution via TCP
- Minimal dependencies

**Relevance to Relativist:** MODERATE conceptually — Node Crunch's simplicity is closest to Relativist's model (coordinator sends work units, workers return results). However, it lacks the specific features Relativist needs (partitioning awareness, BSP rounds, border redex handling).

**Assessment:** REJECT as dependency. VALIDATE as proof that simple coordinator-worker over TCP is a viable pattern in Rust.

### 2.6 Tokio Ecosystem (not a framework, but essential infrastructure)

Relativist's actual distributed infrastructure is built on:

| Crate | Purpose | Relativist Usage |
|-------|---------|-----------------|
| `tokio` | Async runtime | Coordinator event loop, worker async recv |
| `tokio::net::TcpListener/TcpStream` | TCP | All network I/O |
| `tokio::sync` | Channels, mutexes | Internal coordination |
| `tokio::time` | Timeouts, intervals | Heartbeat, round timeouts |
| `tokio-util` | Framing codecs | Length-delimited message framing |

This is not a distributed framework but a set of primitives. Relativist assembles these into its own coordinator-worker system. This is the dominant pattern in the Rust ecosystem for purpose-built distributed systems.

---

## 3. Ecosystem Patterns

Analyzing all surveyed crates (PESQ-006 through PESQ-009), clear patterns emerge:

### 3.1 Three Tiers of Abstraction

| Tier | Examples | Relativist Position |
|------|----------|-------------------|
| **High-level frameworks** | Hydro (dataflow), Amadeus (data processing) | Too abstract, wrong model |
| **Mid-level frameworks** | Paladin (declarative ops), Constellation (actors), Ractor | Wrong abstraction for BSP |
| **Low-level primitives** | tokio, serde, bincode | **Relativist builds here** |

**Conclusion:** Relativist correctly positions itself at the primitive level, assembling tokio + serde + bincode into a purpose-built system. No existing framework provides the specific combination of BSP rounds + IC net reduction + border redex handling.

### 3.2 Serialization Convergence

| Framework | Serialization |
|-----------|--------------|
| Constellation | serde + bincode |
| Paladin | serde + postcard/CBOR |
| Hydro | serde + bincode |
| d-engine | protobuf |
| Ractor | serde (format pluggable) |
| **Relativist** | **serde + bincode** |

The ecosystem has converged on serde as the serialization framework, with bincode as the most common binary format. Relativist's choice is validated.

### 3.3 Async Runtime Convergence

Every surveyed crate uses tokio as the async runtime (except Constellation which uses mio directly). Tokio is the de facto standard. Relativist's choice is validated.

### 3.4 No Existing BSP Framework in Rust

None of the surveyed crates implement BSP (Bulk Synchronous Parallel). The closest patterns are:
- **MapReduce:** Implemented ad-hoc in application code (no standard crate)
- **Dataflow:** Hydro (compile-time graph, not runtime BSP)
- **Actor:** Multiple options (but actors ≠ BSP)
- **Consensus:** d-engine (Raft, irrelevant to BSP)

This confirms that Relativist must implement its own BSP coordination logic. There is no off-the-shelf solution.

---

## 4. Dependency Recommendations for Relativist

Based on the full PESQ-006 through PESQ-009 survey, here are the recommended external crates:

### 4.1 Confirmed Dependencies (validated by ecosystem analysis)

| Crate | Purpose | Validated by |
|-------|---------|-------------|
| `tokio` | Async runtime | All frameworks use it |
| `serde` | Serialization framework | Universal in Rust distributed |
| `bincode` | Binary format | Constellation, Hydro, Node Crunch |
| `clap` | CLI parsing | Standard for Rust CLIs |
| `tracing` | Structured logging | See PESQ-014/015 |
| `thiserror` | Error types | Paladin, Hydro patterns |
| `proptest` | Property testing | See PESQ-022 |
| `criterion` | Benchmarks | Standard for Rust benchmarks |

### 4.2 Conditional Dependencies (feature-flagged)

| Crate | Purpose | Feature Flag | Validated by |
|-------|---------|-------------|-------------|
| `rustls` | TLS 1.3 | `tls` | See PESQ-017 |
| `prometheus` | Metrics export | `metrics` | See PESQ-016 |
| `tracing-opentelemetry` | Distributed tracing | `otel` | See PESQ-014 |
| `tokio-rustls` | Async TLS | `tls` | Ecosystem standard |

### 4.3 Rejected Dependencies

| Crate | Why Rejected |
|-------|-------------|
| `paladin-core` | Wrong abstraction (Monoid-based, AMQP transport) |
| `constellation-rs` | Nightly-only, no Windows, process model overkill |
| `ractor` / `coerce-rs` / `kameo` | Actor model doesn't fit BSP |
| `d-engine` | Consensus not needed (single coordinator) |
| `hydroflow` | Compile-time dataflow graph, wrong model |
| `anyhow` | Too loose for library error handling; prefer `thiserror` |

---

## 5. Lessons for Relativist (Consolidated)

### L1: Build on Primitives, Not Frameworks [ADOPT]
No existing Rust distributed framework matches Relativist's requirements. The correct approach is to build on tokio + serde + bincode, implementing custom BSP coordination. This is what most production Rust distributed systems do.
→ Informs: SPEC-13

### L2: serde + bincode is the Ecosystem Standard [ADOPT]
Three out of four surveyed frameworks use this combination. It's well-tested, performant, and has excellent Rust integration. No reason to deviate.
→ Informs: SPEC-06, SPEC-13

### L3: tokio is Non-Negotiable [ADOPT]
Every significant Rust distributed system uses tokio. The ecosystem (tracing, rustls, hyper) is built around it. Using anything else would mean fighting the ecosystem.
→ Informs: SPEC-13

### L4: thiserror > anyhow for Libraries [ADOPT]
Paladin and Hydro both use structured error types (not `anyhow`). For a system where error classification matters (transient vs fatal, as identified in PESQ-007), typed errors via `thiserror` are superior.
→ Informs: SPEC-13

### L5: Feature Flags for Optional Components [ADOPT]
Multiple frameworks use Cargo features to gate optional functionality (TLS, metrics, specific backends). Relativist should do the same: `tls`, `metrics`, `otel` feature flags.
→ Informs: SPEC-13

### L6: No BSP Crate Exists — Build Custom [ADOPT]
The absence of a Rust BSP framework confirms that Relativist's grid cycle (SPEC-05) must be a custom implementation. This is expected for a research system.
→ Informs: SPEC-05, SPEC-13

### L7: In-Memory Mode for Testing [ADOPT]
Both Paladin (PESQ-007 L5) and Hydro (PESQ-006) provide in-process/simulation modes. Relativist should implement an in-memory grid mode where coordinator and workers run in the same process, enabling deterministic testing without network I/O.
→ Informs: SPEC-08, SPEC-13

---

## 6. Sources

| Source | URL | Accessed |
|--------|-----|----------|
| d-engine GitHub | https://github.com/deventlab/d-engine | 2026-03-26 |
| d-engine docs.rs | https://docs.rs/d-engine/latest/d_engine/ | 2026-03-26 |
| Ractor GitHub | https://github.com/slawlor/ractor | 2026-03-26 |
| Coerce-rs GitHub | https://github.com/LeonHartley/Coerce-rs | 2026-03-26 |
| Kameo crates.io | https://crates.io/crates/kameo | 2026-03-26 |
| Rust in Distributed Systems 2025 | https://disant.medium.com/rust-in-distributed-systems-2025-edition-175d95f825d6 | 2026-03-26 |
| Rust Distributed Ecosystem overview | https://andrewodendaal.com/rust-distributed-systems-ecosystem/ | 2026-03-26 |
| crates.io distributed-systems keyword | https://crates.io/keywords/distributed-systems | 2026-03-26 |
