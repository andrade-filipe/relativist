# SPEC-11: Observability

**Status:** Draft v1
**Depends on:** SPEC-00 (Glossary), SPEC-05 (Merge and Grid Cycle), SPEC-06 (Wire Protocol), SPEC-07 (Deployment), SPEC-13 (System Architecture)
**Gray zones resolved:** ---
**Research consumed:** PESQ-003 (Ray architecture, observability patterns), PESQ-014 (OpenTelemetry for Rust), PESQ-015 (tracing crate ecosystem), PESQ-016 (Prometheus metrics exposition in Rust), PESQ-023 (Decision Matrix, Decision D5: Observability Architecture)
**Discussions consumed:** DISC-006 v2 (overhead anatomy, break-even analysis -- informs metric selection for overhead breakdown)

---

## 1. Purpose

This spec defines the observability architecture for Relativist v1: structured logging via the `tracing` crate as the sole instrumentation API, Prometheus-compatible metrics exposition via `prometheus-client` and `axum` (feature-gated under `metrics`), HTTP health and readiness endpoints, and optional OpenTelemetry distributed tracing (feature-gated under `otel`). Observability enables operators to monitor grid cycle progress, diagnose performance problems, and collect the quantitative data required by the benchmark suite (SPEC-09). The design follows the layered subscriber architecture validated by the Rust ecosystem (PESQ-014 L2, PESQ-015 L1): application code uses `tracing` macros exclusively, and all backends (console output, JSON logs, Prometheus, OTel) are configured as subscriber layers at startup.

## 2. Definitions

Terms defined in SPEC-00 (Glossary) are used without redefinition. Terms introduced or refined in this spec:

| Term | Definition |
|------|-----------|
| **Structured Logging** | Logging where each event carries typed key-value fields (not just a freeform string). Enables machine parsing, filtering, and aggregation. In Relativist, all logging uses `tracing` events with structured fields. |
| **Span** | A period of time during which a program is executing in a particular context. Spans have a name, typed fields, a start time, and an end time. Spans nest hierarchically. In Relativist, key operations (`grid_cycle`, `split`, `reduce`, `merge`, `dispatch`) are modeled as spans. |
| **Event** | A point-in-time occurrence within a span. Events have a level (TRACE, DEBUG, INFO, WARN, ERROR), a message, and structured fields. Created via `tracing::info!()`, `tracing::warn!()`, etc. |
| **Subscriber** | A `tracing` backend that receives span and event notifications. Relativist uses `tracing_subscriber::Registry` as the root subscriber, with composable layers. |
| **Layer** | A composable component attached to a `Registry` subscriber. Each layer processes span/event data independently. Relativist uses: `fmt::Layer` (always on), Prometheus metrics layer (feature `metrics`), OpenTelemetry layer (feature `otel`). |
| **Metric** | A numerical measurement exposed for external collection. Three types: **Counter** (monotonically increasing, e.g., total rounds), **Histogram** (distribution of values, e.g., round duration), **Gauge** (current value that can go up or down, e.g., active workers). |
| **Scrape** | The Prometheus pull model: the Prometheus server periodically sends HTTP GET requests to `/metrics` endpoints on target services to collect current metric values. |
| **Health Check (Liveness)** | An HTTP endpoint (`/health`) that returns 200 OK if the process is alive and responsive. Does not check application-level readiness. |
| **Readiness Check** | An HTTP endpoint (`/ready`) that returns 200 OK if the application is ready to perform its function (coordinator has entered `WaitingForWorkers` state or later), or 503 Service Unavailable otherwise. |
| **OTLP** | OpenTelemetry Protocol. A vendor-neutral protocol for exporting telemetry data (traces, metrics, logs) to a collector. Relativist uses OTLP over HTTP (not gRPC) for simplicity (PESQ-014 L4). |
| **Trace Context** | Metadata (trace_id + span_id) that links spans across process boundaries. Optionally carried in wire protocol messages when the `otel` feature is enabled (PESQ-014 L3). |

---

## 3. Requirements

### 3.1 Structured Logging (tracing)

