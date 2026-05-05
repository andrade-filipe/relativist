# TEST-SPEC-0385: Coordinator round loop — Round 0 + delta-round body + DC-C3 strict/lenient branching

**Task:** TASK-0385
**Spec:** SPEC-19 §3.3 R21 phase 1 (Round 0 Initial Dispatch), R21 phase 2
  (Rounds 1+ Delta Rounds), R23 (`RoundStart` payload construction),
  R26 (`RoundResult` payload consumption), R40 (4-cell matrix completeness).
**Spec-critic amendments incorporated:**
- DC-C1 (ratified) — coordinator does NOT block on `InitialPartition` ack;
  enters round-1 dispatch immediately
- DC-C2 (ratified) — round loop calls `WorkerDispatch` trait methods SYNCHRONOUSLY
- DC-C3 (FLIP from task-splitter default A → option C) — round loop MUST branch on
  `config.strict_bsp`. Both `(delta_mode=true, strict_bsp=true)` and
  `(delta_mode=true, strict_bsp=false)` cells of R40's matrix MUST be implemented.
**Provenance:** `docs/spec-reviews/SPEC-19-section-3.3-2.26C-design-choices-2026-04-17.md` §DC-C1, §DC-C2, §DC-C3
**Generated:** 2026-04-17

---

## Scope note

TASK-0385 fleshes out `run_grid_delta_inner`:

- **Round 0 (R21.1):** `dispatch.dispatch_initial(&plan)`. No worker
  results collected. No `BorderGraph` update.
- **Round 1+ loop (R21.2):** for each round:
  1. Detect `BorderGraph` redexes.
  2. Call `BorderResolver::resolve(...)` (2.26-B).
  3. Package per-worker `RoundStartDispatch`.
  4. `dispatch.dispatch_round_start(&payload)` → `Vec<RoundResultPayload>`.
  5. `border_graph.apply_deltas(...)` per worker.
  6. Update `coordinator_partition_cache` per worker (re-uses
     `apply_border_deltas_to_partition`).
  7. Accumulate metrics.
  8. Convergence check (TASK-0386 helper).
  9. Max-rounds cap (TASK-0388 helper).
- **Final State Collection (R21.3) → TASK-0387 helper.**

**DC-C3 dual-branch:** the round loop MUST branch on
`config.strict_bsp`:
- `strict_bsp == false` (lenient): resolver runs immediately on the
  current round's reported deltas; resolutions ship in THIS round's
  next-dispatch `RoundStart`. `R_delta_lenient = 1` round suffices in
  the absence of cascades.
- `strict_bsp == true` (strict): resolver runs and stores resolutions
  in a pending buffer; dispatched as round k+1's `RoundStart.border_deltas`.
  `R_delta_strict ≤ N`.

The exact branching pin: strict mode defers EXACTLY one round
(matching v1 strict mode discipline).

---

## Test target file paths

- `relativist-core/src/merge/grid.rs` — inline `#[cfg(test)] mod tests`.
  Five new `#[test]` fns for the round-loop body (UT-0385-01..05).
- `relativist-core/tests/grid_delta_roundloop.rs` — NEW integration
  test file. Three new `#[test]` fns covering the DC-C3 4-cell matrix
  (UT-0385-06..08).

All tests are synchronous. No `tokio`, no `async`.

---

## Test fixtures

`CapturingDispatch` — records every `dispatch_*` call into per-method
vectors of recorded arguments; serves canned `RoundResultPayload`
responses queue-style for `dispatch_round_start` and canned `Partition`
responses for `dispatch_final_state_request`. Enables deterministic
replay of any round-loop scenario.

```rust
struct CapturingDispatch {
    initial_dispatches: Vec<PartitionPlan>,
    round_start_dispatches: Vec<RoundStartDispatch>,
    final_state_dispatches: Vec<u32>,  // round numbers
    canned_round_results: VecDeque<Vec<RoundResultPayload>>,
    canned_final_states: Option<Vec<Partition>>,
}
```

Lives in `#[cfg(test)] mod tests` in `merge/grid.rs` AND
re-exported (or re-defined) in `tests/grid_delta_roundloop.rs`.

---

## Unit Tests (inline in `merge/grid.rs::tests`)

### UT-0385-01: `run_grid_delta_inner_round_zero_dispatches_initial_partition_only`

**Purpose:** R21.1 + DC-C1 — Round 0 dispatches `InitialPartition` to
every worker exactly ONCE; no `RoundStart`, no `FinalStateRequest`
fired before workers reply.

**Target:** `merge/grid.rs::tests`

