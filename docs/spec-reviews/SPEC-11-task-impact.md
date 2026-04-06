# SPEC-11 Task Impact Report

**Date:** 2026-04-05
**Trigger:** SPEC-11 revision from Draft v1 to Revised v2 (16 critic issues resolved)
**Spec review:** SPEC-11-round2-defender.md
**Tasks affected:** 16 updated, 2 created, 0 removed

---

## 1. Summary Table

| Task ID | Title | Action | Reason |
|---------|-------|--------|--------|
| TASK-0141 | Define LogFormat and ProcessRole enums | **UPDATED** | Added `ProcessRole::Local` variant (R33a, SC-013) |
| TASK-0142 | Define ObservabilityConfig struct | **UPDATED** | Added `default_local()` constructor (R33a) |
| TASK-0144 | Implement init_tracing with fmt::Layer and EnvFilter | **UPDATED** | Added R33a (local mode) acceptance criteria |
| TASK-0146 | Add #[instrument] to reduction reduce_all() | **UPDATED** | Renamed `partition_id` to `partition_index` (SC-016) |
| TASK-0148 | Add #[instrument] to dispatch() and handle_message() | **UPDATED** | Renamed `partition_id` to `partition_index` (SC-016) |
| TASK-0150 | Define CoordinatorMetrics struct and registration | **UPDATED** | Added `interactions_by_rule_total` Family metric (SC-008); R18 MUST->SHOULD (SC-004) |
| TASK-0151 | Define protocol metrics | **UPDATED** | Fixed message names: `DispatchPartition`->`AssignPartition`, `ReturnPartition`->`PartitionResult` (SC-001) |
| TASK-0152 | Extend WorkerRoundStats with observability fields | **REWRITTEN** | Eliminated `WorkerMetricsReport`; fields moved to `WorkerRoundStats` extension (SC-002) |
| TASK-0153 | Implement coordinator metric aggregation from worker reports | **REWRITTEN** | Updated to use `WorkerRoundStats` instead of `WorkerMetricsReport`; added per-rule aggregation (SC-002, SC-008) |
| TASK-0154 | Add axum dependency and scaffold metrics_router | **REWRITTEN** | `AtomicU8` replaced with `AtomicBool` (R22a, SC-012); added `OPENMETRICS_CONTENT_TYPE` constant |
| TASK-0155 | Implement /health and /ready endpoints | **REWRITTEN** | `AtomicU8` replaced with `AtomicBool` (R22a); added `Error` state returning 503 (R22, SC-003) |
| TASK-0156 | Implement /metrics endpoint with Prometheus encoding | **UPDATED** | Added explicit Content-Type header requirement (T8a, SC-011) |
| TASK-0157 | Implement axum HTTP server spawn as background tokio task | **REWRITTEN** | `AtomicU8` replaced with `AtomicBool` (R22a); added graceful shutdown via CancellationToken (R24a, SC-014); added R33a note |
| TASK-0158 | Add OTel dependencies and init_tracing OTel layer | **UPDATED** | Added `ProcessRole::Local` -> `"relativist-local"` service name (R33a, SC-013) |
| TASK-0159 | Optional trace context in wire protocol messages | **UPDATED** | Fixed message names: `DispatchPartition`->`AssignPartition`, `ReturnPartition`->`PartitionResult` (SC-001) |
| TASK-0061 | Define WorkerRoundStats struct | **NOT MODIFIED** | Upstream dependency for TASK-0152; extension is additive. SPEC-05 R37 unchanged. |
| TASK-0213 | Implement ERROR-level logging requirements (R9a) | **NEW** | New requirement R9a (SC-007): ERROR logging for invariant violations, protocol errors, FSM Error transitions, FatalError, merge failures |
| TASK-0214 | Wire AtomicBool readiness flag in coordinator FSM (R22a) | **NEW** | New requirement R22a (SC-003, SC-012): FSM-side wiring of `is_ready` AtomicBool flag |