**R1.** Relativist MUST use the `tracing` crate as the sole instrumentation API. No `println!`, `eprintln!`, `dbg!`, or `log` crate usage is permitted in any module. **(MUST)**

Source: PESQ-014 L2, PESQ-015 L1. Rationale: a single instrumentation API enables all backends (console, JSON, OTel, metrics) to be configured via subscriber layers without modifying application code.

**R2.** Relativist MUST use `tracing_subscriber::Registry` as the root subscriber with composable `Layer` instances. **(MUST)**

**R3.** The `fmt::Layer` MUST support two output formats, selected at startup via configuration. **(MUST)**

| Format | Selector | Use Case |
|--------|----------|----------|
| Human-readable (`Full`) | `--log-format text` or default when `$TERM` is a TTY | Development, interactive debugging |
| JSON | `--log-format json` or default when `$TERM` is not a TTY | Production, Docker, log aggregation |

Source: PESQ-015 L2, PESQ-015 Section 2.4.

**R4.** Log level filtering MUST be configurable via the `RUST_LOG` environment variable using `tracing_subscriber::EnvFilter`. If `RUST_LOG` is not set, the default filter MUST be applied. **(MUST)**

**R5.** The default per-component log levels MUST be as follows. **(MUST)**

| Target | Default Level | Rationale |
|--------|--------------|-----------|
| `relativist::coordinator` | INFO | Round lifecycle, worker management |
| `relativist::worker` | INFO | Partition receipt and return |
| `relativist::reduction` | WARN | Hot path; only errors by default. TRACE for debugging. |
| `relativist::protocol` | WARN | Hot path; only errors by default. DEBUG for message inspection. |
| `relativist::partition` | INFO | Split/merge statistics |
| `relativist::net` | WARN | Only structural errors |
| `relativist::observability` | INFO | Init confirmation |
| `relativist::security` | INFO | Auth events, TLS status |

Source: PESQ-015 Section 3.1. The default filter string is: `"relativist::coordinator=info,relativist::worker=info,relativist::reduction=warn,relativist::protocol=warn,relativist::partition=info,relativist::net=warn,relativist::observability=info,relativist::security=info,warn"`.

**R6.** Key functions MUST be annotated with `#[tracing::instrument]` to create spans with structured fields. At minimum, the following functions MUST be instrumented. **(MUST)**

| Function | Module | Span Fields |
|----------|--------|-------------|
| `split()` | `partition` | `input_agents`, `k`, `strategy` |
| `reduce_all()` | `reduction` | `partition_id`, `initial_redexes` |
| `merge()` | `merge` | `partition_count`, `border_redexes` |
| `dispatch()` | `coordinator` | `worker_id`, `partition_id`, `size_bytes` |
| `handle_message()` | `protocol` | `message_type`, `peer` |

Source: PESQ-015 L4.

**R7.** Every coordinator and worker FSM state transition (SPEC-13, R19-R25) MUST be logged at INFO level with the fields `from_state`, `to_state`, and `event`. **(MUST)**

Source: PESQ-013 L3. Example:
```
INFO relativist::coordinator: state transition from_state="WaitingForResults" to_state="Merging" event="AllPartitionsReturned" round=5
```

**R8.** All log events in the grid cycle SHOULD include structured fields for contextual correlation. Recommended fields by context. **(SHOULD)**

| Context | Fields |
|---------|--------|
| Grid round | `round` (u32) |
| Worker operations | `worker_id` (WorkerId) |
| Partition operations | `partition_id` (u32) |
| Timing | `duration_ms` (u64) |
| Reduction | `redexes` (usize), `interactions` (usize) |
| Protocol | `message_type` (str), `size_bytes` (usize) |

**R9.** The `fmt::Layer` MUST include the target (module path), thread ID, and timestamp in all output. File name and line number MAY be included but SHOULD be disabled by default (noise in production). **(MUST for target/thread/timestamp; SHOULD for file/line off by default)**

### 3.2 Metrics (Prometheus)

**R10.** All Prometheus metrics functionality MUST be feature-gated under the `metrics` Cargo feature. When the `metrics` feature is not enabled, zero metrics-related code is compiled and no HTTP server is started. **(MUST)**

