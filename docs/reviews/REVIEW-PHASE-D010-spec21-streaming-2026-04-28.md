# Review: D-010 Phase B-F — SPEC-21 Streaming Generation (Commits df80fe1..61e86a1)

**Date:** 2026-04-28
**Reviewer:** REVIEWER agent (Stage 4, unified code quality + architecture)
**Branch:** v2-development
**Verdict:** ACCEPT_WITH_FIXES

**Commits reviewed:**

| SHA | Phase | What |
|-----|-------|------|
| `df80fe1` | B | Foundation types: `ConnectionDirective`, `AgentBatch`, `StreamingPartitionStats`, `ChunkedPartitionResult`, `StreamingPartitionStrategy` trait |
| `85ea1ac` | C | `RoundRobinStreamingStrategy`, `FennelStreamingStrategy` |
| `f82a658` | D | `default_chunked_iter`, `ep_annihilation_stream`, `dual_tree_stream`, `make_net_stream` default impl |
| `79124ba` | E | `PartitionAccumulator`, `AccumulatorNet`, `install_connection`, `generate_and_partition_chunked` |
| `2f751a4` | F W1 | `GridConfig` streaming fields + CLI flags |
| `42ee4ac` | F W2 | R26 short-circuit, `FreePortInterface`, T6/T8 isomorphism oracle |
| `6cef832` | F W3 | `Message::RequestWork`/`NoMoreWork`, PROTOCOL_VERSION 5→6 |
| `3895cd2` | hotfix | `#[allow(clippy::assertions_on_constants)]` |
| `59ae74a` | F W4 | Coordinator FSM pull-dispatch states |
| `a635022` | F W5 | Worker FSM pull-dispatch + sim harness |
| `9d95f21` | F W6 | `BorderGraph::extend_with_chunk_borders` |
| `7527dea` | F W7 | Strategy A streaming gate + `free_list_pops` counter |
| `e3852f3` | F W8 | Strategy B `BorderClean` precision recycling + `free_list_pops_border/non_border` |
| `61e86a1` | F W9 | `streaming-no-recycle` cargo feature gate |

**Code quality verdict:** PASS WITH NOTES
**Architecture verdict:** MINOR DRIFT
**Spec compliance:** SPEC-21 R1-R37g, SPEC-22 R10b/R10c mostly implemented; integration gaps documented below

**Test floor:** 1662 default / 1705 zero-copy / 1661 streaming-no-recycle (v1 floor 690 preserved). Clippy clean. fmt clean.

---

## Top-3 Most Concerning Items

1. **MF-001 — `extend_with_chunk_borders` is a dead method at integration time.** The R37f call-site discipline (coordinator calls `BorderGraph::extend_with_chunk_borders` before each subsequent `AssignPartition` under `delta_mode && streaming_active`) is never exercised from production code paths. The method exists in `border_graph.rs`, tested in isolation, but `generate_and_partition_chunked` has zero `BorderGraph` interaction, and the coordinator FSM (`CoordinatorPullContext`) similarly has no border-graph update on `GeneratingNext`. Under `delta_mode && streaming_active` the coordinator's `BorderGraph` becomes stale after chunk 1, missing cross-chunk active pairs. G1 would be violated at production scale.

2. **MF-002 — `enter_streaming_mode` / `exit_streaming_mode` are never called from production code.** These helpers (added in W5) set `net.is_in_delta_round = true/false` as the proxy for R37b SPEC-22 broadening. They are only called in tests. The consequence: Strategy A and Strategy B free-list gates (`is_in_delta_round` checks in `create_agent`) are never activated during an actual streaming dispatch. The entire R37b protection mechanism is functional in unit tests but absent at the integration point.

3. **SF-001 — `free_list_pops_border` counter has misleading semantics that no test catches.** The counter increments inside the `else` branch (the successful-pop branch) based on `is_border_protected(id)`. Because the Strategy B gate at line 299 already re-pushes border IDs when `is_in_delta_round && BorderClean`, the counter can only fire when `is_in_delta_round = false` (push mode, no protection). UT-0590-07 exercises exactly this scenario but asserts only `free_list_pops == 1` without asserting `free_list_pops_border`. The name "border pops" implies "pops that were protected", but the counter actually measures "unprotected pops of shadow-present IDs" — the exact failure mode the spec intends to prevent.

---

## Summary

