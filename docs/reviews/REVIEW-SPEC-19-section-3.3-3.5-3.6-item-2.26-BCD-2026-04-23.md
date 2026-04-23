# Review: SPEC-19 §3.3 + §3.5 + §3.6 — Item 2.26 B/C/D — Unified Stage 4

**Reviewer:** reviewer agent (manual, Option B precedent — matches REVIEW-SPEC-19-section-3.4 format)
**Date:** 2026-04-23
**Bundle scope:** Three DEV-complete sub-bundles of item 2.26, reviewed jointly per user directive "full review at the end, when all features are implemented":

- **2.26-B** — SPEC-19 §3.3 item 2.26-B — Coordinator-side border-redex resolution (R13, R14, R15 parts 1-2, R19), TASK-0372..TASK-0377.
- **2.26-C** — SPEC-19 §3.3 item 2.26-C — Coordinator wire-layer dispatch + `run_grid_delta` BSP loop + stateful worker handlers (R20-R30 operational).
- **2.26-D** — SPEC-19 §3.5 + §3.6 — Invariant amendments (R38-R40 narrative) + `GridConfig.delta_mode` + `run_grid_delta` gating (R41-R42).

**Files reviewed (primary):**

| Path | LoC | Sub-bundle |
|---|--:|:-:|
| `relativist-core/src/merge/border_resolver.rs` | 2600 | 2.26-B |
| `relativist-core/src/merge/internal/mod.rs` | 8 | 2.26-B |
| `relativist-core/src/merge/internal/pure_core_guard.rs` | 62 | 2.26-B |
| `relativist-core/src/merge/grid.rs` (delta additions only: L305–L784, L2400–end) | 3552 | 2.26-C |
| `relativist-core/src/worker.rs` (delta additions: `WorkerDeltaState`, `handle_initial_partition`, `handle_round_start`, `handle_final_state_request`) | 1067 | 2.26-C |
| `relativist-core/src/merge/types.rs` (`GridConfig.delta_mode`, `GridMetrics.delta_mode/.delta_max_rounds_hit`, `WorkerDispatch`, `RoundResultPayload`) | 733 | 2.26-C/D |
| `relativist-core/src/config.rs` (CLI `--delta-mode` on `CoordinatorArgs` / `LocalArgs`, plumbing into `build_grid_config[_from_local]`) | ~1150 (new: ~30) | 2.26-D |

**Files confirmed NOT modified in 2.26-B/C/D (expected):** `protocol/messages.rs` (wire variants shipped 2.26-A), `coordinator.rs` (async `impl WorkerDispatch for CoordinatorConnection` explicitly deferred per DC-C2).

**Test counts (per `docs/pipeline-state.md` L14):** baseline 1009 → **1039** lib default (+30) / baseline 1049 → **1079** lib `--features zero-copy` (+30). Gates green at 2.26-B close; 2.26-C and 2.26-D DEV counts folded into the same figure because no commit happened between bundles (user directive).

**Code quality verdict:** **PASS WITH NOTES**
**Architecture verdict:** **ALIGNED** (SPEC-13 R6-R8 preserved; DC-C2 `WorkerDispatch` pure-core trait honoured; DC-B8/B9 forbidden-imports canary shipped).
**Spec compliance:** SPEC-19 §3.3 R13, R14, R15 parts 1-2, R19, R20, R21 (phases 1-3), R22, R23 (partial — see MF-001), R24, R25, R26 (partial — see MF-001), R27, R28, R29, R30; §3.5 R38, R39, R40 (narrative + R40 operational via the BSP loop); §3.6 R41, R42, R43 (documented), R44 (documented). R45-R47 (delta metrics) are **explicitly out of scope** per the 2.26-D bundle index and are not a deficiency of this review slice.

---

## Executive Summary

Three sub-bundles compose a coherent delta-protocol vertical slice that closes DEFERRED-WORK D-003 in principle (R13/R14/R15 parts 1-2 all ship with ~2600 LoC of resolver code plus the `run_grid_delta` BSP loop calling into it). The pure-core discipline is preserved end-to-end: `merge/border_resolver.rs` passes the programmatic R19 canary (DC-B8/B9), and `run_grid_delta` takes `&mut dyn WorkerDispatch` rather than pulling tokio into `merge/` (DC-C2).