---

## 2. Details for Each Changed Task

### TASK-0141: Define LogFormat and ProcessRole enums

**Changes:**
- Added requirement R33a to requirements list
- Added `ProcessRole::Local` variant with doc comment explaining local mode semantics
- Updated acceptance criteria: `ProcessRole` now has 3 variants (Coordinator, Worker, Local)
- Updated context paragraph to mention R33a and Local mode
- Updated test expectations to verify `Local != Coordinator`
- Added note about SC-013 change history

**Root cause:** SC-013 identified that the spec had no observability behavior defined for local mode (`relativist reduce`). R33a and the `Local` variant were added.

### TASK-0142: Define ObservabilityConfig struct

**Changes:**
- Added requirement R33a to requirements list
- Added `default_local()` constructor with `ProcessRole::Local`
- Added test expectation for `default_local().role == ProcessRole::Local`

**Root cause:** The `Local` process role needs a corresponding config constructor.

### TASK-0144: Implement init_tracing with fmt::Layer and EnvFilter

**Changes:**
- Added R33a to requirements list
- Added two acceptance criteria for local mode: structured logging applies normally, and HTTP endpoints must not be started (enforced by callers)

**Root cause:** R33a requires that `init_tracing()` works with `ProcessRole::Local` and that callers know not to start HTTP endpoints.

### TASK-0146: Add #[instrument] to reduction reduce_all()

**Changes:**
- All occurrences of `partition_id` renamed to `partition_index` (field name, parameter name, span field name, documentation)

**Root cause:** SC-016 renamed `partition_id` to `partition_index` throughout the spec to distinguish from the prohibited Prometheus label name.

### TASK-0148: Add #[instrument] to coordinator dispatch() and protocol handle_message()

**Changes:**
- All occurrences of `partition_id` renamed to `partition_index` (span field, parameter)

**Root cause:** SC-016 field rename.

### TASK-0150: Define CoordinatorMetrics struct and registration

**Changes:**
- Updated metric count from 9 to 10 in acceptance criteria
- Added `interactions_by_rule_total: Family<Vec<(String, String)>, Counter>` field to struct definition
- Added `Family` import to code block
- Added test expectation for `interactions_by_rule_total` usage
- Changed R18 registration from "MUST" to "SHOULD" with note about inline alternative
- Updated test count in test expectations

**Root cause:** SC-008 added per-rule interaction counter. SC-004 changed R18 from MUST to SHOULD.

### TASK-0151: Define protocol metrics

**Changes:**
- Fixed message type string examples: `"DispatchPartition"` -> `"AssignPartition"`, `"ReturnPartition"` -> `"PartitionResult"`

**Root cause:** SC-001 corrected message variant names to match SPEC-06 canonical names.

### TASK-0152: Extend WorkerRoundStats with observability fields (REWRITTEN)

**Previous title:** "Define WorkerMetricsReport struct"
**Previous content:** Defined a standalone `WorkerMetricsReport` struct with `reduce_duration_secs`, `redexes_reduced`, and `interactions_by_rule` fields.

**New content:** Extends the canonical `WorkerRoundStats` (SPEC-05 R37, TASK-0061) with two additional fields: `reduce_duration_secs: f64` and `interactions_by_rule: [u64; 6]`. The `redexes_reduced` field was dropped (already captured by `local_redexes` in the canonical struct).

**Key differences:**
- No standalone `WorkerMetricsReport` type
- Dependency changed from TASK-0140 to TASK-0061
- Adds `serde::Serialize` and `serde::Deserialize` derives to `WorkerRoundStats`
- Extension is additive and backward-compatible
- Cross-spec dependency tracked in SPEC-11 OQ-1

**Root cause:** SC-002 identified that `WorkerMetricsReport` duplicated and conflicted with `WorkerRoundStats`. The critic's option (a) was adopted: extend the canonical type.

### TASK-0153: Implement coordinator metric aggregation from worker reports (REWRITTEN)

