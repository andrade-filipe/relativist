# TEST-SPEC-0387: Coordinator final-state collection + final `merge()` (R21.3, R27, R29)

**Task:** TASK-0387
**Spec:** SPEC-19 R21 phase 3 (Final State Collection), R27 (coordinator
  sends `FinalStateRequest`), R29 (coordinator collects + invokes
  `merge()`), R38 (G1 amendment: `reduce_all(net) ~ extract_result(run_grid_delta(net, n))`).
**Spec-critic notes:** No DC-Cn directly amends TASK-0387. Implicit DC-C2
  applies — `WorkerDispatch::dispatch_final_state_request` is synchronous and
  returns `Result<Vec<Partition>, GridError>`. Implicit DC-C6 applies if any
  fixture constructs disconnection deltas (must use
  `crate::net::DISCONNECTED`).
**Generated:** 2026-04-17

---

## Scope note

TASK-0387 ships two pure-core helpers and one E2E integration:

1. `run_grid_delta_final_collect(dispatch, cache, border_graph, metrics) -> Result<Net, GridError>` —
   dispatches `FinalStateRequest` to every worker, validates response count,
   reconstructs a `PartitionPlan`, calls v1 `merge()`, records merge time
   in metrics.

2. `reconstruct_partition_plan_from_collected(partitions, &border_graph) -> PartitionPlan` —
   sorts partitions by `worker_id`, builds `border_map` from the remaining
   `BorderGraph::borders`, populates `worker_assignments`.

3. **E2E integration** in `tests/grid_delta_e2e.rs` — a 2-worker, 4-CON-CON
   fixture that verifies `extract_result(run_grid_delta(net, 2))` yields the
   SAME final net as v1 `run_grid` (G1 spot check under R38).

**`BorderState → BorderEntry` conversion.** SPEC-05 defines `BorderEntry`
shape; this TEST-SPEC assumes the natural mapping `BorderState { side_a,
side_b, worker_a, worker_b }` → `BorderEntry { worker_a, worker_b }`. If
the actual SPEC-05 struct has more fields (e.g., a wire ID), the
implementation reads it and tests assert identity over the full struct.

---

## Test target file paths

- `relativist-core/src/merge/grid.rs` — inline `#[cfg(test)] mod tests`.
  Six new `#[test]` fns (synchronous).
- `relativist-core/tests/grid_delta_e2e.rs` — NEW integration test file.
  One synchronous `#[test]` fn for G1/R38 spot check.

---

## Unit Tests (inline in `merge/grid.rs`)

### UT-0387-01: `run_grid_delta_final_collect_empty_border_graph`

**Purpose:** Happy-path final collection with all borders resolved during
the loop. Verify merged net's live-agent count equals sum of partitions
and `border_redex_count == 0`.

**Target:** `merge/grid.rs::tests`

**Given:**
- 2 partitions: `p0` with 3 agents, `p1` with 5 agents (8 total).
- `coordinator_partition_cache: HashMap<WorkerId, Partition>` with both
  entries (used only for size sanity).
- `border_graph`: empty (`BorderGraph::default()`).
- `metrics`: fresh `GridMetrics::default()` with `rounds = 1`.
- Mock `WorkerDispatch` (`StaticDispatch`): `dispatch_final_state_request`
  returns `Ok(vec![p0.clone(), p1.clone()])`.

**When:** `let net = run_grid_delta_final_collect(&mut dispatch, cache, border_graph, &mut metrics)?;`

**Then:**
- `net.live_agent_count() == 8`.
- `metrics.merge_time_per_round.len() == 1` (one entry recorded for the
  final merge step).
- The internally-computed `border_redex_count == 0` (asserted via the
  invariant in the doc-comment; if the function returns it, test asserts
  directly).

**Assertions:** Trivial union — empty border map → no boundary reconnection.

**SPEC-19 R covered:** R29 (collect + merge), R21.3.

---

### UT-0387-02: `run_grid_delta_final_collect_remaining_borders`

**Purpose:** Final collection with NON-empty `border_graph` (e.g., partial
convergence path or inert-border scenario). Asserts merge succeeds and
returned net is well-formed.

