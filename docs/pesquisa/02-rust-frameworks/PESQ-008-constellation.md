---
pesq_id: PESQ-008
title: "Constellation: Distributed Actor Framework for Rust"
category: Rust Distributed Frameworks
date: 2026-03-26
status: Complete
cross_references:
  specs: [SPEC-06, SPEC-13]
  pesqs: [PESQ-006, PESQ-007, PESQ-009]
  discs: [DISC-005, DISC-008]
---

# PESQ-008: Constellation — Distributed Actor Framework for Rust

**Category:** Rust Distributed Frameworks
**Status:** Complete
**Cross-references:**
- Specs: SPEC-06 (wire protocol), SPEC-13 (system architecture)
- PESQs: PESQ-006 (Hydro), PESQ-007 (Paladin), PESQ-009 (others)
- Discussions: DISC-005 v2 (cross-boundary protocol), DISC-008 (shared memory → distributed)

---

## 1. Subject Overview

Constellation is a Rust framework for distributed programming that draws from Erlang/OTP, MPI, and CSP. It provides process spawning with resource constraints, inter-process TCP channels, and asynchronous serialization via serde + bincode. The project aims to make Rust competitive for distributed computing with a process-centric model.

### 1.1 Origin and Purpose

Constellation was created as an infrastructure layer for distributed data processing in Rust. Its companion project **Amadeus** provides higher-level parallel/distributed data analysis (similar to Apache Spark) built on top of Constellation's process primitives. Together they form The Constellation Project ecosystem.

The core abstraction is the **process** — not OS threads, but lightweight logical processes that can be spawned across a cluster. Each process gets a unique PID and communicates via typed channels, similar to Erlang's actor model but with Rust's type safety.

### 1.2 Project Scale and Maturity

| Metric | Value |
|--------|-------|
| Repository | `constellation-rs/constellation` on GitHub |
| Stars | ~599 |
| Language | Rust (nightly required) |
| License | Apache 2.0 |
| Commits | ~277 total |
| Size | Medium (~5-10K SLoC estimated) |
| Platform | Linux (kernel ≥3.9), macOS 10.10+; **no Windows support** |
| Dependencies | serde, serde_closure, bincode, mio, futures-rs |

**Maintenance concern:** The project requires Rust nightly and has no Windows support. Activity appears to have slowed. The nightly requirement is a significant barrier for production adoption.

### 1.3 Ecosystem

| Crate | Purpose |
|-------|---------|
| `constellation-rs` | Core: process spawning, channels, deployment |
| `constellation-internal` | Internal utilities |
| `amadeus` | Data processing layer (Spark-like) on top of constellation |
| `tcp_typed` | Typed TCP channels |
| `palaver` | Cross-platform process utilities |
| `serde_closure` | Serializable closures for remote execution |
| `deploy` | Deployment primitives |

---

## 2. Architecture & Design

### 2.1 Process Model

Constellation uses a **process-per-task** model inspired by Erlang:

```rust
use constellation::*;

fn main() {
    init(Resources::default());

    // Spawn a remote process
    let pid = spawn(
        Resources { mem: 1_000_000, cpu: 1.0 },
        FnOnce!(|parent| {
            // This runs on a remote node
            let receiver = Receiver::<String>::new(parent);
            let msg = receiver.recv().await;
            println!("Got: {}", msg);
        })
    ).expect("spawn failed");

    // Send data to the remote process
    let sender = Sender::<String>::new(pid);
    sender.send("hello".to_string()).await;
}
```

Key characteristics:
- **`init()`** initializes the runtime (must be first call)
- **`spawn(resources, closure)`** creates a new process with resource constraints
- **`Sender<T>` / `Receiver<T>`** — typed, async channels over TCP
- **PIDs** are cluster-wide addresses (no IP/port management)
- Closures must be serializable via `serde_closure`

### 2.2 Communication Layer