**Previous content:** Aggregated `WorkerMetricsReport` into `CoordinatorMetrics`.
**New content:** Aggregates `WorkerRoundStats` (extended) into `CoordinatorMetrics`, including per-rule interaction counts via `interactions_by_rule_total` Family metric.

**Key differences:**
- Function renamed from `aggregate_worker_report()` to `aggregate_worker_stats()`
- Parameter type changed from `&WorkerMetricsReport` to `&WorkerRoundStats`
- Added per-rule interaction aggregation into `interactions_by_rule_total` Family metric
- Logging includes `worker_id` and `local_redexes` fields from canonical stats

**Root cause:** SC-002 (type elimination) and SC-008 (per-rule aggregation).

### TASK-0154: Add axum dependency and scaffold metrics_router (REWRITTEN)

**Previous content:** Used `Arc<AtomicU8>` for coordinator state.
**New content:** Uses `Arc<AtomicBool>` for readiness flag. Added `OPENMETRICS_CONTENT_TYPE` constant.

**Key differences:**
- `AppState.coordinator_state: Arc<AtomicU8>` -> `AppState.is_ready: Arc<AtomicBool>`
- `metrics_router` parameter `coordinator_state: Arc<AtomicU8>` -> `coordinator_is_ready: Arc<AtomicBool>`
- Added `OPENMETRICS_CONTENT_TYPE` constant definition
- Added R22a to requirements list
- Removed notes about ordinal encoding; replaced with AtomicBool explanation

**Root cause:** SC-012 (AtomicU8 -> AtomicBool), SC-006 (Content-Type), SC-011 (explicit header).

### TASK-0155: Implement /health and /ready endpoints (REWRITTEN)

**Previous content:** Used `AtomicU8` ordinal comparison (`>= 1` = ready). Only `Init` returned 503.
**New content:** Uses `AtomicBool` flag. Both `Init` and `Error` states return 503.

**Key differences:**
- Readiness check uses `AtomicBool` load instead of `AtomicU8 >= 1`
- Added R22a to requirements list
- Test expectations updated: added test for `Error` state returning 503
- Removed ordinal encoding notes

**Root cause:** SC-003 (Error state omitted) and SC-012 (AtomicBool replacement).

### TASK-0156: Implement /metrics endpoint with Prometheus encoding

**Changes:**
- Added T8a to requirements list
- Added explicit acceptance criterion for Content-Type header using tuple response pattern
- Updated handler return type from `impl IntoResponse` to explicit tuple type
- Updated test expectations to include T8a Content-Type verification

**Root cause:** SC-011 identified that the handler did not set the Content-Type header explicitly. T8a was added as a separate test requirement.

### TASK-0157: Implement axum HTTP server spawn as background tokio task (REWRITTEN)

**Previous content:** Used `AtomicU8`. No graceful shutdown. No local mode consideration.
**New content:** Uses `AtomicBool`. Graceful shutdown via `CancellationToken`. Local mode documented.

**Key differences:**
- Added R24a and R33a to requirements list
- Parameter `coordinator_state: Arc<AtomicU8>` -> `coordinator_is_ready: Arc<AtomicBool>`
- Added `shutdown_token: CancellationToken` parameter
- Server uses `with_graceful_shutdown()` for clean termination
- Added note that function must not be called in local mode (R33a)
- Added `tokio_util` dependency note
- Added test for shutdown token cancellation

**Root cause:** SC-012 (AtomicBool), SC-014 (graceful shutdown), SC-013 (local mode).

### TASK-0158: Add OTel dependencies and init_tracing OTel layer

**Changes:**
- Added R33a to requirements list
- Added `ProcessRole::Local => "relativist-local"` match arm in service name selection
- Updated acceptance criteria: service.name now has 3 values

**Root cause:** SC-013 (local mode OTel service name).

### TASK-0159: Optional trace context in wire protocol messages

