# SPEC-11 — Round 1: Spec Critic Review

**Date:** 2026-04-05
**Reviewer:** Spec Critic (adversarial)
**Target:** SPEC-11-observability.md (status: Draft v1)
**Predecessors consulted:** SPEC-00, SPEC-01, SPEC-05, SPEC-06, SPEC-07, SPEC-13

---

## Overall Assessment

SPEC-11 is a well-structured and generally thorough observability specification that correctly adopts the layered subscriber pattern from the `tracing` ecosystem and makes sensible feature-gating decisions. However, it suffers from several naming inconsistencies with predecessor specs (using phantom message variant names that do not exist in SPEC-06), introduces a `WorkerMetricsReport` type that duplicates and conflicts with the canonical `WorkerRoundStats` from SPEC-05/SPEC-06, omits the `Error` FSM state from its readiness logic, and has a requirement (R18) with contradictory RFC 2119 severity. The metrics exposition Content-Type is also technically incorrect for the `prometheus-client` crate's actual encoding behavior.

**Verdict:** NEEDS REVISION

---

## Issues

### SC-001: Message variant names do not match SPEC-06

**Severity:** HIGH
**Axis:** Consistency
**Section:** Section 3.2 (R13), Section 3.4 (R30), Section 4.4, Section 5
**Requirement:** R13, R30
**Problem:** SPEC-11 references `ReturnPartition` (R13, R30, Section 4.4, Section 5) and `DispatchPartition` (R30) as wire protocol message variants. However, SPEC-06 Section 4.1 defines the `Message` enum with variants `PartitionResult` (not `ReturnPartition`) and `AssignPartition` (not `DispatchPartition`). These phantom names do not exist anywhere in SPEC-06's normative text.
**Impact if unresolved:** An implementer reading SPEC-11 in isolation would search for `ReturnPartition` and `DispatchPartition` variants in the `Message` enum, find them absent, and be confused about where to embed the worker metrics report and the OTel trace context. The spec becomes ambiguous about exactly which protocol messages carry observability data.
**Suggested resolution:** Replace all instances of `ReturnPartition` with `PartitionResult` and all instances of `DispatchPartition` with `AssignPartition` to match SPEC-06 R2/R3 and the canonical `Message` enum in SPEC-06 Section 4.1.

---

### SC-002: `WorkerMetricsReport` duplicates and conflicts with `WorkerRoundStats`

**Severity:** HIGH
**Axis:** Consistency
**Section:** Section 4.4
**Requirement:** R13
**Problem:** SPEC-11 Section 4.4 introduces a new type `WorkerMetricsReport` with fields `reduce_duration_secs: f64`, `redexes_reduced: u64`, and `interactions_by_rule: [u64; 6]`. However, SPEC-05 R37 and SPEC-06 R12 already define the canonical type `WorkerRoundStats` (with fields `worker_id`, `agents_before`, `agents_after`, `local_redexes`). The SPEC-06 `Message::PartitionResult` variant already includes a `stats: WorkerRoundStats` field. SPEC-11 now introduces a second, incompatible type with overlapping purpose but different field names and different granularity (per-rule breakdown vs. aggregate count).

This creates a design conflict: should the worker return `WorkerRoundStats`, `WorkerMetricsReport`, or both? If both, how are they composed in the `PartitionResult` message? If one replaces the other, which predecessor spec needs revision?
**Impact if unresolved:** Two competing types for the same concept. Implementer will not know which to use, which fields are normative, and whether the `Message::PartitionResult` variant needs modification.
**Suggested resolution:** Either (a) extend `WorkerRoundStats` in SPEC-05/SPEC-06 to include the additional fields (`reduce_duration_secs`, `interactions_by_rule`), making SPEC-11 reference the extended type; or (b) define `WorkerMetricsReport` as a companion struct alongside `WorkerRoundStats` in the `PartitionResult` message, with clear text stating that `WorkerRoundStats` carries grid-cycle metrics (for `GridMetrics`) and `WorkerMetricsReport` carries Prometheus-specific metrics. Option (a) is preferred for simplicity.

---

### SC-003: `Error` FSM state omitted from readiness logic

