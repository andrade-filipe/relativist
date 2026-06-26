# Bundle: SPEC-19 §3.3 — Stateful Worker Lifecycle + Delta BSP Loop (item 2.26-C)

**Created:** 2026-04-17
**Owner:** task-splitter (orchestrated by sdd-pipeline)
**Stage:** 1 SPLITTING — complete (this file + 10 TASK-NNNN.md files, IDs 0379..0388).
  **Stage 1.5 SPEC-CRITIC — COMPLETE (2026-04-17).** Verdict at
  `docs/spec-reviews/SPEC-19-section-3.3-2.26C-design-choices-2026-04-17.md`.
  Six design choices ruled (DC-C1..DC-C6, the sixth emerged during splitting).
  **APPROVED WITH AMENDMENTS to TASK files only — 0 SPEC edits.**
  Amendments required before Stage 2 TESTS dispatches:
    - DC-C3 (BLOCKING): TASK-0384 must REMOVE `strict_bsp` assertion; TASK-0385
      must add strict/lenient branch + 3 tests (flip bundle-index DC-C3 pick A→C).
    - DC-C5 (BLOCKING): TASK-0386 predicate gains third conjunct
      `all stats.local_redexes == 0`; one test polarity inverts; doc-comment rewrites.
    - DC-C1 / DC-C2 / DC-C4 / DC-C6 (non-blocking wording + 2 new TASK-0382 tests).
  **Stage 2 TESTS blocked on task-updater applying DC-C3 and DC-C5 amendments
    AND on 2.26-A + 2.26-B merge (hard DEV-time dependency).**
**Test baseline before bundle:** TBD — depends on the landing counts of 2.26-A (wire variants)
  and 2.26-B (coordinator border resolver). Both sibling bundles MUST be merged before 2.26-C
  enters Stage 3 DEV. Splitting (this document) proceeds now per the orchestrator brief.
**Hard floor (CLAUDE.md):** 690 lib tests post-bundle; v1 baseline must never decrease.
**Expected final counts:** +25 to +40 lib / integration tests across 10 tasks
  (TEST-SPEC-0379..0388 in Stage 2 will pin exact counts).
**Estimated total LoC:** ~1200 across 10 atomic tasks (see table).
**Tier 1 break-even path (V2-FEATURE-MATRIX):** third of 4 sub-bundles under item 2.26;
  consumes the outputs of 2.26-A (wire) and 2.26-B (resolver) and produces the actual
  delta-mode BSP loop `run_grid_delta`.

## Naming convention

The bundle is called **2.26-C** (the ITEM's sub-bundle label, per the task-splitter
briefing). The file name uses "§3.3" to match SPEC-19's "Delta-Only Protocol"
section. This bundle ships R20..R30 of SPEC-19 §3.3.

## Scope (in vs out)

**In scope (SPEC-19 §3.3, R20..R30 — stateful worker + delta BSP loop):**

- **R20** — `GridConfig.delta_mode: bool` READ (field declared under 2.26-D).
  This bundle only **consumes** the flag; if 2.26-D has not landed at DEV time
  a stub constant `const DELTA_MODE_DEFAULT: bool = false;` is acceptable (see
  DC-C4). `run_grid` (v1) runs unchanged when the flag is `false`; `run_grid_delta`
  (new) is dispatched when the flag is `true`.
- **R21** — the three-phase lifecycle: (1) Round 0 Initial Dispatch,
  (2) Rounds 1+ Delta Rounds, (3) Final State Collection on convergence.
- **R22** — worker stores `Partition` in persistent local state across rounds.
- **R23** — coordinator sends `RoundStart` with `border_deltas`, `resolved_borders`,
  `new_borders` (the `Message::RoundStart` variant already ships from 2.26-A;
  this bundle populates and consumes it).
- **R24** — worker apply-deltas → `reduce_all` → rebuild index → compute own deltas.
- **R25** — worker maintains `previous_border_state: HashMap<u32, PortRef>` and
  emits only changed entries as `BorderDelta`s.
- **R26** — worker emits `RoundResult { round, border_deltas, stats,
  has_border_activity }` (the `Message::RoundResult` variant from 2.26-A).