**Changes:**
- All occurrences of `DispatchPartition` renamed to `AssignPartition`
- All occurrences of `ReturnPartition` renamed to `PartitionResult`

**Root cause:** SC-001 (canonical message variant names from SPEC-06).

---

## 3. Details for Each New Task

### TASK-0213: Implement ERROR-level logging requirements (R9a)

**Requirement covered:** R9a (added by SC-007)
**Priority:** P1
**Complexity:** M (50-200 LoC) -- cross-cutting across 7+ modules
**Dependencies:** TASK-0144 (init_tracing must be active), Phase 1-6 (error paths must exist)

**Why new task:** No existing task covered ERROR-level logging. The original spec only specified INFO and WARN levels for FSM transitions and hot paths. R9a defines five categories of events that must be logged at ERROR level. This cannot be added to any single existing task because it spans all modules.

### TASK-0214: Wire AtomicBool readiness flag in coordinator FSM (R22a)

**Requirement covered:** R22a (added by SC-003, SC-012)
**Priority:** P1
**Complexity:** S (< 50 LoC) -- two `store()` calls at FSM transition points
**Dependencies:** TASK-0108 (coordinator FSM), TASK-0154 (provides the AtomicBool type)

**Why new task:** The HTTP-side reading of the readiness flag is covered by TASK-0155 (updated). But the FSM-side writing of the flag is a separate concern that belongs in the coordinator module. No existing Phase 8 task modifies the coordinator FSM to write the readiness flag. TASK-0108 (coordinator FSM transition function) is a SPEC-13 task and should not be extended with SPEC-11 concerns. A dedicated task maintains clean separation.

---

## 4. Requirement Coverage Verification

