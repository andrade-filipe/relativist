# TEST-SPEC-0589: SPEC-22 R10b Strategy A (`DisableUnderDelta`) wiring under SPEC-21 streaming pipeline

**SPEC-21 §7 ID:** plumbing only (production-side closure of TEST-SPEC-0515 amendment-level coverage; cross-spec via SPEC-22 R10b broadening).
**Owning task:** TASK-0589.
**Parent spec:** SPEC-21 §3.7 R37b (G1 free-list interaction; closes SC-007); §3.8 A6 (consumer of TASK-0515).
**Type:** unit + integration (gate-condition extension at free-list pop sites + ARG-005 oracle under streaming).
**Theory anchor:** ARG-001 G1 (BSP determinism under streaming — preserved by Strategy A conservative gate); ARG-005 P7/P8 (delta border completeness — extended to streaming via this gate).

---

## Inputs / Fixtures

- **Canonical SPEC-22 R10b fixture (REUSED, NOT DUPLICATED) per TEST-SPEC-T9a:** "worker 0 owns IDs `[0, 100)`, border at `AgentPort(47, 0)`" — sourced from TEST-SPEC-0482 / TEST-SPEC-T9a fixture helpers.
- **NEW SPEC-21 EXTENSION:** the canonical fixture extended with a streaming chunk producing a fresh border at slot 47 mid-stream (per TEST-SPEC-0515 line 14).
- A `Net` with non-empty `free_list` (forced via prior `remove_agent` calls).
- `GridConfig` instances:
  - `cfg_streaming_delta`: `dispatch_mode = Pull`, `delta_mode = true`, `streaming_active = true`, `recycle_under_delta = DisableUnderDelta`.
  - `cfg_streaming_only`: `dispatch_mode = Pull`, `delta_mode = false`, `streaming_active = true`, `recycle_under_delta = DisableUnderDelta`.
  - `cfg_push_no_delta`: `dispatch_mode = Push`, `delta_mode = false`, `streaming_active = false`, `recycle_under_delta = DisableUnderDelta`.
- A test-only debug counter `Net.free_list_pops: AtomicU64` (gated on `#[cfg(test)]` or `#[cfg(debug_assertions)]`).
- The `streaming_no_recycle` cargo feature gate, exercised in BOTH states (cross-cut with TEST-SPEC-0591).

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0589-01 | `streaming_active_strategy_a_no_pop_during_chunk` | canonical fixture; `cfg_streaming_delta`; mid-chunk reduction | invoke `Net::create_agent(...)` requiring a fresh ID | `create_agent` allocates fresh ID via `next_id` increment, NOT pop from free-list. (Strategy A behavior under streaming — TASK-0589 acceptance line 26.) |
| UT-0589-02 | `streaming_active_no_delta_strategy_a_no_pop` | canonical fixture; `cfg_streaming_only`; mid-chunk | `create_agent(...)` | NO pop; broadening triggers on streaming alone (`(delta_mode \|\| streaming_active)` disjunction). (R37b broadening — TASK-0589 acceptance line 28.) |
| UT-0589-03 | `push_no_delta_strategy_a_pop_normally` | canonical fixture; `cfg_push_no_delta` | `create_agent(...)` | free-list pop occurs (SPEC-22 R3 path UNCHANGED). (TASK-0589 regression test line 30.) |
| UT-0589-04 | `streaming_active_flag_set_on_first_chunk_assign_partition` | worker FSM; receives first chunked `AssignPartition` | observe `worker_state.streaming_active` | `true` (set by FSM hook per TASK-0589 line 22; cross-cut with TEST-SPEC-0578). |
| UT-0589-05 | `streaming_active_flag_cleared_on_final_reduction_send_final_result` | worker FSM; transitions `FinalReduction → SendFinalResult` | observe flag | `streaming_active = false`. (TASK-0589 NOTE line 69; allows post-streaming reductions to recycle normally.) |
| UT-0589-06 | `gate_condition_extends_disjunction` | the live `Net::create_agent` source | grep gate condition | the condition is `(cfg.delta_mode \|\| worker_state.streaming_active) && cfg.recycle_under_delta == DisableUnderDelta`. (R37b broadening; mirrors TEST-SPEC-0515 UT-0515-01 at the production layer.) |
| UT-0589-07 | `free_list_pops_counter_zero_during_streaming` | `cfg_streaming_delta`; full 8-chunk streaming run | check `Net.free_list_pops.load()` after run | `== 0`. (Operational closure of UT-0589-01 across the full pipeline.) |
| UT-0589-08 | `free_list_pops_counter_nonzero_in_push_mode` | `cfg_push_no_delta`; same workload (force some `remove_agent` to populate the free-list) | check `Net.free_list_pops.load()` after run | `> 0` (positive control; SPEC-22 R3 path active). |

