---
title: Security & observability
summary: Operations reference for Relativist's 3-tier security model (token auth, TLS 1.3) and tracing/metrics/health observability stack.
keywords: [security, tls, mtls, token, auth, tier, observability, tracing, metrics, prometheus, health, logging]
modules: [security, observability]
specs: [SPEC-10, SPEC-11]
audience: [user, contributor]
status: reference
updated: 2026-06-26
---

# Security & observability

Operations reference for running Relativist securely and observing it in
production. Authoritative specs: `docs/specs/SPEC-10-security.md` (security)
and `docs/specs/SPEC-11-observability.md` (observability). Code lives in
`relativist-core/src/security/` and `relativist-core/src/observability/`.

Legend: **default** = always compiled; **feature-gated** = requires a Cargo
feature flag (`--features <name>`) and is fully compiled out otherwise.

---

## security-model

### three-tier-model

The active tier is detected **purely from CLI flags** (no inference from bind
address or environment). Tier is logged at `INFO` on startup. Source:
`security::detect_tier`, `security::SecurityTier`.

| Tier | Name | Auth | Encryption | Integrity | Bind default | Use case |
|------|------|------|------------|-----------|--------------|----------|
| 1 | `Development` | none | none | CRC32 (wire) | `127.0.0.1:9000` | local testing, single-host Docker Compose |
| 2 | `PrivateNetwork` | token | none | CRC32 (wire) | explicit `--bind` | LAN, VPN, private cloud |
| 3 | `Production` | token | TLS 1.3 | TLS (subsumes CRC32) | explicit `--bind` | untrusted network, cloud |

Detection rule (`detect_tier(has_token, has_tls)`):

- no `--token`, no TLS flags -> **Tier 1**
- `--token` present, no TLS flags -> **Tier 2**
- `--token` present **and** TLS flags present -> **Tier 3**

Tier 1 requires **zero** configuration: `relativist coordinator --workers 2
--input net.bin` runs without any security flags and binds to localhost.

TLS flags **without** `--token` are rejected at config-build time with a
configuration error (`build_security_config`): TLS alone encrypts but lets any
host that trusts the CA register as a worker.

### token-authentication

- **Type:** `AuthToken` — 256-bit (32-byte) value (`security::token::AuthToken`).
- **Generation:** `--token auto` generates via `rand::rngs::OsRng` (a CSPRNG).
- **Encoding:** base64 standard alphabet, 44 characters (`to_base64` /
  `from_base64`). Transmitted on the wire as 32 raw bytes (not base64).
- **Provided value:** `--token <base64>` decodes and uses the given token
  (length-checked; wrong length or bad base64 is a `TokenError`).
- **Comparison:** **constant-time** via `subtle::ConstantTimeEq` inside
  `AuthToken::verify()`. `AuthToken` deliberately does **not** implement
  `PartialEq`/`Eq`, so all comparisons route through the timing-safe path.
- **Redaction:** `Debug` for `AuthToken` prints `AuthToken("[REDACTED]")`; the
  raw value never appears in logs, metrics, errors, or health responses after
  the single startup display.
- **Lifetime:** per-session. Generated once at coordinator startup, valid until
  shutdown. No rotation, refresh, or expiry.

**Worker token sources** (precedence): `--token <value>` CLI flag, then the
`RELATIVIST_TOKEN` environment variable. CLI flag wins if both are set.

### token-file

When a token is generated, the coordinator writes it to a file
(`security::write_token_file`):

- **Default path:** `./relativist-token`, configurable via `--token-file`.
- **Permissions:** `0600` (owner read/write) on Unix via `#[cfg(unix)]`. On
  non-Unix (Windows) the file is created with platform-default permissions —
  this is a documented limitation; protect the file by other means.

### bind-warnings

Default bind is `127.0.0.1:9000` to avoid accidental network exposure. Binding
elsewhere requires an explicit `--bind` (e.g. `--bind 0.0.0.0:9000`).
`security::check_bind_warnings` emits, when binding to `0.0.0.0` (unspecified
address):

- always: `WARN` "Binding to all interfaces (0.0.0.0). Ensure authentication is
  enabled for non-trusted networks."
- additionally if no token: `WARN` "No authentication configured while binding
  to all interfaces. Use --token for production deployments."

The coordinator **proceeds** in both cases (warn, never refuse).

### tls-1.3 (feature-gated: `tls`)

Compiled only with `--features tls`. Without the feature, no dependency on
`rustls`, `tokio-rustls`, or `rustls-pemfile` is built
(`security::tls` module is `#[cfg(feature = "tls")]`).