Source: PESQ-023 D3, PESQ-016 L1.

**R11.** Relativist MUST use the `prometheus-client` crate for metric definitions and encoding. **(MUST)**

Source: PESQ-016 L1. Rationale: official Prometheus Rust client, OpenMetrics-compliant, minimal dependencies.

**R12.** The coordinator MUST maintain a `prometheus_client::registry::Registry` containing the following metrics. **(MUST)**

```rust
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::metrics::histogram::Histogram;
use prometheus_client::registry::Registry;
use std::sync::atomic::AtomicU64;

/// Metrics registry for the Relativist coordinator.
pub struct CoordinatorMetrics {
    /// Total BSP rounds completed.
    pub rounds_total: Counter,
    /// Wall-clock duration of each round (seconds).
    pub round_duration_seconds: Histogram,
    /// Number of currently connected workers.
    pub active_workers: Gauge<i64, std::sync::atomic::AtomicI64>,
    /// Total partitions dispatched across all rounds.
    pub partitions_dispatched_total: Counter,
    /// Border redexes detected after last merge.
    pub border_redexes: Gauge<i64, std::sync::atomic::AtomicI64>,
    /// Duration of the merge phase (seconds).
    pub merge_duration_seconds: Histogram,
    /// Duration of the split phase (seconds).
    pub split_duration_seconds: Histogram,
    /// Total bytes dispatched to workers.
    pub dispatch_bytes_total: Counter,
    /// Total bytes received back from workers.
    pub return_bytes_total: Counter,
}
```

Source: PESQ-016 Section 2.1.

**R13.** Worker-side metrics MUST be reported to the coordinator as part of the `ReturnPartition` protocol message (SPEC-06), NOT via per-worker HTTP endpoints. The coordinator MUST aggregate worker metrics into its own registry. **(MUST)**

Source: PESQ-016 L4. Rationale: avoids requiring workers to run HTTP servers; keeps the metrics scrape target as a single endpoint on the coordinator.

**R14.** Protocol-level metrics MUST be maintained by the coordinator. **(MUST)**

| Metric Name | Type | Labels | Description |
|-------------|------|--------|-------------|
| `relativist_messages_sent_total` | Counter | `type` | Messages sent by message type |
| `relativist_messages_received_total` | Counter | `type` | Messages received by message type |
| `relativist_message_size_bytes` | Histogram | `type` | Message size distribution |
| `relativist_heartbeat_latency_seconds` | Histogram | --- | Heartbeat round-trip time |

Source: PESQ-016 Section 2.3.

**R15.** All histograms MUST use custom bucket boundaries tuned to IC reduction latencies: `[0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0, 30.0]` seconds. Default Prometheus buckets (designed for web request latencies) MUST NOT be used. **(MUST)**

Source: PESQ-016 L5.

**R16.** Label cardinality MUST be kept low. Acceptable labels. **(MUST)**

| Label | Max Cardinality | Acceptable |
|-------|----------------|------------|
| `worker_id` | 8 (bounded by cluster size) | Yes |
| `rule` | 6 (CON-CON, CON-DUP, CON-ERA, DUP-DUP, DUP-ERA, ERA-ERA) | Yes |
| `type` (message type) | ~10 (bounded by protocol spec) | Yes |
| `partition_id` | Unbounded | **NO -- DO NOT use as label** |
| `round` | Unbounded | **NO -- DO NOT use as label** |

Source: PESQ-016 Section 2.4.

**R17.** All metric names MUST be prefixed with `relativist_` to avoid collisions when running alongside other instrumented services. **(MUST)**

**R18.** The `CoordinatorMetrics` struct MUST provide a `register(registry: &mut Registry)` method that registers all metrics with their descriptions. **(SHOULD)**

### 3.3 HTTP Endpoints

**R19.** HTTP endpoints MUST be feature-gated under the `metrics` Cargo feature (same gate as Prometheus metrics). **(MUST)**

**R20.** The coordinator MUST serve HTTP endpoints using `axum` on a dedicated port, separate from the binary grid protocol port (SPEC-06). The default metrics port MUST be `9090`, configurable via `--metrics-port`. **(MUST)**

