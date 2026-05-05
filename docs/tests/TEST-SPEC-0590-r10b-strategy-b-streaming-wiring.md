# TEST-SPEC-0590: SPEC-22 R10b Strategy B (`BorderClean`) wiring under SPEC-21 streaming pipeline

**SPEC-21 §7 ID:** plumbing only (production-side closure of TEST-SPEC-0515 amendment-level coverage; cross-spec via SPEC-22 R10b broadening — precision-path opt-in).
**Owning task:** TASK-0590.
**Parent spec:** SPEC-21 §3.7 R37b Strategy B (G1 free-list interaction; closes SC-007); §3.8 A6 (consumer of TASK-0515).
**Type:** unit + integration (per-id membership gate + cross-strategy isomorphism + `border_entries` cache lifecycle).
**Theory anchor:** ARG-001 G1 (BSP determinism under streaming — preserved by precision gate); ARG-005 P7/P8 (delta border completeness — extended to streaming via per-id protection).

---

## Inputs / Fixtures

- **Canonical SPEC-22 R10b fixture (REUSED, NOT DUPLICATED) per TEST-SPEC-T9b:** "worker 0 owns IDs `[0, 100)`, border at `AgentPort(47, 0)`" — sourced from TEST-SPEC-0482 / TEST-SPEC-T9b fixture helpers.
- **NEW SPEC-21 EXTENSION:** the canonical fixture extended with a streaming chunk producing a fresh border at slot 47 mid-stream (per TEST-SPEC-0515 line 14).
- A `Net` with non-empty `free_list` AND mixed border / non-border IDs (e.g., free-list contains `[47, 50, 92, 73]` where `border_entries = {47, 92}`).
- `GridConfig` instances:
  - `cfg_strategy_b_streaming`: `recycle_under_delta = BorderClean`, `dispatch_mode = Pull`, `streaming_active = true`, `delta_mode = true`.
  - `cfg_strategy_b_streaming_no_delta`: same but `delta_mode = false`.
  - `cfg_strategy_a_streaming`: same as first but `recycle_under_delta = DisableUnderDelta` (for cross-strategy isomorphism).
- Per-worker `border_entries: HashSet<AgentId>` cache (TASK-0590 type).
- Test-only debug counters `Net.free_list_pops_border: AtomicU64` and `Net.free_list_pops_non_border: AtomicU64` (separated by classification; gated on `#[cfg(test)]`).
- The `nets_isomorphic` helper for cross-strategy comparison.

## Unit Tests (border_entries cache + per-id gate)

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0590-01 | `border_entries_cache_updated_on_assign_partition` | worker; `cfg_strategy_b_streaming`; receives `AssignPartition` with partition borders `{47, 92}` | observe `border_entries` after message processing | `border_entries == {47, 92}`. (TASK-0590 acceptance line 28.) |
| UT-0590-02 | `border_entries_cache_cleared_on_done` | worker reaches `Done` state | check `border_entries` post-`Done` | empty / cleared. (TASK-0590 acceptance line 32 — UT-0590-04 explicit; no leak between runs.) |
| UT-0590-03 | `strategy_b_no_pop_for_border_id` | canonical fixture; `cfg_strategy_b_streaming`; free-list head is `47` (border ID); `border_entries = {47}` | `Net::create_agent(...)` | does NOT pop `47`; falls through to `next_id` increment. `Net.free_list_pops_border == 0`. (TASK-0590 acceptance line 30 — UT-0590-01 task-side.) |
| UT-0590-04 | `strategy_b_pop_for_non_border_id` | same fixture; free-list head is `50` (non-border); `border_entries = {47, 92}` | `create_agent` | pops `50`; `Net.free_list_pops_non_border == 1`. (TASK-0590 acceptance line 31 — UT-0590-02 task-side.) |
| UT-0590-05 | `strategy_b_o1_membership_check` | `border_entries` populated with 100 IDs | `create_agent(...)` 1000 times | per-id check is O(1) HashSet lookup; total runtime acceptable per profiling budget (R37b — TASK-0590 acceptance line 31). |
| UT-0590-06 | `strategy_b_streaming_alone_triggers_gate` | `cfg_strategy_b_streaming_no_delta` (delta=false, streaming=true) | `create_agent` for a border ID | gate triggers (broadening per `(delta_mode \|\| streaming_active)` disjunction); border ID is NOT popped. (Mirrors UT-0589-02 for Strategy B side.) |
| UT-0590-07 | `strategy_b_pop_when_streaming_inactive_and_no_delta` | `cfg_strategy_b_streaming` but with `streaming_active = false` AND `delta_mode = false`; same border ID | `create_agent` | gate does NOT trigger; pop is allowed normally (R10b discipline only engages with the disjunction). |
| UT-0590-08 | `border_entries_subset_of_coordinator_bordergraph` | post-`AssignPartition`; coordinator's `BorderGraph` contains 200 entries; this worker's partition has 50 border IDs | inspect `border_entries` | `border_entries.len() == 50`; subset relationship preserved. (TASK-0590 NOTE line 79 — worker-local view sufficient.) |