- **Library:** `rustls` (pure Rust) + `tokio-rustls`, **TLS 1.3 only**
  (`builder_with_protocol_versions(&[&rustls::version::TLS13])`); no TLS 1.2
  fallback.
- **Mode:** **Server TLS only** — coordinator presents a cert, workers verify
  it. mTLS (client certs) is **not** in v1 (`with_no_client_auth()`).
- **Coordinator side:** `TlsServerConfig::from_pem_files(cert, key)` loads a
  PEM cert + private key.
- **Worker side:** `TlsClientConfig::from_ca_pem(ca)` loads the CA used to
  verify the coordinator. Self-signed CAs are supported; no external PKI is
  required.
- TLS wraps the existing wire framing transparently at the TCP-stream level;
  framing, serialization, and the transport interface are unchanged. TLS
  applies only to `TcpTransport` (not `ChannelTransport`).

### security-cli-flags

Coordinator (`CoordinatorArgs`):

| Flag | Default | Notes |
|------|---------|-------|
| `--bind`, `-b` | `127.0.0.1:9000` | bind address; `tailscale[:PORT]` shorthand supported |
| `--token` | none | `auto` to generate, or a base64 token value |
| `--token-file` | `./relativist-token` | path for the generated token |
| `--tls-cert` | none | PEM cert path; **feature-gated `tls`**, requires `--tls-key` |
| `--tls-key` | none | PEM private-key path; **feature-gated `tls`**, requires `--tls-cert` |

Worker (`WorkerArgs`):

| Flag | Default | Notes |
|------|---------|-------|
| `--coordinator`, `-c` | (required) | coordinator `HOST:PORT` |
| `--token` | none | base64 token, or use `RELATIVIST_TOKEN` env var |
| `--tls-ca` | none | PEM CA path; **feature-gated `tls`**; presence triggers a TLS handshake |

Notes / current limitations:

- The spec defines an `--insecure` flag (suppress bind warnings); it is **not
  yet wired** in `config.rs`.
- Registration flow: worker sends `Register{ protocol_version, auth_token }`;
  coordinator validates (constant-time), replies `RegisterAck` (with assigned
  `WorkerId`) or `RegisterNack{ reason: "authentication failed" }` and closes
  the connection. Error text is intentionally generic (no internal-state leak).

### not-in-v1

Excluded by SPEC-10 R37: mTLS, token rotation/expiry, Byzantine fault
tolerance, certificate rotation, ACLs, encryption at rest, HMAC message
integrity, the `zeroize` crate. Connection limits (`max_connections` default
1024) and idle timeout (default 30 s) exist in `SecurityConfig` but are
operational defaults, not full DoS protection.

---

## observability

All application code logs through `tracing` only (no `println!`/`log`).
Backends attach as subscriber layers at startup. Entry point:
`observability::init_tracing` (call exactly once; a second call panics).

### structured-logging (default)

- **Init:** `init_tracing(&ObservabilityConfig{ log_format, role })`. Output
  goes to **stderr** (keeps stdout clean for CSV/data pipelines).
- **Formats** (`--log-format`): `text` (human-readable, dev default when stdout
  is a TTY) or `json` (machine-parseable, default when not a TTY). Both include
  target (module path), thread ID, and timestamp; file/line are off by default.
- **Process role** (`ProcessRole`): `Coordinator`, `Worker`, or `Local`
  (the `relativist local` reduction mode — logging on, HTTP endpoints off).

### log-levels

Filtering via the `RUST_LOG` environment variable
(`tracing_subscriber::EnvFilter`). If `RUST_LOG` is unset, `DEFAULT_LOG_FILTER`
applies (`observability::tracing_init::DEFAULT_LOG_FILTER`):

```
relativist::coordinator=info,relativist::worker=info,
relativist::reduction=warn,relativist::protocol=warn,
relativist::partition=info,relativist::net=warn,
relativist::observability=info,relativist::security=info,warn
```

Hot paths (`reduction`, `protocol`, `net`) default to `WARN`; lifecycle modules
(`coordinator`, `worker`, `partition`) to `INFO`. Trailing `warn` is the
catch-all. Override per target, e.g. `RUST_LOG=relativist::reduction=trace`.
FSM state transitions and error categories (invariant violations, protocol
errors, FSM `Error` transitions, fatal/merge failures) are logged at
`INFO`/`ERROR` per SPEC-11 R7/R9a. Log levels are fixed at startup — no runtime
changes; no log rotation/retention (use external tooling).

