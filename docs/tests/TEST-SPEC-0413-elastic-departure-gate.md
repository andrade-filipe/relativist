# TEST-SPEC-0413 — Elastic Departure Gate (SPEC-06 R25 / SPEC-20 §3.8 A1)

**Task:** TASK-0413
**File:** `relativist-core/src/protocol/coordinator.rs` (inline `#[cfg(test)] mod tests`)
**Stage:** backfilled at Stage 6 REFACTOR (RV-006 / QA-TASK-0413-2026-04-25)

## Scope

Unit tests for the two pure gate helpers added by TASK-0413:
- `handle_connection_loss(worker_id, error_description, elastic_departure)`
- `handle_phase_timeout(worker_id, elapsed, elastic_departure)`

And their shared return type `ConnectionLossOutcome` / `DepartureEventKind`.

## Test Table

| ID | Test function | Spec citation | What it verifies |
|----|--------------|---------------|------------------|
| UT-0413-01 | `test_handle_connection_loss_elastic_false_returns_abort` | SPEC-06 R25 | `elastic_departure=false` → `Abort` with error description |
| UT-0413-02 | `test_handle_connection_loss_elastic_true_returns_recovery_triggered` | SPEC-20 §3.3 R19 | `elastic_departure=true` → `RecoveryTriggered { worker_id=3, kind=ConnectionLost }` |
| UT-0413-03 | `test_handle_connection_loss_false_branch_propagates_description_verbatim` | SPEC-06 R25 | `Abort` carries error description verbatim (backward compat) |
| UT-0413-04 | `test_handle_phase_timeout_elastic_false_returns_abort` | SPEC-13 R21 | `elastic_departure=false` → `Abort` with elapsed and contractual "phase timeout after" prefix |
| UT-0413-05 | `test_handle_phase_timeout_elastic_true_returns_recovery_triggered` | SPEC-20 §3.3 R18 | `elastic_departure=true` → `RecoveryTriggered { worker_id=2, kind=PhaseTimeout }` |
| UT-0413-06 | `test_connection_loss_outcome_derives_debug_partial_eq` | SPEC-13 R21, SPEC-20 §3.8 A1 | `Debug + PartialEq` on both variants; `kind` field participates in equality |
| UT-0413-07 | `test_handle_connection_loss_worker_id_zero_is_valid` | SPEC-20 §3.3 R19 | `worker_id=0` is valid; not a sentinel |
| UT-0413-08 | `test_handle_phase_timeout_false_embeds_elapsed_duration` | SPEC-13 R21 | Different elapsed → distinct `Abort` strings |
| UT-0413-09 | `test_r25_default_path_always_aborts` | SPEC-06 R25 | Both helpers return `Abort` when `elastic_departure=false` |
| UT-0413-10 | `test_a1_elastic_path_always_recovers` | SPEC-20 §3.8 A1 | Both helpers return `RecoveryTriggered` with correct `kind` when `elastic_departure=true` |
| UT-0413-11 | `test_run_coordinator_elastic_departure_true_suppresses_connection_loss` | SPEC-20 §3.8 A1 | `#[ignore]` — blocked until TASK-0415 lands `elastic_departure` field |
| UT-0413-12 | `test_run_coordinator_elastic_departure_false_connection_loss_aborts` | SPEC-06 R25 | `#[ignore]` — blocked until TASK-0438 wires `run_coordinator` |
| UT-0413-13 | `test_handle_connection_loss_worker_id_max_is_accepted` | QA-002 / EC-A | `WorkerId(u32::MAX)` accepted; no truncation (type is `u32`, not `usize`) |
| UT-0413-14 | `test_handle_phase_timeout_duration_max_does_not_panic` | EC-D | `Duration::MAX` does not panic; Abort carries contractual prefix |
| UT-0413-15 | `test_handle_phase_timeout_precision_boundary_rounds_correctly` | QA-005 / EC-H | `9.999s` rounds to `"10.00"` under `{:.2}` precision |

## Preconditions / not-yet-active tests

- **UT-0413-11**: blocked by `GridConfig.elastic_departure` field landing (TASK-0415) AND `run_coordinator` being wired (TASK-0438). The field is not the only blocker.
- **UT-0413-12**: blocked by `run_coordinator` wiring only (TASK-0438). The `GridConfig` default of `false` is already correct once TASK-0415 lands; the real gate is the call-site wiring.

## Coverage notes

- EC-B (worker_id=0): covered by UT-0413-07.
- EC-C (elapsed=Duration::ZERO): LOW gap; not covered. A fixture asserting `"0.00s"` in the Abort string would be welcome.
- EC-E (empty error_description): LOW gap; not covered. Non-panic property.
- EC-F (error_description with newlines): LOW gap; caller responsibility.
- EC-G (concurrent invocation): helpers are pure; acceptable per QA-010.
- EC-I (default-value drift): deferred to TASK-0415 time per QA-007.