- **R27** — coordinator sends `FinalStateRequest` at convergence.
- **R28** — worker responds with `FinalStateResult { round, partition }`.
- **R29** — coordinator `merge()` from collected partitions + remaining BorderGraph.
- **R30** — `max_rounds` cap: return partially reduced net + non-convergence
  indicator in `GridMetrics`.

**Out of scope (separate sub-bundles under item 2.26 — task-splitter MUST NOT
generate tasks for these):**

- **2.26-A** — wire-protocol extensions (R31..R37). The 5 new `Message` variants
  (`InitialPartition`, `RoundStart`, `RoundResult`, `FinalStateRequest`,
  `FinalStateResult`) and `BorderDelta` serde wiring ship under 2.26-A
  (TASK-0366..0371). **This bundle consumes those variants but does not modify
  `protocol/*`.**

- **2.26-B** — coordinator border resolver (R13..R15). The pure-core
  `BorderResolver` module in `merge/border_resolver.rs` with
  `BorderResolution` and `RoundStartDispatch` ships under 2.26-B
  (TASK-0372..0377). **This bundle consumes `BorderResolver::resolve(..)`
  inside `run_grid_delta` but does not modify `border_resolver.rs`.**

- **2.26-D** — invariant amendments + config (R38..R42). `GridConfig.delta_mode`
  field declaration, G1 / D3 / D6 amendments, and delta-specific `GridMetrics`
  counters (beyond the minimal observability extension in TASK-0388) ship under
  2.26-D. **This bundle consumes `config.delta_mode` but does not modify
  `config.rs`.**

**Files YOU may touch (hard list):**

- `relativist-core/src/worker.rs` — add stateful delta-mode code path; v1 logic
  MUST remain unchanged when `delta_mode == false`.
- `relativist-core/src/merge/grid.rs` — add `run_grid_delta` as a separate function
  next to `run_grid`. `run_grid` itself is NOT modified in this bundle.
- `relativist-core/src/merge/types.rs` — MAY extend `GridMetrics` with a minimal
  `delta_mode: bool` marker and `delta_rounds_converged: Option<u32>` for R30
  observability. Larger delta-specific metric additions belong to 2.26-D.
- `relativist-core/src/coordinator.rs` — READ-ONLY consumer; 2.26-B dispatcher
  plumbing is already in place. TASK-0384..0388 treat this file as reference only.
  (If a thin `run_grid_delta` entry point is needed here, that is flagged as
  DC-C2 and may be deferred to 2.26-D.)
- `relativist-core/tests/` — integration tests for the delta loop end-to-end.

**Files FORBIDDEN (to avoid merge conflicts with parallel sub-bundles):**

- `relativist-core/src/merge/border_graph.rs` — already shipped by §3.2 bundle.
- `relativist-core/src/merge/border_resolver.rs` — owned by 2.26-B.
- `relativist-core/src/config.rs` (`GridConfig.delta_mode`) — owned by 2.26-D.
- `relativist-core/src/protocol/types.rs` / `messages.rs` / `frame.rs` — owned by 2.26-A.
- `specs/*` — read only.

## Pure-core layer compliance (R19 — inherited)

`merge/grid.rs` has always been pure-core (no tokio, no async, no I/O) and
`run_grid_delta` preserves this property. The worker-side state machine in
`worker.rs` is allowed to interact with tokio because `worker.rs` is outside
the pure-core layer (it lives at the crate root alongside `coordinator.rs`,
per SPEC-13 R6-R8). All delta computation helpers called from the worker
(`apply_border_deltas_to_partition`, `compute_outgoing_deltas`) live in
pure-core helpers and MUST NOT depend on tokio.

## Design choices flagged for spec-critic (Stage 1.5)

**DC-C1 — Round 0 → Round 1 transition signal (TASK-0380 / TASK-0385).**
How does the worker know the coordinator has finished Round 0 and is entering
Round 1? Three candidates:
  (a) explicit `RoundStartAck` sent after `InitialPartition` is stored
      (requires a NEW `Message` variant — cross-bundle change into 2.26-A);
  (b) no explicit ack — the worker transitions on arrival of the first
      `RoundStart` message (arrival-triggered FSM);
  (c) piggyback an ack inside a synthetic empty `RoundResult` (requires
      SPEC-19 §3.3 amendment because R26 does not cover empty-round
      semantics).
