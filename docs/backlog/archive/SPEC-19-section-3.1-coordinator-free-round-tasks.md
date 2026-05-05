# Bundle: SPEC-19 §3.1 — Coordinator-Free Round (item 2.34)

**Created:** 2026-04-16
**Owner:** task-splitter (orchestrated by sdd-pipeline)
**Stage:** 1 SPLITTING — complete; awaiting Stage 2 TESTS dispatch.
**Test baseline before bundle:** 850 lib + 4 integration (post-SPEC-18 ship).
**Hard floor (CLAUDE.md):** 850 lib tests post-bundle.
**Estimated total LoC:** ~300 across 4 atomic tasks (each <200 LoC).
**Tier 1 break-even path (V2-FEATURE-MATRIX):** confirmed next after item 2.23
  (`2.22 → 2.23 → 2.34 → 2.24 → 2.25 → 2.35 → 2.26`).

## Scope (in vs out)

**In scope (SPEC-19 §3.1, R1-R7):**
- R1 — worker inspects `free_port_index` post-`reduce_all` for principal-port
  border endpoints.
- R2 — `has_border_activity: bool` field on `WorkerRoundStats` is the carrier
  for the worker's R1 result; rides inside the existing
  `Message::PartitionResult { stats, … }` payload (no new variant — R7).
- R3 — coordinator tracks per-worker activity; MAY skip merge when ALL workers
  report `has_border_activity == false`.
- R4 — Global Normal Form termination: coordinator MUST terminate when ALL
  workers report `has_border_activity == false` AND `local_redexes == 0`.
- R5 — confluence safety (T4 guarantees identical Normal Form whether merge
  ran or not).
- R6 — SHOULD use under strict BSP. Bundle decision: implement the skip ONLY
  when `strict_bsp == true && coordinator_free_rounds == true` (lenient mode
  collapses to 1 round anyway; SHOULD allows this restriction).
- R7 — compatibility with both v1 protocol and v2 wire format from SPEC-18.
  The `has_border_activity` field is an additive payload change to an existing
  `Message` variant; bincode v2 with `config::standard()` handles struct field
  append safely as long as both sides are rebuilt together (this bundle ships
  them together).

**Out of scope (separate bundles, do NOT pull in):**
- §3.2 BorderGraph and delta-based merge (item 2.35).
- §3.3 full Delta-Only Protocol with stateful workers (item 2.26).
- §3.4 new `Message` variants (`InitialPartition`, `RoundStart`, `RoundResult`,
  `FinalStateRequest`, `FinalStateResult`) — all delta-protocol-only.
- §3.5 invariant amendments (G1, D3, D6 reformulations) — design exists, formal
  proof is OQ-1, ARG-005 work item.
- §3.6 `delta_mode` config (this bundle adds only `coordinator_free_rounds`).
- §3.7 R45 per-round delta byte / time vectors — only the
  `coordinator_free_rounds: u32` counter is shipped here (R45 partial).

## Wire change in scope

The orchestrator brief explicitly authorises a bincode v2 message-format
change for the new `WorkerRoundStats.has_border_activity` field, with the
hard constraint that no existing serde test (SPEC-18 baseline) breaks.
Validation:

- The change is **additive within an existing `Message::PartitionResult`
  variant** — no discriminant shuffling, no new variant.
- bincode v2 with `config::standard()` (post-SPEC-18 TASK-0343) serialises
  structs field-by-field via `#[derive(Serialize)]`; appending a `bool`
  field at the end of `WorkerRoundStats` is forward-compatible **as long
  as encoder and decoder are rebuilt together** — which holds because this
  bundle changes both ends in the same compilation unit.
- Existing tests (`merge::types::tests::test_worker_round_stats_serde`,
  `frame::tests::v2_pipeline_round_trip_all_message_variants_*`) continue
  to pass because they construct fresh `WorkerRoundStats` literals on
  both sides and round-trip them; both sides see the new field.

## Task graph (DAG)