Source: PESQ-016 L2, L3.

**R21.** The following HTTP routes MUST be served. **(MUST)**

| Route | Method | Response | Content-Type |
|-------|--------|----------|-------------|
| `GET /metrics` | GET | Prometheus text exposition format (OpenMetrics) | `application/openmetrics-text` |
| `GET /health` | GET | `"ok"` with status 200 | `text/plain` |
| `GET /ready` | GET | Status 200 if ready, 503 if not ready | `text/plain` |

**R22.** The `/ready` endpoint MUST return 200 OK when the coordinator FSM (SPEC-13, R19) is in state `WaitingForWorkers` or any subsequent state (`Partitioning`, `Dispatching`, `WaitingForResults`, `Merging`, `CheckTermination`, `Done`). It MUST return 503 Service Unavailable when the coordinator is in state `Init`. **(MUST)**

**R23.** The axum HTTP server MUST run as a background tokio task within the coordinator's async runtime. It MUST NOT block the grid protocol event loop. **(MUST)**

**R24.** The HTTP server MUST bind to the same address as the main coordinator listener (respecting `--bind`) but on the metrics port. **(SHOULD)**

### 3.4 Distributed Tracing (OpenTelemetry)

**R25.** All OpenTelemetry functionality MUST be feature-gated under the `otel` Cargo feature. When `otel` is not enabled, zero OTel-related code is compiled. **(MUST)**

Source: PESQ-014 L1, PESQ-023 D3.

**R26.** When the `otel` feature is enabled, Relativist MUST add a `tracing_opentelemetry::OpenTelemetryLayer` to the subscriber registry. This layer bridges `tracing` spans to OpenTelemetry spans. **(MUST)**

Source: PESQ-014 L2.

**R27.** The OTel exporter MUST use OTLP over HTTP (not gRPC). The endpoint MUST be configurable via the `OTEL_EXPORTER_OTLP_ENDPOINT` environment variable (OTel standard), defaulting to `http://localhost:4318`. **(MUST)**

Source: PESQ-014 L4.

**R28.** OTel resource attributes MUST include. **(MUST)**

| Attribute | Value |
|-----------|-------|
| `service.name` | `"relativist-coordinator"` or `"relativist-worker"` |
| `service.version` | `env!("CARGO_PKG_VERSION")` |
| `host.name` | System hostname |

Source: PESQ-014 Section 2.3.

**R29.** When `otel` is enabled, the coordinator SHOULD create a root span for each grid round (`grid_cycle` span with field `round`). Workers SHOULD create child spans under the coordinator's trace context. **(SHOULD)**

**R30.** The wire protocol (SPEC-06) MAY include an optional trace context header (trace_id + span_id, 32 bytes total) in `DispatchPartition` and `ReturnPartition` messages. When the `otel` feature is not enabled, this header MUST be omitted (zero overhead). **(MAY for inclusion; MUST for zero overhead when disabled)**

Source: PESQ-014 L3.

### 3.5 Initialization

**R31.** The `observability` module MUST expose an initialization function that configures all tracing layers at startup. **(MUST)**

```rust
/// Configuration for the observability subsystem.
pub struct ObservabilityConfig {
    /// Log output format.
    pub log_format: LogFormat,
    /// Optional Prometheus metrics registry (when `metrics` feature enabled).
    #[cfg(feature = "metrics")]
    pub metrics_registry: Option<prometheus_client::registry::Registry>,
    /// Whether this process is a coordinator or worker (affects OTel service.name).
    pub role: ProcessRole,
}

#[derive(Debug, Clone, Copy)]
pub enum LogFormat {
    /// Human-readable output (tracing_subscriber::fmt::format::Full).
    Text,
    /// Machine-parseable JSON output.
    Json,
}

#[derive(Debug, Clone, Copy)]
pub enum ProcessRole {
    Coordinator,
    Worker,
}

/// Initialize the tracing subscriber with all configured layers.
///
/// MUST be called once at startup, before any other tracing events.
/// Panics if called more than once.
pub fn init_tracing(config: ObservabilityConfig) {
    // 1. Build EnvFilter from RUST_LOG or defaults (R4, R5)
    // 2. Build fmt::Layer with selected format (R3)
    // 3. Optionally add OpenTelemetryLayer (R26, feature "otel")
    // 4. Set global default subscriber (R2)
    todo!()
}
```