The D-010 bundle is a large, well-structured feature delivery. Phases B through F Wave 3 land the streaming pipeline foundation cleanly: types are idiomatic, strategies are deterministic and documented, the benchmark default-impl path correctly preserves backward compatibility, the R26 short-circuit works, and the wire-protocol changes (RequestWork/NoMoreWork, PROTOCOL_VERSION 5→6) are implemented correctly with the `PREVIOUS_LIVE_VERSION + 1` defensive pattern and a compile-time `const_assert`. The FSM additions in W4 and W5 faithfully represent the SPEC-13 A5 pull-dispatch state diagrams.

Two integration gaps surface in F W5-W6:

- `generate_and_partition_chunked` is the streaming orchestrator but operates in isolation from `BorderGraph` — the delta+streaming conjunction (R37f MUST) is untested at the system level.
- The `enter/exit_streaming_mode` helpers that arm the R37b free-list protection are never called from the protocol layer, rendering the arena safety guarantees nominal rather than enforced.

These are arch-level concerns, not logic errors in the individual components. All unit tests pass and individually exercise correct behavior. The integration wiring is what is missing.

---

## Must-Fix Issues

### MF-001: `extend_with_chunk_borders` has no production call-site — R37f is unenforceable

**Category:** Architecture / Spec Violation
**Principle/Spec:** SPEC-21 R37f (MUST under delta+streaming); SPEC-21 §3.8 A7
**Files:** `relativist-core/src/merge/border_graph.rs:631` (implementation), `relativist-core/src/partition/streaming.rs` (orchestrator, no call), `relativist-core/src/coordinator.rs` (FSM, no call)

**Problem:** `BorderGraph::extend_with_chunk_borders` is defined and tested in isolation (W6, `border_graph.rs` lines 3464-3672), but no production code path calls it. `generate_and_partition_chunked` has zero `use` of `BorderGraph`; the `CoordinatorPullContext.transition()` on `GeneratingNext` does not call it either. Under the conjunction `delta_mode && streaming_active`, R37f states this MUST be called after each `install_connection` invocation that yields a border wire before chunk N+1 is dispatched. Without the call-site, the coordinator `BorderGraph` is stale after chunk 1: it misses cross-chunk active pairs and the M5 milestone milestone target is unreachable.

**Before (coordinator `PullCoordinatorState::GeneratingNext` transition — current):**
```rust
// No BorderGraph update here; border map from install_connection results is
// discarded without propagation.
PullCoordinatorState::AwaitingResults => {
    if event == PullCoordinatorEvent::RequestWorkReceived { .. } {
        // ...generate next chunk, call install_connection...
        // NO extend_with_chunk_borders call
        PullCoordinatorState::AwaitingResults
    }
}
```

**After (schematic — DEVELOPER implements actual wiring):**
```rust
// After generate_and_partition_chunked_with_chunk_size returns a new border_map:
if cfg!(delta_mode) && self.streaming_active {
    // SPEC-21 R37f: before dispatching AssignPartition for chunk N+1,
    // extend the coordinator's BorderGraph with chunk N's new borders.
    border_graph.extend_with_chunk_borders(&new_chunk_result.borders);
}
```

The exact call-site depends on whether `generate_and_partition_chunked` or its `_with_chunk_size` variant is the integration point. The fix requires `CoordinatorPullContext` (or its caller) to hold a reference to both the stream iterator and the `BorderGraph`, and to call `extend_with_chunk_borders` on the `ChunkedPartitionResult.borders` of each chunk before dispatching.

**Why:** R37f is graded MUST under `delta_mode && streaming_active`. The border-graph staleness it prevents is a G1 violation (cross-chunk wire identity ambiguity). W6 delivered only the half of the contract that lives in `BorderGraph` (the extension method); the other half (the call-site discipline) was deferred without a tracking item.

---

### MF-002: `enter_streaming_mode` / `exit_streaming_mode` are never called from production code — R37b SPEC-22 broadening is nominal

**Category:** Architecture / Spec Violation
**Principle/Spec:** SPEC-21 R37b; SPEC-22 R10b A6 broadening; SPEC-13 worker FSM
**Files:** `relativist-core/src/worker.rs:926-939` (helpers), `relativist-core/src/protocol/` (no call)