## Integration tests (Strategy B end-to-end + cross-strategy isomorphism)

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| IT-0590-01 | `strategy_b_zero_border_pops_nonzero_non_border_pops` | 4 workers; 8 chunks; mixed border / non-border IDs; `cfg_strategy_b_streaming` | run pipeline; aggregate counters | `Net.free_list_pops_border == 0`; `Net.free_list_pops_non_border > 0` (precision is achieved). (TASK-0590 acceptance lines 64-65 — UT-0590-01/02 task-side.) |
| IT-0590-02 | `cross_strategy_isomorphism_a_vs_b` | same workload run under `cfg_strategy_a_streaming` and `cfg_strategy_b_streaming` separately | merge both; compare | `nets_isomorphic(merged_a, merged_b) == true`. (G1 / ARG-005 preserved by both — TASK-0590 acceptance line 33; UT-0590-03 task-side.) |
| IT-0590-03 | `strategy_b_baseline_regression` | `cargo test` default | run baseline 1181/1224 suite | passes. (TASK-0590 acceptance line 34.) |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | `border_entries` is empty (worker has no border-classified IDs) | Strategy B pops freely (no IDs match); behaves like SPEC-22 R3. |
| EC-2 | `border_entries` contains an ID NOT in the free-list (e.g., border ID 47 has not been remove_agent'd) | the membership check is harmless (just an irrelevant entry); pops of OTHER non-border IDs proceed. |
| EC-3 | Mid-stream border emergence: a new border at slot 47 is added to `border_entries` AFTER slot 47 was already popped | the post-update protection is too late for that allocation; the next round MUST handle the inconsistency via SPEC-19 reconstruct or R10c protected-tombstone semantics. (Documents the call-site ordering boundary; cross-cut with TEST-SPEC-0515 UT-0515-09.) |
| EC-4 | Worker FSM enters multiple streams (multi-cycle) | `border_entries` cleared on each `Done` per UT-0590-02; no leak across runs. |
| EC-5 | `recycle_under_delta = DisableUnderDelta` (Strategy A) — wrong policy | this task's branch is NOT taken; Strategy A's branch (TEST-SPEC-0589) is taken. Mutually exclusive at runtime. |

## Invariants asserted

- R37b Strategy B (under `BorderClean`, workers MAY pop only for non-border IDs while delta or streaming active; closes SC-007 streaming-precision half).
- §3.8 A6 (SPEC-22 R10b broadening — production wiring of Strategy B streaming side).
- G1 (BSP determinism under streaming — preserved by precision gate).
- ARG-005 INV-REC (delta-recoverability — preserved under streaming via per-id protection).
- D2 / D3 (border completeness, cross-round border discovery) — preserved.

## ARG/DISC/REF citation

- ARG-001 G1 (operational closure under streaming, precision-path).
- ARG-005 P7/P8 (delta border completeness, extended via per-id protection).

## Determinism notes

**CANONICAL FIXTURE REUSE:** This TEST-SPEC EXTENDS the SPEC-22 R10b canonical fixture from TEST-SPEC-T9b + TEST-SPEC-0482; do NOT duplicate the fixture body. Cite the source TEST-SPECs explicitly per TASK-0590 cross-coordination.

**BORDER_ENTRIES LIFECYCLE:** The cache is updated on every `AssignPartition` receipt under streaming mode (UT-0590-01) and cleared on `Done` (UT-0590-02). The lifecycle is per-stream (not per-chunk), per TASK-0590 line 25-26. UT-0590-04 task-side asserts no leak between runs.

**O(1) MEMBERSHIP:** `border_entries` is a `HashSet<AgentId>`; UT-0590-05 budget-checks the membership cost. If the cache exceeds 10000 entries (large delta scenarios), revisit the data structure (consider a probabilistic Bloom filter pre-filter); document budget revisits in this TEST-SPEC.

**TOKIO ORDERING:** UT-0590-01 (cache update on `AssignPartition`) and UT-0590-03 (per-id gate at `create_agent`) MUST happen in the correct order WITHIN THE SAME FSM TICK. Cross-cut with TEST-SPEC-0515 UT-0515-09 (`mid_stream_border_does_not_corrupt_protected_tombstones`). Use `#[tokio::test(flavor = "current_thread")]` for deterministic ordering.

**ISOMORPHISM (NOT BYTE-EQUALITY) FOR STRATEGY B:** Unlike Strategy A (TEST-SPEC-0589 IT-0589-03 byte-equality oracle), Strategy B permits non-border slot reuse, so byte-identical layout is NOT preserved across chunks. Only ISOMORPHISM is asserted (IT-0590-02). This is a documented trade-off per TASK-0590 NOTE line 78: precision recycling vs. byte-identical determinism.

**COORDINATOR ↔ WORKER VIEW DIVERGENCE:** Per TASK-0590 NOTE line 79, the worker-local `border_entries` is conceptually a SUBSET of the coordinator's `BorderGraph`. R10c protected-tombstone semantics ensure that a freshly-popped non-border ID cannot CAUSE a cross-worker border violation; the local view is sufficient. UT-0590-08 enforces the subset relationship.

**CARGO FEATURE GATE COORDINATION:** Same as Strategy A — feature is an ADDITIONAL safety net per TASK-0590 NOTE line 78 + cross-cut with TEST-SPEC-0591.

## Cross-test dependencies

- **TEST-SPEC-0515 (SPEC-22 R10b broadening amendment-level coverage)** — predecessor; this task is the Strategy B production-side closure.
- **TEST-SPEC-0482 (SPEC-22 RecyclePolicy + `is_border_protected`)** — provides the canonical fixture and Strategy B baseline.
- **TEST-SPEC-T9b (SPEC-22 Strategy B delta-only path)** — SIBLING and FIXTURE SOURCE.
- **TEST-SPEC-T9a (SPEC-22 Strategy A delta-only path)** — sibling for the conservative-path strategy.
- **TEST-SPEC-0578 (worker FSM)** — predecessor; FSM populates `border_entries` from `AssignPartition` payload (UT-0590-01) and clears on `Done` (UT-0590-02).
- **TEST-SPEC-0554 (orchestrator)** — predecessor; coordinator's `BorderGraph` is the source-of-truth for partition borders propagated to workers.
- **TEST-SPEC-0589 (Strategy A streaming wiring)** — sibling; pre-coordinated branching (TASK-0590 line 38). Mutually exclusive at runtime.
- **TEST-SPEC-0591 (cargo feature gate)** — cross-cut on the feature-flag matrix (IT-0590-02 isomorphism MUST hold under both feature states).
- **TEST-SPEC-0588 (BorderGraph extension call-site)** — coordinator-side companion; the per-worker `border_entries` cache is propagated independently from the call-site (TEST-SPEC-0588 EC-3).