## Integration tests (Strategy A end-to-end + ARG-005 oracle)

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| IT-0589-01 | `streaming_strategy_a_zero_pops_8_chunks` | 4 workers; 8 chunks; `ep_annihilation_pure(64)`; `cfg_streaming_delta` | run pipeline; aggregate `Net.free_list_pops` across all workers | sum `== 0`. (TASK-0589 acceptance line 28.) |
| IT-0589-02 | `streaming_strategy_a_g1_preserved` | same workload | merge result; compare to batch baseline `split() + reduce_all + merge` | `nets_isomorphic(merged_streaming, merged_batch) == true`. (G1 preserved; ARG-005 INV-REC operationalized.) |
| IT-0589-03 | `streaming_strategy_a_arg005_oracle` | `cfg_streaming_delta`; delta-mode active across 8 streaming chunks; one chunk introduces a border at a previously-recycled-eligible slot | merge | merged result equals batch baseline byte-equality (Strategy A is the default-safe path; recycle suspension makes the slot stay live). (TASK-0589 acceptance line 60 — UT-0589-04 in task file.) |
| IT-0589-04 | `streaming_strategy_a_baseline_regression` | `cargo test` with no special features | run baseline 1181/1224 test suite | all tests pass. (TASK-0589 acceptance line 31 — additive-compat regression.) |

## Integration tests (cross-cut with cargo feature gate)

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| IT-0589-05 | `strategy_a_with_streaming_no_recycle_feature_on` | `cargo test --features streaming-no-recycle`; `cfg_streaming_delta` | run | the runtime gate becomes redundant (the cargo feature short-circuits all pops); `free_list_pops == 0`; same merged result as gate-OFF run. (TASK-0589 NOTE line 71 — feature is an ADDITIONAL safety net, not a replacement.) |
| IT-0589-06 | `strategy_a_with_streaming_no_recycle_feature_off` | default `cargo test`; same setup | run | runtime gate from this task is the load-bearing path; `free_list_pops == 0`; same merged result as gate-ON run. |
| IT-0589-07 | `cross_feature_isomorphism` | merge results from IT-0589-05 and IT-0589-06 | compare | `nets_isomorphic == true`. (Cross-cut with TEST-SPEC-0591.) |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | `streaming_active = true` AND `delta_mode = true` (combined regime) | Strategy A disables ALL pops unconditionally; same as either flag alone. (Conservative gate semantics.) |
| EC-2 | Worker arena exhausts ID space (`next_id == u32::MAX`) under Strategy A | error path: `Net::create_agent` returns `NetError::ArenaExhausted` (or panics in debug); document that Strategy A is more memory-eager than free-list recycling. |
| EC-3 | `recycle_under_delta = BorderClean` (Strategy B) — wrong policy for this task | this task's branch is NOT taken; Strategy B's gate (TEST-SPEC-0590) is taken instead. (Cross-cut.) |
| EC-4 | Worker enters streaming, completes, exits, then enters streaming again (multi-stream lifecycle) | `streaming_active` toggles correctly between runs; no leak of the flag across stream boundaries. (UT-0589-05 covers single transition; this EC is multi-cycle.) |