- **Transport:** TCP with automatic connection setup/teardown
- **Serialization:** serde + bincode (same as Relativist's choice)
- **I/O:** epoll-based event loop (via mio)
- **Async:** Future-based, compatible with tokio and futures-rs
- Channels implement `Future`, `Stream`, and `Sink` traits
- Supports `select()` and `join()` combinators from futures-rs

### 2.3 Resource Management

Processes declare resource requirements (memory, CPU) at spawn time. The runtime schedules processes to nodes with available resources. This is a **declarative resource model** — you say what you need, not where to run.

### 2.4 Deployment

- Bare metal: Direct cluster deployment (Linux/macOS)
- Kubernetes: Backend for managed clusters
- Local: For development/testing

---

## 3. Key Mechanisms

### 3.1 Process Spawning

The `spawn()` function:
1. Serializes the closure using `serde_closure`
2. Finds a node with sufficient resources
3. Ships the closure to the target node
4. Deserializes and executes it
5. Returns PID for communication

**Comparison with Relativist:** Relativist doesn't spawn processes — workers are pre-deployed and register with the coordinator. The coordinator dispatches *data* (partitioned nets), not *code*. This is a fundamental architectural difference: Constellation moves code to data; Relativist moves data to code.

### 3.2 Typed Channels

Channels are parameterized by message type `T: Serialize + DeserializeOwned`:
- One `Sender<T>` per direction per process pair
- Connection established lazily on first send
- Automatic teardown when channel is dropped
- Back-pressure via async/await

**Comparison with Relativist:** Relativist uses a fixed message enum (`Message` type in SPEC-06) over persistent TCP connections. Constellation's approach is more flexible but heavier — each channel is a separate TCP connection. Relativist's single-connection-per-worker is simpler and sufficient.

### 3.3 Fault Detection

Constellation handles process failure through:
- Process exit status monitoring
- Channel disconnection detection
- Resource limit enforcement (OOM kills)

**Not provided:** Automatic restart, supervision trees (unlike Erlang/OTP). Fault tolerance is left to the application layer.

### 3.4 Serializable Closures

The `serde_closure` crate enables shipping closures across the network. This is a unique Rust capability that mirrors Erlang's ability to send functions between nodes. However:
- Requires Rust nightly
- Imposes constraints on what closures can capture
- Adds complexity to the compilation model

---

## 4. Comparison with Relativist's Context

### 4.1 Core Architecture

| Dimension | Constellation | Relativist | Assessment |
|-----------|--------------|------------|------------|
| Primary abstraction | Process (actor) | Worker (stateless reducer) | Different models |
| Code mobility | Yes (ship closures) | No (ship data) | Relativist simpler |
| Communication | Typed channels per pair | Single TCP + message enum | Relativist simpler |
| Serialization | serde + bincode | serde + bincode | **Identical choice** |
| Async runtime | mio + futures-rs | tokio | Relativist more mainstream |
| Platform | Linux + macOS only | Cross-platform target | Constellation limited |
| Rust version | Nightly required | Stable target | Relativist more practical |

### 4.2 Coordination & Scheduling

| Dimension | Constellation | Relativist | Assessment |
|-----------|--------------|------------|------------|
| Topology | Peer-to-peer (any→any) | Star (coordinator→workers) | Different patterns |
| Scheduling | Resource-based placement | Round-robin partition dispatch | Relativist simpler |
| Task model | Arbitrary closures | Fixed: reduce(partition) | Relativist more constrained |
| Synchronization | None (async channels) | BSP rounds | Relativist has global barriers |
| State management | Per-process local state | Coordinator holds global state | Relativist centralized |
| Process lifecycle | Dynamic spawn/die | Static register/deregister | Relativist more predictable |

### 4.3 Fault Tolerance & Testing

| Dimension | Constellation | Relativist | Assessment |
|-----------|--------------|------------|------------|
| Fault model | Process crash detection | Worker timeout + deregister | Similar basic level |
| Recovery | None (app-level) | Re-dispatch partition | Relativist has recovery |
| Supervision | None | Coordinator monitors all | Relativist has coordinator |
| Testing | Standard Rust tests | DST + property-based (SPEC-08) | Relativist more rigorous |
| Determinism | Non-deterministic | Deterministic reduction (P1) | Relativist by design |

### 4.4 Deployment

| Dimension | Constellation | Relativist | Assessment |
|-----------|--------------|------------|------------|
| Binary model | Library (linked into app) | Single binary, 4 subcommands | Different models |
| Container | Dockerfile provided | Docker multi-stage (SPEC-07) | Both support Docker |
| Kubernetes | Backend available | Compose + K8s planned | Constellation ahead |
| Configuration | Programmatic | CLI flags + env vars | Relativist simpler |

---

## 5. Lessons for Relativist

### L1: Process-per-Task is Overkill for IC Reduction [REJECT]
Constellation's process model assumes arbitrary, heterogeneous tasks. Relativist's workload is homogeneous: every worker runs the same `reduce()` function on different data. Spawning processes adds overhead with no benefit.
→ Informs: SPEC-13

### L2: Typed Channels are Elegant but Unnecessary [REJECT]
Constellation's `Sender<T>` / `Receiver<T>` pattern is type-safe and composable. However, Relativist has exactly one message protocol (SPEC-06) with a fixed enum. A typed channel per message type would fragment connections needlessly.
→ Informs: SPEC-06

### L3: serde + bincode Validation [ADOPT]
Constellation independently chose serde + bincode for network serialization — the same stack Relativist specifies. This validates the choice: compact binary, zero-copy where possible, well-maintained, production-tested.
→ Informs: SPEC-06, SPEC-13

### L4: Resource Declarations at Spawn [ADAPT]
Constellation requires memory/CPU budgets per process. Relativist could adapt this lightly: workers report available resources at registration, coordinator uses this for partition sizing (larger partitions to beefier workers). Not v1, but architecturally sound.
→ Informs: SPEC-13 (future roadmap)

### L5: Avoid Nightly Rust [REJECT]
Constellation's requirement for Rust nightly (due to `serde_closure`) is a major practical barrier. Relativist MUST target stable Rust. This confirms the decision to avoid `serde_closure` and any nightly-only features.
→ Informs: SPEC-13

### L6: PID-based Addressing is Clean [ADAPT]
Constellation abstracts away IP/port into PIDs. Relativist already does something similar: workers have logical IDs assigned at registration, and the coordinator maps logical→physical. The lesson is to ensure the wire protocol uses logical worker IDs, not raw addresses.
→ Informs: SPEC-06

### L7: Amadeus Data Processing Layer [REJECT]
Amadeus (Constellation's Spark-like layer) provides map/filter/reduce/join on distributed datasets. This is far more general than Relativist needs. IC net reduction is a single, specific computation — not a general data pipeline.
→ Informs: SPEC-13

### L8: Lazy Connection Establishment [ADAPT]
Constellation creates TCP connections lazily (on first channel use). Relativist uses persistent connections (established at registration). However, the lazy pattern could apply to inter-worker connections if Relativist ever supports peer-to-peer communication (not in v1).
→ Informs: SPEC-06 (future consideration)

### L9: No Windows = No Go [REJECT]
Constellation's Linux/macOS-only support is a dealbreaker for a general-purpose tool. Relativist should work on any platform where Rust compiles. This reinforces avoiding platform-specific APIs (like Constellation's `/proc` dependency).
→ Informs: SPEC-07, SPEC-13

### L10: Kubernetes Backend [ADAPT]
Constellation has a Kubernetes deployment backend. While Relativist v1 targets Docker Compose, the architecture should not preclude Kubernetes deployment. Keep the coordinator/worker discovery mechanism abstract enough that a K8s service discovery backend could be added later.
→ Informs: SPEC-07

---

## 6. Sources

| Source | URL | Accessed |
|--------|-----|----------|
| GitHub repository | https://github.com/constellation-rs/constellation | 2026-03-26 |
| Constellation Project website | https://constellation.rs/ | 2026-03-26 |
| Amadeus repository | https://github.com/constellation-rs/amadeus | 2026-03-26 |
| constellation-rs crate | https://lib.rs/crates/constellation-rs | 2026-03-26 |

---

## Appendix A: Constellation vs Paladin vs Hydro

| Dimension | Constellation | Paladin | Hydro | Relativist |
|-----------|--------------|---------|-------|------------|
| Abstraction | Process/Actor | Operation/Monoid | Dataflow graph | Coordinator-Worker |
| Communication | Typed TCP channels | AMQP queues | Lattice streams | Custom TCP protocol |
| Serialization | serde + bincode | postcard + CBOR | serde + bincode | serde + bincode |
| Task model | Ship code | Ship operations | Compile dataflow | Ship data |
| Fault tolerance | Detection only | Fatal/Transient | Compile-time CALM | Timeout + re-dispatch |
| Rust version | Nightly | Stable | Stable | Stable |
| Platform | Linux/macOS | Any | Any | Any |
| Maturity | Low-medium | Low | High (academic) | In development |

**Key insight:** All three frameworks solve different problems than Relativist. Constellation is for heterogeneous distributed actors, Paladin for algebraically decomposable tasks, Hydro for dataflow graphs. Relativist's BSP coordinator-worker pattern with homogeneous reduction is simpler than all three, which is appropriate for its focused scope.