**Severity:** MEDIUM
**Axis:** Completeness
**Section:** Section 3.3 (R22), Section 4.3
**Requirement:** R22
**Problem:** R22 enumerates the coordinator FSM states from SPEC-13 R19 where `/ready` returns 200: `WaitingForWorkers`, `Partitioning`, `Dispatching`, `WaitingForResults`, `Merging`, `CheckTermination`, `Done`. This list omits the `Error` state, which is defined in SPEC-13 R19 and can be reached from any state via `FatalError`. The specification does not say what `/ready` should return when the coordinator is in `Error` state. The implementation in Section 4.3 uses `state_val >= 1`, which would return 200 for the `Error` state if it has a numeric value >= 1.
**Impact if unresolved:** If the coordinator enters `Error` state, the `/ready` endpoint would report "ready" to a health-check system (e.g., Kubernetes, Docker Compose), which would not trigger a restart. The endpoint becomes semantically incorrect: the process is alive but NOT ready to perform useful work.
**Suggested resolution:** Explicitly state in R22 that `/ready` MUST return 503 for both `Init` and `Error` states. Update the Section 4.3 implementation to check for `Error` state explicitly rather than relying on ordinal comparison. This likely means the `AtomicU8` approach needs to be replaced or augmented with a separate "healthy" flag, since ordinal comparison (`>= 1`) cannot distinguish `Error` from valid working states.

---

### SC-004: R18 has contradictory RFC 2119 keywords

**Severity:** MEDIUM
**Axis:** Consistency
**Section:** Section 3.2
**Requirement:** R18
**Problem:** R18 reads: "The `CoordinatorMetrics` struct **MUST** provide a `register(registry: &mut Registry)` method that registers all metrics with their descriptions. **(SHOULD)**". The body text uses "MUST" (mandatory) while the severity tag says "(SHOULD)" (recommended). These are contradictory RFC 2119 levels.
**Impact if unresolved:** An implementer cannot determine whether the `register()` method is mandatory or recommended. If MUST, it is a blocking requirement. If SHOULD, it can be skipped with justification.
**Suggested resolution:** Decide which level is intended. Given that metrics registration is an internal implementation detail and the coordinator could register metrics inline rather than via a dedicated method, SHOULD seems appropriate. Change the body text from "MUST" to "SHOULD".

---

### SC-005: `--metrics-port` CLI argument not declared in SPEC-07 or SPEC-13

**Severity:** MEDIUM
**Axis:** Completeness
**Section:** Section 3.3 (R20)
**Requirement:** R20
**Problem:** R20 specifies that the metrics port is "configurable via `--metrics-port`". However, neither SPEC-07 R3 (coordinator CLI arguments) nor SPEC-13 R44 (coordinator subcommand arguments) include `--metrics-port` in their argument lists. SPEC-07 defines `--metrics <PATH>` (path for output file), which is a different argument entirely. The `--metrics-port` argument exists only in SPEC-11.
**Impact if unresolved:** The CLI argument inventory is inconsistent across specs. An implementer following SPEC-07/SPEC-13 for CLI definition would miss `--metrics-port`. Additionally, there is a potential user confusion between `--metrics` (file path, SPEC-07) and `--metrics-port` (HTTP port, SPEC-11).
**Suggested resolution:** (a) Add `--metrics-port <PORT>` to SPEC-07 R3 and SPEC-13 R44 as an optional argument, conditional on the `metrics` feature. (b) Consider renaming to `--prometheus-port` or `--observability-port` to disambiguate from `--metrics <PATH>`.

---

### SC-006: Content-Type `application/openmetrics-text` may be incorrect

**Severity:** LOW
**Axis:** Completeness
**Section:** Section 3.3 (R21)
**Requirement:** R21
**Problem:** R21 specifies the Content-Type for the `/metrics` endpoint as `application/openmetrics-text`. However, the `prometheus-client` crate's `text::encode()` function (used in Section 4.3) produces OpenMetrics text format, whose correct Content-Type is `application/openmetrics-text; version=1.0.0; charset=utf-8` (per the OpenMetrics specification). The simplified Content-Type without the version and charset parameters may cause some Prometheus server versions to fall back to the classic exposition format parser, potentially misinterpreting metric types. Additionally, the metrics handler in Section 4.3 returns a bare `String` without setting the Content-Type header, which would default to `text/plain` in axum.
**Impact if unresolved:** The `/metrics` endpoint may serve content with the wrong Content-Type header (or no explicit Content-Type), causing Prometheus scrape warnings or misinterpretation of metric types.
**Suggested resolution:** (a) Specify the full Content-Type string including version and charset. (b) Update the `metrics_handler` in Section 4.3 to return a response with explicit Content-Type header rather than a bare `String`.