### metrics (feature-gated: `metrics`)

Compiled only with `--features metrics` (`prometheus-client` + `axum`). Without
it, no HTTP server starts and no metrics code is built
(`observability::{http, metrics}` are `#[cfg(feature = "metrics")]`).

- **Client:** `prometheus-client`, OpenMetrics-compliant. All metric names are
  prefixed `relativist_`.
- **Model:** the **coordinator** owns a `prometheus_client::registry::Registry`
  and is the single scrape target. Workers do **not** run HTTP servers; their
  measurements piggyback on the `PartitionResult` message
  (`WorkerRoundStats`) and the coordinator aggregates them.
- **Metric kinds:** counters (e.g. rounds, partitions dispatched, bytes,
  interactions-by-rule), histograms (round/split/merge durations, message
  size), gauges (active workers, border redexes). Histograms use IC-tuned
  buckets `[0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0, 30.0]` seconds.
- **Cardinality:** keep labels bounded — `worker_id` (<=8), `rule` (6), message
  `type` (~10). Never use `round` or `partition_index` as Prometheus labels
  (unbounded).

### health-readiness (feature-gated: `metrics`)

Served by `axum` (`observability::http::metrics_router`), spawned as a
background tokio task (`spawn_metrics_server`) on a dedicated port (spec default
`9090`, separate from the grid protocol port) and shut down gracefully via a
oneshot signal.

| Route | Method | Success | Content-Type |
|-------|--------|---------|--------------|
| `/metrics` | GET | 200, OpenMetrics text | `application/openmetrics-text; version=1.0.0; charset=utf-8` |
| `/health` | GET | 200 `ok` (liveness) | `text/plain` |
| `/ready` | GET | 200 `ready` / 503 `not ready` | `text/plain` |

`/ready` reads an `AtomicBool` (`is_ready`) set by the coordinator FSM: `true`
once it leaves `Init` (entering `WaitingForWorkers`..`Done`), `false` in `Init`
or `Error`. The boolean flag avoids fragile enum-ordinal comparison.

Note: `--metrics-port` is specified (SPEC-11 R20) but **not yet wired** in
`config.rs` (marked for a later phase); the router and server helper exist and
are exercised by tests.

### distributed-tracing (feature-gated: `otel`)

Compiled only with `--features otel` (`opentelemetry`, `opentelemetry_sdk`).
Intended to bridge `tracing` spans to OpenTelemetry and export via **OTLP over
HTTP** (not gRPC), endpoint from `OTEL_EXPORTER_OTLP_ENDPOINT` (default
`http://localhost:4318`), with resource attributes `service.name`
(`relativist-coordinator`/`-worker`/`-local`), `service.version`, `host.name`.
OTel is used for **traces only**; all metrics flow through `prometheus-client`.
Current code: the `otel` Cargo feature and its deps exist and `init_tracing`
reports `otel_enabled`, but the `OpenTelemetryLayer` wiring is minimal — treat
OTel export as spec-defined and not fully exercised in this build.

### feature-flag-summary

| Feature | Adds | Provides |
|---------|------|----------|
| (default) | `tracing`, `tracing-subscriber` | structured logging, `fmt::Layer`, `EnvFilter` |
| `metrics` | `prometheus-client`, `axum` | `/metrics`, `/health`, `/ready` endpoints |
| `otel` | `opentelemetry`, `opentelemetry_sdk` | distributed tracing export (OTLP/HTTP) |
| `tls` | `rustls`, `tokio-rustls`, `rustls-pemfile` | TLS 1.3 server-side transport (Tier 3) |

---

## quick-reference

```bash
# Tier 1 (dev, localhost, no auth)
relativist coordinator --workers 2 --input net.bin

# Tier 2 (token auth on a private network)
relativist coordinator --workers 4 --bind 0.0.0.0:9000 --token auto --input net.bin
RELATIVIST_TOKEN=<base64> relativist worker --coordinator HOST:9000

# Tier 3 (token + TLS 1.3; build with --features tls)
relativist coordinator --workers 4 --bind 0.0.0.0:9000 \
  --token auto --tls-cert cert.pem --tls-key key.pem --input net.bin
relativist worker --coordinator HOST:9000 --token <base64> --tls-ca ca.pem

# Observability: JSON logs + raise reduction verbosity
RUST_LOG=relativist::reduction=trace \
  relativist coordinator --workers 2 --log-format json --input net.bin
# Build with metrics endpoints: cargo build --features metrics  ->  GET :9090/metrics
```