**Given:**
- 2-worker partition plan.
- `CapturingDispatch` whose `canned_round_results` queue is loaded
  with one converged round (`has_border_activity = false`,
  `local_redexes == 0`, empty `border_deltas`).
- `canned_final_states = Some(vec![partition_a, partition_b])`.

**When:** Call `run_grid_delta(net, &cfg, &strategy, &mut dispatch)`.

**Then:**
- `dispatch.initial_dispatches.len() == 1`
- The single recorded `PartitionPlan` matches the plan from `split(net, 2, &strategy)`.
- `dispatch.round_start_dispatches.len() == 1` (one delta round before convergence).
- `dispatch.final_state_dispatches.len() == 1`.

**Assertions:** Round 0 fires `dispatch_initial` exactly once and
makes no other dispatch call before the round-1 `RoundStart`.

**SPEC-19 R covered:** R21.1, DC-C1.

---

### UT-0385-02: `run_grid_delta_inner_single_delta_round_converges`

**Purpose:** Happy-path single round — net converges in Round 1.

**Target:** `merge/grid.rs::tests`

**Given:**
- 2-worker partition plan; net with one local redex per worker.
- `CapturingDispatch` with one canned converged round result.

**When:** Run `run_grid_delta`.

**Then:**
- `metrics.rounds == 1`
- `metrics.converged == true`
- `metrics.delta_mode == true`
- `metrics.delta_max_rounds_hit == None`
- `dispatch.round_start_dispatches.len() == 1`
- `dispatch.final_state_dispatches.len() == 1`.

**Assertions:** One round, converged naturally.

**SPEC-19 R covered:** R21 phase 2, R26 consumption.

---

### UT-0385-03: `run_grid_delta_inner_multi_round_records_metrics`

**Purpose:** 3-round scenario — assert per-round metric vectors all
have length 3.

**Target:** `merge/grid.rs::tests`

**Given:**
- 2-worker plan.
- `CapturingDispatch` with three canned `RoundResultPayload` batches:
  rounds 1, 2 carry `has_border_activity = true`; round 3 converged.

**When:** Run `run_grid_delta`.