---

### SC-007: No ERROR-level logging requirements specified

**Severity:** MEDIUM
**Axis:** Completeness
**Section:** Section 3.1
**Requirement:** (missing)
**Problem:** SPEC-11 specifies INFO-level logging for state transitions (R7) and default log levels per module (R5), but never specifies when ERROR-level events should be emitted. The `tracing` level hierarchy includes ERROR as the highest severity, yet no requirement mandates its use. Critical situations such as invariant violations (SPEC-01 T1-T7), protocol checksum mismatches (SPEC-06 R29), merge failures, or unexpected disconnections have no logging requirement at ERROR level. The default filter in R5 only goes down to WARN.
**Impact if unresolved:** In production, genuine errors (invariant violations, connection losses, merge failures) may be logged at WARN level or lower, making them difficult to find. Error-level events are the standard severity for actionable failures.
**Suggested resolution:** Add a requirement (e.g., R7a) specifying that the following events MUST be logged at ERROR level: invariant violations detected by `assert_all_invariants()`, protocol errors (checksum mismatch, message too large, connection lost), coordinator/worker FSM transitions to the `Error` state, and any `FatalError` events.

---

### SC-008: Worker-side metrics for Prometheus not aggregated per rule

**Severity:** LOW
**Axis:** Completeness
**Section:** Section 3.2 (R12, R13), Section 4.4
**Requirement:** R12, R13
**Problem:** The `CoordinatorMetrics` struct (R12) does not include any per-rule interaction counter. However, the `WorkerMetricsReport` (Section 4.4) reports `interactions_by_rule: [u64; 6]` from each worker. R13 says "The coordinator MUST aggregate worker metrics into its own registry," but there is no corresponding Prometheus metric in R12 to receive this per-rule data. The `CoordinatorMetrics` struct has no `interactions_by_rule_total` counter with a `rule` label.

Meanwhile, R16 explicitly allows `rule` as a label with cardinality 6, suggesting the intent was to have per-rule metrics, but the actual metric definition is missing.
**Impact if unresolved:** Per-rule interaction data from workers would be received by the coordinator but discarded because there is no Prometheus metric to aggregate it into. The `rule` label allowance in R16 becomes dead specification.
**Suggested resolution:** Add a counter metric to `CoordinatorMetrics`:
```rust
/// Total interactions by rule type, across all workers and rounds.
pub interactions_by_rule_total: Family<RuleLabel, Counter>,
```
where `RuleLabel` has variants matching the 6 rules.

---

### SC-009: `--log-format` CLI argument not declared in predecessor specs

**Severity:** LOW
**Axis:** Completeness
**Section:** Section 3.1 (R3)
**Requirement:** R3
**Problem:** R3 specifies `--log-format text` and `--log-format json` as CLI selectors for the log output format. Like `--metrics-port` (SC-005), this argument is not declared in SPEC-07 or SPEC-13's CLI argument lists. SPEC-11 introduces it but no predecessor spec accounts for it.
**Impact if unresolved:** CLI argument inventory is fragmented across specs. An implementer building the `clap` argument parser from SPEC-07/SPEC-13 would miss `--log-format`.
**Suggested resolution:** Either (a) add `--log-format` to SPEC-07/SPEC-13 CLI argument lists, or (b) add a note in SPEC-11 R3 stating that this argument MUST be added to the `CoordinatorArgs` and `WorkerArgs` structs defined in SPEC-13.

---

### SC-010: `relativist::security` target in default log filter but no security spec referenced

**Severity:** LOW
**Axis:** Consistency
**Section:** Section 3.1 (R5)
**Requirement:** R5
**Problem:** R5 includes `relativist::security` in the default log level table with "Auth events, TLS status" as rationale. However, SPEC-11's dependency list does not include any security spec (the project does not have a formal SPEC for security -- SPEC-10 through SPEC-13 are the newest specs). The `security` module appears in SPEC-13 R5 as a module, but its behavior is not specified by any dedicated spec. SPEC-11 assumes this module exists and produces log events, but this is ungrounded.
**Impact if unresolved:** Minor: the default filter string includes a target that may not produce any events if the security module is not implemented. This is harmless but represents specification of behavior for an unspecified module.
**Suggested resolution:** Either (a) note that `relativist::security` is included provisionally and MAY be removed if the security module is not implemented, or (b) reference SPEC-10 (if it covers security) or SPEC-13 R5 as the source of the module definition.