**Problem:** `enter_streaming_mode(net)` and `exit_streaming_mode(net)` are defined in `worker.rs` lines 926 and 938. Their purpose is to set `net.is_in_delta_round = true/false` as the proxy for R37b's `(delta_mode || streaming_active)` broadening. Every production code path for the worker (the actual protocol handler in `relativist-core/src/protocol/`) invokes neither helper. The `WorkerPullContext.transition()` does not call `enter_streaming_mode` on `AssignPartitionReceived`; the coordinator does not trigger it either.

As a result:
- Under pull dispatch, `net.is_in_delta_round` remains `false` throughout a streaming run (unless the run also uses delta mode for an independent reason).
- Strategy A gate (`is_in_delta_round && DisableUnderDelta`) is inactive — workers CAN pop from the free-list during streaming, in violation of R37b.
- Strategy B gate is similarly inactive.
- The `streaming-no-recycle` feature gate (W9) is correct and would close G1 at compile time, but it is a feature not enabled by default.

**Before (`WorkerPullContext::try_transition_inner` on `AssignPartitionReceived` — current):**
```rust
WorkerPullEvent::AssignPartitionReceived { worker_id } => {
    self.streaming_active = true;
    // net is not accessible here; enter_streaming_mode never called
    Ok(WorkerPullState::ReducingChunk)
}
```

**After (schematic):**
```rust
WorkerPullEvent::AssignPartitionReceived { worker_id } => {
    self.streaming_active = true;
    // SPEC-21 R37b / SPEC-22 A6: arm the is_in_delta_round gate so
    // Strategy A/B free-list protection activates for this chunk.
    enter_streaming_mode(net); // net must be threaded in from caller
    Ok(WorkerPullState::ReducingChunk)
}
```

The design challenge is that `WorkerPullContext` does not hold a reference to `Net`. The fix requires either passing `&mut Net` through the transition call or having the caller invoke `enter_streaming_mode` immediately after a `AssignPartitionReceived` transition. The existing `enter_streaming_mode` helper in `worker.rs` is the correct API surface; it only needs a call-site in the protocol handler.

**Why:** Without this wiring, the entire R37b safety guarantee is unit-tested but not enforced in practice. The default build (without `streaming-no-recycle`) relies entirely on runtime gates that are never armed. This is MF severity because G1 safety at M5 scale depends on this activation path.

---

## Should-Fix Issues

### SF-001: `free_list_pops_border` counter semantics mismatch — name says "protected pops" but increments on unprotected pops

**Category:** Code Quality / Test Coverage
**Principle/Spec:** SPEC-21 R37b; SPEC-22 R10b
**File:** `relativist-core/src/net/core.rs:381-386` (counter increment), `relativist-core/tests/spec22_strategy_b.rs:280-282` (missing assertion in UT-0590-07)

**Problem:** `free_list_pops_border` doc comment says "pops where the popped ID IS in `border_entries_shadow`", implying it counts successful pops of border-referenced IDs — the G1 violation scenario. But the counter is incremented inside the `else` branch (line 305 — the successful pop branch), after Strategy B has already rejected and re-pushed truly protected IDs. The counter therefore increments only when:
- `is_in_delta_round == false` (push mode), OR
- `recycle_policy != BorderClean`, AND
- `is_border_protected(id)` is true.

In other words, the counter fires on unprotected pops of shadow-present IDs (the pre-fix bug condition). UT-0590-07 demonstrates exactly this: `is_in_delta_round = false`, border ID 47 in shadow, pop succeeds. The test asserts `free_list_pops == 1` but does NOT check `free_list_pops_border`. The counter will show 1 in this scenario, but no test catches or explains it.

**Before (`net/core.rs` line 381-386):**
```rust
// TASK-0589: count successful pops for test observability.
self.free_list_pops += 1;
// TASK-0590: classify pop as border vs non-border for Strategy B tests.
if self.is_border_protected(id) {
    self.free_list_pops_border += 1;
} else {
    self.free_list_pops_non_border += 1;
}
```

**After — rename to clarify semantics:**
```rust
// TASK-0589: count successful pops for test observability.
self.free_list_pops += 1;
// TASK-0590: classify by border membership. Note: any pop counted here
// succeeded (the ID was not re-pushed). If border_protection was active
// (is_in_delta_round && BorderClean), this ID was NOT shadow-protected
// (it passed the re-push guard). If border_protection was inactive (push mode
// or DisableUnderDelta), the ID may be in border_entries_shadow — that is
// the push-mode pop scenario, NOT a G1 violation (protection is inactive).
if self.is_border_protected(id) {
    // Popped ID is in border_entries_shadow but protection was INACTIVE
    // (otherwise it would have been re-pushed above). In tests, this counter
    // fires when is_in_delta_round=false and a shadow ID is recycled (expected).
    self.free_list_pops_border += 1;
} else {
    self.free_list_pops_non_border += 1;
}
```