**Given:**
- 2 partitions, each with one free port forming an inert (auxiliary-on-both-sides)
  border pair.
- `border_graph`: 1 remaining border entry.
- Mock dispatch returns the two partitions.

**When:** call `run_grid_delta_final_collect`.

**Then:**
- `Ok(net)` returned.
- `net.live_agent_count()` matches expected.
- The inert border still appears as a wire in the merged net (confirm via
  net structure inspection, not via `BorderGraph` re-derivation).
- `border_redex_count == 0` (border was inert — auxiliary–auxiliary).

**Assertions:** Merge with non-trivial border map produces a valid net.

**SPEC-19 R covered:** R29 (border-map handling).

---

### UT-0387-03: `run_grid_delta_final_collect_mismatched_partitions_errors`

**Purpose:** Sanity-check failure path — dispatch returns fewer partitions
than the cache expects (protocol bug or worker drop).

**Given:**
- `coordinator_partition_cache`: 2 entries.
- Mock dispatch returns `Ok(Vec::new())` (zero partitions — simulates
  worker mass-drop).
- `border_graph`: empty.

**When:** call `run_grid_delta_final_collect`.

**Then:**
- Returns `Err(GridError::Protocol(msg))` (or new
  `GridError::FinalCollectionMismatch { expected, actual }` if added —
  this TEST-SPEC accepts either, since the TASK noted reuse-or-new at
  developer discretion).
- The error message includes both `expected` count (2) and `actual`
  count (0) — assert via `format!("{}", err).contains("expected 2")` etc.
- `metrics.merge_time_per_round.len() == 0` (no merge attempted).

**Assertions:** Caller is alerted to the protocol mismatch BEFORE the
malformed `PartitionPlan` reaches `merge()`.

**SPEC-19 R covered:** R29 (sanity check on collected count).

---

### UT-0387-04: `reconstruct_partition_plan_sorts_by_worker_id`

**Purpose:** Lock the sort contract — `merge()` consumes `PartitionPlan`
with partitions in `worker_id` ascending order (SPEC-04, SPEC-05).

**Given:**
- `partitions: Vec<Partition>` shuffled order:
  `[partition(worker_id=2), partition(worker_id=0), partition(worker_id=1)]`.
- `border_graph`: empty.

**When:**
`let plan = reconstruct_partition_plan_from_collected(partitions, &border_graph);`

**Then:**
- `plan.partitions[0].worker_id == WorkerId(0)`
- `plan.partitions[1].worker_id == WorkerId(1)`
- `plan.partitions[2].worker_id == WorkerId(2)`

**Assertions:** Sort is stable and ascending by `worker_id`.

**SPEC-19 R covered:** R29 (PartitionPlan invariant).

---

### UT-0387-05: `reconstruct_partition_plan_preserves_remaining_borders`

**Purpose:** Border map shape — every entry in `border_graph.borders` (or
`remaining_borders()` if that accessor is added) appears in
`plan.border_map` with matching ID.

**Given:**
- 2 partitions (any).
- `border_graph` with 3 remaining borders, IDs `{10, 20, 30}`. Each entry
  has a known `(worker_a, worker_b)` tuple.

**When:** call `reconstruct_partition_plan_from_collected`.

**Then:**
- `plan.border_map.len() == 3`.
- `plan.border_map.contains_key(&10) && contains_key(&20) && contains_key(&30)`.
- For each ID, the `BorderEntry` value matches the expected
  `(worker_a, worker_b)` derived from the source `BorderState`.

**Assertions:** Border map is bijection-preserving (no entries dropped or
silently merged).

**SPEC-19 R covered:** R29 (border-map propagation to `merge()`).

---

### UT-0387-06: `run_grid_delta_final_collect_merge_call_records_time`

**Purpose:** Lock the metrics-update contract — every successful call
appends one entry to `metrics.merge_time_per_round` (mirroring v1
`run_grid` per-round metric shape).