Current bundle assumes **option (b)**: arrival-triggered, no new variant,
coordinator treats `InitialPartition` dispatch as fire-and-forget and blocks
until every worker's first `RoundResult` (round == 1) arrives. This is
symmetric to v1's `AssignPartition` fire-and-forget behavior. **Spec-critic
please rule.** If (a) is required, 2.26-A gains a 6th variant and this
bundle is blocked.

**DC-C2 — wire plumbing for `run_grid_delta` (TASK-0384..0388).**
Does `run_grid_delta` share the async TCP plumbing with the existing
wire-level coordinator FSM in `protocol/coordinator.rs`, or is it a wholly
new I/O path? Three candidates:
  (a) `run_grid_delta` stays at the `merge/grid.rs` level as a pure-core
      orchestrator that takes a `&dyn DeltaTransport` abstraction
      (in-process + TCP + NOOP for tests) — symmetric to `PartitionStrategy`;
  (b) `run_grid_delta` lives entirely in `coordinator.rs` with tokio and
      is NOT pure-core — matches the existing `coordinator.rs` structure;
  (c) split the work: the pure-core skeleton in `merge/grid.rs` calls a
      trait `WorkerDispatch` whose async implementation lives in
      `coordinator.rs`; in-process tests use a synchronous mock.
Current bundle assumes **option (c)**. TASK-0384 defines
`pub trait WorkerDispatch { fn dispatch_round(...) -> Result<Vec<RoundResult>, _>; }`
in `merge/types.rs`; TASK-0385..0387 consume it; the real async
implementation lives OUTSIDE this bundle (belongs to 2.26-C-wire or
2.26-D). A synchronous in-process `LocalDeltaDispatch` mock is provided
in TESTS (Stage 2). **Spec-critic please rule.** Option (a) may be
cleaner; option (b) duplicates control flow but avoids the trait.

**DC-C3 — interaction between `strict_bsp` and `delta_mode` (TASK-0385).**
Do `config.strict_bsp` and `config.delta_mode` combine, or is one strictly
a superset? Three candidates:
  (a) `delta_mode` **requires** `strict_bsp = true` (lenient mode not
      supported in delta mode; config rejects the combination at parse
      time — a 2.26-D concern);
  (b) both flags are independent; lenient delta mode means "run
      reduce_all after applying deltas, do not run reduce_border_once";
  (c) `delta_mode = true` implies `strict_bsp = true` automatically
      (silent override at `run_grid_delta` entry).
Current bundle assumes **option (a)** — `run_grid_delta` asserts at
entry `config.strict_bsp == true`. Lenient delta mode is left for a
future item. **Spec-critic please rule.** This cascades to 2.26-D's
config validation and to TASK-0384 scaffolding.

**DC-C4 — `previous_border_state` initial value (TASK-0379).**
At Round 0 (after `InitialPartition` is stored), what is the initial
content of `previous_border_state`? Two candidates:
  (a) **empty map** — Round 1's first delta computation compares against
      an empty baseline, emitting a delta for EVERY border endpoint
      (effectively a full re-dispatch of border state to the coordinator,
      which mirrors what `BorderGraph::from_partition_plan` already
      knows);
  (b) **seeded from `partition.free_port_index`** at the moment of
      storage — Round 1 emits deltas only for borders actually changed
      by Round 1's local reduction.
Current bundle assumes **option (b)** — seeded at storage time — to
avoid the Round 1 redundant dispatch. **Spec-critic please rule.** Option
(a) is simpler but wastes one round's bandwidth.

**DC-C5 — termination predicate at the coordinator (TASK-0386).**
Is `has_border_activity == false` for all workers AND
`BorderGraph::detect_border_redexes().is_empty()` sufficient for
termination, or also require `stats.local_redexes == 0` for all workers?
R4 says "Global Normal Form = all workers internally stable AND
BorderGraph has zero active pairs". R21.3 says "when the coordinator
determines Global Normal Form, it sends FinalStateRequest". Strict
reading of R4: local_redexes == 0 IS required. But in practice a worker
that reports `local_redexes > 0` in round N will have performed those
interactions during round N's `reduce_all` (so the reported value is
the interactions that HAPPENED, not remaining work). The invariant
that matters is "reduce_all reached a fixed point, so redex_queue is
empty post-round". The worker's `has_border_activity` already reflects
the post-reduction free port index; the correct coordinator predicate
is therefore:
  - all workers report `has_border_activity == false`, AND
  - `BorderGraph::detect_border_redexes()` returns an empty vec.