Additionally, add to UT-0590-07:
```rust
// pops_border must be 1 here: ID 47 IS in shadow, but protection was
// inactive (is_in_delta_round=false). This is the expected push-mode behavior.
assert_eq!(
    net.free_list_pops_border, 1,
    "UT-0590-07: push-mode shadow-present pop must increment free_list_pops_border"
);
```

**Why:** The current doc comment on `free_list_pops_border` actively misleads readers into thinking a non-zero value means "G1 was violated". It does not. Clarifying the comment and adding the assertion in UT-0590-07 makes the semantics unambiguous.

---

### SF-002: Stale forward-reference comments "TASK-0589 will add..." survive past W7

**Category:** Code Quality / Documentation
**File:** `relativist-core/src/worker.rs:924,929`

**Problem:** The `enter_streaming_mode` doc comment (written during W5) says "TASK-0589 will extend the gate to also read a `streaming_active` flag from worker state". TASK-0589 (W7) was completed but chose to reuse `is_in_delta_round` as the proxy without adding a new field. The "will" language refers to a decision that has already been made.

**Before:**
```rust
/// TASK-0589 will extend the gate to also read a `streaming_active` flag from
/// worker state; this helper is the call-site that sets it.
// ...
// TASK-0589 will add a dedicated `streaming_active` field if needed;
// for now, the existing is_in_delta_round gate is sufficient...
```

**After:**
```rust
/// TASK-0589 (Wave 7) chose to reuse `is_in_delta_round` as the conservative
/// proxy for streaming-active. No dedicated `Net.streaming_active` field was
/// added; the existing gate is sufficient because `RecyclePolicy::DisableUnderDelta`
/// checks `is_in_delta_round` already.
// ...
// Decision (TASK-0589 W7): reuse is_in_delta_round; no new Net field needed.
```

**Why:** Forward-reference comments that outlive the task they reference become false documentation. Readers of worker.rs after W7 are misled into expecting future work that already happened.

---

### SF-003: `RoundRobinStreamingStrategy` and `FennelStreamingStrategy` lack standard derives

**Category:** Code Quality / Idiomatic Rust
**Principle/Spec:** CLAUDE.md coding standards (`#[derive(Debug, Clone, ...)]` on public types)
**File:** `relativist-core/src/partition/streaming.rs:338,448`

**Problem:** Both public strategy structs have no `#[derive]` attributes. The project coding standard in `CLAUDE.md` requires `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]` on public types. `RoundRobinStreamingStrategy` can derive all of these. `FennelStreamingStrategy` has a `HashMap<AgentId, WorkerId>` field and `f64 alpha`, so `Serialize`/`Deserialize` are feasible; `PartialEq` / `Eq` would require a manual implementation (f64 NaN issues), but `Debug` and `Clone` are zero-cost derives that are conspicuously absent.

**Before (`streaming.rs` line 338):**
```rust
pub struct RoundRobinStreamingStrategy {
```

**After:**
```rust
#[derive(Debug, Clone)]
pub struct RoundRobinStreamingStrategy {
```

**Before (`streaming.rs` line 448):**
```rust
pub struct FennelStreamingStrategy {
```

**After:**
```rust
#[derive(Debug, Clone)]
pub struct FennelStreamingStrategy {
```

**Why:** `Debug` is especially important for strategy structs: when tests fail or logs fire, the strategy state is invisible. `Clone` enables test harnesses to checkpoint and replay strategy state. The absence of derives is a low-effort fix with material testability benefit.

---

### SF-004: `GridConfig.recycle_under_delta` is declared but never read — propagation to `Net.recycle_policy` is absent

**Category:** Spec Violation (partial — the D-009 SF-003 is half-resolved)
**Principle/Spec:** SPEC-22 R10b; SPEC-21 §3.8 A6
**Files:** `relativist-core/src/merge/types.rs:632` (field declared), `relativist-core/src/protocol/` (no consumer)

