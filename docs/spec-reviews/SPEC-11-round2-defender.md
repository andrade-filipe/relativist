# SPEC-11 — Round 2: Defender Response

**Date:** 2026-04-05
**Defender:** Especialista em Specs
**Target:** SPEC-11-observability.md
**Critic review:** SPEC-11-round1-critic.md
**Spec version:** Draft v1 → Revised v2

---

## Summary

| Response | Count |
|----------|-------|
| ACCEPTED | 13 |
| PARTIALLY ACCEPTED | 3 |
| NOT ADDRESSED | 0 |
| **Total issues** | **16** |

---

## Responses

### SC-001: Message variant names do not match SPEC-06
**Response:** ACCEPTED
**Action taken:** Replaced all occurrences of `ReturnPartition` with `PartitionResult` and all occurrences of `DispatchPartition` with `AssignPartition` throughout the spec. These are the canonical variant names defined in SPEC-06 R2/R3 and Section 4.1. The phantom names were a clear error -- the spec was written referencing conceptual names rather than the normative SPEC-06 `Message` enum.
**Spec sections modified:** Section 3.2 (R13), Section 3.4 (R30), Section 4.4, Section 5

---

### SC-002: `WorkerMetricsReport` duplicates and conflicts with `WorkerRoundStats`
**Response:** ACCEPTED
**Action taken:** Removed the standalone `WorkerMetricsReport` type entirely. Instead, adopted the critic's preferred option (a): extend the canonical `WorkerRoundStats` from SPEC-05 R37 with the additional fields `reduce_duration_secs: f64` and `interactions_by_rule: [u64; 6]`. The extended type definition is provided in Section 4.4 with clear annotations showing which fields come from SPEC-05 R37 (canonical) and which are added by SPEC-11 for observability. A note states that this extension SHOULD be reflected in SPEC-05 R37 during its next revision cycle, and an Open Question (OQ-1) tracks this cross-spec dependency.

This approach was chosen over option (b) (companion struct) because: (1) a single type is simpler for the implementer, (2) the `Message::PartitionResult` variant already carries `stats: WorkerRoundStats`, so no message schema change is needed, and (3) the additional fields are genuinely "round stats" (timing, per-rule counts) rather than a separate concern.
**Spec sections modified:** Section 4.4, Section 8 (Open Questions: OQ-1)

---

### SC-003: `Error` FSM state omitted from readiness logic
**Response:** ACCEPTED
**Action taken:** Updated R22 to explicitly list the `Error` state as returning 503 Service Unavailable, alongside `Init`. The requirement now enumerates all 7 states that return 200 (`WaitingForWorkers` through `Done`) and the 2 states that return 503 (`Init`, `Error`). Added new requirement R22a specifying that the readiness check MUST use an `AtomicBool` (`is_ready`) rather than ordinal comparison. Updated T7 to verify that `Error` state returns 503. Updated the design code in Section 4.3 to use `AtomicBool` (see also SC-012 response).
**Spec sections modified:** Section 3.3 (R22, new R22a), Section 4.3 (ready_handler), Section 7 (T7)

---

### SC-004: R18 has contradictory RFC 2119 keywords
**Response:** ACCEPTED
**Action taken:** Resolved the contradiction by changing the body text from "MUST" to "SHOULD", matching the severity tag. The rationale is that metrics registration is an internal implementation detail -- the coordinator could register metrics inline rather than via a dedicated `register()` method. Added "The coordinator MAY register metrics inline instead" to make the alternative explicit.
**Spec sections modified:** Section 3.2 (R18)

---

### SC-005: `--metrics-port` CLI argument not declared in SPEC-07 or SPEC-13
**Response:** ACCEPTED
**Action taken:** Added a cross-reference note to R20 stating: "This argument MUST be added to the `CoordinatorArgs` struct defined in SPEC-13 R44, conditional on the `metrics` feature being enabled." Since this spec cannot edit SPEC-13, an Open Question (OQ-2) was added to Section 8 tracking the cross-spec dependency. The critic's suggestion to rename to `--prometheus-port` was not adopted because `--metrics-port` is the more common convention in the ecosystem (Kubernetes, Prometheus exporters, etc.) and is unambiguous in context since `--metrics <PATH>` (SPEC-07 R3) accepts a file path while `--metrics-port` accepts a port number -- different types that are not confusable in practice.
**Spec sections modified:** Section 3.3 (R20), Section 8 (Open Questions: OQ-2)

