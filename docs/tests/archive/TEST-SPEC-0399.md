# TEST-SPEC-0399: Integrate D-004 finalizer + flip `SKIP_ASYMMETRIC = false`

**See also:** [docs/backlog/TASK-0399.md](../backlog/TASK-0399.md); [docs/tests/TEST-SPEC-0385.md](./TEST-SPEC-0385.md) UT-0385-06..08.

**Task:** TASK-0399
**Spec:** SPEC-19 §3.3 R26 end-to-end, §3.5 R38 (G1 reformulation — full 6-rule parity).
**Spec-critic verdicts consumed:**
  - **DC-0399-A (proposed, option a):** per-result register_minted_agents call (diagnostic attribution).
  - **DC-0399-B (proposed, option a):** propagate ProtocolViolation via `?` to loop Err branch.
  - **DC-0399-C (proposed, option a):** keep `package_resolutions` post-migration.
**Generated:** 2026-04-23
**Baseline before this task:** 1146 lib default / 1186 lib `--features zero-copy` (post-TASK-0398).
**Cumulative target after this task:** **unchanged count-wise** (1146 / 1186) — TASK-0399 does not add `#[test]` fns; it flips SKIP_ASYMMETRIC=false which activates existing 12 cases within UT-0385-08. Smoke test UT-0399-S adds one new test fn (+1 default / +1 zero-copy = 1147 / 1187).

---

## Scope note

Unlike TASK-0398, this task is INTEGRATION-level. It wires the pure-core primitives into the BSP loop and verifies full 6-rule G1 parity. The key acceptance signal is UT-0385-08 passing all 12 cases (6 fixtures × 2 strict modes) with `SKIP_ASYMMETRIC = false`.

Changes tracked here:
1. **UT-0385-08 activation** (existing test, now runs 12 cases): assertions already in the test body, gated by SKIP_ASYMMETRIC. Flipping the const re-activates them.
2. **UT-0399-S smoke test** (new): end-to-end drive of `run_grid_delta` on a CON-DUP fixture, asserting the pending_new_borders lifecycle fires correctly (enqueue → mint echo → register → add_border_states → observable in final_partitions).

---

## Test target file paths

- `relativist-core/src/merge/grid_delta_integration_tests.rs` — modify UT-0385-08 body (no new `#[test]` fn); add 1 new `#[test]` fn UT-0399-S.

All tests synchronous. No tokio, no async.

---

## Activated Tests (existing, gated by SKIP_ASYMMETRIC flip)

### UT-0385-08 (all 12 cases): `run_grid_delta_result_matches_run_grid_under_both_strict_modes`

**Previously gated behavior:** 6 symmetric cases ran full assertions; 6 asymmetric cases (CON-DUP, CON-ERA, DUP-ERA × 2 strict modes) ran convergence-only assertions.

**Post-TASK-0399 behavior:** all 12 cases run FULL assertions:
- `out_v1` from `run_grid` converges (`metrics.converged == true`).
- `out_delta` from `run_grid_delta` converges.
- `canonicalize(out_v1) == canonicalize(out_delta)` — canonical net equivalence (DC-0395-B option a).
- `metrics_v1.total_interactions == metrics_delta.total_interactions`.
- Diagnostic message on failure: `"fixture={name} strict={strict}"` per DC-0395-C.

**Expected:** all 12 cases pass. A failure in any asymmetric case fires with the diagnostic message pinpointing the fixture + strict mode combination.

**SPEC-19 R covered:** §3.5 R38 (G1 reformulation empirical verification for all 6 IC rules).

**Closes:** DEFERRED-WORK D-003 acceptance signal: "2-worker grid converges on an input that requires at least one border-redex resolution... final merge() reconstructs the same output net that v1 produces on identical input" — now true for ALL 6 rules.

---

## New Test

### UT-0399-S: `dc_b5_round_n_plus_2_finalizer_smoke_observable_pending_lifecycle`

**Purpose:** adversarial smoke — drive `run_grid_delta` on a CON-DUP fixture and observe that:
1. `border_graph.pending_new_borders` is NON-empty at the end of the round in which the CON-DUP redex resolves (post-resolver, pre-dispatch of round N+1).
2. `border_graph.pending_new_borders.is_empty()` after the round N+2 `register_minted_agents` fires.
3. `border_graph.borders` gains the newly-materialized borders from `add_border_states`.
4. The output net is G1-parity with `run_grid`.

**Target:** `grid_delta_integration_tests.rs` — NEW `#[test]` fn (not gated by SKIP_ASYMMETRIC).

**Given:** `build_fixture_con_dup()` — CON-DUP cross-partition redex.

**When:** Run `run_grid_delta(net.clone(), &cfg, &ContiguousIdStrategy, &mut dispatch)` where `cfg = { num_workers: 2, delta_mode: true, ..Default::default() }`. **Observability hook:** expose `border_graph` state via `LocalDeltaDispatch` (add an `observed_pending_borders_per_round: Vec<Vec<PendingNewBorder>>` accumulator field to the dispatch struct, populated inside `dispatch_round_start` by inspecting the graph state before round N+1 dispatch). This requires `LocalDeltaDispatch` to hold a clone of `border_graph` post-resolver — or, simpler, inspect the RESULT state at the end of the run.

