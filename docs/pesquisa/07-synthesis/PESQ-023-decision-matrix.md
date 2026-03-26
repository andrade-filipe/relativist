---
pesq_id: PESQ-023
title: "Decision Matrix: Open Architecture Decisions"
category: Synthesis
date: 2026-03-26
status: Complete
cross_references:
  specs: [SPEC-05, SPEC-06, SPEC-08, SPEC-09, SPEC-10, SPEC-11, SPEC-12, SPEC-13]
  pesqs: [ALL]
  discs: [DISC-005, DISC-006, DISC-007, DISC-008]
---

# PESQ-023: Decision Matrix — Open Architecture Decisions

**Category:** Synthesis
**Status:** Complete
**Purpose:** Consolidate all open decisions from progress.md, evaluate options using evidence from PESQ-001 through PESQ-022, and provide recommended resolutions.

---

## 1. Decision D1: Workspace Structure

**Question:** Single crate or multi-crate workspace?

| Option | Pros | Cons | Evidence |
|--------|------|------|----------|
| **A: Single crate** | Simpler build, single binary, easier refactoring | All code in one compilation unit, longer builds | Paladin: single lib crate (PESQ-007). Most surveyed systems are monolithic. |
| **B: Multi-crate workspace** | Parallel compilation, clearer boundaries, reusable core | More boilerplate, cross-crate changes harder | Hydro: 20+ crates (PESQ-006). Constellation: 5 crates (PESQ-008). |
| **C: Single crate, feature-gated modules** | Single binary but optional components | Feature flag complexity | Common in Rust ecosystem for optional dependencies. |

### Analysis

Relativist has ~10 modules but a single binary. The core (net, reduction, partition) has no I/O; the infrastructure (protocol, coordinator, worker) has I/O. This suggests Option B with **two crates**:

- `relativist-core`: net, reduction, partition, merge (pure, no async, no I/O)
- `relativist`: coordinator, worker, protocol, CLI, observability, security (async, I/O)

But for a research project with one developer, the overhead of multi-crate is real. Paladin (PESQ-007) manages well as a single crate with clear module boundaries.

### **Recommendation: Option C — Single crate with modules + feature flags**

Rationale:
- Single `relativist` crate with `src/` organized by module
- Feature flags for `tls`, `metrics`, `otel`
- If the project grows, extract `relativist-core` later (module boundaries make this easy)
- Evidence: Paladin (PESQ-007 L9) recommends module separation within single crate

---

## 2. Decision D2: Error Handling

**Question:** thiserror vs anyhow vs custom error types?

| Option | Pros | Cons | Evidence |
|--------|------|------|----------|
| **thiserror** | Typed errors, `#[derive(Error)]`, pattern matching | More boilerplate than anyhow | Paladin uses structured errors (PESQ-007). PESQ-009 L4 recommends thiserror. |
| **anyhow** | Ergonomic, `?` everywhere, context via `.context()` | Type information lost, can't pattern match | Only for applications, not libraries |
| **Custom** | Full control | Most boilerplate | Over-engineering for this scale |

### Analysis

Relativist needs error classification:
- **Transient errors** (network timeout, temporary failure) → retry or re-dispatch
- **Fatal errors** (invalid net structure, protocol violation) → abort
- **Operational errors** (worker crashed, heartbeat missed) → handle gracefully

This requires typed errors, not `anyhow::Error`. Each module defines its own error enum via thiserror; the top-level error unifies them.

### **Recommendation: thiserror**

```rust
// Per-module errors
#[derive(Debug, thiserror::Error)]
pub enum ReductionError {
    #[error("invalid redex: agents {0} and {1} are not connected")]
    InvalidRedex(AgentId, AgentId),
    #[error("net invariant violated: {0}")]
    InvariantViolation(String),
}

#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("connection lost: {0}")]
    ConnectionLost(#[source] std::io::Error),
    #[error("invalid message: {0}")]
    InvalidMessage(String),
    #[error("authentication failed")]
    AuthFailed,
}

// Top-level
#[derive(Debug, thiserror::Error)]
pub enum RelativistError {
    #[error(transparent)]
    Reduction(#[from] ReductionError),
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
    // ...
}
```

---

## 3. Decision D3: Feature Flags

**Question:** Which components are feature-flagged?

| Feature | Default | Crates Gated |
|---------|---------|-------------|
| `tls` | Off | rustls, tokio-rustls, rustls-pemfile |
| `metrics` | Off | prometheus-client, axum (for /metrics endpoint) |
| `otel` | Off | opentelemetry, opentelemetry-sdk, opentelemetry-otlp, tracing-opentelemetry |

### **Recommendation:**

```toml
[features]
default = []
tls = ["dep:rustls", "dep:tokio-rustls", "dep:rustls-pemfile"]
metrics = ["dep:prometheus-client", "dep:axum"]
otel = ["dep:opentelemetry", "dep:opentelemetry-sdk", "dep:opentelemetry-otlp", "dep:tracing-opentelemetry"]
full = ["tls", "metrics", "otel"]
```

Evidence: PESQ-009 L5, PESQ-014 L1, PESQ-017 L2.

---

## 4. Decision D4: Security Model

**Question:** What authentication and encryption to use?

### **Recommendation: Three-tier model** (PESQ-018 L5)

