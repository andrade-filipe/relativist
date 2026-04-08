# Review: Phase 8 — Observability (SPEC-11)

**Tasks:** TASK-0140 through TASK-0155
**Spec:** SPEC-11 (Revised v2)
**Date:** 2026-04-08
**Files reviewed:** `src/observability/{mod.rs, types.rs, metrics.rs, http.rs, tracing_init.rs}`

---

## Stage 4: Code Cleaner

### MF-01: Duplicate LogFormat enum (naming/cohesion)

`src/config.rs:65` defines `LogFormat` with `#[derive(ValueEnum)]` for clap.
`src/observability/types.rs:7` defines a separate `LogFormat` without `ValueEnum`.
`main.rs:13` calls `init_tracing(&ObservabilityConfig::default())` ignoring the CLI `--log-format` flag entirely.

The observability `LogFormat` is **dead code** — it is never constructed from the CLI `--log-format` argument. The config `LogFormat` (clap) is parsed but never converted to the observability one. **This means `--log-format` is never wired up (R3 violation).**

**Fix:** Outside observability scope — but note the issue. In `types.rs`, add a `From<config::LogFormat>` impl or unify. For now, document the gap.

**Classification:** MF (spec violation — R3 dead code path)

### SF-01: `init_tracing` does not log the default filter string (R33)

R33 requires logging "the default filter string". The current `tracing::info!` logs `log_format`, `role`, `metrics_enabled`, `otel_enabled` but NOT the filter string that was selected.

**Classification:** SF (spec gap, easy fix)

### SF-02: `init_tracing` does not detect double-init (R32)

R32 says "Calling it more than once MUST panic." The current code relies on `tracing_subscriber`'s `.init()` which calls `set_global_default` — this returns `Err` if already set, and `.init()` silently ignores the error (it does NOT panic). So double-call does NOT panic as R32 requires.

**Classification:** MF (spec violation — R32)

### NTH-01: `ObservabilityConfig::default()` uses `ProcessRole::Local`

Reasonable default. No action needed.

### NTH-02: Code duplication in `init_tracing` Text vs Json branches

The two `match` arms are nearly identical except for `.json()`. Could be refactored with a helper but the duplication is minor and readable.

**Classification:** NTH

---

## Stage 5: Architecture / Spec Compliance

### Spec Compliance Matrix (MUST requirements only)

| Req | Status | Notes |
|-----|--------|-------|
| R1 (tracing only) | PARTIAL | `main.rs:27` uses `eprintln!`. `commands.rs` and `io/mod.rs` use `println!`. These are OUTSIDE observability module scope but noted. |
| R2 (Registry + layers) | PASS | Uses `tracing_subscriber::registry()` with `.with()` layers. |
| R3 (--log-format text/json) | **FAIL** | `--log-format` parsed in config.rs but never wired to `init_tracing`. `init_tracing` always uses `ObservabilityConfig::default()` (Text). Dead code. |
| R4 (RUST_LOG + EnvFilter) | PASS | `EnvFilter::try_from_default_env()` with fallback. |
| R5 (default levels) | PASS | `DEFAULT_LOG_FILTER` matches spec exactly. |
| R9 (target/thread/timestamp) | PASS | `with_target(true)`, `with_thread_ids(true)`, file/line off by default. |
| R10 (metrics feature gate) | PASS | `#[cfg(feature = "metrics")]` on mod.rs for http and metrics modules. |
| R11 (prometheus-client) | PASS | Uses `prometheus_client` crate. |
| R12 (CoordinatorMetrics) | **FAIL** | Missing `interactions_by_rule_total: Family<..., Counter>`. 9 of 10 metrics present. |
| R14 (protocol metrics) | **NOT IMPL** | `messages_sent_total`, `messages_received_total`, `message_size_bytes`, `heartbeat_latency_seconds` not defined. Noted — these require protocol module changes, outside observability scope. |
| R15 (custom buckets) | PASS | `HISTOGRAM_BUCKETS` matches spec exactly. |
| R17 (relativist_ prefix) | PASS | All metric names prefixed. |
| R18 (register method) | PASS | `CoordinatorMetrics::register()` method exists. |
| R19 (HTTP feature gate) | PASS | `#[cfg(feature = "metrics")]` on http module. |
| R20 (metrics port) | **PARTIAL** | Default port 9090 is used in spawn_metrics_server (caller provides). `--metrics-port` arg NOT yet in CoordinatorArgs (comment says "will be added in Phase 8"). |
| R21 (HTTP routes) | PASS | `/metrics`, `/health`, `/ready` all present. |
| R22 (ready states) | PASS | AtomicBool checked. |
| R22a (AtomicBool) | PASS | Uses `AtomicBool` explicitly. |
| R23 (background task) | PASS | `tokio::spawn` in `spawn_metrics_server`. |
| R24a (HTTP shutdown) | PASS | `with_graceful_shutdown` with oneshot receiver. |
| R31 (init function) | PASS | `init_tracing()` exposed. |
| R32 (panic on double init) | **FAIL** | `.init()` does NOT panic on second call. |
| R33 (log init confirmation) | **PARTIAL** | Logs format/role/features but NOT the filter string. |

### MF-02: Missing `interactions_by_rule_total` metric (R12)

SPEC-11 R12 explicitly lists `interactions_by_rule_total: Family<Vec<(String, String)>, Counter>` in the `CoordinatorMetrics` struct. This is the most important metric for the benchmark suite (SPEC-09) — it tracks per-rule interaction counts. The current implementation omits it entirely.

**Classification:** MF

### MF-03: `--metrics-port` not added to CoordinatorArgs (R20)

