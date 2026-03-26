---
pesq_id: PESQ-014
title: "OpenTelemetry for Rust (2025)"
category: Observability & Tracing
date: 2026-03-26
status: Complete
cross_references:
  specs: [SPEC-11, SPEC-13]
  pesqs: [PESQ-003, PESQ-015, PESQ-016]
  discs: []
---

# PESQ-014: OpenTelemetry for Rust (2025)

**Category:** Observability & Tracing
**Status:** Complete

---

## 1. Subject Overview

OpenTelemetry (OTel) is a vendor-neutral observability framework providing APIs and SDKs for collecting traces, metrics, and logs. The Rust implementation (`opentelemetry-rust`) integrates with the `tracing` ecosystem via `tracing-opentelemetry`.

### 1.1 Crate Ecosystem

| Crate | Purpose | Required? |
|-------|---------|-----------|
| `opentelemetry` | Core API (Context, Propagators, Metrics API, Tracing API) | Yes (if OTel used) |
| `opentelemetry-sdk` | SDK implementation (Logging, Metrics, Tracing SDKs) | Yes |
| `opentelemetry-otlp` | OTLP exporter (HTTP or gRPC) | Yes (production) |
| `tracing-opentelemetry` | Bridge: tracing spans → OTel spans | Yes (bridges tracing) |
| `opentelemetry-prometheus` | Prometheus exporter for OTel metrics | Optional |

### 1.2 Integration Architecture

```
Application Code
    │  (uses tracing::info!, tracing::span!)
    ▼
tracing Subscriber
    │
    ├── fmt::Layer (console/JSON output) ─── always on
    │
    ├── OpenTelemetryLayer ─── feature-flagged ("otel")
    │       │
    │       ▼
    │   OTel SDK → OTLP Exporter → Collector → Jaeger/Tempo/etc.
    │
    └── PrometheusLayer ─── feature-flagged ("metrics")
            │
            ▼
        /metrics endpoint → Prometheus scrape
```

**Key insight:** Application code uses `tracing` exclusively. OTel is a subscriber layer — zero application code changes to enable/disable it.

---

## 2. Key Mechanisms

### 2.1 Span Propagation

`tracing-opentelemetry` automatically:
- Creates OTel spans from tracing spans
- Propagates trace context (trace_id, span_id) across async boundaries
- Supports W3C TraceContext and B3 propagation formats

For Relativist, this means:
- Coordinator creates a root span per grid round
- Each `DispatchPartition` message carries trace context in headers
- Worker creates child spans under the coordinator's trace
- The full round lifecycle is visible as a single distributed trace

### 2.2 Metrics Bridge

OTel SDK provides metrics (counters, histograms, gauges) that can be exported to Prometheus. The `opentelemetry-prometheus` crate creates a Prometheus-compatible `/metrics` endpoint from OTel metrics.

### 2.3 Resource Detection

OTel attaches resource metadata (service name, version, host) to all telemetry. For Relativist:
- `service.name = "relativist-coordinator"` or `"relativist-worker"`
- `service.version = env!("CARGO_PKG_VERSION")`
- `host.name` = hostname

---

## 3. Relevance to Relativist

### 3.1 SPEC-11 Requirements Mapping

| SPEC-11 Requirement | OTel Solution |
|---------------------|---------------|
| Structured logging | `tracing` + `fmt::Layer` (JSON) |
| Metrics export | OTel metrics → Prometheus exporter |
| Health endpoints | Separate (not OTel concern) |
| Distributed tracing | `tracing-opentelemetry` → OTLP |

### 3.2 Dependency Cost

Adding full OTel support means ~10 additional crates. This is significant for a research project. The recommendation is to make OTel **optional** via feature flag.

---

## 4. Lessons for Relativist

### L1: OTel as Optional Feature Flag [ADOPT]
`tracing` is always-on. OTel distributed tracing is behind `--features otel`. This keeps the default binary lean while enabling deep observability when needed.
→ Informs: SPEC-11, SPEC-13

### L2: Use tracing as the Single Instrumentation API [ADOPT]
Application code ONLY uses `tracing` macros. All other observability (OTel, Prometheus, JSON logs) is configured via subscriber layers. This is the recommended Rust pattern and is validated by the OTel project itself.
→ Informs: SPEC-11, SPEC-13

### L3: Trace Context in Wire Protocol [ADAPT]
To enable distributed tracing, the wire protocol (SPEC-06) should have an optional header field for trace context (trace_id + span_id). When `otel` feature is disabled, this field is empty/skipped.
→ Informs: SPEC-06, SPEC-11

### L4: OTLP over HTTP for Simplicity [ADOPT]
Use OTLP/HTTP (not gRPC) exporter. HTTP is simpler, doesn't require protobuf toolchain, and works with all modern collectors (Jaeger, Tempo, Datadog).
→ Informs: SPEC-11

---

## 5. Sources

| Source | URL | Accessed |
|--------|-----|----------|
| OpenTelemetry Rust docs | https://opentelemetry.io/docs/languages/rust/ | 2026-03-26 |
| tracing-opentelemetry docs.rs | https://docs.rs/tracing-opentelemetry | 2026-03-26 |
| opentelemetry-rust GitHub | https://github.com/open-telemetry/opentelemetry-rust | 2026-03-26 |
| OTel Rust Getting Started | https://opentelemetry.io/docs/languages/rust/getting-started/ | 2026-03-26 |
| SigNoz OTel Rust guide | https://signoz.io/blog/opentelemetry-rust/ | 2026-03-26 |
| Datadog Rust OTel monitoring | https://www.datadoghq.com/blog/monitor-rust-otel/ | 2026-03-26 |