**R32.** `init_tracing()` MUST be called exactly once, before any other work. Calling it more than once MUST panic. **(MUST)**

**R33.** `init_tracing()` MUST log an INFO event confirming successful initialization, including the active log format, active features (`metrics`, `otel`), and the default filter string. **(MUST)**

### 3.6 Exclusions (v1)

**R34.** Relativist v1 MUST NOT implement log rotation or log retention policies. Log output goes to stdout/stderr; operators use external tools (e.g., Docker log drivers, `logrotate`) for management. **(MUST NOT)**

**R35.** Relativist v1 MUST NOT include alerting rules, Grafana dashboard definitions, or any monitoring infrastructure configuration. These are operational concerns outside the software. **(MUST NOT)**

**R36.** Relativist v1 MUST NOT define custom OTel metrics (counters, histograms) via the OTel SDK directly. All metrics MUST flow through the `prometheus-client` registry (R12-R14). The OTel layer is used exclusively for distributed tracing (spans), not metrics. **(MUST NOT for OTel metrics; MUST for prometheus-client)**

**R37.** Relativist v1 MUST NOT implement dynamic log level changes at runtime. Log levels are fixed at startup via `RUST_LOG` and the default filter (R4, R5). **(MUST NOT)**

---

## 4. Design

### 4.1 Subscriber Architecture

The layered subscriber architecture follows the standard Rust pattern (PESQ-014 Section 1.2, PESQ-015 Section 1.2):

```
Application Code
    |  (tracing::info!, tracing::span!, #[instrument])
    v
tracing_subscriber::Registry
    |
    +-- fmt::Layer (always on)
    |     +-- Format: Text (dev) or JSON (production)
    |     +-- Filter: EnvFilter (RUST_LOG or defaults)
    |
    +-- [metrics feature] Metrics observation
    |     (coordinator code updates prometheus-client counters/histograms
    |      directly; no tracing layer needed for metrics)
    |
    +-- [otel feature] OpenTelemetryLayer
          +-- tracing spans -> OTel spans
          +-- OTLP/HTTP exporter -> Collector (Jaeger/Tempo)
```

### 4.2 Initialization Sequence

```
main() or worker_main()
    |
    1. Parse CLI config (clap)
    2. Build ObservabilityConfig from CLI flags + env vars
    3. Call observability::init_tracing(config)
    |     +-- Set global subscriber
    |     +-- Log "observability initialized" at INFO
    |
    4. [metrics feature] Create CoordinatorMetrics, register in Registry
    5. [metrics feature] Spawn axum HTTP server on --metrics-port
    6. Proceed with coordinator/worker FSM startup
```

### 4.3 Metrics HTTP Router

```rust
#[cfg(feature = "metrics")]
pub fn metrics_router(
    registry: std::sync::Arc<prometheus_client::registry::Registry>,
    coordinator_state: std::sync::Arc<std::sync::atomic::AtomicU8>,
) -> axum::Router {
    use axum::{routing::get, extract::State, http::StatusCode, Router};

    #[derive(Clone)]
    struct AppState {
        registry: std::sync::Arc<prometheus_client::registry::Registry>,
        coordinator_state: std::sync::Arc<std::sync::atomic::AtomicU8>,
    }

    async fn metrics_handler(
        State(state): State<AppState>,
    ) -> String {
        let mut buf = String::new();
        prometheus_client::encoding::text::encode(&mut buf, &state.registry)
            .expect("encoding metrics");
        buf
    }

    async fn health_handler() -> &'static str {
        "ok"
    }

    async fn ready_handler(
        State(state): State<AppState>,
    ) -> StatusCode {
        // Init = 0, WaitingForWorkers = 1, ...
        let state_val = state.coordinator_state
            .load(std::sync::atomic::Ordering::Relaxed);
        if state_val >= 1 {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        }
    }

    let state = AppState { registry, coordinator_state };

    Router::new()
        .route("/metrics", get(metrics_handler))
        .route("/health", get(health_handler))
        .route("/ready", get(ready_handler))
        .with_state(state)
}
```