Config.rs has a TODO comment but the argument is missing. However, this is in `src/config.rs`, outside observability module scope. Noted but not fixed here.

**Classification:** MF (but outside scope — config.rs)

---

## Stage 6: QA Bug Hunt

### BUG-01: HTTP server shutdown — oneshot drop behavior

If the oneshot `Sender` is dropped without sending, the `Receiver` resolves to `Err(Canceled)`. The shutdown handler does `let _ = shutdown.await;` which correctly handles both `Ok(())` and `Err(Canceled)`, so the server shuts down in either case. **No bug.**

### BUG-02: Metric overflow/wrapping

`Counter` in prometheus-client uses `AtomicU64` internally. At 10 billion interactions per second it would take ~58 years to overflow. **No practical risk.**

`Gauge<i64, AtomicI64>` for `active_workers` and `border_redexes` — i64 overflow is impossible in practice (would require 2^63 workers). **No bug.**

### BUG-03: Thread safety of shared state

`AppState` contains `Arc<Registry>` and `Arc<AtomicBool>`. `Registry` is behind `Arc` (immutable after registration). `AtomicBool` is inherently thread-safe. The `Ordering::Relaxed` for `is_ready` is acceptable — readiness is a monotonic flag that doesn't need happens-before guarantees with other memory operations. **No bug.**

### BUG-04: `metrics_handler` encoding error path

If `prometheus_client::encoding::text::encode` fails, the handler returns 500 with `text/plain`. The error path works correctly. However, the return types differ between success and error paths — success returns `"application/openmetrics-text; ..."` while error returns `"text/plain"`. Both branches return the same tuple type `(StatusCode, [(HeaderName, &str); 1], String)` so this compiles and is correct. **No bug.**

### BUG-05: `init_tracing` silent double-init

As noted in SF-02/R32, `tracing_subscriber::registry().init()` calls `tracing::subscriber::set_global_default()` which returns `Result`. The `.init()` method from `SubscriberInitExt` calls `.try_init()` and ignores the error. A second call would silently succeed (no panic), violating R32.

**Classification:** MF (already tracked as SF-02)

### OBSERVATION: `Ordering::Relaxed` is appropriate

For the `is_ready` atomic: this flag transitions false->true once (Init->WaitingForWorkers) and true->false once (->Error). The readiness endpoint is advisory — a stale read is harmless (returns 503 briefly after becoming ready, or 200 briefly after error). `Relaxed` is correct.

---

## Summary of Findings

| ID | Severity | Description | File | Fix in Stage 7? |
|----|----------|-------------|------|-----------------|
| MF-01 | MF | `--log-format` never wired to init_tracing | main.rs, types.rs | NO (outside scope) |
| MF-02 | MF | Missing `interactions_by_rule_total` metric (R12) | metrics.rs | **FIXED** |
| MF-03 | MF | Missing `--metrics-port` CLI arg (R20) | config.rs | NO (outside scope) |
| SF-01 | SF | R33: init log missing filter string | tracing_init.rs | **FIXED** |
| SF-02 | MF | R32: double-init does not panic | tracing_init.rs | **FIXED** |
| NTH-02 | NTH | Code duplication in Text/Json branches | tracing_init.rs | NO |
| SF-03 | SF | Clippy: useless `.into_iter()` on `HISTOGRAM_BUCKETS` | metrics.rs | **FIXED** |

---

## Stage 7: Refactoring Applied

### Fix MF-02: Added `interactions_by_rule_total` metric

- Added `RuleLabel` (struct with `EncodeLabelSet`) and `RuleValue` (enum with `EncodeLabelValue`) to `metrics.rs`
- Added `RULE_LABELS: [RuleValue; 6]` constant indexed by `SpecificRule` ordinal
- Added `interactions_by_rule_total: Family<RuleLabel, Counter>` field to `CoordinatorMetrics`
- Registered as `"relativist_interactions_by_rule_total"` in `register()`
- Added `test_interactions_by_rule_total` test exercising get_or_create + encoding
- Updated `test_prometheus_encoding` to assert presence in output

### Fix SF-02/R32: Double-init panic guard

- Added `static TRACING_INITIALIZED: AtomicBool` guard with `Ordering::SeqCst`
- `init_tracing()` now panics on second call with clear message citing SPEC-11 R32
- Added `reset_init_guard()` (test-only) for test isolation
- Added `test_double_init_guard_panics` test

### Fix SF-01/R33: Log filter source in init confirmation

- `init_tracing()` now checks `RUST_LOG` env var presence and reports `filter_source` ("RUST_LOG" or "defaults")
- Added `default_filter = DEFAULT_LOG_FILTER` field to the init log event

### Fix SF-03: Clippy `.into_iter()` removal

- Removed unnecessary `.into_iter()` on `HISTOGRAM_BUCKETS` array (3 occurrences)

### Test Results

- All 19 observability tests pass (including 2 new tests)
- Full suite: 565 passed, 5 failed (pre-existing `encoding::arithmetic` only)
- Clippy: 0 observability warnings; 1 pre-existing warning in `io/text_dsl.rs` (outside scope)

### Remaining Issues (outside observability scope)

1. **MF-01**: `--log-format` CLI arg parsed but never wired to `init_tracing` (requires `main.rs` + `config.rs` changes)
2. **MF-03**: `--metrics-port` not added to `CoordinatorArgs` (requires `config.rs` change)
3. **R14**: Protocol-level metrics not defined (requires `protocol/` module changes)
4. **R1**: `println!`/`eprintln!` usage in `commands.rs`, `io/mod.rs`, `main.rs`