---

### SC-011: Metrics handler does not set Content-Type header in design code

**Severity:** MEDIUM
**Axis:** Testability
**Section:** Section 4.3
**Requirement:** R21
**Problem:** The `metrics_handler` function in Section 4.3 returns `String`, which in axum defaults to `Content-Type: text/plain; charset=utf-8`. However, R21 specifies the Content-Type as `application/openmetrics-text`. The design code does not set the Content-Type header, making the implementation non-conformant with R21 as written. Test T8 ("response parseable as Prometheus text exposition format") could pass even with the wrong Content-Type, since parsing is content-based, not header-based.
**Impact if unresolved:** The implementation would serve metrics with the wrong Content-Type, potentially confusing Prometheus scrapers. T8 would not catch this because it only checks parseability.
**Suggested resolution:** (a) Update `metrics_handler` to return `([(header::CONTENT_TYPE, "application/openmetrics-text; ...")], body)` or an axum `Response` with explicit headers. (b) Add a test requirement (e.g., T8a) that verifies the Content-Type header, not just body parseability.

---

### SC-012: `ready_handler` uses `AtomicU8` ordinal comparison -- brittle encoding

**Severity:** MEDIUM
**Axis:** Testability
**Section:** Section 4.3
**Requirement:** R22
**Problem:** The `ready_handler` implementation uses `coordinator_state.load(Ordering::Relaxed)` and compares `state_val >= 1` to determine readiness. This assumes a specific numeric encoding where `Init = 0` and all "ready" states have values >= 1. However, the `CoordinatorState` enum in SPEC-13 R19 has 9 variants including `Error`. The ordinal assignment depends on the Rust enum discriminant order, which is fragile: reordering the variants changes the behavior. Moreover, adding new states in the future could break the comparison silently.

This also ties into SC-003 (the `Error` state would pass the `>= 1` check).
**Impact if unresolved:** Fragile implementation that silently breaks if the `CoordinatorState` enum is reordered. Combined with SC-003, the `Error` state would be reported as "ready".
**Suggested resolution:** Replace the `AtomicU8` ordinal approach with either (a) an `AtomicBool` named `is_ready` that is explicitly set to `true` when entering `WaitingForWorkers` and set to `false` when entering `Init` or `Error`, or (b) an `Arc<Mutex<CoordinatorState>>` that the handler matches on explicitly. The specification should prescribe the semantic contract, not the numeric encoding.

---

### SC-013: No observability for local mode (`relativist reduce`)

**Severity:** LOW
**Axis:** Completeness
**Section:** All
**Requirement:** (missing)
**Problem:** SPEC-11 focuses entirely on coordinator/worker observability in distributed mode. SPEC-13 R41 specifies a local execution mode (`relativist reduce`) that bypasses the coordinator/worker infrastructure. SPEC-11 does not specify whether the local mode should initialize `tracing` (R31), whether it should expose HTTP endpoints (presumably not), or whether it should report any metrics. The `init_tracing()` function signature includes `ProcessRole` (Coordinator/Worker) but has no variant for local mode.
**Impact if unresolved:** An implementer would not know how to initialize observability in local mode. Should `init_tracing()` be called? With which `ProcessRole`? Should `RUST_LOG` filtering work in local mode?
**Suggested resolution:** (a) Add a `ProcessRole::Local` variant. (b) State that `init_tracing()` MUST be called in local mode with `ProcessRole::Local`. (c) State that HTTP endpoints MUST NOT be started in local mode (no coordinator to attach them to). (d) State that structured logging (R1-R9) applies to all modes.

---

### SC-014: No graceful shutdown for the HTTP server