**Problem:** `GridConfig.recycle_under_delta: RecyclePolicy` was added in W1 (`2f751a4`), which closes the D-009 SF-003 public-control-surface gap. However, the field is never read by any production code path to populate `net.recycle_policy` on the worker subnet. The only assignments to `net.recycle_policy` in the codebase are in `#[test]` modules inside `net/core.rs`. At a grid run, the worker's `Net` always gets the default `RecyclePolicy::DisableUnderDelta` regardless of what `GridConfig.recycle_under_delta` says.

This means Strategy B (`BorderClean`) is inaccessible from any coordinator-driven run, even if a user sets `GridConfig.recycle_under_delta = RecyclePolicy::BorderClean`. The field is a dead configuration knob.

**Suggested fix:** In the worker initialization path (wherever `build_subnet` or `build_subnet_with_config` is called before dispatch), propagate the config:
```rust
// After building the subnet:
subnet.recycle_policy = config.recycle_under_delta;
```

This is a one-liner per subnet initialization site; the exact location depends on D-009 MF-002's fix (once `split()` routes through `build_subnet_with_config`, that is the natural propagation point).

**Why:** Without propagation, `GridConfig.recycle_under_delta` is a public API that silently does nothing. Users opting into Strategy B will see no effect and no error.

---

## Nice-to-Have

### NTH-001: `default_chunked_iter` signature documents `chunk_size` as "ignored" but the parameter name uses a leading underscore only in the trait default impl

**Category:** Code Quality / Documentation
**Files:** `relativist-core/src/bench/streaming.rs:55`, `relativist-core/src/bench/mod.rs:133`

The standalone `default_chunked_iter(net: Net)` (bench/streaming.rs line 55) has no `chunk_size` parameter at all, while the trait default impl at bench/mod.rs line 133 names it `_chunk_size` (underscore prefix signals "unused"). The two signatures are inconsistent: callers of the standalone function cannot pass a chunk size even for experimentation. Consider giving `default_chunked_iter` a `chunk_size: usize` parameter with a comment explaining it is ignored ("always emits one batch — parameter exists for API uniformity"), matching the trait default's intention.

---

### NTH-002: `REF-TBD` citations in `FennelStreamingStrategy` doc comment (SC-020 deferral)

**Category:** Code Quality / Documentation
**File:** `relativist-core/src/partition/streaming.rs:425-428`

The two `REF-TBD` annotations (Tsourakakis 2014, Stanton & Kliot 2012) are noted as pending registration in `docs/theory-bridge.md` by BIBLIOTECARIO. This is an explicit SC-020 deferral, which is spec-allowed. Flagged here for tracking only; this should be resolved before the v2 open-source publication.

---

### NTH-003: `StreamingStrategyConfig::build` casts `f32 alpha` to `f64` silently

**Category:** Code Quality / Idiomatic Rust
**File:** `relativist-core/src/merge/types.rs:398`

```rust
StreamingStrategyConfig::Fennel { alpha } => Box::new(
    crate::partition::FennelStreamingStrategy::new(num_workers, *alpha as f64),
)
```

The `f32 → f64` cast is implicit and silent. Since `FennelStreamingStrategy` stores `f64 alpha` but `GridConfig` serializes `f32 alpha` (to reduce config file noise), the precision loss on deserialization is by design. A comment or `From` conversion would make the intentional truncation explicit.

---

## Passed Checks

