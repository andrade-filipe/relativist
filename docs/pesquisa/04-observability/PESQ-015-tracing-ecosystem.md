---
pesq_id: PESQ-015
title: "tracing Crate Ecosystem"
category: Observability & Tracing
date: 2026-03-26
status: Complete
cross_references:
  specs: [SPEC-11, SPEC-13]
  pesqs: [PESQ-014, PESQ-016]
  discs: []
---

# PESQ-015: tracing Crate Ecosystem

**Category:** Observability & Tracing
**Status:** Complete

---

## 1. Subject Overview

`tracing` is the de facto standard for structured, event-based diagnostics in Rust. Maintained by the tokio project, it provides:
- **Spans:** Structured scopes of execution with typed fields
- **Events:** Point-in-time occurrences within spans
- **Subscribers:** Pluggable backends that process spans and events
- **Layers:** Composable subscriber components

### 1.1 Core Crates

| Crate | Purpose | Downloads/month |
|-------|---------|----------------|
| `tracing` | Core API (macros, span, event) | ~20M |
| `tracing-subscriber` | Subscriber implementations | ~15M |
| `tracing-core` | Minimal API (for library authors) | ~20M |
| `tracing-appender` | Non-blocking file output | ~5M |
| `tracing-error` | SpanTrace for error context | ~3M |

### 1.2 Subscriber Architecture

```
tracing::info!("reducing partition", partition_id = %id, redexes = count)
    │
    ▼
Registry (stores span data)
    │
    ├── Layer 1: fmt::Layer (console output)
    │     └── Format: Full / Compact / Pretty / JSON
    │
    ├── Layer 2: OpenTelemetryLayer (distributed traces) [optional]
    │
    └── Layer 3: Custom MetricsLayer (extract metrics from spans) [optional]
```

---

## 2. Key Mechanisms

### 2.1 Filtering

**EnvFilter** — Dynamic filtering via `RUST_LOG` environment variable:
```
RUST_LOG=relativist=debug,relativist::reduction=trace,tokio=warn
```

**Targets filter** — Static, per-target level:
```rust
let filter = Targets::new()
    .with_target("relativist::coordinator", Level::DEBUG)
    .with_target("relativist::worker", Level::INFO)
    .with_target("relativist::reduction", Level::TRACE);
```

**Per-layer filtering** — Different layers can have different filters:
```rust
let subscriber = Registry::default()
    .with(fmt_layer.with_filter(EnvFilter::from_default_env()))
    .with(otel_layer.with_filter(LevelFilter::INFO));
```

### 2.2 Structured Fields

```rust
#[tracing::instrument(skip(partition), fields(
    partition_id = %partition.id,
    agent_count = partition.agents.len(),
    redex_count = partition.redex_queue.len(),
))]
fn reduce(partition: &mut Partition) -> ReduceResult {
    // spans automatically track entry/exit + duration
    tracing::debug!("starting reduction");
    // ...
    tracing::info!(reduced = steps, remaining = queue.len(), "round complete");
}
```

### 2.3 Async Support

`tracing` integrates with tokio's async runtime:
- Spans follow `.await` points correctly
- `#[tracing::instrument]` works on async functions
- `tracing-futures` provides `.instrument(span)` combinator

### 2.4 Output Formats

| Format | Use Case | Relativist |
|--------|----------|------------|
| `Full` | Development (human-readable) | Default for dev |
| `Compact` | Development (less verbose) | Alternative |
| `Pretty` | Interactive debugging | Not needed |
| `JSON` | Production (machine-parseable) | **Default for production** |

JSON output example:
```json
{"timestamp":"2026-03-26T10:30:00Z","level":"INFO","target":"relativist::coordinator",
 "fields":{"message":"round complete","round":5,"duration_ms":142,"border_redexes":3},
 "spans":[{"name":"grid_cycle","round":5}]}
```

---

## 3. Relevance to Relativist

### 3.1 Log Level Strategy per Component

| Component | Default Level | Rationale |
|-----------|--------------|-----------|
| `relativist::coordinator` | INFO | Round lifecycle, worker management |
| `relativist::worker` | INFO | Partition receipt/return |
| `relativist::reduction` | WARN | Only errors; TRACE for debugging |
| `relativist::protocol` | WARN | Only errors; DEBUG for message inspection |
| `relativist::partition` | INFO | Split/merge stats |
| `relativist::net` | WARN | Only structural errors |

### 3.2 Key Spans for Relativist

| Span | Fields | Level |
|------|--------|-------|
| `grid_cycle` | `round`, `workers`, `partitions` | INFO |
| `split` | `input_agents`, `k`, `strategy` | INFO |
| `dispatch` | `worker_id`, `partition_id`, `size_bytes` | DEBUG |
| `reduce` | `partition_id`, `initial_redexes` | INFO |
| `merge` | `partitions`, `border_redexes` | INFO |
| `heartbeat` | `worker_id`, `latency_ms` | TRACE |

### 3.3 Recommended Setup

```rust
use tracing_subscriber::{fmt, prelude::*, EnvFilter, Registry};

pub fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("relativist=info,warn"));

    let fmt_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_file(false);

    // JSON in production, human-readable in dev
    let subscriber = Registry::default()
        .with(fmt_layer.with_filter(env_filter));

    // Optional: add OTel layer if feature enabled
    #[cfg(feature = "otel")]
    let subscriber = subscriber.with(otel_layer());

    tracing::subscriber::set_global_default(subscriber)
        .expect("failed to set tracing subscriber");
}
```

---

## 4. Lessons for Relativist

### L1: tracing is the Foundation [ADOPT]
All logging, tracing, and diagnostics go through `tracing`. No `println!`, no `log` crate, no custom logging. This is the ecosystem standard and enables all other observability features.
→ Informs: SPEC-11, SPEC-13

### L2: JSON Format for Production [ADOPT]
Use `tracing_subscriber::fmt::format::Json` for production deployments. This enables log aggregation, searching, and analysis with standard tools (jq, Loki, ELK).
→ Informs: SPEC-11

### L3: Per-Component Log Levels [ADOPT]
Define default log levels per Rust module target. Allow override via `RUST_LOG` environment variable. This gives operators fine-grained control.
→ Informs: SPEC-11, SPEC-07

### L4: #[instrument] for Key Functions [ADOPT]
Use `#[tracing::instrument]` on `split()`, `reduce()`, `merge()`, `dispatch()`, and protocol handlers. This automatically captures function arguments, duration, and nesting.
→ Informs: SPEC-11, SPEC-13

### L5: tracing-appender for File Output [ADAPT]
For long-running deployments, use `tracing-appender` with non-blocking writer and rolling file output. Not for v1 CLI usage, but for production Docker deployments.
→ Informs: SPEC-07, SPEC-11

---

## 5. Sources

| Source | URL | Accessed |
|--------|-----|----------|
| tracing crate docs | https://docs.rs/tracing | 2026-03-26 |
| tracing-subscriber docs | https://docs.rs/tracing-subscriber | 2026-03-26 |
| tracing GitHub | https://github.com/tokio-rs/tracing | 2026-03-26 |
| Getting Started with Tracing (Shuttle) | https://www.shuttle.dev/blog/2024/01/09/getting-started-tracing-rust | 2026-03-26 |
| tracing crates.io | https://crates.io/crates/tracing | 2026-03-26 |