### 4.4 Worker Metrics Reporting

Workers do not expose HTTP endpoints. Instead, worker-side measurements are included in the `ReturnPartition` message payload (SPEC-06):

```rust
/// Metrics reported by a worker alongside its reduced partition.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkerMetricsReport {
    /// Wall-clock reduction time in seconds.
    pub reduce_duration_secs: f64,
    /// Number of redexes reduced in this round.
    pub redexes_reduced: u64,
    /// Interactions broken down by rule type.
    pub interactions_by_rule: [u64; 6],
}
```

The coordinator aggregates these reports into its Prometheus registry upon receipt (R13).

### 4.5 Span Hierarchy for a Grid Round

```
[grid_cycle] round=5 workers=4 partitions=4              (coordinator, INFO)
  |
  +-- [split] input_agents=10000 k=4 strategy="id_range"  (coordinator, INFO)
  |
  +-- [dispatch] worker_id=0 partition_id=0 size_bytes=...  (coordinator, DEBUG)
  +-- [dispatch] worker_id=1 partition_id=1 size_bytes=...  (coordinator, DEBUG)
  +-- [dispatch] worker_id=2 partition_id=2 size_bytes=...  (coordinator, DEBUG)
  +-- [dispatch] worker_id=3 partition_id=3 size_bytes=...  (coordinator, DEBUG)
  |
  +-- [reduce] partition_id=0 initial_redexes=500           (worker 0, INFO)
  +-- [reduce] partition_id=1 initial_redexes=480           (worker 1, INFO)
  +-- [reduce] partition_id=2 initial_redexes=510           (worker 2, INFO)
  +-- [reduce] partition_id=3 initial_redexes=490           (worker 3, INFO)
  |
  +-- [merge] partition_count=4 border_redexes=12           (coordinator, INFO)
```

When the `otel` feature is enabled, this hierarchy is exported as a distributed trace. The coordinator's `grid_cycle` span is the root; worker `reduce` spans are children (linked via trace context in the wire protocol, R30).

### 4.6 Feature Flag Summary

| Feature | Crates Added | Functionality |
|---------|-------------|---------------|
| (always on) | `tracing`, `tracing-subscriber` | Structured logging, `fmt::Layer`, `EnvFilter` |
| `metrics` | `prometheus-client`, `axum` | `/metrics`, `/health`, `/ready` HTTP endpoints |
| `otel` | `opentelemetry`, `opentelemetry-sdk`, `opentelemetry-otlp`, `tracing-opentelemetry` | Distributed tracing export via OTLP/HTTP |

Source: PESQ-023 D3.

---

## 5. Rationale

**Why tracing as the sole API (R1)?** The Rust ecosystem has converged on `tracing` as the standard for structured diagnostics. Using a single API means application code is decoupled from all backends. Enabling OTel or Prometheus requires zero code changes -- only subscriber layer configuration. This is validated by the OpenTelemetry project itself (PESQ-014 L2) and by every major async Rust framework (tokio, axum, tonic).

**Why prometheus-client over alternatives (R11)?** Three options were evaluated (PESQ-016 Section 1.1): `prometheus-client` (official, OpenMetrics-native), `rust-prometheus` (TiKV port, legacy), and `metrics` facade. The official client was selected for OpenMetrics compliance, active maintenance by the Prometheus project, and minimal dependency footprint.

**Why separate metrics port (R20)?** Mixing HTTP and binary TCP protocols on the same port creates parsing ambiguity and complicates connection handling. Standard practice in distributed systems (Kubernetes, Prometheus itself) is to serve observability endpoints on a dedicated port. Source: PESQ-016 L2.

**Why worker metrics via protocol, not HTTP (R13)?** Requiring each worker to run an HTTP server adds deployment complexity (port management, discovery). Since workers already communicate with the coordinator via the wire protocol, piggy-backing metrics on `ReturnPartition` is simpler and sufficient for v1. The coordinator becomes the single scrape target. Source: PESQ-016 L4.

