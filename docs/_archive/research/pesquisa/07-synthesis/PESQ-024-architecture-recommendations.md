---
pesq_id: PESQ-024
title: "Architecture Recommendations for Relativist"
category: Synthesis
date: 2026-03-26
status: Complete
cross_references:
  specs: [SPEC-00 through SPEC-13]
  pesqs: [ALL]
  discs: [ALL]
---

# PESQ-024: Architecture Recommendations for Relativist

**Category:** Synthesis
**Status:** Complete
**Purpose:** Definitive architecture blueprint for Relativist v1, synthesizing all research (PESQ-001 to PESQ-023), existing specs (SPEC-00 to SPEC-09), discussions (DISC-001 to DISC-008), and formal arguments (ARG-001 to ARG-004).

---

## 1. System Overview

Relativist is a distributed IC (Interaction Combinators) reduction engine using the **BSP (Bulk Synchronous Parallel)** programming model with a **coordinator-worker** architecture.

```
                    ┌─────────────────┐
                    │   CLI / Config  │
                    │   (clap, env)   │
                    └────────┬────────┘
                             │
                    ┌────────▼────────┐
                    │   Coordinator   │
                    │   (FSM, tokio)  │
                    │                 │
                    │  split → dispatch → wait → merge → check
                    │    ↑                                │
                    │    └────────── loop if not done ────┘
                    └──┬─────────┬─────────┬──────────┘
                       │         │         │
              ┌────────▼──┐ ┌───▼────┐ ┌──▼─────────┐
              │  Worker 1  │ │ Worker 2│ │  Worker N  │
              │ (reduce)   │ │(reduce) │ │ (reduce)   │
              └────────────┘ └────────┘ └────────────┘
```

---

## 2. Dependency Map

### 2.1 Always-On Dependencies

| Crate | Version | Purpose | Justification |
|-------|---------|---------|--------------|
| `tokio` | 1.x | Async runtime | Universal standard (PESQ-009 L3) |
| `serde` | 1.x | Serialization framework | Universal standard (PESQ-009 L2) |
| `bincode` | 2.x | Binary encoding | Validated by 3/4 frameworks (PESQ-009 §3.2) |
| `clap` | 4.x | CLI parsing | Standard for Rust CLIs |
| `tracing` | 0.1 | Structured logging | Single instrumentation API (PESQ-015 L1) |
| `tracing-subscriber` | 0.3 | Log formatting + filtering | Required companion to tracing |
| `thiserror` | 2.x | Error type derivation | Typed errors over anyhow (PESQ-023 D2) |
| `rand` | 0.8 | Random number generation | Token generation, test data |

### 2.2 Feature-Gated Dependencies

| Crate | Feature | Purpose |
|-------|---------|---------|
| `rustls` | `tls` | TLS 1.3 implementation (PESQ-017) |
| `tokio-rustls` | `tls` | Async TLS for tokio |
| `rustls-pemfile` | `tls` | PEM file parsing |
| `prometheus-client` | `metrics` | Prometheus metrics (PESQ-016) |
| `axum` | `metrics` | HTTP for /metrics, /health |
| `opentelemetry` | `otel` | OTel core API (PESQ-014) |
| `opentelemetry-sdk` | `otel` | OTel SDK |
| `opentelemetry-otlp` | `otel` | OTLP exporter |
| `tracing-opentelemetry` | `otel` | tracing → OTel bridge |

### 2.3 Dev/Test Dependencies

| Crate | Purpose |
|-------|---------|
| `proptest` | Property-based testing (PESQ-022) |
| `criterion` | Benchmarks (SPEC-09) |
| `tokio-test` | Async test utilities |
| `rcgen` | Certificate generation for TLS tests |

---

## 3. Component Architecture

### 3.1 Core Layer (no async, no I/O)