---

### SC-006: Content-Type `application/openmetrics-text` may be incorrect
**Response:** ACCEPTED
**Action taken:** Updated the Content-Type in R21's route table from `application/openmetrics-text` to `application/openmetrics-text; version=1.0.0; charset=utf-8` per the OpenMetrics specification. Updated the design code in Section 4.3 to define a `OPENMETRICS_CONTENT_TYPE` constant with the full string and use it in the response header.
**Spec sections modified:** Section 3.3 (R21), Section 4.3 (metrics_handler, OPENMETRICS_CONTENT_TYPE constant)

---

### SC-007: No ERROR-level logging requirements specified
**Response:** ACCEPTED
**Action taken:** Added new requirement R9a specifying that the following events MUST be logged at ERROR level: invariant violations detected by `assert_all_invariants()` (SPEC-01 T1-T7), protocol errors (checksum mismatch, message too large, connection lost), coordinator/worker FSM transitions to `Error` state, any `FatalError` event, and merge failures (unresolved borders, merge invariant violations). The requirement is placed in Section 3.1 immediately after R9, organized as a table mapping event categories to conditions and modules.
**Spec sections modified:** Section 3.1 (new R9a)

---

### SC-008: Worker-side metrics for Prometheus not aggregated per rule
**Response:** ACCEPTED
**Action taken:** Added `interactions_by_rule_total: Family<Vec<(String, String)>, Counter>` to the `CoordinatorMetrics` struct in R12. This metric uses the `rule` label (with cardinality 6, already approved in R16) to aggregate per-rule interaction counts from workers across all rounds. The `Family` type from `prometheus-client` is used with a label set containing a single `("rule", rule_name)` pair. This closes the gap where R16 allowed `rule` as a label but no metric existed to use it.
**Spec sections modified:** Section 3.2 (R12 -- `CoordinatorMetrics` struct)

---

### SC-009: `--log-format` CLI argument not declared in predecessor specs
**Response:** ACCEPTED
**Action taken:** Added a cross-reference note to R3 stating: "The `--log-format` argument MUST be added to both `CoordinatorArgs` and `WorkerArgs` structs defined in SPEC-13 R44/R45." This is tracked alongside `--metrics-port` in OQ-2 (Section 8). Since SPEC-11 cannot edit SPEC-13, this note ensures the implementer knows to add the argument when building the CLI.
**Spec sections modified:** Section 3.1 (R3), Section 8 (Open Questions: OQ-2)

---

### SC-010: `relativist::security` target in default log filter but no security spec referenced
**Response:** ACCEPTED
**Action taken:** Added a parenthetical note to the `relativist::security` row in R5's log level table: "(provisional; MAY be removed if the security module is not implemented -- see SPEC-13 R5)". This acknowledges that the security module is specified in SPEC-13 R5 as part of the module structure but has no dedicated behavioral spec. If the module is not implemented, the filter target is harmless (no events emitted) but should be removed for cleanliness.
**Spec sections modified:** Section 3.1 (R5 table)

---

### SC-011: Metrics handler does not set Content-Type header in design code
**Response:** ACCEPTED
**Action taken:** Rewrote `metrics_handler` in Section 4.3 to return `([(axum::http::header::CONTENT_TYPE, OPENMETRICS_CONTENT_TYPE)], String)` instead of bare `String`. This sets the Content-Type header explicitly using axum's tuple response pattern. Added test requirement T8a that verifies the `Content-Type` header contains `application/openmetrics-text`, separate from T8 which checks body parseability.
**Spec sections modified:** Section 4.3 (metrics_handler), Section 7 (new T8a)

---