- [x] Wire protocol: `Message::RequestWork` disc=17, `NoMoreWork` disc=18 — appended at tail, no collisions
- [x] `PROTOCOL_VERSION = 6` with `PREVIOUS_LIVE_VERSION = 5`; `const_assert` guards relative +1 increment — R37c satisfied
- [x] No hardcoded `6` or `5` in the version bump code; always `PREVIOUS_LIVE_VERSION + 1` — R37c defensive language honored
- [x] `UnsupportedVersion` rejection path: pre-bump deserializers reject post-bump payloads — R37c clause 2 satisfied
- [x] Discriminant stability: new variants appended at end of enum, `#[non_exhaustive]` preserved — SPEC-06 R37 satisfied
- [x] `RequestWork { worker_id }` carries `WorkerId` as a newtype (not raw `u32`) — SPEC-02 newtype discipline satisfied
- [x] `generate_and_partition_chunked` returns `Err(PartitionError::UnresolvedForwardReferences)` on non-empty pending store after last chunk — R19 satisfied
- [x] R26 short-circuit: `chunk_size == u32::MAX` materializes full stream and delegates to `split()` — R26 satisfied
- [x] `FreePortInterface` variant added to `ConnectionDirective` for Lafont interface ports — R29b closure correct
- [x] `CHUNK_SIZE_MAX_SENTINEL = u32::MAX` exported from `partition::streaming` — R26 sentinel visible
- [x] `ChunkedPartitionResult` implements `From<ChunkedPartitionResult> for PartitionPlan` — R21 satisfied
- [x] `StreamingPartitionStats.chunks_processed` is pipeline-owned (strategy returns 0 as placeholder) — SC-021 convention satisfied
- [x] `RoundRobinStreamingStrategy`: O(1) per agent, deterministic, C1 guaranteed — R4/R7/R8 satisfied
- [x] `FennelStreamingStrategy`: assignment_cache size O(total_agents) — R6 satisfied
- [x] `FennelStreamingStrategy` tiebreak = lowest WorkerId (not HashMap iteration order) — R8 determinism satisfied
- [x] `PartitionAccumulator` backed by `SparseNet` by default — SC-006 closure satisfied
- [x] `GridConfig` gains `chunk_size`, `streaming_strategy`, `dispatch_mode`, `max_pending_lifetime` — SPEC-21 §3.8 A3 satisfied
- [x] `GridConfig.recycle_under_delta` field present (resolves D-009 SF-003 declaration gap) — SPEC-22 R10b control surface satisfied (declaration; propagation gap is SF-004)
- [x] `DispatchMode` enum: Auto/Push/Pull with correct `Auto` default — R34 satisfied
- [x] All 4 `GridConfig` streaming fields have `#[serde(default)]` — backward-compatible with old config files
- [x] Coordinator FSM: 5 `Pull*` states added with correct transition table (R32 steps 1-7 coverage) — SPEC-13 A5 satisfied
- [x] Worker FSM: `AwaitingChunkAfterResult` and `FinalReduction` states added — SPEC-13 A5 satisfied
- [x] R37d BSP barrier: `PartitionResult` arrivals buffered in `PullAwaitingResults`, merge deferred to `PullAwaitingFinalResults` — R37d / SC-019 satisfied
- [x] R37e: `assert_no_more_work_not_in_pull_mode` guard present; push-mode FSM never enters `NoMoreWork` path — R37e satisfied
- [x] `Strategy B is_in_delta_round` gate correctly requires `is_in_delta_round == true` before per-id check — W8 fix for pre-W8 push-mode misfire is correct
- [x] `free_list_pops` / `free_list_pops_border` / `free_list_pops_non_border`: all three gated on `#[cfg(debug_assertions)]` — zero overhead in release
- [x] All three counters: `#[serde(skip)]` and `#[cfg_attr(feature = "zero-copy", rkyv(with = rkyv::with::Skip))]` applied uniformly at every `Net` literal site (`core.rs`, `sparse.rs`, `partition/{compact,helpers,remap}.rs`, `tests/partition_amendments.rs`) — W7/W8 site updates complete
- [x] `streaming-no-recycle = []` feature gate declared in `Cargo.toml`; short-circuit gated on `is_in_delta_round == true` — W9 R37b alternative closure correct
- [x] `streaming-no-recycle` feature does not disable Strategy A/B runtime gates — both gates remain present for non-feature builds (W9 acceptance line 24 honored)
- [x] CI matrix gains `streaming-no-recycle` column in `.github/workflows/ci.yml` — W9 CI gate present
- [x] `extend_with_chunk_borders` idempotency on previously-seen border IDs — R37f idempotency clause satisfied at method level
- [x] `extend_with_chunk_borders` is a no-op on empty `new_borders` — R37f no-op guard present
- [x] Core layer (`partition/streaming.rs`, `net/core.rs`, `bench/streaming.rs`) has no async, no tokio, no I/O — R9/R16/SPEC-13 purity satisfied
- [x] Module dependency direction: `partition/streaming.rs` imports from `net::`, `partition::types` only (no `protocol`, no `merge`, no `reduction`) — SPEC-13 inviolable dependency preserved
- [x] No `unwrap()` in production code in the new files (`streaming.rs`, `border_graph.rs` new sections, `coordinator.rs`, `worker.rs` new sections)
- [x] D-009 MF-002 (`split()` bypasses `build_subnet_with_config`) was fixed in this bundle — regression gate at `split.rs:225-230` present
- [x] D-009 SF-002 (stale I3 doc comment on `next_id`) was fixed: `net/core.rs:50` now cites I3' correctly
- [ ] `extend_with_chunk_borders` called from production coordinator path under delta+streaming — NOT satisfied (MF-001)
- [ ] `enter_streaming_mode` called from production worker protocol path on `AssignPartitionReceived` — NOT satisfied (MF-002)
- [ ] `GridConfig.recycle_under_delta` propagated to `net.recycle_policy` at worker init — NOT satisfied (SF-004)
- [ ] `RoundRobinStreamingStrategy` and `FennelStreamingStrategy` have `#[derive(Debug, Clone)]` — NOT satisfied (SF-003)