```
TASK-0348 (S, ~80)  ─┬─→ TASK-0349 (S, ~50)  ─┐
                     │                          ├─→ TASK-0351 (M, ~130)
                     └─→ TASK-0350 (S, ~40) ───┘
                          (parallelisable
                           with 0349)
```

| ID | Title | Spec Reqs | Size | LoC est. | Depends |
|------|---------------------------------------------------------|-------------|------|----------|----------------|
| 0348 | Add `has_border_activity` field + `compute_border_activity` helper | R1, R2 | S | ~80 | none |
| 0349 | Populate `has_border_activity` at every WorkerRoundStats build site | R2 | S | ~50 | 0348 |
| 0350 | Add `coordinator_free_rounds` config flag + metrics counter | R6, R41p, R45p | S | ~40 | none (logical: 0348) |
| 0351 | Coordinator skip-merge logic + Global Normal Form termination | R3, R4, R5, R6, R7 | M | ~130 | 0348, 0349, 0350 |

**Total:** ~300 LoC, all <200 per task. No cycles. Implementable in topological
order: 0348 → 0349 ‖ 0350 → 0351.

## Per-task v1+v2 protocol compatibility (R7)

Each task individually preserves R7:

- **TASK-0348:** field added under existing `serde` derive — both v1 (in-process
  call sites) and v2 (wire format from SPEC-18) carriers read/write the same
  bincode v2 layout.
- **TASK-0349:** workers populate the field in BOTH the local-simulation path
  (`merge::grid::run_grid`) AND the wire-protocol path (`worker.rs`,
  `protocol/worker.rs`), so the value is available regardless of transport
  mode.
- **TASK-0350:** new config field defaults to `false` (R43 SHOULD for v1
  mode); zero existing call site changes behavior.
- **TASK-0351:** skip logic is implemented in `run_grid` (the local path);
  the wire FSM in `protocol/coordinator.rs` is untouched in this bundle.
  When item 2.26 ships, the FSM gains the same skip branch using the
  already-populated field — zero rework.

## Spec ambiguities flagged for spec-critic (none escalated)

After full spec read-through against the four task drafts, no ambiguity in
R1-R7 rises to the level of needing spec-critic review before TESTS:

- R3's MAY ("MAY skip the merge-redistribute cycle") is converted to an
  opt-in flag (`coordinator_free_rounds`) in TASK-0350 — standard MAY
  handling.
- R6's SHOULD ("SHOULD be the primary convergence acceleration mechanism
  in strict BSP mode") is honored by gating the skip on `strict_bsp = true`
  in TASK-0351; the documented design choice is that lenient mode already
  collapses to 1 round, making the optimization moot. This is a routine
  SHOULD interpretation, not a spec gap.
- R7's wording ("Under v1, the coordinator simply does not send
  `AssignPartition` for the skipped round") maps cleanly onto the
  local-simulation analog ("do not call merge/split/dispatch this round")
  in TASK-0351.

If the reviewer or QA agent flags any of the above as ambiguous after
implementation, route to spec-critic at that point. **No pre-implementation
spec-critic dispatch is needed for this bundle.**

## Acceptance gate for the whole bundle

- All 4 tasks shipped GREEN through Stages 3-6.
- Test count ≥ 850 lib tests (CLAUDE.md hard floor) and ≥ 4 integration tests.
- Clippy clean (`cargo clippy --workspace --all-targets -- -D warnings`).
- fmt clean.
- Release smoke `compute add 3 5 → 8` works.
- G1 spot check: `church_add(2, 3)` at w=2 strict BSP produces identical
  decoded result with `coordinator_free_rounds` toggled on vs off (TASK-0351
  test).

## Stage advancement

- **Stage 1 SPLITTING:** complete (this file + 4 TASK-NNNN.md files).
- **Stage 2 TESTS:** ready to dispatch — invoke `test-generator` with bundle
  spec = SPEC-19 §3.1 (R1-R7), task list = TASK-0348..0351, deliverable =
  `docs/tests/TEST-SPEC-0348.md` … `TEST-SPEC-0351.md`.
- Orchestrator pause requested by parent: STOP after Stage 1 and confirm
  before Stage 2 dispatch.