### SC-012: `ready_handler` uses `AtomicU8` ordinal comparison -- brittle encoding
**Response:** ACCEPTED
**Action taken:** Replaced the `AtomicU8` ordinal approach with `AtomicBool` named `is_ready`. The `metrics_router` function now accepts `coordinator_is_ready: Arc<AtomicBool>` instead of `coordinator_state: Arc<AtomicU8>`. The `ready_handler` now simply loads the boolean. New requirement R22a specifies that the coordinator FSM MUST set `is_ready = true` on `Init -> WaitingForWorkers` transition and `is_ready = false` on any transition to `Error` state. This is robust against enum reordering and correctly handles the `Error` state (addresses SC-003 simultaneously).
**Spec sections modified:** Section 3.3 (new R22a), Section 4.3 (metrics_router signature, AppState, ready_handler)

---

### SC-013: No observability for local mode (`relativist reduce`)
**Response:** ACCEPTED
**Action taken:** Added `ProcessRole::Local` variant to the `ProcessRole` enum in Section 3.5. Added new requirement R33a specifying that: (a) `init_tracing()` MUST be called with `ProcessRole::Local` in local mode, (b) structured logging (R1-R9, R9a) applies to all modes including local, (c) HTTP endpoints MUST NOT be started in local mode, and (d) OTel `service.name` MUST be `"relativist-local"` when role is `Local`. This ensures observability behavior is well-defined for all execution modes.
**Spec sections modified:** Section 3.5 (R31 code block -- ProcessRole enum, new R33a)

---

### SC-014: No graceful shutdown for the HTTP server
**Response:** ACCEPTED
**Action taken:** Added new requirement R24a specifying that the HTTP server MUST be shut down when the coordinator FSM enters `Done` or `Error` state. The coordinator SHOULD use a `tokio_util::sync::CancellationToken` (or equivalent mechanism) to signal the background HTTP task to terminate. The mechanism is SHOULD (not MUST) to leave the implementer free to use `JoinHandle::abort()` or another approach if more appropriate.
**Spec sections modified:** Section 3.3 (new R24a)

---

### SC-015: Test T5 is vague about "one simulated grid round"
**Response:** PARTIALLY ACCEPTED
**Action taken:** Rewrote T5 to specify the exact test setup: (a) create a `CoordinatorMetrics` instance and register it in a `Registry`, (b) call metric update methods directly (e.g., `metrics.rounds_total.inc()`, `metrics.round_duration_seconds.observe(1.5)`, `metrics.interactions_by_rule_total.get_or_create(&rule_label).inc()`), (c) assert specific values (`rounds_total.get() == 1`, etc.). The test is explicitly decoupled from the full grid cycle.

The fix differs from the critic's suggestion in one aspect: the critic suggested asserting only `rounds_total` and `round_duration_seconds`, but the revised T5 also includes `interactions_by_rule_total` to validate the new per-rule counter added by SC-008. The "counter values > 0 for all counters" criterion was dropped as the critic correctly noted some counters (e.g., `border_redexes`) could legitimately be 0.
**Spec sections modified:** Section 7 (T5)

---

### SC-016: Span fields in R6 are inconsistent with SPEC-05 and SPEC-06 terminology
**Response:** PARTIALLY ACCEPTED
**Action taken:** Renamed `partition_id` to `partition_index` in all span field references (R6, R8, Section 4.5 span hierarchy example, Section 6) to distinguish from the prohibited Prometheus label. The "(0..k position within round)" annotation was added to clarify semantics.

However, the critic's concern about the inconsistency between span fields and Prometheus labels is addressed by noting in R16 that `partition_index` is "acceptable as span field since spans are ephemeral" -- span fields and Prometheus labels serve fundamentally different purposes with different cardinality constraints, so the same name can appear in one context but not the other. The rename from `_id` to `_index` makes this distinction clearer.

The critic's suggestion to add explanatory text about the difference between span fields and Prometheus labels was partially adopted: the R16 table now includes the note directly in the "Acceptable" column rather than as a separate paragraph.
**Spec sections modified:** Section 3.1 (R6, R8), Section 3.2 (R16), Section 4.5, Section 6

---

## Changes Made to SPEC-11

### Header
- Status changed from "Draft v1" to "Revised v2"

### Section 3.1 (Structured Logging)
- R3: Added cross-reference note about `--log-format` in SPEC-13 R44/R45
- R5: Added "(provisional; MAY be removed...)" note for `relativist::security`
- R6: Renamed `partition_id` to `partition_index` with "(0..k position within round)" annotation
- R8: Renamed `partition_id` to `partition_index` in recommended fields table
- New R9a: ERROR-level logging requirements for invariant violations, protocol errors, FSM error transitions, fatal errors, and merge failures