**Given:**
- 2 partitions, empty `border_graph`, mock dispatch returns 2 partitions.
- `metrics.merge_time_per_round.len() == 0` initially.

**When:** call `run_grid_delta_final_collect`.

**Then:**
- `metrics.merge_time_per_round.len() == 1`.
- The recorded duration is `> Duration::ZERO` (sanity, not exact value —
  microsecond floor).

**Assertions:** Final merge time is captured for benchmark/reporting
parity with v1.

**SPEC-19 R covered:** R29 (metric shape parity).

---

## End-to-End Integration Test

### IT-0387-01: `run_grid_delta_matches_run_grid_on_4_con_con_fixture`  (G1 / R38 spot check)

**Purpose:** SPEC-01 G1 amended by SPEC-19 R38 — for any input net `n`,
`extract_result(run_grid_delta(net, n)) == reduce_all(net)`. Verified here
with a 2-worker, 4-CON-CON fixture.

**Target:** NEW file `relativist-core/tests/grid_delta_e2e.rs`.

**Given:**
- A fresh `Net` containing 4 active CON-CON redexes such that 2 are local
  to W0, 2 are local to W1 after `split_partition_plan(net, 2)`.
- A v1 `Net` clone for the reference computation.
- An in-process mock `WorkerDispatch` that simulates 2 workers (in-process
  state — does NOT use `tokio` or TCP) by holding a `WorkerContext` per
  worker and routing dispatched messages through the pure handlers
  (`handle_initial_partition`, `handle_round_start`, `handle_final_state_request`).
- `GridConfig { workers: 2, max_rounds: Some(100), ... }`.

**When:**
1. `let v1_net = run_grid(net.clone(), &config).expect("v1 run");`
2. `let v2_net = run_grid_delta(net.clone(), &config, &mut mock_dispatch).expect("v2 run");`

**Then:**
- `v2_net.live_agent_count() == v1_net.live_agent_count()`
- Net structure equivalence — assert via canonical-form hash
  (`Net::canonical_hash()` if available) OR field-by-field on the
  agent/wire vectors after sorting.
- (Stretch) `v2_net == v1_net` if `PartialEq` is derived on `Net`.
- `metrics.converged == true`, `metrics.delta_max_rounds_hit == None`,
  `metrics.rounds <= 2` (4 CON-CON redexes resolve in ≤2 BSP rounds).

**Assertions:** G1 holds across the v1 and v2 reduction paths on this
non-trivial workload. Single failing assertion here is a SHOWSTOPPER for
the entire 2.26-C bundle.

**SPEC-19 R covered:** R38 (G1 under delta mode), R29 (final merge
correctness).

---

## Fixture Notes

**`StaticDispatch` mock.** The unit tests use a tiny canned-response struct:

```rust
struct StaticDispatch {
    final_response: Vec<Partition>,
    final_response_err: Option<GridError>,
}
impl WorkerDispatch for StaticDispatch {
    fn dispatch_initial(&mut self, _: Vec<(WorkerId, Partition)>) -> Result<(), GridError> {
        Ok(())
    }
    fn dispatch_round_start(&mut self, _: u32, _: Vec<(WorkerId, Vec<BorderDelta>)>)
        -> Result<Vec<RoundResultPayload>, GridError> { Ok(Vec::new()) }
    fn dispatch_final_state_request(&mut self, _: u32) -> Result<Vec<Partition>, GridError> {
        if let Some(e) = self.final_response_err.take() { return Err(e); }
        Ok(std::mem::take(&mut self.final_response))
    }
}
```

**`InProcessTwoWorkerDispatch` for IT-0387-01.** Holds two `WorkerContext`
instances in `Vec<RefCell<WorkerContext>>`; each `dispatch_*` method routes
the message through the matching handler and collects the returned
`Vec<WorkerAction>`, extracting the outgoing `Message::*Result` payload.
Defined inline in `grid_delta_e2e.rs`. Roughly 80-120 LoC of test
scaffolding.

**`Partition` construction.** Tests build minimal partitions via
`Partition::new(worker_id, net)` where `net` is hand-built with 1-3 agents.