```
net::Net           — agents: Vec<Agent>, wires: Vec<Wire>
net::Agent         — id: AgentId, agent_type: AgentType, ports: [PortId; 3]
net::Wire          — id: WireId, port_a: PortId, port_b: PortId
net::Port          — id: PortId, owner: AgentId, slot: Slot (Principal/Left/Right)

reduction::reduce(net: &mut Net) -> ReduceResult
reduction::find_redexes(net: &Net) -> Vec<Redex>
reduction::apply_rule(net: &mut Net, redex: Redex) -> RuleApplied

partition::split(net: &Net, k: usize) -> Vec<Partition>
partition::Partition — id, agents, wires, border_ports, metadata

merge::merge(partitions: Vec<Partition>) -> MergeResult
merge::resolve_borders(net: &mut Net, border_map: &BorderMap)
```

**No traits needed at this layer.** Pure functions operating on concrete types.

### 3.2 Protocol Layer (async)

```
protocol::Message — enum { Register, RegisterAck, DispatchPartition, ReturnPartition, Heartbeat, ... }
protocol::Transport — trait { async fn send(msg); async fn recv() -> msg; }
protocol::TcpTransport — impl Transport for TCP (production)
protocol::ChannelTransport — impl Transport for mpsc (testing)
protocol::frame — length-prefix + CRC32 encoding/decoding
```

### 3.3 Coordinator (async, FSM)

```
coordinator::Coordinator — state: CoordinatorState, workers: HashMap<WorkerId, WorkerInfo>
coordinator::CoordinatorState — enum { Init, WaitingForWorkers, Partitioning, Dispatching, WaitingForResults, Merging, CheckTermination, Done }
coordinator::transition(state, event) -> (state, Vec<Action>)
coordinator::run(config, transport) -> Result<Net>
```

FSM follows stimulus-response pattern (PESQ-013 L2):
- Events: WorkerRegistered, PartitionReturned, HeartbeatTimeout, ...
- Actions: SendMessage, StartTimer, EmitMetric, ...

### 3.4 Worker (async)

```
worker::Worker — state: WorkerState, transport: Box<dyn Transport>
worker::WorkerState — enum { Init, Idle, Reducing, Returning, Done }
worker::run(config, coordinator_addr) -> Result<()>
```

### 3.5 Observability (feature-gated)

```
observability::init_tracing(config) — setup subscriber layers
observability::MetricsRegistry — prometheus-client registry
observability::metrics_server(registry, port) — axum HTTP server
```

### 3.6 Security (feature-gated)

```
security::Token — generate(), validate()
security::tls_server_config(cert_path, key_path) -> ServerConfig
security::tls_client_config(ca_path) -> ClientConfig
```

---

## 4. Data Flow

```
Input (.bin / .txt / .json)
    │
    ▼
Parse → Net (in-memory)
    │
    ▼
Coordinator::Init
    │
    ▼
[Workers register]
    │
    ▼
┌── Round r ──────────────────────────────────────┐
│                                                  │
│   split(net, k) → [P₁, P₂, ..., Pₖ]           │
│       │                                          │
│       ▼                                          │
│   dispatch(Pᵢ → Workerᵢ) for all i              │
│       │                                          │
│       ▼                                          │
│   Workers: reduce(Pᵢ) → Pᵢ'                     │
│       │                                          │
│       ▼                                          │
│   Coordinator: collect all Pᵢ'                   │
│       │                                          │
│       ▼                                          │
│   merge([P₁', P₂', ..., Pₖ']) → net'            │
│       │                                          │
│       ▼                                          │
│   Check: border_redexes(net') == 0?              │
│       │                                          │
│     No → net = net', goto Round r+1              │
│     Yes → Done                                   │
└──────────────────────────────────────────────────┘
    │
    ▼
Output (reduced net + metrics)
```

---

## 5. Coordinator FSM (Formal)

States: {Init, WaitingForWorkers, Partitioning, Dispatching, WaitingForResults, Merging, CheckTermination, Done, Error}

Transitions:

| From | Event | To | Actions |
|------|-------|----|---------|
| Init | ConfigLoaded | WaitingForWorkers | Bind TCP, log |
| WaitingForWorkers | WorkerRegistered(id) | WaitingForWorkers (if < min) | Send RegisterAck |
| WaitingForWorkers | WorkerRegistered(id) | Partitioning (if >= min) | Send RegisterAck, log |
| Partitioning | SplitComplete(partitions) | Dispatching | — |
| Dispatching | AllDispatched | WaitingForResults | Start round timer |
| WaitingForResults | PartitionReturned(id, P) | WaitingForResults (if not all) | Store partition |
| WaitingForResults | PartitionReturned(id, P) | Merging (if all received) | — |
| WaitingForResults | HeartbeatTimeout(id) | WaitingForResults | Mark failed, re-dispatch |
| Merging | MergeComplete(net, borders) | CheckTermination | — |
| CheckTermination | borders == 0 | Done | Write output |
| CheckTermination | borders > 0 | Partitioning | net = merged_net |
| Any | FatalError(e) | Error | Log, shutdown |

---

## 6. Worker FSM (Formal)

States: {Init, Idle, Reducing, Returning, Error, Done}

Transitions:

| From | Event | To | Actions |
|------|-------|----|---------|
| Init | Connected | Idle | Send Register |
| Idle | ReceivePartition(P) | Reducing | — |
| Reducing | ReductionComplete(P') | Returning | — |
| Returning | SendComplete | Idle | — |
| Idle | Shutdown | Done | Close connection |
| Reducing | ReductionError(e) | Error | Send Error to coordinator |
| Error | — | Idle | (coordinator re-dispatches) |
| Any | ConnectionLost | Init | Reconnect |

---

## 7. CLI Design

```
relativist coordinator --bind 0.0.0.0:9000 --workers 4 --input net.bin [--tls-cert cert.pem --tls-key key.pem]
relativist worker --coordinator 10.0.0.1:9000 --token <TOKEN> [--tls-ca ca.pem]
relativist reduce --input net.bin --output result.bin   (local, no grid)
relativist inspect net.bin                               (print net summary)
relativist generate <example> --size N --output net.bin  (generate test nets)
```

---

## 8. Consolidated Lessons (ADOPT only)

All lessons marked ADOPT across PESQ-001 to PESQ-022, for quick reference:

| # | Lesson | Source | Informs |
|---|--------|--------|---------|
| 1 | serde + bincode for serialization | PESQ-008 L3, PESQ-009 L2 | SPEC-06, SPEC-13 |
| 2 | tokio for async runtime | PESQ-009 L3 | SPEC-13 |
| 3 | thiserror for errors | PESQ-009 L4 | SPEC-13 |
| 4 | Feature flags for optional components | PESQ-009 L5 | SPEC-13 |
| 5 | Build on primitives, not frameworks | PESQ-009 L1 | SPEC-13 |
| 6 | No BSP crate exists — build custom | PESQ-009 L6 | SPEC-05, SPEC-13 |
| 7 | In-memory grid mode for testing | PESQ-009 L7, PESQ-020 L1 | SPEC-08, SPEC-13 |
| 8 | Heartbeat is universal | PESQ-010 L2 | SPEC-06 |
| 9 | BSP barrier correct for IC reduction | PESQ-010 L4, PESQ-012 L1 | SPEC-05, SPEC-13 |
| 10 | Star topology sufficient for v1 | PESQ-010 L6 | SPEC-13 |
| 11 | No worker-to-worker communication | PESQ-010 L7 | SPEC-06, SPEC-13 |
| 12 | Relativist IS BSP | PESQ-012 L1 | SPEC-05, SPEC-13 |
| 13 | BSP cost model for benchmarks | PESQ-012 L2 | SPEC-09 |
| 14 | Enum-based FSM for all state | PESQ-013 L1 | SPEC-13 |
| 15 | Stimulus-response for coordinator | PESQ-013 L2 | SPEC-08, SPEC-13 |
| 16 | Log every state transition | PESQ-013 L3 | SPEC-11, SPEC-13 |
| 17 | FSM enables deterministic testing | PESQ-013 L5 | SPEC-08 |
| 18 | OTel as optional feature flag | PESQ-014 L1 | SPEC-11, SPEC-13 |
| 19 | tracing as single instrumentation API | PESQ-014 L2, PESQ-015 L1 | SPEC-11, SPEC-13 |
| 20 | JSON format for production logging | PESQ-015 L2 | SPEC-11 |
| 21 | #[instrument] for key functions | PESQ-015 L4 | SPEC-11 |
| 22 | prometheus-client crate | PESQ-016 L1 | SPEC-11, SPEC-13 |
| 23 | Separate metrics port | PESQ-016 L2 | SPEC-11 |
| 24 | axum for HTTP endpoints | PESQ-016 L3 | SPEC-11, SPEC-13 |
| 25 | Worker metrics via protocol | PESQ-016 L4 | SPEC-06, SPEC-11 |
| 26 | rustls + tokio-rustls | PESQ-017 L1 | SPEC-10, SPEC-13 |
| 27 | TLS as feature flag | PESQ-017 L2 | SPEC-10, SPEC-13 |
| 28 | Server TLS only for v1 | PESQ-017 L3 | SPEC-10 |
| 29 | TLS wraps existing protocol | PESQ-017 L5 | SPEC-06, SPEC-10 |
| 30 | Shared token authentication | PESQ-018 L1 | SPEC-06, SPEC-10 |
| 31 | Token via env var | PESQ-018 L2 | SPEC-07, SPEC-10 |
| 32 | Three security tiers | PESQ-018 L5 | SPEC-10 |
| 33 | Default bind to localhost | PESQ-019 L1 | SPEC-07, SPEC-10 |
| 34 | Message size limits | PESQ-019 L2 | SPEC-06, SPEC-10 |
| 35 | Trait-abstract network I/O | PESQ-020 L2, PESQ-021 L3 | SPEC-06, SPEC-08, SPEC-13 |
| 36 | Seeded RNG for reproducibility | PESQ-020 L4 | SPEC-08 |
| 37 | proptest for invariant tests | PESQ-022 L1 | SPEC-08 |
| 38 | Custom net generators | PESQ-022 L2 | SPEC-08 |

---

## 9. What's NOT in v1

Explicitly excluded (documented in roadmap, not implementation scope):

| Feature | Why Excluded | PESQ Reference |
|---------|-------------|---------------|
| Multi-crate workspace | Over-engineering for one developer | PESQ-023 D1 |
| mTLS | PKI complexity | PESQ-017 L3 |
| Full DST (Turmoil/MadSim) | Disproportionate effort | PESQ-021 L1 |
| Work stealing | Incompatible with BSP | PESQ-011 L1 |
| Byzantine fault tolerance | Redundant computation not justified | PESQ-019 L5 |
| Coordinator HA | Single coordinator acceptable | PESQ-010 §3.3 |
| Actor model | Wrong abstraction for BSP | PESQ-008 L1 |
| Consensus (Raft) | Single coordinator, no election | PESQ-009 §2.1 |
| Token rotation | Per-session token sufficient | PESQ-018 L4 |
| rayon intra-worker parallelism | Sequential reduction simpler | PESQ-011 L2 |

---

## 10. Next Steps

With all research complete, the path forward:

1. **SPEC-13 (System Architecture):** Write using this document as primary input. Resolves all D1-D8 decisions.
2. **SPEC-10 (Security):** Write using PESQ-017/018/019 + D4.
3. **SPEC-11 (Observability):** Write using PESQ-014/015/016 + D5.
4. **SPEC-12 (User I/O):** Write using CLI design (§7) + existing SPEC-07.
5. **Rust scaffolding:** Cargo.toml, src/ structure matching §3, tests/ structure.
6. **Implementation:** Follow phase order from `codigo/relativist/docs/progress.md`.