**Spec-critic please rule** whether `local_redexes == 0` is a redundant
additional predicate, a correctness requirement, or a defensive check.
Current bundle uses the two-predicate form (no `local_redexes` check)
and documents the reasoning in TASK-0386.

## Task graph (DAG)

```
         TASK-0379 (S, ~100)  — worker persistent state struct
               │
               ▼
         TASK-0380 (S, ~100)  — worker Round 0 handler (InitialPartition)
               │
               ▼
         TASK-0381 (M, ~180)  — worker delta-round handler (RoundStart → RoundResult)
               │
               ├────────────┐
               ▼            ▼
         TASK-0382 (S,    TASK-0383 (S, ~100)  — worker final-state handler
         ~120) — delta     (FinalStateRequest → FinalStateResult)
         computation
               │
               ▼
         TASK-0384 (S, ~100)  — run_grid_delta scaffolding (dispatch on delta_mode)
               │
               ▼
         TASK-0385 (L, ~200)  — coordinator round loop (Round 0 + delta rounds)
               │
               ▼
         TASK-0386 (S, ~100)  — coordinator convergence detection (GNF)
               │
               ▼
         TASK-0387 (M, ~150)  — coordinator final-state collection + merge
               │
               ▼
         TASK-0388 (S, ~80)   — max_rounds cap + non-convergence indicator
```

| ID   | Title | Spec Reqs | Size | LoC est. | Depends |
|------|-------|-----------|------|----------|---------|
| 0379 | Worker persistent state `WorkerDeltaState` + init from `InitialPartition` | R22, R25 | S | ~100 | none (consumes 2.26-A wire) |
| 0380 | Worker Round 0 handler — receive `InitialPartition`, store, seed state | R21.1, R22 | S | ~100 | 0379 |
| 0381 | Worker delta-round handler — apply deltas, reduce, rebuild, report | R23, R24, R26 | M | ~180 | 0380 |
| 0382 | Worker delta computation — diff current vs `previous_border_state` | R25 | S | ~120 | 0381 |
| 0383 | Worker final-state handler — `FinalStateRequest` → `FinalStateResult` | R21.3, R28 | S | ~100 | 0380 |
| 0384 | `run_grid_delta` scaffolding + `delta_mode` dispatch + v1 fallback | R20, R21 | S | ~100 | 0379..0383 (compile-time), 2.26-B (DEV-time) |
| 0385 | Coordinator round loop — Round 0 dispatch + delta round orchestration | R21.1, R21.2, R23, R26 | L | ~200 | 0384 |
| 0386 | Coordinator convergence detection — Global Normal Form predicate | R4, R21.3 | S | ~100 | 0385 |
| 0387 | Coordinator final-state collection + final `merge()` | R21.3, R27, R29 | M | ~150 | 0386 |
| 0388 | `max_rounds` cap + non-convergence indicator in `GridMetrics` | R30 | S | ~80 | 0385, 0386 |

**Total:** ~1230 LoC across 10 tasks. Implementable in topological order:
0379 → 0380 → {0381, 0383} → 0382 → 0384 → 0385 → 0386 → 0387 → 0388.

## Dependencies on sibling bundles

**Hard block on 2.26-A (DEV-time):** requires `Message::InitialPartition`,
`Message::RoundStart`, `Message::RoundResult`, `Message::FinalStateRequest`,
`Message::FinalStateResult`, and `crate::protocol::BorderDelta` to exist.
SPLITTING (this document) proceeds now; DEV stage cannot start until 2.26-A
TASKs 0366..0371 merge.

**Hard block on 2.26-B (DEV-time):** requires
`crate::merge::border_resolver::{BorderResolver, BorderResolution,
RoundStartDispatch}` to exist. TASK-0385 / 0386 / 0387 import these types.
SPLITTING proceeds now; DEV stage cannot start until 2.26-B TASKs 0372..0376
merge.

**Soft block on 2.26-D:** requires `GridConfig.delta_mode: bool`. If
2.26-D has not landed at DEV time, TASK-0384 introduces a file-local
stub constant `DELTA_MODE_DEFAULT = false` and a cfg-gated test hook so
integration tests can exercise the delta path. Spec-critic should rule
on whether this stub is acceptable (DC-C4) or whether 2.26-D must land
first.