**Severity:** LOW
**Axis:** Completeness
**Section:** Section 3.3 (R23)
**Requirement:** R23
**Problem:** R23 specifies that the axum HTTP server "MUST run as a background tokio task" and "MUST NOT block the grid protocol event loop." However, there is no requirement for graceful shutdown of the HTTP server when the coordinator enters `Done` or `Error` state. The background task could outlive the coordinator, keeping the process alive after reduction is complete.
**Impact if unresolved:** The process may hang after completing reduction because the HTTP server task is still listening. In Docker deployments, the container would not exit cleanly.
**Suggested resolution:** Add a requirement that the HTTP server MUST be shut down (via a cancellation token or `JoinHandle::abort()`) when the coordinator FSM enters `Done` or `Error` state.

---

### SC-015: Test T5 is vague about "one simulated grid round"

**Severity:** LOW
**Axis:** Testability
**Section:** Section 7
**Requirement:** T5
**Problem:** T5 states "the `CoordinatorMetrics` registry MUST contain all expected metrics after one simulated grid round (counter values > 0, histogram observations > 0)." However, "one simulated grid round" is not defined in test terms. Does this mean running the full coordinator FSM with a `ChannelTransport`? Manually incrementing counters? Using a test harness that simulates events? The assertion "counter values > 0" is also imprecise: which counters? All of them? Some counters (like `border_redexes`) could legitimately be 0 after a round with no border redexes.
**Impact if unresolved:** The test is ambiguous and may not be reliably implemented. Different implementers would write different tests.
**Suggested resolution:** Specify the test setup: (a) create a `CoordinatorMetrics` instance, (b) simulate one BSP round by calling the metric update methods directly (e.g., `metrics.rounds_total.inc()`, `metrics.round_duration_seconds.observe(1.5)`), (c) assert that `rounds_total.get() == 1` and `round_duration_seconds` has 1 observation. This decouples the test from the full grid cycle.

---

### SC-016: Span fields in R6 are inconsistent with SPEC-05 and SPEC-06 terminology

**Severity:** LOW
**Axis:** Consistency
**Section:** Section 3.1 (R6)
**Requirement:** R6
**Problem:** R6 lists `partition_id` as a span field for `reduce_all()`. However, SPEC-05's `WorkerRoundStats` and SPEC-04's `Partition` struct use `worker_id` to identify the partition assignment, not `partition_id`. The term `partition_id` does not appear as a field in any predecessor spec's type definition. R16 in Section 3.2 also flags `partition_id` as unbounded and explicitly says "DO NOT use as label." There is a conceptual inconsistency: if `partition_id` should not be a Prometheus label (R16), why is it a span field (R6)?
**Impact if unresolved:** Minor confusion about naming. The span field is harmless (span fields are not Prometheus labels), but the inconsistency makes the spec harder to follow.
**Suggested resolution:** (a) Clarify in R6 that `partition_id` refers to the partition's position index within the round (0..k), not an unbounded ID. (b) Consider renaming to `partition_index` to distinguish from the prohibited Prometheus label. (c) Note that span fields and Prometheus labels serve different purposes and have different cardinality constraints.

---

## Summary Table

| ID | Severity | Axis | Requirement | Short Description |
|----|----------|------|-------------|-------------------|
| SC-001 | HIGH | Consistency | R13, R30 | Message variant names (`ReturnPartition`, `DispatchPartition`) do not exist in SPEC-06 |
| SC-002 | HIGH | Consistency | R13 | `WorkerMetricsReport` duplicates/conflicts with canonical `WorkerRoundStats` |
| SC-003 | MEDIUM | Completeness | R22 | `Error` FSM state omitted from `/ready` logic |
| SC-004 | MEDIUM | Consistency | R18 | Contradictory RFC 2119 keywords (MUST in body, SHOULD in tag) |
| SC-005 | MEDIUM | Completeness | R20 | `--metrics-port` CLI arg absent from SPEC-07/SPEC-13 |
| SC-006 | LOW | Completeness | R21 | Content-Type lacks version/charset parameters |
| SC-007 | MEDIUM | Completeness | (missing) | No ERROR-level logging requirements |
| SC-008 | LOW | Completeness | R12, R13 | Per-rule interaction counter missing from `CoordinatorMetrics` |
| SC-009 | LOW | Completeness | R3 | `--log-format` CLI arg absent from SPEC-07/SPEC-13 |
| SC-010 | LOW | Consistency | R5 | `relativist::security` target references unspecified module |
| SC-011 | MEDIUM | Testability | R21 | Design code does not set Content-Type header; test T8 would not catch it |
| SC-012 | MEDIUM | Testability | R22 | `AtomicU8` ordinal comparison is brittle; does not exclude `Error` state |
| SC-013 | LOW | Completeness | (missing) | No observability spec for local mode (`relativist reduce`) |
| SC-014 | LOW | Completeness | R23 | No graceful shutdown for HTTP server |
| SC-015 | LOW | Testability | T5 | "Simulated grid round" is vague; assertion criteria imprecise |
| SC-016 | LOW | Consistency | R6 | `partition_id` span field vs. prohibited Prometheus label; no predecessor type defines it |