### Section 3.2 (Metrics)
- R12: Added `interactions_by_rule_total: Family<Vec<(String, String)>, Counter>` to `CoordinatorMetrics` struct
- R13: Fixed `ReturnPartition` -> `PartitionResult`
- R16: Renamed `partition_id` to `partition_index`; added note about span field acceptability
- R18: Changed body text from "MUST" to "SHOULD"; added "MAY register inline" alternative

### Section 3.3 (HTTP Endpoints)
- R20: Added cross-reference note about `--metrics-port` in SPEC-13 R44
- R21: Updated Content-Type to `application/openmetrics-text; version=1.0.0; charset=utf-8`
- R22: Added `Error` state to the 503 response list; enumerated all 7 ready states explicitly
- New R22a: Explicit `AtomicBool` readiness flag requirement
- New R24a: HTTP server graceful shutdown requirement

### Section 3.4 (Distributed Tracing)
- R30: Fixed `DispatchPartition` -> `AssignPartition`, `ReturnPartition` -> `PartitionResult`

### Section 3.5 (Initialization)
- Added `ProcessRole::Local` variant to the enum
- New R33a: Local mode observability behavior specification

### Section 4.3 (Metrics HTTP Router)
- Changed `coordinator_state: Arc<AtomicU8>` parameter to `coordinator_is_ready: Arc<AtomicBool>`
- Changed `AppState.coordinator_state` to `AppState.is_ready` with `AtomicBool`
- Added `OPENMETRICS_CONTENT_TYPE` constant
- Changed `metrics_handler` return type to include Content-Type header tuple
- Changed `ready_handler` to use `AtomicBool` load instead of ordinal comparison

### Section 4.4 (Worker Metrics Reporting)
- Removed `WorkerMetricsReport` struct entirely
- Replaced with extended `WorkerRoundStats` that includes both canonical SPEC-05 R37 fields and SPEC-11 observability fields
- Added backward-compatibility note and cross-spec dependency tracking

### Section 4.5 (Span Hierarchy)
- All `partition_id` references renamed to `partition_index`

### Section 5 (Rationale)
- Fixed `ReturnPartition` -> `PartitionResult`

### Section 6 (Haskell Prototype Reference)
- Fixed `partition_id` -> `partition_index` in correlation fields list

### Section 7 (Test Requirements)
- T5: Rewritten with explicit test setup (create, update, assert) decoupled from grid cycle
- T7: Added `Error` state verification (must return 503)
- New T8a: Content-Type header verification for `/metrics` endpoint

### Section 8 (Open Questions)
- Changed from "None" to two tracked cross-spec dependencies (OQ-1: WorkerRoundStats extension, OQ-2: CLI arguments)

---

## Residual Risks

No issues were NOT ADDRESSED. All 16 critic issues received concrete fixes in the spec:

- All 4 CRITICAL/HIGH issues (SC-001, SC-002, SC-003, SC-004) were ACCEPTED with direct fixes.
- All 6 MEDIUM issues (SC-005, SC-007, SC-011, SC-012, SC-003, SC-004) were ACCEPTED.
- All 6 LOW issues (SC-006, SC-008, SC-009, SC-010, SC-013, SC-014, SC-015, SC-016) were ACCEPTED or PARTIALLY ACCEPTED.

The two remaining risks are tracked as Open Questions:

1. **OQ-1 (cross-spec):** The `WorkerRoundStats` extension defined in Section 4.4 has not been propagated to SPEC-05 R37 or SPEC-06 R12. Until those specs are revised, there is a temporary inconsistency where SPEC-11 defines an extended type that differs from the canonical type in predecessor specs. This is acceptable because SPEC-11 is explicitly additive (all original fields preserved) and the extension is marked as the normative definition for observability purposes.

2. **OQ-2 (cross-spec):** The `--metrics-port` and `--log-format` CLI arguments are defined in SPEC-11 but not yet reflected in SPEC-13 R44/R45 or SPEC-07 R3. The cross-reference notes in R3 and R20 ensure the implementer knows to add them, but the CLI argument inventory remains fragmented until the predecessor specs are revised.
