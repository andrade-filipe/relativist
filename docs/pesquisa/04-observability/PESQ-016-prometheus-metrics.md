---
pesq_id: PESQ-016
title: "Prometheus Metrics Exposition in Rust"
category: Observability & Tracing
date: 2026-03-26
status: Complete
cross_references:
  specs: [SPEC-11, SPEC-13]
  pesqs: [PESQ-003, PESQ-014, PESQ-015]
  discs: []
---

# PESQ-016: Prometheus Metrics Exposition in Rust

**Category:** Observability & Tracing
**Status:** Complete

---

## 1. Subject Overview

Prometheus is the standard for metrics collection in cloud-native systems. It uses a **pull model**: Prometheus server scrapes `/metrics` endpoints on target services at configured intervals.

### 1.1 Rust Crate Options

| Crate | Maintainer | Style | OpenMetrics | Status |
|-------|-----------|-------|-------------|--------|
| `prometheus-client` | Prometheus project | Builder pattern | Yes (native) | **Recommended** |
| `prometheus` (rust-prometheus) | TiKV | Go-port style | Partial | Legacy, still popular |
| `metrics` + `metrics-exporter-prometheus` | metrics-rs | Facade pattern | Yes | Alternative ecosystem |

### 1.2 Recommendation: `prometheus-client`

The official Prometheus client for Rust. OpenMetrics-compliant, well-maintained, minimal dependencies.

```rust
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::histogram::Histogram;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::registry::Registry;

let mut registry = Registry::default();

// Counter: monotonically increasing
let interactions_total = Counter::default();
registry.register("relativist_interactions_total", "Total interaction rules applied", interactions_total.clone());

// Histogram: distribution of values
let reduce_duration = Histogram::new([0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0].into_iter());
registry.register("relativist_reduce_duration_seconds", "Reduction step latency", reduce_duration.clone());

// Gauge: current value
let active_workers = Gauge::<i64, AtomicI64>::default();
registry.register("relativist_active_workers", "Currently connected workers", active_workers.clone());
```

---

## 2. Metrics Design for Relativist

### 2.1 Coordinator Metrics

| Metric Name | Type | Labels | Description |
|------------|------|--------|-------------|
| `relativist_rounds_total` | Counter | — | Total BSP rounds completed |
| `relativist_round_duration_seconds` | Histogram | — | Wall-clock time per round |
| `relativist_active_workers` | Gauge | — | Currently connected workers |
| `relativist_partitions_dispatched_total` | Counter | — | Total partitions dispatched |
| `relativist_border_redexes` | Gauge | — | Border redexes after last merge |
| `relativist_merge_duration_seconds` | Histogram | — | Merge phase latency |
| `relativist_split_duration_seconds` | Histogram | — | Split phase latency |
| `relativist_dispatch_bytes_total` | Counter | — | Total bytes dispatched |
| `relativist_return_bytes_total` | Counter | — | Total bytes received back |

### 2.2 Worker Metrics (reported to coordinator)

| Metric Name | Type | Labels | Description |
|------------|------|--------|-------------|
| `relativist_reduce_duration_seconds` | Histogram | `worker_id` | Reduction time per round |
| `relativist_interactions_total` | Counter | `worker_id`, `rule` | Interactions by rule type |
| `relativist_redexes_reduced` | Counter | `worker_id` | Total redexes reduced |

### 2.3 Protocol Metrics

| Metric Name | Type | Labels | Description |
|------------|------|--------|-------------|
| `relativist_messages_sent_total` | Counter | `type` | Messages by type |
| `relativist_messages_received_total` | Counter | `type` | Messages received by type |
| `relativist_message_size_bytes` | Histogram | `type` | Message size distribution |
| `relativist_heartbeat_latency_seconds` | Histogram | — | Heartbeat round-trip time |

### 2.4 Label Strategy

Prometheus best practices:
- Keep label cardinality low (< 100 distinct values)
- `worker_id` is acceptable (bounded by cluster size)
- `rule` has 6 values (CON-CON, CON-DUP, CON-ERA, DUP-DUP, DUP-ERA, ERA-ERA) — fine
- Do NOT use `partition_id` or `round` as labels (unbounded cardinality)

---

## 3. HTTP Endpoint for Metrics

### 3.1 Minimal HTTP Server

The coordinator needs a `/metrics` endpoint for Prometheus scraping. Options:

| Option | Crate | Complexity |
|--------|-------|-----------|
| Embed in coordinator's tokio runtime | `hyper` or `axum` | Medium |
| Separate thread with `tiny_http` | `tiny_http` | Low |
| Use `metrics-exporter-prometheus` | `metrics-exporter-prometheus` | Low (auto-creates server) |

**Recommendation:** Use `axum` (already depends on tokio) with a minimal router:

```rust
async fn metrics_handler(registry: State<Registry>) -> String {
    let mut buf = String::new();
    encode(&mut buf, &registry).unwrap();
    buf
}

async fn health_handler() -> &'static str { "ok" }
async fn ready_handler(state: State<AppState>) -> StatusCode {
    if state.is_ready() { StatusCode::OK } else { StatusCode::SERVICE_UNAVAILABLE }
}

let app = Router::new()
    .route("/metrics", get(metrics_handler))
    .route("/health", get(health_handler))
    .route("/ready", get(ready_handler));
```

### 3.2 Port Configuration

- Default: coordinator main port (e.g., 9000) for grid protocol
- Metrics/health: separate port (e.g., 9090) — avoids mixing binary protocol with HTTP
- Configurable via `--metrics-port` CLI flag

---

## 4. Lessons for Relativist

### L1: prometheus-client Crate [ADOPT]
Use the official `prometheus-client` crate for metrics. It's OpenMetrics-compliant, minimal, and well-maintained. Feature-flag it under `metrics`.
→ Informs: SPEC-11, SPEC-13

### L2: Separate Metrics Port [ADOPT]
Run the HTTP metrics/health server on a separate port from the binary grid protocol. This is standard practice and avoids protocol confusion.
→ Informs: SPEC-11, SPEC-13

### L3: axum for HTTP Endpoints [ADOPT]
Use `axum` (built on tokio + hyper) for `/metrics`, `/health`, `/ready`. It shares the tokio runtime, is lightweight, and is the most popular Rust HTTP framework.
→ Informs: SPEC-11, SPEC-13

### L4: Worker Metrics via Protocol [ADAPT]
Workers report metrics to the coordinator as part of `ReturnPartition` messages (not via their own HTTP endpoints). The coordinator aggregates and exposes all metrics. This avoids requiring workers to run HTTP servers.
→ Informs: SPEC-06, SPEC-11

### L5: Histogram Buckets Tuned to IC Reduction [ADOPT]
Use custom histogram buckets matching expected latencies: `[0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0, 30.0]` seconds. Default Prometheus buckets are designed for web requests, not computational workloads.
→ Informs: SPEC-11

---

## 5. Sources

| Source | URL | Accessed |
|--------|-----|----------|
| prometheus-client docs.rs | https://docs.rs/prometheus-client | 2026-03-26 |
| prometheus-client GitHub | https://github.com/prometheus/client_rust | 2026-03-26 |
| rust-prometheus (TiKV) GitHub | https://github.com/tikv/rust-prometheus | 2026-03-26 |
| Prometheus metrics in Rust (LogRocket) | https://blog.logrocket.com/using-prometheus-metrics-in-a-rust-web-service/ | 2026-03-26 |
| metrics-exporter-prometheus docs | https://docs.rs/metrics-exporter-prometheus | 2026-03-26 |