---

## Mandatory Fixes (MUST resolve before approval)

1. **SC-001:** Fix message variant names to match SPEC-06 (`PartitionResult`, `AssignPartition`).
2. **SC-002:** Reconcile `WorkerMetricsReport` with `WorkerRoundStats` -- either extend the canonical type or clearly define both with non-overlapping responsibilities.
3. **SC-003:** Specify `/ready` behavior for `Error` state (MUST return 503).
4. **SC-004:** Resolve RFC 2119 contradiction in R18.

## Recommended Fixes (SHOULD resolve)

5. **SC-005:** Declare `--metrics-port` in SPEC-07/SPEC-13 or note its absence explicitly.
6. **SC-007:** Add ERROR-level logging requirements for invariant violations, protocol errors, and FSM error transitions.
7. **SC-011:** Fix `metrics_handler` to set Content-Type and add a test for the header.
8. **SC-012:** Replace `AtomicU8` ordinal approach with explicit readiness semantics.
9. **SC-013:** Add `ProcessRole::Local` and specify observability behavior in local mode.

## May Fix (quality improvements)

10. **SC-006:** Full Content-Type with version/charset parameters.
11. **SC-008:** Add per-rule interaction counter to `CoordinatorMetrics`.
12. **SC-009:** Declare `--log-format` in predecessor specs.
13. **SC-010:** Note that `relativist::security` is provisional.
14. **SC-014:** Add HTTP server shutdown requirement.
15. **SC-015:** Clarify T5 test setup.
16. **SC-016:** Clarify `partition_id` terminology.

---

## Invariant Preservation Checklist

| Check | Result |
|-------|--------|
| Observability does not affect reduction semantics (T4, T5, T6) | PASS -- all logging/metrics are side-effect-free read operations; no spec requirement modifies net state |
| Feature gates ensure zero overhead when disabled | PASS -- R10, R25 explicitly require zero compiled code when features disabled |
| `tracing` instrumentation does not introduce shared mutable state between workers | PASS -- `tracing` spans are thread-local; metrics updates are atomic counters |
| Metrics collection does not violate BSP barrier semantics (SPEC-13 R2) | PASS -- worker metrics piggyback on existing `PartitionResult` message, not a separate communication phase |
| Histogram observations are not subject to label explosion (SPEC-11 R16) | PASS -- R16 explicitly bounds label cardinality and prohibits unbounded labels |

## Cross-Spec Consistency Checklist

| Check | Result |
|-------|--------|
| Terms match SPEC-00 glossary | PASS -- Agent, Net, Active Pair, etc. used correctly |
| Type names match SPEC-02/SPEC-04/SPEC-05 | FAIL -- `WorkerMetricsReport` conflicts with `WorkerRoundStats` (SC-002) |
| Message variant names match SPEC-06 | FAIL -- `ReturnPartition`/`DispatchPartition` are phantom names (SC-001) |
| FSM states match SPEC-13 R19 | PARTIAL FAIL -- `Error` state omitted from readiness logic (SC-003) |
| CLI arguments consistent with SPEC-07/SPEC-13 | FAIL -- `--metrics-port` and `--log-format` not declared (SC-005, SC-009) |
| Feature flag names match SPEC-13 R37 | PASS -- `metrics`, `otel` match exactly |
| Dependency crates match SPEC-13 R11/R12 | PASS -- `prometheus-client`, `axum`, `opentelemetry-*` all declared in SPEC-13 R12 |
| Log levels follow `tracing` conventions (TRACE < DEBUG < INFO < WARN < ERROR) | PASS -- R5 table uses standard levels |