## Per-task feature/build compatibility

- Bundle is feature-flag-neutral. None of the ten tasks add a cargo feature.
- Both `cargo test --workspace` and `cargo test --workspace --features zero-copy`
  MUST stay GREEN at every task boundary. Bundle adds tests on top of both baselines.
- All production code is unconditional (no `#[cfg]` outside standard `#[cfg(test)]`
  inline-test pattern).
- v1 behavior (all tests with `delta_mode = false`) MUST remain bit-identical.

## Acceptance gate for the whole bundle

- All 10 tasks shipped GREEN through Stages 3-6.
- `cargo test --workspace` test count ≥ 690 lib baseline (CLAUDE.md hard floor);
  bundle adds ~25-40 lib / integration tests on top per Stage 2 estimate.
- `cargo test --workspace --features zero-copy` equally GREEN.
- `cargo clippy --workspace --all-targets -- -D warnings` clean both with and
  without `--features zero-copy`.
- `cargo fmt --check` clean.
- Release smoke `compute add 3 5 → 8` works (default features, delta_mode off).
- Delta smoke: `compute add 3 5 → 8` ALSO works with `delta_mode = true` in
  the in-process `LocalDeltaDispatch` path (TASK-0385..0387 demonstrate this).
- R20 preservation: every existing test with `delta_mode = false` (default)
  produces byte-identical metrics to the pre-bundle run.
- No changes to `merge/border_graph.rs`, `merge/border_resolver.rs`,
  `protocol/*`, or `config.rs`.

## Stage advancement

- **Stage 1 SPLITTING:** complete (this file + 10 TASK-NNNN.md files).
- **Stage 1.5 SPEC-CRITIC:** **COMPLETE (2026-04-17).** Six design
  choices ruled at
  `docs/spec-reviews/SPEC-19-section-3.3-2.26C-design-choices-2026-04-17.md`.
  Summary:
    | DC | Pick | Task impact |
    |----|------|-------------|
    | DC-C1 (Round 0 transition) | **B** — arrival-triggered, no ack | TASK-0380/0385 wording |
    | DC-C2 (run_grid_delta plumbing) | **C** — pure-core + WorkerDispatch trait | TASK-0384/0385 pins |
    | DC-C3 (strict_bsp × delta_mode) | **C** — orthogonal (R40 matrix) ⚠️ flipped from default A | TASK-0384 remove assert + TASK-0385 strict/lenient branch + 3 tests |
    | DC-C4 (previous_border_state seed) | **B** — seed from free_port_index | TASK-0379 wording (code already correct) |
    | DC-C5 (termination predicate) | **B** — three-predicate (R40 literal) ⚠️ flipped from default A | TASK-0386 predicate + test polarity inversion + doc-comment |
    | DC-C6 (disconnection encoding) | **C** — DISCONNECTED sentinel (§3.2 DC-1 precedent) | TASK-0382 doc-comment + 2 new tests |

  **BLOCKING amendments (must land before TESTS):** DC-C3 (TASK-0384 +
  TASK-0385), DC-C5 (TASK-0386). Non-blocking amendments apply during
  the same task-updater pass.

  **Cross-bundle flags:**
    - 2.26-A: no cascade (5 variants stand; `BorderDelta` serde derives
      covered by §3.4 DC-A1 amendment).
    - 2.26-B: verify `BorderResolver::resolve` is mode-agnostic (callable
      in both lenient inline and strict deferred paths).
    - 2.26-D: config validator MUST permit all four `(delta_mode, strict_bsp)`
      cells — in particular `(true, false)` for delta lenient.

- **Stage 2 TESTS:** blocked on (i) task-updater applying DC-C3 + DC-C5
  amendments and (ii) 2.26-A + 2.26-B merge. Once unblocked, will
  dispatch `test-generator` with bundle spec = SPEC-19 §3.3 (R20..R30),
  task list = TASK-0379..0388, deliverable =
  `docs/tests/TEST-SPEC-0379.md` … `TEST-SPEC-0388.md`.
- Orchestrator pause requested by parent: STOP after Stage 1.5 and
  await task-updater dispatch before Stage 2 (standard parallel-sub-
  bundle protocol).