**Why custom histogram buckets (R15)?** Default Prometheus histogram buckets (0.005 to 10.0) are optimized for HTTP request latencies. IC reduction rounds can range from sub-millisecond (small nets) to 30+ seconds (large nets). Custom buckets `[0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0, 30.0]` provide better resolution across this range. Source: PESQ-016 L5.

**Why OTLP/HTTP not gRPC (R27)?** OTLP/gRPC requires a protobuf toolchain and `tonic` as additional dependencies. OTLP/HTTP uses standard HTTP POST with JSON or protobuf encoding, is supported by all modern collectors (Jaeger, Tempo, Datadog), and is simpler to configure. Source: PESQ-014 L4.

**Why no custom OTel metrics (R36)?** The OTel metrics API in Rust is less mature than the tracing/span API. Using `prometheus-client` directly for metrics and OTel only for distributed tracing avoids API overlap and keeps the metrics path simple. The two concerns (metrics vs. traces) remain orthogonal.

---

## 6. Haskell Prototype Reference

The Haskell prototype (`grid_computing_interaction_combinators_prototype_v1/`) has minimal observability:

- **Logging:** Uses `putStrLn` with unstructured text output. No log levels, no structured fields, no filtering. Every grid round prints messages like `"Round 1: dispatching 4 partitions..."`.
- **Metrics:** The prototype collects `GridMetrics` (a Haskell record with `totalRounds`, `totalInteractions`, etc.) within the `go` loop of `IC.Grid`. However, these metrics are only printed as a summary at the end of execution -- they are not exposed for external collection during execution.
- **Timing:** Wall-clock timing uses `getCurrentTime` and `diffUTCTime`. No per-phase breakdown.
- **Tracing:** No distributed tracing support.

**What Relativist changes and why:**

1. Structured logging with `tracing` replaces `putStrLn`, enabling machine-parseable output, level filtering, and correlation fields (round, worker_id, partition_id).
2. Prometheus exposition enables real-time metrics collection during long-running executions (critical for the benchmark suite, SPEC-09).
3. Per-phase timing (split, dispatch, reduce, merge) enables the overhead breakdown analysis required by DISC-006 v2 and SPEC-09's overhead ratio metric.
4. Optional OTel distributed tracing provides visibility into the full lifecycle of a grid round across coordinator and workers -- something impossible with the prototype's print-based approach.

---

## 7. Test Requirements

**T1.** The tracing subscriber MUST initialize without panic when `init_tracing()` is called with default configuration. **(MUST)**

**T2.** Calling `init_tracing()` a second time MUST panic (double initialization detection). **(MUST)**

**T3.** JSON log output format MUST be verified: capture output of a test event and parse it as valid JSON containing `"level"`, `"target"`, `"fields"`, and `"timestamp"` keys. **(MUST)**

**T4.** The `RUST_LOG` environment variable MUST correctly override default log levels: setting `RUST_LOG=relativist::reduction=trace` MUST cause TRACE events from the reduction module to appear in output. **(MUST)**

**T5.** When the `metrics` feature is enabled, the `CoordinatorMetrics` registry MUST contain all expected metrics after one simulated grid round (counter values > 0, histogram observations > 0). **(MUST)**

**T6.** `GET /health` MUST return HTTP 200 with body `"ok"`. **(MUST)**

**T7.** `GET /ready` MUST return HTTP 503 when the coordinator state is `Init`, and HTTP 200 when the coordinator state is `WaitingForWorkers` or later. **(MUST)**

**T8.** `GET /metrics` MUST return a response parseable as Prometheus text exposition format (OpenMetrics). The response MUST contain at least one `relativist_` prefixed metric. **(MUST)**

**T9.** When the `metrics` feature is NOT enabled, the HTTP server MUST NOT be started, and no metrics-related code should be compiled (verified by building without the feature and checking binary size or compile errors on direct metrics usage). **(MUST)**

---

## 8. Open Questions

None. All design decisions for v1 observability have been resolved by PESQ-014, PESQ-015, PESQ-016, and PESQ-023 (D5).