| Tier | Auth | Encryption | Integrity | Use Case |
|------|------|-----------|-----------|----------|
| Development | None | None | CRC32 | Local testing |
| Private network | Token | None | CRC32 | LAN/VPN |
| Production | Token | TLS 1.3 | TLS | Cloud/untrusted |

- Token: 256-bit random, base64, generated by coordinator, distributed out-of-band
- TLS: rustls + tokio-rustls, server cert, self-signed OK for dev
- Default bind: 127.0.0.1 (PESQ-019 L1)
- Max message size: 256 MB (PESQ-019 L2)

---

## 5. Decision D5: Observability Architecture

**Question:** How to structure logging, metrics, and tracing?

### **Recommendation:**

| Component | Solution | Feature |
|-----------|----------|---------|
| Logging | `tracing` + `tracing-subscriber` (JSON) | Always on |
| Metrics | `prometheus-client` + `axum` on separate port | `metrics` |
| Distributed tracing | `tracing-opentelemetry` → OTLP/HTTP | `otel` |
| Health | `/health` + `/ready` via axum | `metrics` |

- Single instrumentation API: `tracing` macros only
- Worker metrics reported via protocol (no HTTP per worker)
- Coordinator exposes all metrics via `/metrics`
- Per-component log levels (PESQ-015 §3.1)
- Every state transition logged (PESQ-013 L3)

Evidence: PESQ-014 L2, PESQ-015 L1, PESQ-016 L1/L2/L3.

---

## 6. Decision D6: Testing Strategy

**Question:** What testing approach for the distributed system?

### **Recommendation:**

| Layer | Approach | Tool |
|-------|----------|------|
| Unit (pure logic) | Example-based + property-based | `#[test]` + `proptest` |
| Integration (grid cycle) | In-memory grid mode | `ChannelTransport` + `#[tokio::test]` |
| Protocol | Serialization roundtrip | `proptest` |
| Full system | Docker Compose | `docker compose up` + CLI test |
| DST | Not in v1 | Turmoil (v2 roadmap) |

- `trait Transport` abstracts TCP vs channel vs simulation (PESQ-020 L2, PESQ-021 L3)
- Property tests for P1, P2, P3, T1-T7, D1-D6 (PESQ-022 L1)
- In-memory grid for integration tests (PESQ-020 L1)
- Seeded RNG for reproducibility (PESQ-020 L4)

---

## 7. Decision D7: System Architecture (Module Boundaries)

**Question:** How to organize modules and their public APIs?

### **Recommendation:**

```
src/
├── lib.rs              # Re-exports, top-level error type
├── main.rs             # CLI (clap), entry point
├── net/                # SPEC-02: Net, Agent, Wire, Port
│   ├── mod.rs
│   ├── agent.rs
│   ├── wire.rs
│   └── port.rs
├── reduction/          # SPEC-03: reduce(), redex detection
│   └── mod.rs
├── partition/          # SPEC-04: split()
│   └── mod.rs
├── merge/              # SPEC-05: merge(), border resolution
│   └── mod.rs
├── protocol/           # SPEC-06: Message, Transport trait, framing
│   ├── mod.rs
│   ├── message.rs
│   ├── transport.rs    # trait Transport
│   ├── tcp.rs          # TcpTransport
│   └── channel.rs      # ChannelTransport (for testing)
├── coordinator/        # SPEC-13: Coordinator FSM, round management
│   └── mod.rs
├── worker/             # SPEC-13: Worker FSM, reduction loop
│   └── mod.rs
├── config/             # SPEC-07: CLI config, environment
│   └── mod.rs
├── observability/      # SPEC-11: tracing setup, metrics registry
│   └── mod.rs
└── security/           # SPEC-10: TLS, token auth
    └── mod.rs
```

Module dependency rules:
- `net`, `reduction`, `partition`, `merge` → **no async, no I/O** (pure core)
- `protocol`, `coordinator`, `worker` → **async (tokio), I/O**
- `observability`, `security` → **infrastructure, feature-gated**
- Core depends on nothing. Infrastructure depends on core.

---

## 8. Decision D8: Programming Model Classification

**Question:** How to classify Relativist's computation model?

### **Recommendation: BSP (Bulk Synchronous Parallel)**

Evidence: PESQ-012 provides exhaustive comparison. The mapping is exact:
- Superstep = Grid round
- Local computation = reduce(partition)
- Communication = ReturnPartition
- Barrier = Coordinator waits for all workers

NOT MapReduce (lacks iteration, structural merge). NOT Dataflow (single operation, not graph).

---

## 9. Summary: All Decisions

| # | Decision | Resolution | Key Evidence |
|---|----------|-----------|-------------|
| D1 | Workspace | Single crate + feature flags | PESQ-007, PESQ-009 |
| D2 | Error handling | thiserror | PESQ-007, PESQ-009 |
| D3 | Feature flags | tls, metrics, otel | PESQ-014, PESQ-017 |
| D4 | Security | 3-tier (none/token/token+TLS) | PESQ-017, PESQ-018, PESQ-019 |
| D5 | Observability | tracing + prometheus-client + OTel | PESQ-014, PESQ-015, PESQ-016 |
| D6 | Testing | proptest + in-memory grid + Transport trait | PESQ-020, PESQ-021, PESQ-022 |
| D7 | Module structure | 10 modules, core/infra split | PESQ-010, PESQ-013 |
| D8 | Programming model | BSP | PESQ-012 |