**`BorderGraph` with N remaining borders.** Same approach as TEST-SPEC-0386
fixture notes — build a `PartitionPlan` and call `BorderGraph::from_partition_plan`,
then optionally apply deltas to leave the desired remaining set.

---

## Coverage mapping

| Requirement / DC | Covered by |
|---|---|
| R21 phase 3 — Final Collection sequence | UT-0387-01, UT-0387-02, IT-0387-01 |
| R27 — coordinator sends `FinalStateRequest` to every worker | UT-0387-01 (dispatch invocation), IT-0387-01 |
| R29 — collect + reconstruct + invoke `merge()` | UT-0387-01, UT-0387-02, UT-0387-04, UT-0387-05 |
| R29 — sanity check on response count | UT-0387-03 |
| R29 — `border_redex_count == 0` post-convergence | UT-0387-01, UT-0387-02 |
| R38 — G1 under delta mode (`reduce_all ~ extract_result(run_grid_delta)`) | IT-0387-01 |
| Metric shape parity (`merge_time_per_round`) | UT-0387-06 |
| `PartitionPlan` sort invariant | UT-0387-04 |
| Border-map preservation | UT-0387-05 |

---

## Adversarial angles (QA candidates for Stage 5)

| # | Scenario | Why dangerous |
|---|---|---|
| QA-0387-A | Refactor drops the size-sanity check — silently passes a too-short partition list to `merge()` | UT-0387-03 fires; without it, `merge()` may panic or produce malformed net |
| QA-0387-B | `partitions.sort_by_key(\|p\| p.worker_id)` accidentally uses unstable sort with custom Ord — partial-equal worker_ids reorder | UT-0387-04 covers the nominal case; QA candidate: add a duplicate-worker_id panic test (which would itself be a different bug) |
| QA-0387-C | `reconstruct_partition_plan` drops the BorderGraph silently when `borders` is empty (returns plan with `border_map: HashMap::new()` always) | UT-0387-05 fires |
| QA-0387-D | `merge_time_per_round.push` happens BEFORE `merge()` returns — records 0ns | UT-0387-06 may pass (>0 is true even at ns granularity); QA candidate: assert `>= 100ns` |
| QA-0387-E | E2E test `run_grid_delta` produces structurally equivalent but not bit-identical net (different agent IDs after merge) | IT-0387-01's canonical-form check accommodates this; raw `==` would fire |
| QA-0387-F | E2E mock dispatch holds workers' state in `Mutex` and a deadlock occurs across the 3-phase exchange | Sync mock should not deadlock; QA candidate: add timeout-bounded test |
| QA-0387-G | `BorderState → BorderEntry` mapping silently drops `wire_id` field | UT-0387-05 fires if `BorderEntry` includes wire_id and we assert structural equality on full struct |
| QA-0387-H | Final-collection error path leaks the `coordinator_partition_cache` (memory leak in long-running coordinator) | Not caught by these tests; QA candidate: add a `Drop` instrumentation test |

---

## Acceptance gate

- `cargo test --workspace --lib` floor: +6 new `#[test]` fns (inline).
- `cargo test --workspace --test grid_delta_e2e` floor: +1 new `#[test]` fn.
- Combined gate: +7 tests across the bundle attributable to TASK-0387.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo fmt --check` clean.
- No regression on v1 690-test baseline.
- IT-0387-01 (G1 spot check) is the headline gate — failure blocks the
  entire 2.26-C merge.

---

## Out of scope (deferred)

- Multi-grid / nested grid scenarios → SPEC-19 R31 (future).
- Wire-level integration with real TCP `protocol/coordinator.rs` → 2.26-C-wire.
- Border resolution path (this task assumes resolution happened during the
  loop) → 2.26-B `BorderResolver` (shipped).
- Partial-net QA on max-rounds cap path → TASK-0388 / TEST-SPEC-0388.
- `BorderGraph::remaining_borders()` accessor design → if not present,
  TEST-SPEC-0387 falls back to direct field access via `#[cfg(test)]`
  re-export inside `grid.rs` (per TASK-0387 Files Forbidden note).