| Requirement | Status | Task(s) |
|-------------|--------|---------|
| R1 (tracing sole API) | Covered | TASK-0140, TASK-0144 |
| R2 (Registry root subscriber) | Covered | TASK-0144 |
| R3 (fmt::Layer formats) | Covered | TASK-0141, TASK-0144 |
| R4 (RUST_LOG override) | Covered | TASK-0143 |
| R5 (default per-target levels) | Covered | TASK-0143 |
| R6 (#[instrument] on key functions) | Covered | TASK-0145, TASK-0146, TASK-0147, TASK-0148 |
| R7 (FSM transition logging) | Covered | TASK-0149 |
| R8 (contextual correlation fields) | Covered | TASK-0145, TASK-0146, TASK-0147, TASK-0148, TASK-0149 |
| R9 (fmt::Layer includes target/thread/timestamp) | Covered | TASK-0144 |
| **R9a (ERROR-level logging)** | **Covered (NEW)** | **TASK-0213** |
| R10 (metrics feature gate) | Covered | TASK-0140, TASK-0150 |
| R11 (prometheus-client crate) | Covered | TASK-0150 |
| R12 (CoordinatorMetrics struct) | Covered | TASK-0150 (updated: +interactions_by_rule_total) |
| R13 (worker metrics via protocol) | Covered | TASK-0152 (rewritten), TASK-0153 (rewritten) |
| R14 (protocol metrics) | Covered | TASK-0151 |
| R15 (custom histogram buckets) | Covered | TASK-0150, TASK-0151 |
| R16 (label cardinality) | Covered | TASK-0150, TASK-0151 |
| R17 (relativist_ prefix) | Covered | TASK-0150, TASK-0151 |
| R18 (register method -- SHOULD) | Covered | TASK-0150 (updated: MUST->SHOULD) |
| R19 (HTTP feature gate) | Covered | TASK-0154 |
| R20 (metrics port, axum) | Covered | TASK-0154, TASK-0157 |
| R21 (HTTP routes) | Covered | TASK-0154, TASK-0155, TASK-0156 |
| R22 (readiness probe states) | Covered | TASK-0155 (updated: +Error state) |
| **R22a (AtomicBool readiness flag)** | **Covered (NEW)** | **TASK-0154, TASK-0155, TASK-0157, TASK-0214** |
| R23 (background tokio task) | Covered | TASK-0157 |
| R24 (metrics port binding) | Covered | TASK-0157 |
| **R24a (HTTP graceful shutdown)** | **Covered (NEW)** | **TASK-0157** |
| R25 (otel feature gate) | Covered | TASK-0140, TASK-0158 |
| R26 (OpenTelemetryLayer) | Covered | TASK-0158 |
| R27 (OTLP/HTTP exporter) | Covered | TASK-0158 |
| R28 (OTel resource attributes) | Covered | TASK-0158 (updated: +Local service name) |
| R29 (root span for grid round) | Covered | TASK-0159 |
| R30 (optional trace context) | Covered | TASK-0159 (updated: message names) |
| R31 (init_tracing function) | Covered | TASK-0141, TASK-0142, TASK-0144 |
| R32 (single init, panic on double) | Covered | TASK-0144 |
| R33 (init confirmation log) | Covered | TASK-0144 |
| **R33a (local mode observability)** | **Covered (NEW)** | **TASK-0141, TASK-0142, TASK-0144, TASK-0157, TASK-0158** |
| R34 (no log rotation) | Covered | Exclusion -- no task needed |
| R35 (no alerting/dashboards) | Covered | Exclusion -- no task needed |
| R36 (no custom OTel metrics) | Covered | Exclusion -- no task needed |
| R37 (no dynamic log levels) | Covered | Exclusion -- no task needed |
| T1 (init without panic) | Covered | TASK-0144 |
| T2 (double init panics) | Covered | TASK-0144 |
| T3 (JSON output valid) | Covered | TASK-0144 |
| T4 (RUST_LOG override) | Covered | TASK-0144, TASK-0143 |
| T5 (CoordinatorMetrics verification) | Covered | TASK-0150 (updated: +interactions_by_rule_total) |
| T6 (GET /health) | Covered | TASK-0155 |
| T7 (GET /ready states) | Covered | TASK-0155 (updated: +Error state), TASK-0214 |
| T8 (GET /metrics parseable) | Covered | TASK-0156 |
| **T8a (GET /metrics Content-Type)** | **Covered (NEW)** | **TASK-0156** |
| T9 (metrics feature gate) | Covered | TASK-0140 |

**Coverage:** All 37 MUST/SHOULD requirements and 10 test requirements have at least one task mapped. No orphan tasks or uncovered requirements remain.

---

## 5. Cross-Task Impact Outside Phase 8

| External Task | Impact | Action |
|---------------|--------|--------|
| TASK-0061 (WorkerRoundStats, SPEC-05) | Upstream dependency for TASK-0152 | No modification needed; extension is additive |
| TASK-0108 (Coordinator FSM, SPEC-13) | TASK-0214 adds AtomicBool writes at transition points | TASK-0214 depends on TASK-0108 |
| TASK-0082 (Message enum, SPEC-06) | TASK-0159 uses corrected variant names | Updated in TASK-0159 |
| TASK-0090 (Coordinator collect, SPEC-06) | Calls TASK-0153 with WorkerRoundStats | Naturally compatible |

---

## 6. Changes NOT Made (and why)

| Item | Reason not changed |
|------|--------------------|
| TASK-0140 | No spec changes affect module structure or dependencies |
| TASK-0143 | Default filter string unchanged; R5 provisional note on `relativist::security` is informational |
| TASK-0145 | `split()` span fields (`input_agents`, `k`, `strategy`) unchanged |
| TASK-0147 | `merge()` span fields (`partition_count`, `border_redexes`) unchanged |
| TASK-0149 | FSM transition logging format unchanged; Error state transitions covered by R9a (TASK-0213) |
| TASK-0117 | SPEC-13 task, not modified by SPEC-11 changes |
| TASK-0118 | SPEC-13 task, not modified by SPEC-11 changes |
| TASK-0061 | SPEC-05 task; SPEC-11 extension is handled by TASK-0152 |