## Invariants asserted

- R37b Strategy A (under `DisableUnderDelta`, workers MUST NOT pop while delta or streaming active; closes SC-007 streaming half).
- §3.8 A6 (SPEC-22 R10b broadening — production wiring of streaming side).
- G1 (BSP determinism under streaming — preserved by conservative gate).
- ARG-005 INV-REC (delta-recoverability — preserved under streaming).

## ARG/DISC/REF citation

- ARG-001 G1 (operational closure under streaming).
- ARG-005 P7/P8 (delta border completeness, extended via the broadening amendment).

## Determinism notes

**CANONICAL FIXTURE REUSE:** This TEST-SPEC EXTENDS the SPEC-22 R10b canonical fixture from TEST-SPEC-T9a + TEST-SPEC-0482; do NOT duplicate the fixture body. Cite the source TEST-SPECs explicitly in the test docstring per TASK-0589 cross-coordination (matches TEST-SPEC-0515 dependency note line 63).

**STREAMING_ACTIVE FLAG PROPAGATION:** The flag is set/cleared by the worker FSM (TEST-SPEC-0578 EC-4) and consumed by `Net::create_agent`. The plumbing flows through `worker_state: WorkerStreamingState` (TASK-0589 file `worker/streaming_state.rs`); this file is the SOURCE OF TRUTH for the flag's lifecycle. UT-0589-04 / UT-0589-05 test the FSM hook directly.

**TOKIO ORDERING:** UT-0589-04 / UT-0589-05 use `#[tokio::test(flavor = "current_thread")]` for deterministic FSM ordering. The flag toggle MUST happen BEFORE the next `create_agent` call within the same FSM tick; multi-thread runtimes would race the flag against the arena access.

**FREE_LIST_POPS COUNTER:** Atomic; incremented only inside the `else` branch (when pop succeeds). Reset at test start via fixture teardown. NOT shared across tests.

**CARGO FEATURE GATE COORDINATION:** UT-0589-05 / IT-0589-05 / IT-0589-06 / IT-0589-07 cross-cut with TEST-SPEC-0591. The feature gate is an ADDITIONAL safety net (TASK-0589 NOTE line 71); the runtime gate must REMAIN PRESENT AND CORRECT regardless of the feature flag. This dual-coverage protects against feature-flag drift at landing.

**ARG-005 ORACLE (IT-0589-03):** byte-equality with batch baseline is asserted because Strategy A's full-recycle suspension preserves slot identity across chunks. Under Strategy B (TEST-SPEC-0590), only isomorphism is asserted (precision recycling permits non-border slot reuse).

## Cross-test dependencies

- **TEST-SPEC-0515 (SPEC-22 R10b broadening amendment-level coverage)** — predecessor; this task is the Strategy A production-side closure for streaming.
- **TEST-SPEC-0482 (SPEC-22 RecyclePolicy + protected tombstones)** — provides the canonical fixture and Strategy A baseline gate.
- **TEST-SPEC-T9a (SPEC-22 Strategy A delta-only path)** — SIBLING and FIXTURE SOURCE; this task's streaming-side gate is a parallel discipline.
- **TEST-SPEC-T9b (SPEC-22 Strategy B delta-only path)** — sibling for the precision-path strategy.
- **TEST-SPEC-0578 (worker FSM pull-dispatch)** — predecessor; FSM sets/clears `streaming_active` at chunk boundaries (UT-0589-04 / UT-0589-05).
- **TEST-SPEC-0554 (orchestrator)** — predecessor; sets `streaming_active` at orchestrator entry.
- **TEST-SPEC-0590 (Strategy B streaming wiring)** — sibling; mutually exclusive at runtime via `recycle_under_delta` selection.
- **TEST-SPEC-0591 (cargo feature gate)** — cross-cut on IT-0589-05 / IT-0589-06 / IT-0589-07; feature is an ADDITIONAL safety net.