The review surfaces **two Must-Fix issues** — one real spec-compliance gap (MF-001: the worker never acts on `RoundStart.pending_commutations` and never populates `RoundResult.minted_agents`, so CON-DUP and CON-ERA/DUP-ERA resolutions cannot actually complete end-to-end despite the resolver emitting them), and one acceptance-signal gap (MF-002: D-003's mandated integration test — "2-worker grid converges on an input that requires at least one border-redex resolution... final `merge()` reconstructs the same output net that v1 produces" — is not present; the `tests/grid_delta_roundloop.rs` file cited in `grid.rs:2557` as the G1-parity home does not exist). Five Should-Fix items and a handful of Nice-to-Have notes round out the report.

**Recommended Stage 5 QA focus:** drive the worker FSM through a `RoundStart` containing a real `pending_commutations` vector and confirm the current code path silently drops it (i.e., prove MF-001 empirically); build a 2-worker G1-parity harness comparing `run_grid` vs `run_grid_delta` outputs on a CON-CON cross-partition redex.

---

## Spec Compliance Matrix

### 2.26-B — SPEC-19 §3.2 R13, R14, R15 parts 1-2, R19 (resolver)

| Req | Scope | Status | Evidence |
|---|---|:-:|---|
| **R13** | Coordinator dispatches border redex locally, reads agents from worker partitions. | ✅ | `border_resolver.rs:448-525` (`resolve_border_redex`): dispatches on `(sym_a, sym_b)` to one of 6 rule bodies; `materialize_agent` (L388-398) + `assert_agent` (L404-418) read from `&[Partition]` per DC-B1. |
| **R14** | 6 IC rules (CON-CON, DUP-DUP, ERA-ERA, CON-DUP ±, CON-ERA ±, DUP-ERA ±) all present; no new rules. | ✅ | Dispatch match at L475-521 covers all 9 symmetric cells; 6 distinct rule bodies (`resolve_con_con`, `resolve_dup_dup`, `resolve_era_era`, `resolve_con_dup`, `resolve_non_era_era` — shared by CON-ERA / DUP-ERA under canonical orientation). Matches SPEC-03 `interact_anni`/`interact_comm`/`interact_eras`/`interact_void` without calling them (pure-core mirror — R19). |
| **R15 part 1** | Package port updates as `BorderDelta`s keyed to workers. | ✅ | `WorkerDeltas { border_deltas, local_reconnections }` at L93-96; `package_resolutions` (L196-241) fans per-worker buckets. DC-B3 split (`local_reconnections` separate from `border_deltas`) faithfully implemented. |
| **R15 part 2** | Update `BorderGraph` after resolution via `remove_border` or `apply_deltas`. | ✅ | `resolve_border_redex` L523 calls `graph.remove_border(border_id)` unconditionally after each dispatch body. The three same-symbol paths emit no new borders; the CON-DUP / asymmetric paths emit `pending_new_borders` for round-N+2 finalization (DC-B5). |
| **R19** | `merge/` pure-core: no tokio, no async, no protocol dependency. | ✅ | `merge/internal/pure_core_guard.rs` (62 LoC, `#[cfg(test)]`) defines 5 `FORBIDDEN_USE_PREFIXES` (`use tokio`, `use async_trait`, `use crate::protocol`, `use crate::coordinator`, `use crate::worker`) per DC-B9; one test at `border_resolver.rs:2530-2564` includes a cardinality canary asserting `len() == 5` and a per-prefix presence check. |

### 2.26-B Design-Choice (DC-B1..B9) Compliance

| DC | Rule | Status | Evidence |
|---|---|:-:|---|
| **DC-B1** | Coordinator holds shadow `&[Partition]` cache; resolver is pure. | ✅ | `resolve_border_redex` signature L448-454 takes `partitions: &[Partition]`; mutation is on `graph` + allocators only. Cache maintenance lives in 2.26-C per DC-B1 contract (confirmed: `run_grid_delta_inner` at `grid.rs:475-482` + L568-608 applies deltas to `partitions_vec`). |
| **DC-B2** | `materialize_agent` keeps `Option` return; caller-side `assert_agent` helper panics uniformly. | ✅ | Helper at `border_resolver.rs:404-418`; panic message includes "border_resolver: agent missing for border", side name, "cache desync", and "DC-B1". Verified by `resolve_border_redex_panics_with_dc_b2_message_on_missing_agent` (L1437-1491). |
| **DC-B3** | `BorderDelta` unchanged (2 fields); `local_reconnections` travels as a sibling `Vec<(PortRef, PortRef)>` on `WorkerDeltas` / `RoundStartDispatch`. | ✅ | `WorkerDeltas` L93-96; `RoundStartDispatch` L162-168 includes `local_reconnections`. Wire-layer sibling field on `Message::RoundStart` confirmed in `protocol/frame.rs` tests (delta_wire_tests.rs:692). |
| **DC-B4** | Border pinning in worker `reduce_all` — R40c invariant (amendment owned by 2.26-D). | ⚠️ PARTIAL | The resolver's documentation cites the pinning discipline; but `handle_round_start` (`worker.rs:326-413`) calls vanilla `reduce_all(&mut state.partition.subnet)` at L363 with NO pinning filter, and no `reduce_all_with_skip` variant is added. The race window DC-B4 was designed to close is **not mechanically closed** — if a border-principal agent has a locally-adjacent redex it would still be consumed locally in the same round the coordinator resolves its border. See MF-003. |
| **DC-B5** | 2-phase AgentId allocation: coordinator emits `CommutationBatch`; worker allocates from its `IdRange`; `minted_agents` echoes back. | ⚠️ PARTIAL | Coordinator side is present (L644-786 emits `pending_commutations` + `pending_new_borders`). **Worker side is NOT wired** — `handle_round_start` signature at `worker.rs:326-332` does not take `pending_commutations`, and `Message::RoundResult.minted_agents` is hard-coded to `Vec::new()` at L410. See **MF-001**. |
| **DC-B6** | CON-ERA / DUP-ERA preserve existing auxiliary border via `apply_deltas` semantics, not `add_border_states`. | ✅ | `emit_erasure_principal` L937-994: the `Some(existing)` branch reuses `bid` in a `PendingNewBorder`; the resolver NEVER calls `graph.add_border_states` (verified by test `resolve_con_era_preserves_auxiliary_border_via_apply_deltas` L1791-1836 and UT-0376-05/06). |
| **DC-B7** | `resolved_borders: Vec<(u32, WorkerId, WorkerId)>` triples; `package_resolutions` fans into both sides. | ✅ | `BorderResolution.resolved_borders: Vec<(u32, WorkerId, WorkerId)>` L114; fan at L211-216 in `package_resolutions`. The self-border guard `wa != wb` at L213 prevents double-push on same-worker borders. |
| **DC-B8** | Shared `assert_no_forbidden_imports` helper in `merge/internal/pure_core_guard.rs`; opt-in per file. | ✅ | Helper L45-62; module wiring `merge/internal/mod.rs` L7-8 (`#[cfg(test)] pub(crate) mod pure_core_guard`). Only `border_resolver.rs` opts in this bundle (by design — `border_graph.rs` migration deferred to ROADMAP 2.43). |
| **DC-B9** | Forbidden-prefix list includes `use crate::coordinator` and `use crate::worker`. | ✅ | `FORBIDDEN_USE_PREFIXES` constant at L29-35 carries all 5 entries. Cardinality canary in `border_resolver.rs:2536-2544` triggers if the list shrinks. |

### 2.26-C — SPEC-19 §3.3 R20-R30 (BSP loop + worker lifecycle)

| Req | Scope | Status | Evidence |
|---|---|:-:|---|
| **R20** | `GridConfig.delta_mode` gates `run_grid_delta`. | ⚠️ | `run_grid_delta` entry at `grid.rs:339-385` carries a `TODO(2.26-D)` at L346-349 saying "once `GridConfig.delta_mode` lands, assert it here. Until then, this function is unconditionally delta-mode at entry." 2.26-D has landed `delta_mode` (it's in `merge/types.rs:318`), but the assert was NOT added. However, there is no gate between v1 `run_grid` and `run_grid_delta` anywhere in the codebase — nothing reads `cfg.delta_mode` to choose which entry point runs. See **SF-001**. |
| **R21 phase 1** | Round 0 `InitialPartition` fire-and-forget (DC-C1). | ✅ | `run_grid_delta_inner` L433 calls `dispatch.dispatch_initial(&plan)?` exactly once; worker handler at `worker.rs:272-289` stores partition + transitions to `DeltaIdle` without emitting any ack. |
| **R21 phase 2** | Rounds 1+ delta round: `RoundStart` → `RoundResult`. | ✅ | `grid.rs:457-610` round loop calls `dispatch.dispatch_round_start(&pending_dispatch)` → results drain into `BorderGraph.apply_deltas` → `resolve_border_redex` → `package_resolutions` → next `pending_dispatch`. |
| **R21 phase 3** | `FinalStateRequest` + `FinalStateResult` + final `merge()`. | ✅ | `run_grid_delta_final_collect` L716-760 dispatches `FinalStateRequest`, reassembles partitions (with in-process test fallback), calls `super::core::merge(plan)`. |
| **R22** | Worker stores partition in persistent local state across rounds. | ✅ | `WorkerDeltaState { partition, previous_border_state, round }` at `worker.rs:100-105`; `WorkerContext.delta_state: Option<WorkerDeltaState>` at L132. |
| **R23** | `RoundStart` payload: `round`, `border_deltas`, `resolved_borders`, `new_borders`, `local_reconnections` (DC-B3), `pending_commutations` (DC-B5). | ⚠️ PARTIAL | The *wire* variant carries all six fields (confirmed by `protocol/frame.rs` and the 2.26-A review). The *worker handler* `handle_round_start(ctx, round, border_deltas, resolved_borders, new_borders)` at `worker.rs:326-332` takes only 5 arguments — `local_reconnections` and `pending_commutations` are silently unused. See **MF-001**. |
| **R24** | 5-step pipeline (apply deltas → reduce → rebuild index → diff → report). | ⚠️ | Steps 1/2/3/4/5 present at `worker.rs:352-412`, BUT step 1 is missing two of R23's sub-fields (see MF-001). Step 2 uses vanilla `reduce_all` without the DC-B4 pinning skip (MF-003). |
| **R25** | Worker maintains `previous_border_state` seeded from `partition.free_port_index` (DC-C4 Option B). | ✅ | `WorkerDeltaState::from_initial_partition` at `worker.rs:111-118` seeds from `partition.free_port_index.clone()`; round N updates at `worker.rs:379` snapshot the freshly-rebuilt `free_port_index`. |
| **R26** | `RoundResult` payload: `round`, `border_deltas`, `stats`, `has_border_activity`, `minted_agents`. | ⚠️ | Wire variant carries all 5 fields; **worker's `Message::RoundResult` emission at `worker.rs:405-411` hard-codes `minted_agents: Vec::new()`**. The worker never allocates any agents in response to `pending_commutations` because it never receives them (see MF-001). |
| **R27** | Coordinator dispatches `FinalStateRequest` to every worker at GNF. | ✅ | `run_grid_delta_final_collect` L727 calls `dispatch.dispatch_final_state_request(final_round)`. |
| **R28** | Worker responds with `FinalStateResult` carrying its current full partition. | ✅ | `handle_final_state_request` at `worker.rs:437-458` extracts the partition via `take()` and emits `Message::FinalStateResult`. |
| **R29** | Coordinator performs final `merge()` from collected partitions + remaining `BorderGraph`. | ✅ | `reconstruct_partition_plan_from_collected` L771-784 + `super::core::merge(plan)` at L757. Includes the cache-fallback path (L735-753) for in-process tests where the dispatch returns an empty `Vec`. |
| **R30** | `max_rounds` cap preserved; GNF not required to enter Final Collection. | ✅ | `check_max_rounds_cap` L682-687 checked at loop head (L459-463); on cap hit, `metrics.delta_max_rounds_hit = Some(true)`, `metrics.converged = false`, then break into `run_grid_delta_final_collect` regardless. |

### 2.26-C Design-Choice (DC-C1..C6) Compliance

| DC | Rule | Status | Evidence |
|---|---|:-:|---|
| **DC-C1** | Fire-and-forget Round 0; no `RoundStartAck` variant. | ✅ | `dispatch.dispatch_initial(&plan)?` returns `()` — no ack. Worker `handle_initial_partition` emits only a log action. No new `Message` variant. |
| **DC-C2** | Pure-core `run_grid_delta` + sync `WorkerDispatch` trait; async binding deferred. | ✅ | Trait defined in `merge/types.rs:206-227` (synchronous methods, no `Send + Sync`). `run_grid_delta` signature at `grid.rs:339-344` takes `&mut dyn WorkerDispatch` — no tokio import in `merge/grid.rs` (pure-core canary not yet opted in for this file, but no tokio is imported either). |
| **DC-C3** | `strict_bsp × delta_mode` fully orthogonal; branch at round loop, no assert. | ✅ | `run_grid_delta` L346-349 explicitly refuses to assert; branch at `grid.rs:517-562` runs strict-pre-resolve vs lenient-post-resolve convergence checks. |
| **DC-C4** | Seed `previous_border_state` from `partition.free_port_index` at Round 0. | ✅ | `worker.rs:112` `let previous_border_state = partition.free_port_index.clone();`. |
| **DC-C5** | Three-conjunct GNF predicate: `!has_border_activity && local_redexes == 0 && graph.detect_border_redexes().is_empty()`. | ✅ | `check_delta_convergence` at `grid.rs:653-661` — all three conjuncts with `&&`. Rationale documented in the doc-comment (L635-651). |
| **DC-C6** | Disconnection via `DISCONNECTED = PortRef::FreePort(u32::MAX)` sentinel; no `Option<PortRef>`. | ✅ | `BorderDelta.new_target: PortRef` (frozen by SPEC-19 R33 + DC-1 precedent). Worker's delta computation uses the helper `compute_outgoing_deltas` which emits `DISCONNECTED` on erased borders (confirmed by 2.26-A wire tests). |

### 2.26-D — SPEC-19 §3.5 R38-R40 + §3.6 R41-R42

| Req | Scope | Status | Evidence |
|---|---|:-:|---|
| **R38** | G1 reformulation to `run_grid_delta`; formal proof deferred to §8. | ✅ | `GridConfig.delta_mode` doc-comment `merge/types.rs:285-317` explicitly states "functionally equivalent to the v1 loop up to isomorphism (G1 amendment, R38)" and cites T4 strong confluence. Per the 2.26-D design-choices doc, AMB-D-3 ruling (Option C) requires the ROADMAP narrative to say "proof deferred" for R38. I could not verify the ROADMAP edit directly from the source (ROADMAP.md not in the review scope), but the code-side reformulation anchor is present. |
| **R39** | D3 amendment — `BorderGraph.detect_border_redexes` replaces exhaustive scan. | ✅ | Operational behaviour lives in the BSP loop (`grid.rs:510` calls `border_graph.detect_border_redexes()`). |
| **R40** | D6 amendment — lenient (R_delta_lenient = 1) vs strict (R_delta_strict ≤ N); GNF progress guarantee. | ✅ | Operational discharge per AMB-D-3: lenient at `grid.rs:559-562`; strict at L517-524; GNF predicate at L653-661. Progress guarantee follows from `metrics.total_interactions += local_interactions_this_round + border_interactions_this_round` at L553 — every round adds ≥ 0, bounded by T7. |
| **R41** | `GridConfig.delta_mode: bool` + `coordinator_free_rounds: bool`. | ✅ | `merge/types.rs:318` (`delta_mode`) + L339 (`coordinator_free_rounds` — already shipped in 2.34 per AMB-D-1); both in `Default` impl at L342-354. |
| **R42** | Default preserves v1 behaviour; zero behavioural regression. | ✅ | `Default` impl sets `delta_mode: false` (L349); `grid_config_default_disables_delta_mode` test at L440-445; **R42 smoke test** `r42_default_delta_mode_preserves_v1_smoke_output` at L1968-2039 runs `church_add(2,3)` through `run_grid` with `delta_mode: false` explicit vs default and asserts `total_interactions`, `rounds`, `interactions_by_rule` all match. |
| **R43** | `coordinator_free_rounds` defaults to `true` when `delta_mode` is `true`. | ⚠️ NOT IMPLEMENTED | `Default` sets both to `false` unconditionally. No builder pattern flips `coordinator_free_rounds` when `delta_mode` is enabled. Per 2.26-D bundle scope (L25-32: "R41 — GridConfig.delta_mode: bool field (default false)"), R43 was not listed as in-scope. The design-choices doc doesn't touch it either. However, R43 is a MUST. See **SF-002**. |
| **R44** | `coordinator_free_rounds = true, delta_mode = false` MAY work (coordinator-free without stateful workers). | ✅ | No assert blocks this combination. Documented in the `coordinator_free_rounds` doc-comment (`merge/types.rs:320-339`). |

### CLI Plumbing (2.26-D §3.6)

| Req | Scope | Status | Evidence |
|---|---|:-:|---|
| R41 CLI | `--delta-mode` on `CoordinatorArgs` and `LocalArgs`. | ✅ | `config.rs:197-202` (coord), L300-303 (local), both `#[arg(long, default_value_t = false)]`. |
| R41 threading | `build_grid_config` / `build_grid_config_from_local` thread the flag. | ✅ | `config.rs:532` + L543 (both assign `delta_mode: args.delta_mode`). |
| Coverage | Four CLI tests (`cli_delta_mode_default_is_false_*`, `cli_delta_mode_flag_threads_through_*`). | ✅ | `config.rs:1066-1149`. |

---

## Must-Fix Issues

### MF-001 — Worker drops two `RoundStart` payload fields; `minted_agents` never echoed; CON-DUP / CON-ERA / DUP-ERA resolutions cannot complete end-to-end

**Category:** Spec Violation (SPEC-19 R23 + R26 + DC-B5 second half)
**File:** `relativist-core/src/worker.rs:326-413` (`handle_round_start` signature and body); `relativist-core/src/worker.rs:405-412` (`RoundResult` emission)
**Principle/Spec:** SPEC-19 R23 mandates the worker consume `local_reconnections` + `pending_commutations` from `RoundStart`; R26 mandates the worker emit `minted_agents` matching each `PendingCommutation.request_id`. DC-B5 (spec-critic 2026-04-17) makes this a protocol invariant ("The coordinator MUST treat a `MintedAgent` response whose `request_id` does not match any outstanding `PendingCommutation` as a protocol violation" — R48).

**Problem:** The coordinator's resolver (2.26-B) emits `CommutationBatch`es that describe new agents the worker must mint. The wire protocol (2.26-A) carries these as `pending_commutations: Vec<PendingCommutation>` on `Message::RoundStart` and expects `minted_agents: Vec<MintedAgent>` on `Message::RoundResult`. The current worker handler silently ignores both vectors:

**Before:**
```rust
// worker.rs:326-332
pub fn handle_round_start(
    ctx: &mut WorkerContext,
    round: u32,
    border_deltas: Vec<BorderDelta>,
    resolved_borders: Vec<u32>,
    new_borders: Vec<(u32, PortRef)>,
) -> Vec<WorkerAction> {
    // ...
    // worker.rs:405-411 — emission
    WorkerAction::SendMessage(Box::new(Message::RoundResult {
        round,
        border_deltas: outgoing,
        stats,
        has_border_activity,
        minted_agents: Vec::new(),           // ← hard-coded empty
    })),
```

**After (minimum fix — signature + stub echoing):**
```rust
pub fn handle_round_start(
    ctx: &mut WorkerContext,
    round: u32,
    border_deltas: Vec<BorderDelta>,
    resolved_borders: Vec<u32>,
    new_borders: Vec<(u32, PortRef)>,
    local_reconnections: Vec<LocalReconnection>,          // ← add (R23 DC-B3)
    pending_commutations: Vec<PendingCommutation>,        // ← add (R23 DC-B5)
) -> Vec<WorkerAction> {
    // ... existing apply_border_deltas call ...
    // (a) Apply local_reconnections to state.partition.subnet:
    for lr in &local_reconnections {
        state.partition.subnet.connect(
            PortRef::AgentPort(lr.agent_id, lr.port),
            lr.new_target,
        );
    }
    // (b) For each PendingCommutation, allocate an AgentId from
    //     state.partition.id_range, create the agent locally,
    //     and record (request_id, minted_agent_id) for the echo.
    let mut minted_agents: Vec<MintedAgent> = Vec::with_capacity(pending_commutations.len());
    for pc in &pending_commutations {
        let new_id = state.partition.allocate_agent_id()?;   // helper needed
        state.partition.subnet.create_agent_at(new_id, pc.symbol_type);
        minted_agents.push(MintedAgent { request_id: pc.request_id, minted_agent_id: new_id });
    }
    // ... existing reduce_all + rebuild_free_port_index + outgoing deltas ...
    WorkerAction::SendMessage(Box::new(Message::RoundResult {
        round,
        border_deltas: outgoing,
        stats,
        has_border_activity,
        minted_agents,                                      // ← populated
    })),
```

**Why:** Without this wiring, any workload with a CON-DUP, CON-ERA, or DUP-ERA cross-partition redex (three of the six IC rules) will not reduce correctly end-to-end — the coordinator will detect the redex, call `resolve_border_redex`, emit `pending_commutations` into the next `RoundStart`, the worker will silently drop them, and `minted_agents` will arrive empty, causing the coordinator's round-N+2 finalizer (not yet written — also a gap, but subsumed by MF-002's integration-test absence) to fail to resolve the `PendingPortRef::Pending` tokens. The resolver tests pass because they only assert the coordinator-side emission shape; the worker never reads the emission.

Note that `local_reconnections` dropping is less catastrophic than the commutation drop for same-symbol rules — the CON-CON / DUP-DUP dispatcher parks its reconnections in `worker_deltas[0].local_reconnections` today (per `resolve_con_con` at L547-557 and the `package_resolutions` fan at L207-209), which the worker MUST apply for the reduction to match `merge()` output. Silently dropping them yields wrong topology.

---

### MF-002 — D-003 acceptance-signal integration test absent; G1 parity unverified

**Category:** Spec Violation (SPEC-19 R38 operational discharge; DEFERRED-WORK.md D-003 closure)
**File:** `relativist-core/tests/` (missing file `grid_delta_roundloop.rs`); referenced in `relativist-core/src/merge/grid.rs:2556-2557`
**Principle/Spec:** DEFERRED-WORK.md D-003 acceptance signal: "An integration test in which a 2-worker grid converges on an input that requires at least one border-redex resolution: the coordinator detects the redex via `BorderGraph::detect_border_redexes`, invokes the right `interact_*` rule, sends `BorderDelta` patches to both workers, and the workers' next-round states reflect the resolution. The final `merge()` reconstructs the same output net that the v1 full-partition protocol produces on identical input." SPEC-19 R38 "operational discharge" per AMB-D-3: convergence test of `run_grid_delta` vs `reduce_all` reference.

**Problem:** The comment at `grid.rs:2556-2557` promises:
```rust
// See TEST-SPEC-0385.md. Covers R21.1 (Round 0 dispatch), R21.2
// (delta rounds), R23 (RoundStart payload), R26 (RoundResult
// consumption), DC-C3 (strict_bsp branching), and DC-C5 (convergence
// predicate) inline. DC-C3 lenient/strict matrix cells + G1 parity
// (UT-0385-06..08) live in tests/grid_delta_roundloop.rs.
```

No file `relativist-core/tests/grid_delta_roundloop.rs` exists in the tree (verified via `find`). The only integration-test file in `relativist-core/tests/` is `cli_integration.rs`. The in-file tests at `grid.rs:2658-end` exercise the round loop against canned `RoundResultPayload` fixtures (UT-0385-01..05) — they cover the *mechanics* of the loop, but they do not drive a real reduction from input net to output net and compare against `run_grid`. The UT-0385-04 test that plausibly approaches that mandate (`run_grid_delta_inner_applies_round_result_deltas_to_border_graph`) comments at L2791-2796 that the synthetic delta "leaves agent 0's principal port DISCONNECTED in the coordinator cache (no worker-side reconnection accompanies the test's delta). In a real worker flow the post-reduction partition would be well-formed; simulate that by supplying empty final partitions".

**Before:** No integration test comparing `run_grid` and `run_grid_delta` outputs on a CON-CON (or any other) cross-partition redex.

**After:** Add `relativist-core/tests/grid_delta_roundloop.rs` with at minimum:

```rust
// tests/grid_delta_roundloop.rs
//! G1 parity tests for run_grid_delta (SPEC-19 R38).
//! DEFERRED-WORK D-003 acceptance signal: output net must match v1.

use relativist_core::merge::{run_grid, GridConfig};
use relativist_core::merge::grid::run_grid_delta; // if pub(crate), expose via tests feature
use relativist_core::net::{Net, PortRef, Symbol};
use relativist_core::partition::ContiguousIdStrategy;

// In-process LocalDeltaDispatch that runs the worker handlers directly.
// (Build this helper in tests/common/ if not already present.)

#[test]
fn run_grid_delta_matches_run_grid_on_con_con_cross_redex() {
    let net = /* two CON agents with principal-principal wire crossing a partition */;

    let cfg_v1 = GridConfig { num_workers: 2, ..Default::default() };
    let (out_v1, m_v1) = run_grid(net.clone(), &cfg_v1, &ContiguousIdStrategy);

    let cfg_v2 = GridConfig { num_workers: 2, delta_mode: true, ..Default::default() };
    let mut dispatch = LocalDeltaDispatch::new(&cfg_v2);
    let (out_v2, m_v2) = run_grid_delta(net, &cfg_v2, &ContiguousIdStrategy, &mut dispatch);

    assert_eq!(out_v1.count_live_agents(), out_v2.count_live_agents());
    assert_eq!(m_v1.total_interactions, m_v2.total_interactions);
    // isomorphism check: canonicalize both nets and byte-compare
    assert_eq!(canonicalize(&out_v1), canonicalize(&out_v2));
}
```

**Why:** Without this test, the review cannot state "D-003 closure signal met". The operational claim of R38/R40 rests on the BSP loop producing the same Normal Form as `reduce_all`; nothing in the current test battery proves it. Q1 of Stage 5 QA should start here.

---

## Should-Fix

### SF-001 — `run_grid_delta` not routed from a public entry point gated on `cfg.delta_mode`

**Category:** Spec Compliance (SPEC-19 R20)
**File:** `relativist-core/src/merge/grid.rs:346-349` (TODO), `relativist-core/src/merge/grid.rs:37-303` (`run_grid` v1 entry — has no fork)
**Principle/Spec:** R20: "The delta protocol MUST be activated via a configuration flag `GridConfig.delta_mode: bool`, defaulting to `false`. When `delta_mode` is `false`, the v1 full-partition protocol ... is used unchanged. When `delta_mode` is `true`, the delta BSP loop is used."

**Problem:** `GridConfig.delta_mode` is a field; the value is threaded through the CLI; but **no call site reads it**. The public `run_grid` function in `grid.rs:37-303` is oblivious to `delta_mode`. Callers must explicitly invoke `run_grid_delta`, which is currently `pub(crate)` (L338-344). A user setting `--delta-mode` on the CLI will **not** actually route through the delta loop — their config flag is inert.

**Before:**
```rust
// grid.rs:339
#[allow(dead_code)] // TASK-0385+ exercises via real coordinator; tests cover degenerate paths today.
pub(crate) fn run_grid_delta(
    net: Net, config: &GridConfig, strategy: &dyn PartitionStrategy,
    dispatch: &mut dyn crate::merge::types::WorkerDispatch,
) -> (Net, GridMetrics) {
    assert!(config.num_workers >= 1, "num_workers must be >= 1");
    // TODO(2.26-D): once `GridConfig.delta_mode` lands, assert it here.
```

**After:** Either (a) add a public dispatcher (cleanest), or (b) fork inside `run_grid`:

```rust
// Option (a) — new public entry in merge/mod.rs or lib.rs:
pub fn run_grid_entry<'a>(
    net: Net, config: &GridConfig, strategy: &dyn PartitionStrategy,
    dispatch: Option<&mut dyn WorkerDispatch>,
) -> (Net, GridMetrics) {
    if config.delta_mode {
        let d = dispatch.expect("delta_mode requires WorkerDispatch");
        run_grid_delta(net, config, strategy, d)
    } else {
        run_grid(net, config, strategy)
    }
}
```

**Why:** Without the fork, R20 is a dead letter: the flag exists but does nothing observable. The R42 smoke test at `grid.rs:1968-2039` passes because `delta_mode: false` is the default AND because `run_grid` is the only caller threaded through `local_main`. A reviewer who trusts the CLI integration test `cli_delta_mode_flag_threads_through_local` (config.rs:1130-1149) would be deceived — it asserts the field is threaded into `GridConfig`, not that turning the field on *changes behaviour*.

The developer scaffolded this fork for later and may be planning to land it in a follow-up bundle (the `TODO(2.26-D)` at L346 reads that way); however, the 2.26-D bundle index lists R20 as in scope by inference (R41 is the config amendment *for* R20). This is borderline between MF and SF; I call it SF because the infrastructure is in place, it's a one-function-addition away, and no v1 caller is broken by the current state.

### SF-002 — R43 not implemented: `coordinator_free_rounds` does not default to `true` when `delta_mode` is `true`

**Category:** Spec Violation (SPEC-19 R43)
**File:** `relativist-core/src/merge/types.rs:342-354` (`impl Default for GridConfig`)
**Principle/Spec:** R43: "`coordinator_free_rounds` MUST default to `true` when `delta_mode` is `true`, and SHOULD default to `false` when `delta_mode` is `false`."

**Problem:** The `Default` impl sets both to `false` unconditionally. No builder pattern or post-construction fix-up flips `coordinator_free_rounds` when a caller sets `delta_mode = true`. A CLI user passing `--delta-mode` without also passing a (non-existent) `--coordinator-free-rounds` flag will run delta mode without the coordinator-free-round optimization — despite the spec mandating it.

**Before:**
```rust
impl Default for GridConfig {
    fn default() -> Self {
        Self {
            num_workers: 1,
            max_rounds: None,
            strict_bsp: false,
            delta_mode: false,
            coordinator_free_rounds: false,
        }
    }
}
```

**After (one option — small builder + post-construction invariant hook):**
```rust
impl GridConfig {
    /// Normalize invariants after field-by-field construction.
    /// Enforces R43: coordinator_free_rounds defaults to true under delta_mode.
    pub fn normalize(mut self) -> Self {
        if self.delta_mode && !self.coordinator_free_rounds_user_set {
            self.coordinator_free_rounds = true;
        }
        self
    }
}
// plus: `build_grid_config` calls .normalize() before returning.
```

Or simpler — add a `with_delta_mode(true)` builder that flips both. Or add the CLI default-tracking bit explicitly.

**Why:** R43 is a MUST. Its practical effect is small today (the coordinator-free-round optimization is a performance win, not a correctness requirement), but its spec status is binding. Bundle 2.26-D's task breakdown (L25-32) explicitly lists R42 as the only R4x requirement in scope — R43 was not allocated a TASK. Flag as follow-up work, not a 2.26-D internal failure, but do not let it close without an answer.

### SF-003 — `run_grid_delta_inner`'s partition cache mutation under `local_reconnections` silently skips self-loops and DISCONNECTED pairs; no metric or log

**Category:** Code Quality (silent failure mode)
**File:** `relativist-core/src/merge/grid.rs:591-600`
**Principle/Spec:** IC rule semantics — `net.connect` debug-asserts against self-sentinel pairs and same-port self-loops; the skip is a legitimate defensive move, but silent.

**Before:**
```rust
for (a, b) in &payload.local_reconnections {
    if *a == crate::net::DISCONNECTED || *b == crate::net::DISCONNECTED || *a == *b {
        continue;  // ← silent
    }
    cached.subnet.connect(*a, *b);
}
```

**After:**
```rust
for (a, b) in &payload.local_reconnections {
    if *a == crate::net::DISCONNECTED || *b == crate::net::DISCONNECTED {
        tracing::trace!(?a, ?b, worker = ?worker_id, round = ?metrics.rounds,
            "skipping local_reconnection with DISCONNECTED endpoint");
        continue;
    }
    if *a == *b {
        tracing::trace!(?a, worker = ?worker_id, round = ?metrics.rounds,
            "skipping local_reconnection self-loop");
        continue;
    }
    cached.subnet.connect(*a, *b);
}
```

**Why:** If the resolver emits such a pair by mistake, the current code swallows it. A `tracing::trace!` makes the skip visible under debug logging without adding a panic. Alternately, a `debug_assert!` with a spec-pointer message would be even better, since the resolver is *not supposed* to emit self-loops.

### SF-004 — `WorkerDispatch` trait and `RoundResultPayload` are `pub(crate)` but must be reachable from the `protocol/coordinator.rs` async impl (future)

**Category:** Architecture (future-proofing for DC-C2's deferred async binding)
**File:** `relativist-core/src/merge/types.rs:182-227`
**Principle/Spec:** DC-C2 Option C (2026-04-17): "`impl WorkerDispatch for CoordinatorConnection` block in `protocol/coordinator.rs`, OUTSIDE this bundle."

**Problem:** The trait's visibility (`pub(crate)`) limits implementations to the `relativist-core` crate. That is fine for the current in-process tests and for the deferred `coordinator.rs` impl (same crate). But the `#[allow(dead_code)]` tags on trait and `RoundResultPayload` (`merge/types.rs:181, 205`) acknowledge no production caller exists. If a consumer crate (benchmarks, integration harness) wanted to stub the dispatch for a scripted end-to-end run, they couldn't implement the trait. Consider promoting to `pub` with `#[doc(hidden)]` or a `#[cfg(any(test, feature = "test-support"))]` exposure, OR add the async impl now.

**Why:** A trait named `WorkerDispatch` with three `dispatch_*` methods that is `pub(crate)` and has zero non-test implementors is a signal that the architecture layer is half-shipped. The reviewer contract's "ISP" check (small focused trait) passes; the "dependency inversion" check (high-level modules depend on traits, not concrete types) passes; but the overall R20 gate (SF-001) depends on SF-004 indirectly — without a concrete async impl, the public `run_grid` cannot fork on `delta_mode`.

### SF-005 — `run_grid_delta_inner`'s debug assertion floor `num_workers >= 2` makes entry from a `config.num_workers == 1` path unreachable, but the entry `run_grid_delta` already handles n=1 by delegating — the assertion is sound but un-exercised

**Category:** Code Quality (over-conservative assert with unclear intent)
**File:** `relativist-core/src/merge/grid.rs:427-430`

**Before:**
```rust
let num_workers = plan.partitions.len();
debug_assert!(
    num_workers >= 2,
    "run_grid_delta_inner: multi-worker path only (num_workers = {num_workers})"
);
```

**After:** Either promote to a full `assert!` (callers outside the current tree could invoke the inner function via re-export), or replace with a typed enum `NumWorkers::Multi(u32)` at the boundary. Current `debug_assert!` is sufficient for internal callers but leaves release builds silently running a theoretically degenerate path if `run_grid_delta` ever forgets to check `num_workers == 1`.

**Why:** Belt-and-braces; follows the `assert!` vs `debug_assert!` convention the rest of the codebase uses for structural invariants (`border_resolver.rs` uses `assert!` in `resolve_border_redex`; consistency). Non-blocking.

---

## Nice-to-Have

### NTH-001 — `#[allow(dead_code)]` proliferation across 2.26-B types

`border_resolver.rs` carries `#[allow(dead_code)]` on 9 items (`WorkerDeltas`, `BorderResolution`, `RoundStartDispatch`, `package_resolutions`, `CommutationBatch`, `PendingPortRef`, `PendingNewBorder`, `BorderIdAllocator`, `CommutationIdAllocator`, `SLOT_MARKER_BASE`, `materialize_agent`). This is *correct* today because 2.26-C's `run_grid_delta_inner` exercises exactly `package_resolutions`, `resolve_border_redex`, `BorderIdAllocator`, and `CommutationIdAllocator` — but the rest (particularly `PendingPortRef`, `PendingNewBorder`, `CommutationBatch`) are unreachable until the worker-side DC-B5 echo lands (MF-001). Once MF-001 is fixed, most of these allows can be removed — and if they *can't* be removed, that's a different alarm bell.

### NTH-002 — Module-level doc on `border_resolver.rs` is excellent but long (~66 lines); consider moving DC-B1..B6 verdicts to a separate `docs/architecture-notes.md`

The `//!` block is 66 lines of DC verdict restatements before the first `use` statement. This is audit-friendly but slows fresh readers. A one-paragraph "what is this file" + a link to the spec-review doc would be lighter.

### NTH-003 — `emit_external_principal` (L803-839) has 8 parameters; `emit_erasure_principal` (L937-994) has 7

Both are tagged `#[allow(clippy::too_many_arguments)]`. The structural case for the helpers is solid (they dispatch wiring-bucket choices), but a small `struct EmissionContext<'a> { new_worker: WorkerId, cid: CommutationId, border_alloc: &'a mut BorderIdAllocator, ... }` would read better. Non-blocking; matches the DEC-C2 Option C precedent of accepting mild parameter-list weight in exchange for pure-core residency.

### NTH-004 — `worker.rs` imports `crate::protocol::Message` (L10); this is fine (worker is the infrastructure layer, not pure-core), but worth a comment

A skim-reader might wonder whether the worker is breaking the pure-core invariant; the natural home for an anchor comment is `worker.rs:10` or the module doc. Current module doc (L1-6) says "SPEC-13 R24-R27" which is accurate but doesn't address the pure-core question. One sentence: "worker.rs is the infrastructure layer; it depends on `protocol::Message` and may use tokio. Only `merge/` is pure-core per SPEC-13 R6-R8."

### NTH-005 — `run_grid_delta_inner` is 211 lines with 16 local bindings plus nested loops

Clean Code's "small functions" guideline would ask for a split: e.g., one helper for "collect round results and apply to graph+cache" (L476-491), one for "resolve border redexes and advance metrics" (L530-554), one for "build next round's dispatch from resolutions" (L567-609). The current function is readable but sits at the boundary of what fits in one head. Non-blocking.

---

## Part A Checklist — Code Quality

- [x] **Meaningful names:** descriptive — `border_resolver`, `package_resolutions`, `check_delta_convergence`, `pending_commutations`. No abbreviations besides `id`, `tcp`, `bid`, `cid`, `wa`/`wb`. Good.
- [x] **Small functions:** `resolve_border_redex` at 77 lines is reasonable given the 9-cell dispatch; per-rule helpers are 10-40 lines. `run_grid_delta_inner` at 211 lines is on the edge (see NTH-005).
- [x] **Single level of abstraction:** per-rule bodies operate at the BorderState + Partition + Vec-of-deltas level consistently. `run_grid_delta_inner` mixes metric bookkeeping with loop control — readable but dense.
- [x] **No magic numbers:** `SLOT_MARKER_BASE: u32 = u32::MAX - 10_000` named (L605); SLOT_P/R/Q/S / SLOT_E1/E2 constants inside each rule body (L676-679, L890-891). Good.
- [x] **No dead code:** all `#[allow(dead_code)]` annotations point to a concrete follow-up task (either 2.26-C or a later bundle); no commented-out blocks. NTH-001 flags the volume but not the practice.
- [x] **Clear control flow:** no nesting > 3 levels outside the CON-DUP wiring block (`resolve_con_dup` L644-786 has 4 levels inside the match; acceptable for topology code).
- [x] **Helpful error messages:** `GridError::DispatchFailed { round, message }` includes round + diagnostic; panic strings in `assert_agent` and `BorderIdAllocator::next` name the invariant and the DC ruling.

### SOLID
- [x] **SRP:** `border_resolver.rs` ≠ one reason to change (it aggregates 6 rules + 2 allocators + 1 dispatcher + packaging). This is idiomatic for a dispatcher module; each helper has one reason. Grid.rs's `run_grid_delta_inner` has several concerns (NTH-005).
- [x] **OCP:** `WorkerDispatch` trait allows new transports without changing `run_grid_delta_inner`. New IC rules would require modifying `resolve_border_redex`'s dispatch arm — that's expected; the 6 rules are closed by the spec.
- [x] **LSP:** `NoopDispatch` and `CapturingDispatch` in tests honour the trait contract.
- [x] **ISP:** `WorkerDispatch` has 3 methods, each load-bearing; no god-trait.
- [x] **DIP:** `run_grid_delta` takes `&mut dyn WorkerDispatch`; `resolve_border_redex` takes `&mut BorderGraph + &[Partition] + &mut allocators` (all concrete but none cross-layer).

### Rust Idioms
- [x] **Ownership:** clones limited to where they're necessary (`plan.partitions.clone()` for the cache at L446, `state.partition.free_port_index.clone()` for the round 0 seed, `BorderState` clone at L455 before `remove_border`).
- [x] **Error handling:** `GridError::DispatchFailed` via `#[from]` chain; `thiserror` in `error.rs` (not verified in detail here — assumed stable from v1).
- [x] **Iterators:** `package_resolutions` uses `.extend` / `.map` / `.enumerate`; `check_delta_convergence` uses `.all`. No raw loops where iterators read naturally.
- [x] **Pattern matching:** exhaustive matches on `(Symbol, Symbol)` (9 cells) and `PortRef` (`AgentPort` vs `FreePort`) — no `_ => unreachable!()` in resolver bodies.
- [x] **Newtype pattern:** `AgentId`, `WorkerId`, `BorderId` via `u32`; `CommutationId` as `u64`. Consistent with v1.
- [ ] **Builder pattern:** `GridConfig` construction uses `..Default::default()` spread (works, idiomatic) — no formal builder. Not required at this scale.
- [x] **Visibility:** `pub(crate)` on all new names except `GridConfig.delta_mode` / `GridMetrics.delta_mode` / `WorkerDeltaState`; the latter three are genuinely public API surfaces.

### Documentation
- [x] All `pub` / `pub(crate)` items have `///` doc comments citing the spec requirement and DC ruling. Module-level `//!` blocks carry per-file intent + pure-core invariant.
- [x] No redundant "// increment counter" style comments noted.
- [x] WHY comments present on the DC-B4 pinning note (L40-44), DC-B5 2-phase flow (L45-56), DC-C3 firewall (L328-332), DC-C5 FLIP (L635-651). Good.

## Part B Checklist — Architecture

- [x] **Core layer is pure:** `border_resolver.rs` canary green (5-entry FORBIDDEN_USE_PREFIXES, cardinality asserted). `grid.rs` imports only `std`, `crate::net`, `crate::partition`, `crate::merge::*`, `crate::reduction::reduce_all`, `crate::error` — all pure-core or core-equivalent. `WorkerDispatch` is a synchronous trait in `merge/types.rs` — no tokio.
- [x] **Infrastructure depends on core:** `worker.rs` imports `crate::merge::BorderDelta`, `crate::net::PortRef`, `crate::partition::Partition`, `crate::protocol::Message` (one-way, correct direction).
- [x] **Feature-gated modules:** no new feature gates added; existing `zero-copy` gating untouched.
- [x] **No cross-module shortcuts:** the resolver does NOT import from `reduction/` (mirrors the 6 rules instead of calling `interact_*`, preserving R19). Confirmed.

### Dependency Direction
- [x] `net` — unchanged.
- [x] `reduction` — not touched by this bundle.
- [x] `partition` — unchanged.
- [x] `merge` — gains `border_resolver.rs`, `internal/pure_core_guard.rs`, `types.rs` extensions, `grid.rs` delta extensions. All edges point into `net` / `partition` / `std`.
- [x] `protocol` — unchanged in 2.26-B/C/D (wire variants shipped 2.26-A).
- [x] No circular dependencies.

### Design Patterns
- [x] FSM pattern: `WorkerState` gains `DeltaIdle` and `DeltaActive` variants — additive, enum-based. Good.
- [x] Transport trait: `WorkerDispatch` is the trait; the async `CoordinatorConnection` binding is deferred per DC-C2. Acceptable given the deferral is explicit and documented.
- [x] Newtype IDs: consistent; `CommutationId = u64` (L250) is a type alias, not a newtype struct — matches existing convention for `AgentId`, `BorderId`.
- [x] Error enums: `GridError::DispatchFailed` added; uses `#[from]` chain via existing `error.rs`.

### Spec Compliance
- [x] All MUST requirements from §3.3 R13-R30 + §3.5 R38-R40 + §3.6 R41-R42 implemented, EXCEPT as flagged in MF-001 (R23 + R26 worker-side gap) and SF-001 (R20 dispatcher-fork absent).
- [x] Type signatures match spec verbatim for `BorderDelta` (SPEC-19 R33 — untouched), `WorkerRoundStats` (SPEC-19 §3.1 R2 — `has_border_activity` present, shipped 2.34), `GridConfig.delta_mode` (R41).
- [x] Invariants from SPEC-01 — T1-T7 preserved (the resolver mirrors the 6 rules exactly; no new IC rule); D3 strengthened via `BorderGraph.detect_border_redexes` (R39); D6 progress guarantee in the round loop (R40 operational).

## Anti-Pattern Scan

- **God struct:** `BorderResolution` has 5 fields, `RoundStartDispatch` has 5 fields, `GridMetrics` has ~22 fields (pre-existing, v1 baseline) — none cross the "> 10 fields or > 500 lines of impl" threshold for new code.
- **Feature envy:** none detected. `resolve_con_dup` reads heavily from `Partition.subnet` via `get_target` — but `Partition` is the natural substrate for topology reads; the alternative (promoting `get_target_via_partition` to `Partition`) would push dispatcher logic into the partition module, violating SRP.
- **Primitive obsession:** `CommutationId = u64` is a type alias, not a newtype — borderline, but matches the v1 convention for `AgentId = u32` / `WorkerId = u32`. Not a new violation.
- **Leaky abstraction:** **NEGATIVE (none found).** The `WorkerDispatch` trait cleanly hides transport details; the resolver's `Partition` access is R19-safe; no `TcpStream` types leak into `merge/`.
- **Temporal coupling:** The `resolve_border_redex` → `package_resolutions` → `apply_border_deltas_to_partition` → next-round `dispatch_round_start` chain is temporally coupled (order is fixed by BSP semantics) but enforced by the flow of `run_grid_delta_inner`'s loop body — there is no way for a caller to accidentally reorder the steps. Acceptable.

## Recommended Stage 5 QA Focus

High-leverage adversarial probes for the 2.26-B/C/D slice. Numbered to match the 2.26-A review's Q-probe convention:

| Q# | Target | Hypothesis |
|---|---|---|
| **Q1** | 2-worker G1 parity on CON-CON cross-partition redex | MF-002 is the integration-test gap. Probe runs `run_grid` (v1) and `run_grid_delta` on the same net with `num_workers=2`; asserts `count_live_agents`, `total_interactions`, and canonicalized net-isomorphism identical. **This is the D-003 acceptance signal.** |
| **Q2** | 2-worker G1 parity on CON-DUP cross-partition commutation | Exercises the DC-B5 2-phase flow end-to-end. **This probe will FAIL today** because MF-001 breaks the `minted_agents` echo. A Q2 failure **is the empirical proof of MF-001**. |
| **Q3** | 2-worker G1 parity on CON-ERA and DUP-ERA cross-partition erasure | Exercises DC-B6 existing-border preservation. Same class as Q2 (worker-side minting required). **Expected to fail via the same MF-001 path.** |
| **Q4** | `RoundStart.local_reconnections` with one non-empty entry → does the worker apply it? | Probes MF-001's `local_reconnections` silent-drop. Build a worker handler invocation with a non-empty `local_reconnections` and assert post-call `state.partition.subnet` reflects the connection. Today the signature doesn't accept the field → probe compilation FAILS, which is the signal. |
| **Q5** | `RoundStart.pending_commutations` with 2 entries → does `RoundResult.minted_agents` have matching `request_id`s? | Probes MF-001 echo path. Today: `minted_agents` is hard-coded `Vec::new()`, so the probe's `request_id` set will be empty while input set was {0, 1} → assert fails, proving MF-001. |
| **Q6** | `run_grid_delta` with `config.delta_mode = false` — does it still run? | Probes SF-001. Today `run_grid_delta` doesn't read `delta_mode` and will run regardless. **Expected:** when the dispatcher-fork lands, calling `run_grid_delta` with `delta_mode = false` should either error or be gated out of the public API. |
| **Q7** | `GridConfig { delta_mode: true, ..Default::default() }` — is `coordinator_free_rounds = true`? | Probes SF-002 (R43). Today: `false`. After fix: `true`. |
| **Q8** | Trigger `BorderIdAllocator` exhaustion via mocked `graph.borders` with `u32::MAX - 1` as max id and force 2 CON-DUPs | Confirms the panic path (`border_resolver.rs:340-345`) fires with the documented message. |
| **Q9** | `CommutationIdAllocator` overflow via direct counter manipulation in a test | Mirror of Q8 for the `u64` allocator. |
| **Q10** | Mutate `FORBIDDEN_USE_PREFIXES` to 4 items via a test-only override and confirm the cardinality canary at `border_resolver.rs:2536-2544` fires FIRST | Confirms DC-B9 drift guard. |
| **Q11** | `check_delta_convergence` with 2 workers where worker A has `has_border_activity = false, local_redexes = 3` and worker B has everything zero | Asserts three-conjunct predicate returns `false` (DC-C5 flip) — falls through the `all_no_local_redexes` conjunct. |
| **Q12** | Double-resolve: call `resolve_border_redex` on the same `border_id` twice | Second call must panic with "border 0 not present in graph" (L455-460). Already covered by UT-0376-07; restate as adversarial check. |
| **Q13** | `run_grid_delta_inner` `max_rounds = Some(0)` — does the loop exit immediately with `delta_max_rounds_hit = Some(true)` and run Final Collection? | R30 preservation. Covered in the existing test battery but worth an adversarial re-pin. |
| **Q14** | Inject `RoundResultPayload` with `worker_id` out of range into `dispatch_round_start` mock — does `border_graph.apply_deltas` panic or silently ignore? | Defensive probe on cache-desync vs bad worker id. |
| **Q15** | Send `FinalStateRequest` before any `RoundStart` in the in-process dispatch — does the worker handler's `debug_assert!` catch it? | Probes the `matches!(ctx.state, DeltaIdle | DeltaActive)` guard at `worker.rs:439-442`. |

**Prioritisation:** Q1, Q2, Q3 are the highest-leverage (D-003 closure signal). Q4, Q5 are direct MF-001 probes. Q6, Q7 are SF-001/SF-002 probes. Q8..Q15 are contract-edge probes.

## Deferred Work Status (D-003)

**D-003 — SPEC-19 R13 / R14 / R15 parts 1-2 (coordinator-side border-redex resolution)** — **PARTIAL closure, blocked on MF-001 + MF-002.**

Resolver code for R13, R14, R15 parts 1-2 ships (2.26-B). The BSP loop invokes it (2.26-C). `GridConfig.delta_mode` gates it in the field sense (2.26-D). However, D-003's acceptance signal is literal:

> "An integration test in which a 2-worker grid converges on an input that requires at least one border-redex resolution: the coordinator detects the redex via `BorderGraph::detect_border_redexes`, invokes the right `interact_*` rule, sends `BorderDelta` patches to both workers, and the workers' next-round states reflect the resolution. **The final `merge()` reconstructs the same output net that the v1 full-partition protocol produces on identical input.**"

That test (MF-002) is not present. Even if it were written, it would fail today for any CON-DUP / CON-ERA / DUP-ERA redex because of MF-001 (worker does not act on `pending_commutations`, does not populate `minted_agents`). For CON-CON / DUP-DUP / ERA-ERA, the `local_reconnections` vector dropping (also MF-001) would produce an output net that differs from v1 in topology even if agent counts matched.

**Recommendation for D-003 row:** keep status `OPEN`; add a note "primitive shipped 2026-04-18; end-to-end closure blocked on 2.26-C MF-001 + MF-002 per REVIEW-SPEC-19-section-3.3-3.5-3.6-item-2.26-BCD-2026-04-23.md". Close after the refactor bundle addressing MF-001 + MF-002 lands with a passing G1 parity test on at least CON-CON, CON-DUP, and CON-ERA cross-partition workloads.

---

## Acceptance Verdict

**APPROVE WITH MANDATORY REFACTOR before advancing to Stage 5 QA on 2.26-C:**

- **2.26-B:** APPROVE for Stage 5. No MF-issues *within the sub-bundle's boundary*. The DC-B5 second half (worker-side echo) was always out of 2.26-B scope per the bundle index (R23/R26 owned by 2.26-A wire + 2.26-C lifecycle).
- **2.26-C:** **NEEDS REFACTOR.** MF-001 (worker drops two `RoundStart` fields; `minted_agents` not echoed) is a R23/R26 spec gap that must be closed before advancing. MF-002 (D-003 integration test absent) must be closed to claim R38 operational discharge.
- **2.26-D:** APPROVE with SF-002 scheduled as follow-up. The bundle did what it scoped (R41/R42); R43 fell between the cracks of the 2.26-D scope definition.

Stage 5 QA may proceed in parallel on the 2.26-B resolver probes (Q8, Q9, Q10, Q12) while the REFACTOR for 2.26-C is in flight. G1-parity probes (Q1-Q5) should wait for MF-001 and MF-002 to land.

---

**Test count delta observed:** 1009 → 1039 default (+30) / 1049 → 1079 zero-copy (+30) per pipeline-state.md L14.
**Gates observed:** `cargo test --workspace --lib` GREEN, `cargo clippy --workspace --all-targets -- -D warnings` GREEN both feature configs, `cargo fmt --check` GREEN (per pipeline-state Stage-History at 2.26-B close; 2.26-C / 2.26-D gates not re-verified by this reviewer — the user directive said to review at the end; I trust the developer's green status but flag that a fresh `cargo test` at the moment of landing the REFACTOR for MF-001 + MF-002 is the next gate signal).

---

## Postscript (2026-04-23) — MF-003 discovered during TASK-0395 DEV

The refactor bundle closing MF-001, MF-002, SF-001, SF-002 shipped on 2026-04-23 with `cargo test --workspace --lib` = **1138** (+29 over baseline 1109) / `--features zero-copy` = **1178** (+29). Four new TASKs completed the DEV cycle:

- **TASK-0397** (SF-002 R43 normalize) — 8 UT
- **TASK-0394** (MF-001 worker R23/R26) — 11 UT
- **TASK-0396** (SF-001 R20 dispatcher fork) — 4 UT
- **TASK-0395** (MF-002 G1 parity integration) — 3 UT shipped, asymmetric branch under `SKIP_ASYMMETRIC`

**A third Must-Fix — MF-003 — was surfaced during TASK-0395 DEV and is NOT closed by this bundle.** The sub-agent attempting end-to-end G1 parity on CON-DUP / CON-ERA / DUP-ERA cross-partition fixtures discovered that the pure-core `RoundResultPayload` struct in `relativist-core/src/merge/types.rs` does NOT carry the `minted_agents: Vec<MintedAgent>` field that the wire-layer `Message::RoundResult` carries. Consequence: the coordinator's BSP loop (`run_grid_delta_inner`) silently discards the worker-side DC-B5 echo, so `PendingPortRef::Pending` tokens emitted by the resolver for asymmetric commutations are never resolved.

**Why this review missed it.** The review audited the wire protocol (2.26-A, shipped) and the worker-side handler (bundle 2.26-C scope, flagged as MF-001) but did not audit the pure-core `RoundResultPayload` shape on the coordinator side of `WorkerDispatch`. MF-001's scope was defined as "worker consumes + emits correctly"; the review assumed the coordinator side was wired because `Message::RoundResult.minted_agents` is present on the wire. In practice, the pure-core `RoundResultPayload` is a SEPARATE struct (derived from the wire message for dispatch-trait ergonomics), and it lacks the field.

**Closure path:** tracked as **DEFERRED-WORK.md D-004**. Symmetric-rule G1 parity (UT-0385-06, UT-0385-07, UT-0385-08 non-asymmetric) passes empirically in this refactor, so D-003 is **partially closed**. Asymmetric-rule closure requires D-004 (~200-400 LoC: extend `RoundResultPayload`, add `BorderGraph::register_minted_agents`, wire in `run_grid_delta_inner`, forward from `LocalDeltaDispatch`, flip `SKIP_ASYMMETRIC = false`).

**Recommended follow-up REVIEW lesson:** when auditing a cross-layer contract, verify BOTH the wire type AND the pure-core dispatch type carry the same set of fields; mismatches between them are a common class of partial shipment not caught by unit tests of either layer individually.