**Then:**
- `metrics.rounds == 3`
- `metrics.partition_time_per_round.len() == 3`
- `metrics.compute_time_per_round.len() == 3`
- `metrics.merge_time_per_round.len() >= 3` (final collection adds one
  more entry post-convergence — the test asserts `>= 3` to tolerate
  TASK-0387's per-final-merge entry)
- `metrics.border_redexes_per_round.len() == 3`
- `metrics.border_reduce_time_per_round.len() == 3`
- `metrics.border_interactions_per_round.len() == 3`
- `metrics.worker_stats_per_round.len() == 3`
- `dispatch.round_start_dispatches.len() == 3`.

**Assertions:** Per-round vectors track loop iterations exactly.

**SPEC-19 R covered:** R21 phase 2 (multi-round metric accumulation).

---

### UT-0385-04: `run_grid_delta_inner_applies_round_result_deltas_to_border_graph`

**Purpose:** Each round's `RoundResultPayload.border_deltas` is fed
into `BorderGraph::apply_deltas` per worker. Lock the apply loop.

**Target:** `merge/grid.rs::tests`

**Given:**
- 2-worker plan with a known initial `BorderGraph` shape.
- `CapturingDispatch` with one round of canned results carrying
  specific `border_deltas` (e.g. worker 1 reports
  `[BorderDelta { border_id: 5, new_target: AgentPort(AgentId(7), 0) }]`,
  worker 2 reports
  `[BorderDelta { border_id: 6, new_target: crate::net::DISCONNECTED }]`).
- The next round is canned-converged.

**When:** Run `run_grid_delta` with these canned results. Inspect the
final `BorderGraph` (snapshot via `metrics`-attached probe OR via the
`coordinator_partition_cache`'s `free_port_index` post-loop).

**Then:**
- `border_graph.borders[&5]` has been updated to reflect
  `AgentPort(AgentId(7), 0)`.
- `border_graph.borders.contains_key(&6) == false`
  (DISCONNECTED → erasure per DC-C6 + 2.26-B's resolver).

**Assertions:** Worker reports propagate into the coordinator's
border-graph state.

**Note for the developer:** if `BorderGraph` is not directly
inspectable from `merge/grid.rs::tests` (visibility), expose a
`#[cfg(test)] pub fn borders_snapshot(&self)` accessor. Do NOT
modify production visibility.

**SPEC-19 R covered:** R26 consumption + R10/R11 from §3.2.

---

### UT-0385-05: `run_grid_delta_inner_caches_partitions_for_resolver`

**Purpose:** Coordinator-side `partition_cache: HashMap<WorkerId,
Partition>` is seeded from `plan.partitions` at Round 0 and updated
each round via `apply_border_deltas_to_partition` (re-using TASK-0381's
helper). Required by 2.26-B's resolver, which reads partition agent
state to compute reductions.

**Target:** `merge/grid.rs::tests`

**Given:**
- 2-worker plan with known initial `partition.free_port_index`.
- `CapturingDispatch` returning round-1 results that REMOVE one border
  ID via DISCONNECTED sentinel.

**When:** Run `run_grid_delta`. Probe the post-loop partition cache
state (via a test-only accessor or via the final partitions returned).

**Then:**
- After Round 0, `cache[worker_id]` matches the plan's partition for
  that worker.
- After Round 1's `apply_deltas`, the removed border is gone from
  `cache[worker_id].free_port_index`.

**Assertions:** Cache stays consistent across rounds (DC-B1 option (a)
symmetry: workers and coordinator hold equivalent border state).

**SPEC-19 R covered:** R26 + DC-B1 (cross-bundle consistency).

---

## Integration Tests (NEW file `tests/grid_delta_roundloop.rs`)

### UT-0385-06: `run_grid_delta_lenient_converges_in_one_round_absent_cascades`  (DC-C3 cell `(true, false)`)

**Purpose:** R40 lenient delta semantics — a partition pair whose only
inter-partition redex resolves in a single resolution cycle. With
`strict_bsp = false` + `delta_mode = true`, the loop converges in
EXACTLY 1 delta round (`R_delta_lenient = 1`).

**Target:** `tests/grid_delta_roundloop.rs`

**Given:**
- 2-worker net with one cross-partition CON-CON redex (one CON in
  worker A, one CON in worker B, connected via a shared `FreePort(K)`).
- `cfg = GridConfig { num_workers: 2, strict_bsp: false, ..Default::default() }`.
- `CapturingDispatch` that returns deterministically: round 1 → both
  workers report converged after the resolver-injected resolution.

**When:** Call `run_grid_delta(net, &cfg, &strategy, &mut dispatch);`

**Then:**
- `metrics.rounds == 1`
- `metrics.converged == true`
- `dispatch.round_start_dispatches.len() == 1`
- The single `RoundStart`'s `border_deltas` (or sibling resolver
  outputs) carries the immediately-resolved cross-partition reduction.

**Assertions:** `R_delta_lenient = 1` matches the spec literal.

**SPEC-19 R covered:** R40 line 242-243 + DC-C3 (FLIP).

---

### UT-0385-07: `run_grid_delta_strict_multi_round_cascade`  (DC-C3 cell `(true, true)`)

**Purpose:** R40 strict delta semantics — same partition pair under
`strict_bsp = true` requires ≥ 2 delta rounds (deferred dispatch:
round k's resolutions ship in round k+1's `RoundStart`).

**Target:** `tests/grid_delta_roundloop.rs`

**Given:** Same net + plan as UT-0385-06, but
`cfg.strict_bsp = true`. `CapturingDispatch` simulates: round 1 →
workers report border activity (resolver computes resolution but
defers dispatch); round 2 → workers receive deferred resolution and
converge.

**When:** Call `run_grid_delta`.

**Then:**
- `metrics.rounds >= 2` (deferred dispatch costs at least one extra
  round vs lenient mode).
- `metrics.converged == true`
- `dispatch.round_start_dispatches.len() >= 2`.

**Assertions:** `R_delta_strict ≤ N` for `N = number of partitions = 2`.

**SPEC-19 R covered:** R40 line 244 + DC-C3 (FLIP).

---

### UT-0385-08: `run_grid_delta_result_matches_run_grid_under_both_strict_modes`  (G1 parity / R38)

**Purpose:** G1 invariant under R38 amendment — `run_grid_delta`'s
output is isomorphic to `run_grid`'s output on the SAME input net,
under the SAME `strict_bsp` value. Verifies both `strict_bsp = true`
and `strict_bsp = false`.

**Target:** `tests/grid_delta_roundloop.rs`

**Given:** A 2-worker net with multiple local + one cross-partition
redex; two configs (`strict_bsp = true` and `strict_bsp = false`).

**When:** For each `strict_bsp` value, call BOTH `run_grid` (v1) and
`run_grid_delta` (v2); compare outputs.

**Then:**
- For `strict_bsp = true`: `run_grid(...).0 ~ run_grid_delta(...).0`
  (isomorphism — same Normal Form modulo agent ID renaming).
- For `strict_bsp = false`: `run_grid(...).0 ~ run_grid_delta(...).0`.

**Assertions:** G1 preserved across delta-mode + lenient/strict.
Use whatever isomorphism check the existing v1 test suite has (e.g.
`net.is_isomorphic_to(&other)` or structural match per SPEC-01 G1).

**SPEC-19 R covered:** R38 (G1 amendment) + DC-C3.

**Note for the developer:** if a strict isomorphism check is not
implemented, fall back to (a) bit-identity if the partitioner produces
deterministic AgentId allocation, OR (b) a weaker invariant like
"both nets produce the same `count_live_agents` and the same
`interactions_by_rule` totals". Document the chosen check in the
test body.

---

## Coverage mapping

| Requirement / DC | Covered by |
|---|---|
| R21 phase 1 (Round 0 dispatch) | UT-0385-01 |
| R21 phase 2 (multi-round delta loop) | UT-0385-02, UT-0385-03 |
| R23 (RoundStart payload construction) | UT-0385-04 (apply path) |
| R26 (RoundResult consumption) | UT-0385-04 |
| R26 + DC-B1 (partition cache symmetry) | UT-0385-05 |
| DC-C1 (ratified) — no `InitialPartition` ack wait | UT-0385-01 |
| DC-C2 (ratified) — synchronous trait calls | UT-0385-01..08 (test fixture is sync) |
| DC-C3 lenient cell `(delta_mode=true, strict_bsp=false)` | UT-0385-06 |
| DC-C3 strict cell `(delta_mode=true, strict_bsp=true)` | UT-0385-07 |
| R38 (G1 amendment) — output parity | UT-0385-08 |
| R40 (4-cell matrix completeness, delta cells) | UT-0385-06, UT-0385-07 |
| Per-round metric vector lengths | UT-0385-03 |

---

## Adversarial angles (QA candidates for Stage 5)

| # | Scenario | Why dangerous |
|---|---|---|
| QA-0385-A | Round-0 path waits on a sync `dispatch_initial` ack — DC-C1 violation by way of trait return value | UT-0385-01 fires (call count differs) |
| QA-0385-B | Lenient branch deletes the resolver call entirely | UT-0385-06 fires — no convergence in 1 round |
| QA-0385-C | Strict branch dispatches resolutions in same round (lenient behavior) | UT-0385-07 fires (convergence in 1 round, expected ≥ 2) |
| QA-0385-D | Per-round metric vectors append twice per round | UT-0385-03 fires (length mismatch) |
| QA-0385-E | `apply_deltas` skipped (deltas dropped on the floor) | UT-0385-04 fires |
| QA-0385-F | `coordinator_partition_cache` never updated post-Round-0 | UT-0385-05 fires |
| QA-0385-G | UT-0385-08 isomorphism check too weak — false-pass on a bug that breaks G1 only at the wire layer | Architecture-review concern; spec-critic may want a stricter check |
| QA-0385-H | `dispatch.dispatch_round_start` retried silently on Err | Out of scope per task notes; QA candidate to verify Err propagates as `(net, metrics_with_converged=false)` |

---

## Acceptance gate

- `cargo test --workspace --lib` floor: +5 new `#[test]` fns
  (inline tests UT-0385-01..05).
- `cargo test --workspace --tests` floor: +3 new `#[test]` fns
  (integration tests UT-0385-06..08 in `tests/grid_delta_roundloop.rs`).
- Total: +8.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo fmt --check` clean.
- No regression on v1 690-test baseline.

---

## Out of scope (deferred)

- Convergence predicate body → TEST-SPEC-0386.
- Final State Collection helper → TEST-SPEC-0387.
- `max_rounds` cap helper → TEST-SPEC-0388.
- Async `impl WorkerDispatch for CoordinatorConnection` → 2.26-C-wire.
- Retry / backoff policies on dispatch errors → out of bundle.
- E2E over-TCP integration → Phase 3 LAN.

---

## Gaps / open questions for the human

- **G1 isomorphism check granularity (UT-0385-08).** If the existing
  test suite lacks a true `is_isomorphic_to` predicate, the developer
  must pick the strongest available check (bit-identity if
  partitioner is deterministic; otherwise live-agent count +
  interactions_by_rule totals). Spec-critic to rule on the
  acceptable strictness; this TEST-SPEC documents the fallback path.
- **`#[ignore]` gating on UT-0384-01.** TEST-SPEC-0384's UT-0384-01
  may need `#[ignore]` until UT-0385-* land. Once TASK-0385 is
  implemented, the human should flip `#[ignore]` off on UT-0384-01.