**Simpler approach:** don't observe mid-loop state; inspect final state:
- After `run_grid_delta` returns, compare metrics:
  - `metrics.rounds >= 3` — CON-DUP requires at least 3 rounds (N emit, N+1 mint, N+2 finalize).
  - Final output net has the expected CON-DUP commutation agents (4 new agents materialized from the 2-original agents after CON-DUP commute).
- Compare to `run_grid`'s output on the same input; canonical equivalence holds.

**Then:**
- `result_v1.1.converged == true` AND `result_delta.1.converged == true`.
- `canonicalize(result_v1.0) == canonicalize(result_delta.0)`.
- `result_delta.1.rounds >= 3` (round counter confirms the 3-round DC-B5 cycle fired).
- `result_delta.1.total_interactions == result_v1.1.total_interactions`.

**Assertions:** End-to-end DC-B5 lifecycle visible via round count + G1 parity.

**SPEC-19 R covered:** R26 end-to-end (wire → worker mint → wire echo → coordinator consume → border materialize), DC-B5 full cycle.

---

## Coverage mapping

| Requirement | Covered by |
|---|---|
| SPEC-19 §3.5 R38 G1 parity, all 6 rules | UT-0385-08 post-flip |
| DC-B5 full 3-round cycle observable | UT-0399-S |
| DEFERRED-WORK D-003 acceptance signal | UT-0385-08 post-flip (canonical equivalence) |
| DEFERRED-WORK D-004 closure | UT-0385-08 post-flip + UT-0399-S |

---

## Adversarial angles (QA candidates for Stage 5 if invoked)

| # | Scenario | Notes |
|---|----------|-------|
| QA-0399-A | BSP run with >3 consecutive CON-DUP resolutions (nested pending resolution across rounds N+2, N+3, N+4, ...) | UT-0398-05's multi-round test covers partial-resolution semantics at the BorderGraph level; UT-0399-S covers a single 3-round cycle. A chain of CON-DUPs triggers multi-generation pending; UT-0385-08 exercises it under the `cascade_cross`-like fixture indirectly. |
| QA-0399-B | Worker echoes MintedAgent with `minted_agent_id` in the coordinator-reserved range (u32::MAX - 10_000 .. u32::MAX) | Worker-side invariant; should never happen under correct worker impl. If injected adversarially, `register_minted_agents` accepts the mint (no range check), but `add_border_states` will panic downstream via debug_assertions on AgentId validity. Flag for separate hardening. |
| QA-0399-C | Concurrent `enqueue_pending_borders` + `register_minted_agents` calls (race) | BSP is sequential; no concurrency. Not applicable. |
| QA-0399-D | Very large pending_new_borders accumulation (>10k entries) | Linear-scan cost O(N*M) becomes noticeable at scale. Not a correctness concern; performance optimization is future work. |

---

## Acceptance gate

1. `cargo test --workspace --lib` count: 1146 → **1147** (+1 from UT-0399-S; UT-0385-08 remains 1 fn).
2. `cargo test --workspace --lib --features zero-copy` count: 1186 → **1187** (+1).
3. `cargo test merge::grid_delta_integration_tests::ut_0385_08 -- --nocapture` — ALL 12 cases pass (pre-flip: 6 cases skip asymmetric assertions; post-flip: 12 full assertions).
4. `cargo test merge::grid_delta_integration_tests::dc_b5_round_n_plus_2_finalizer_smoke_observable_pending_lifecycle` — green.
5. `cargo clippy --workspace --all-targets -- -D warnings` clean (default + zero-copy).
6. `cargo fmt --check` clean.
7. No regression on existing tests. R42 smoke, UT-0394-*, UT-0396-*, UT-0397-*, UT-0398-* all still green.
8. `const SKIP_ASYMMETRIC: bool = false;` at line ~541 of grid_delta_integration_tests.rs.
9. Docblock comments in grid_delta_integration_tests.rs (L516-530) updated to reflect D-004 closure.
10. **DEFERRED-WORK.md:** D-003 and D-004 both in "Resolved Deferrals" section with commit hash.
11. **V2-FEATURE-MATRIX.md:** row 2.26 = `DONE`; M4 criterion de-PARTIAL-ized.

---

## Notes on interaction with other TEST-SPECs

- **TEST-SPEC-0385 UT-0385-06/07/08:** UT-0385-06/07 unchanged (symmetric rules always passed). UT-0385-08 activates asymmetric branches.
- **TEST-SPEC-0398 UT-0398-03/04/05:** unit-level tests of register_minted_agents; independent of this task's integration.
- **TEST-SPEC-0394 UT-0394-*:** worker-side tests; unchanged (the worker continues to mint and echo correctly; D-004 just plumbs the coordinator-side consumption).
- **TEST-SPEC-0396 UT-0396-*:** dispatcher fork tests; unchanged.
- **TEST-SPEC-0397 UT-0397-*:** R43 normalize tests; unchanged.

---

## Out of scope

- **Performance benchmarks** of register_minted_agents on large pending_new_borders — future work.
- **Coordinator-TCP binding** (actual production `WorkerDispatch` implementation over TCP with real workers) — deferred beyond D-004, needs separate async binding work (DC-C2).
- **rkyv serialization of `pending_new_borders` / `resolved_mints`** — coordinator-local state; does not cross the wire. Explicitly not derived.
- **Deletion of `package_resolutions`** — kept for backward-compat per DC-0399-C.
- **Passo 6 M1 exit measurement** — now unblocked by this task; executed as the NEXT bundle.