---

## Test Coverage Assessment

**Phase B–D (foundation + strategies + bench):** Solid. 35 + 16 + 19 tests respectively cover R4/R5/R6/R7/R8 contracts, serde round-trips, R15 monotonicity checker, T6/T8 isomorphism oracles. The T6 and T8 integration tests in `spec21_t6_streaming_vs_batch.rs` and `spec21_t8_chunk_size_independence.rs` correctly use `nets_isomorphic` (not byte-equality) per R26.

**Phase E (accumulator + orchestrator):** 28 tests. Coverage of `install_connection` classification (internal vs. border), `PartitionAccumulator` lifecycle, R19 empty-pending-store enforcement, and R29 ID-range assignment are adequate.

**Phase F W3 (wire protocol):** 17 tests across `wire_v3_streaming_variants.rs` and `wire_v4_protocol_version_bump.rs`. Discriminant stability at 17/18, pre-bump rejection, `PREVIOUS_LIVE_VERSION + 1` contract — all covered.

**Phase F W4–W5 (FSM):** 18 + 16 tests. Transition tables and R37e push-mode separation are tested. Missing: an integration test that exercises the full FSM round-trip through the actual protocol handler (the FSM is tested in isolation; the protocol layer that would call `enter_streaming_mode` is not tested end-to-end).

**Phase F W8 (Strategy B):** 12 tests in `spec22_strategy_b.rs`. Key gap: UT-0590-07 (`push-mode pop of shadow ID`) does not assert `free_list_pops_border` value, leaving the counter's push-mode behavior undocumented by test (see SF-001).

**Phase F W9 (streaming-no-recycle):** 12 tests. Feature-gated tests correctly gate on `cfg(feature = "streaming-no-recycle")` and `cfg(not(...))`. Cross-feature isomorphism tests (IT-0591-01/02) verify that normal-form results are equivalent with and without the feature (though this test requires a running grid, it uses `nets_isomorphic` correctly).

**Overall count:** 1662 default / 1705 zero-copy / 1661 streaming-no-recycle — all above the v2 baseline entering D-010 (1595/1638). The v1 floor of 690 is preserved.

**Notable gap:** No integration test exercises the R37f call-site discipline (delta+streaming combined) end-to-end. MF-001 manifests at this level.

---

## Posture

**Safe to advance to QA Stage 5?** Conditionally.

The bundle is functionally correct for streaming-only (non-delta) mode and is ready for QA at that scope. MF-001 and MF-002 affect the delta+streaming conjunction only; under `delta_mode = false` (the default), the streaming pipeline produces correct results.

The recommended path before Stage 5 closes:
1. MF-001: Developer adds the `extend_with_chunk_borders` call-site in the coordinator's `GeneratingNext` transition (or in `generate_and_partition_chunked_with_chunk_size` when a `BorderGraph` ref is available). This is a one-liner at the correct integration point.
2. MF-002: Developer adds a `enter_streaming_mode(net)` call in the worker protocol handler on `AssignPartitionReceived` (and `exit_streaming_mode(net)` on `FinalReduction → SendFinalResult`). Both helpers already exist; only the call-site is missing.
3. SF-003 (derive attributes) and SF-004 (recycle_under_delta propagation) can be deferred to the first QA fix-pass without blocking Stage 5 entry, provided QA's adversarial scope is restricted to streaming-only mode.
4. SF-001 and SF-002 are documentation cleanups that have zero risk and can be batched with the fix-pass.

Once MF-001 and MF-002 are resolved, the bundle is clear for a full QA pass including the delta+streaming combination.
