# Bundle: SPEC-19 §3.3 — Coordinator-side border-redex dispatch (item 2.26-B)

**Created:** 2026-04-17
**Owner:** task-splitter (orchestrated by sdd-pipeline)
**Stage:** 1 SPLITTING — complete (this file + 6 TASK-NNNN.md files).
  **Stage 1.5 SPEC-CRITIC — complete** on 2026-04-17. Verdict:
  `docs/spec-reviews/SPEC-19-section-3.3-2.26B-design-choices-2026-04-17.md`.
  All 9 DCs (DC-B1..DC-B9) ruled; 7 task files need amendments
  (TASK-0372..0377 + this bundle index). 3 cross-bundle memos
  required: 2.26-A (new fields on `RoundStart` + `RoundResult`,
  new `PendingCommutation` type), 2.26-C (cache maintenance,
  border-pinning, 2-phase CON-DUP finalization, delta ordering),
  2.26-D (new R40c invariant in §3.5, G1 amendment note). **Stage 2
  TESTS unblocked once task-updater applies amendments.**
**Test baseline before bundle:** TBD — depends on 2.26-A's landing
  count (2.26-A is in Stage 1 SPLITTING as of 2026-04-17). The post-
  2.26-A figures will pin this bundle's pre-count.
**Hard floor (CLAUDE.md):** 690 lib tests post-bundle; v1 baseline
  must never decrease.
**Expected final counts:** +20 to +30 lib default / +20 to +30 lib
  `--features zero-copy` across 6 tasks (TEST-SPEC-0372..0377 in
  Stage 2 will pin exact counts).
**Estimated total LoC:** ~600 across 6 atomic tasks (80 + 130 + 150
  + 80 + 130 + 30).
**Tier 1 break-even path (V2-FEATURE-MATRIX):** second of 4 sub-bundles
  under item 2.26; directly closes D-003 in `docs/DEFERRED-WORK.md`.

## Naming convention

The bundle is called **2.26-B** (the ITEM's sub-bundle label, per the
task-splitter briefing). The file name uses "§3.3" because item 2.26
in ROADMAP corresponds to SPEC-19 §3.3 "Delta-Only Protocol". However,
the *requirements* R13, R14, R15 (parts 1 and 2) actually live in
SPEC-19 **§3.2** (BorderGraph and Delta-Based Merge — item 2.35 spec
section). They were deferred from §3.2's bundle because their caller
(the delta BSP loop) doesn't exist until item 2.26 lands. See
`docs/DEFERRED-WORK.md` D-003 for the full rationale.

## Scope (in vs out)

**In scope (SPEC-19 §3.2, R13 + R14 + R15 parts 1-2 — coordinator
dispatch):**

- R13 — when a border redex is detected (both endpoints principal),
  the coordinator resolves it locally: materialize the two agents from
  the involved workers' partitions, run the appropriate IC rule, and
  emit `BorderDelta`s.
- R14 — the coordinator uses the same 6 interaction rules as local
  reduction (SPEC-03: CON-CON, DUP-DUP, ERA-ERA, CON-DUP, CON-ERA,
  DUP-ERA). Mirrors the `interact_*` topology without calling the
  mutating `interact_*` functions directly (resolver operates on
  read-only partition views + the mutable `BorderGraph`).
- R15 part 1 — package resulting port reconnections as `BorderDelta`s
  keyed to the affected workers.
- R15 part 2 — update the `BorderGraph` post-resolution: call
  `remove_border(bid)` for annihilation/void rules (CON-CON, DUP-DUP,
  ERA-ERA), `add_border_states(...)` for CON-DUP (new cross-partition
  wires), and `apply_deltas` for CON-ERA / DUP-ERA (existing border
  endpoints re-point to new ERA principal ports).

**Out of scope (separate sub-bundles under item 2.26 — task-splitter
MUST NOT generate tasks for these):**

- 2.26-A — wire protocol extensions (R31-R37). The 5 new `Message`
  variants (`InitialPartition`, `RoundStart`, `RoundResult`,
  `FinalStateRequest`, `FinalStateResult`) with discriminants 7..=11
  ship under 2.26-A. This bundle's output (`RoundStartDispatch` — pure-
  core mirror of `RoundStart`'s payload) is designed to map 1:1 into
  `Message::RoundStart` when 2.26-C composes it. Any change to
  `protocol/types.rs`, `protocol/messages.rs`, or `protocol/frame.rs`
  is OUT.

- 2.26-C — stateful worker lifecycle (R20-R30). The worker-side
  `WorkerDeltaState` state machine, `previous_border_state` tracking,
  `run_grid_delta` BSP loop, and the coordinator-side async wire-layer
  caller that turns `BorderResolution` + `RoundStartDispatch` into
  actual wire traffic all ship under 2.26-C. Any change to
  `coordinator.rs` (beyond what `merge/` imports already allow),
  `worker.rs`, or `merge/grid.rs` (`run_grid_delta`) is OUT.

- 2.26-D — invariant amendments + config (R38-R42). `GridConfig.delta_mode`
  flag, G1 / D3 / D6 amendments, `GridMetrics` delta counters all
  ship under 2.26-D. Any change to `config.rs` or `merge/types.rs`
  (`GridConfig`) is OUT.

- R15 part 3 (`add_border_states` primitive definition) — ALREADY
  SHIPPED as TASK-0364 in the §3.2 bundle. This bundle's TASK-0374
  CONSUMES the primitive (adds new border entries after CON-DUP
  resolution) but does not redefine it.

**Files YOU may touch (hard list):**

- `relativist-core/src/merge/border_resolver.rs` — **NEW FILE** (this
  bundle's single new source file).
- `relativist-core/src/merge/mod.rs` — add ONE line: `pub mod border_resolver;`.
- `relativist-core/tests/border_resolver_integration.rs` — optional
  integration test; may be replaced by an inline `#[cfg(test)] mod
  integration_tests` per TASK-0376 fallback.

**Files FORBIDDEN (to avoid merge conflicts with parallel sub-bundles):**

- `relativist-core/src/worker.rs` — owned by 2.26-C.
- `relativist-core/src/merge/grid.rs` (`run_grid_delta`) — owned by
  2.26-C.
- `relativist-core/src/config.rs` (`GridConfig.delta_mode`) — owned
  by 2.26-D.
- `relativist-core/src/protocol/types.rs` / `messages.rs` — owned by
  2.26-A.
- `relativist-core/src/protocol/frame.rs` — owned by 2.26-A.
- `relativist-core/src/coordinator.rs` — the pure-core resolver lives
  in `merge/`; coordinator glue (async wire caller) ships under
  2.26-C. If the coordinator needs new imports from `merge/`, that's
  a 2.26-C change, not 2.26-B.
- `specs/*` — read only.

## Pure-core layer compliance (R19 — inherited)

Per CLAUDE.md (Relativist) module-structure rules, the dependency
direction is `net` <- `reduction` <- `partition` <- `merge` <-
`protocol`, and the core layer (`net/`, `reduction/`, `partition/`,
`merge/`) has zero `async`, zero `tokio`, zero I/O.
`border_resolver.rs` lives in `merge/` and inherits this contract.
TASK-0377 adds a programmatic in-source test that locks the property
down permanently.

The grep guard for the bundle:

```
grep -E '^use\s+(tokio|crate::protocol|async_trait)' \
  relativist-core/src/merge/border_resolver.rs
# expected: zero matches
```

Every task in the bundle individually preserves this property;
TASK-0377 upgrades the manual grep to a programmatic `#[test]`.

## Wire compatibility

This bundle adds **no** wire-format changes. The pure-core
`RoundStartDispatch` struct (TASK-0375) is a *mirror* of 2.26-A's
`Message::RoundStart` payload fields — it's designed for the 2.26-C
converter to trivially map into a `Message::RoundStart` value. No
serde derives are added in this bundle because these types never cross
the wire; they are in-process coordinator scratch structures.

v1 protocol path unchanged; v2 hot path unchanged; delta path not
yet wired (blocked on 2.26-C).

## Design choices flagged for spec-critic (Stage 1.5)

**VERDICT: all 9 DCs resolved on 2026-04-17.** Full reasoning in
`docs/spec-reviews/SPEC-19-section-3.3-2.26B-design-choices-2026-04-17.md`.
Compact table:

| DC | Pick | Amend? |
|----|------|--------|
| DC-B1 | Option (A) — coordinator cache, resolver takes `&[Partition]`; cache maintenance owned by 2.26-C | TASK-0372 |
| DC-B2 | Option (A) — caller-side panic via `assert_agent` helper; `materialize_agent` keeps `Option` return | TASK-0372, TASK-0373 |
| DC-B3 | Option (B renamed) — new `local_reconnections` field on `RoundStart`/`RoundStartDispatch`; `BorderDelta` untouched (R31 amended by 2.26-A) | TASK-0373, TASK-0375, TASK-0376 |
| DC-B4 | Option (B) — worker border-pinning; new R40c invariant in §3.5 (owned by 2.26-D) | TASK-0372, TASK-0373 |
| DC-B5 | Option (B) — workers allocate AgentIds, 2-phase echo; new `pending_commutations` + `minted_agents` fields on `RoundStart`/`RoundResult` (R31/R32 amended by 2.26-A) | TASK-0374, TASK-0375, TASK-0376 |
| DC-B6 | Option (A) — preserve auxiliary-border via `apply_deltas` on CON-ERA / DUP-ERA; no new border IDs for those rules | TASK-0374, TASK-0376 |
| DC-B7 | Option (A) — `(bid, worker_a, worker_b)` triples in `BorderResolution.resolved_borders` | TASK-0373, TASK-0375 |
| DC-B8 | Option (C) — shared helper in `merge/internal/pure_core_guard.rs`; opt-in per file; follow-up item 2.43 migrates `border_graph.rs` | TASK-0377; ROADMAP 2.43 |
| DC-B9 | Option (A) — forbidden-prefix list extended with `use crate::coordinator` and `use crate::worker` | TASK-0377 |

Original flagging preserved below for historical context:



**DC-B1 — agent data sourcing (TASK-0372).** How does the coordinator
obtain the two agents being reduced? Three candidates:
(a) coordinator caches a copy of each worker's `Partition` (from the
`InitialPartition` payload shipped under 2.26-A);
(b) coordinator requests the two agents on demand via a new
`AgentRequest` message (adds a round trip per border redex);
(c) workers pre-include the two border-adjacent agents in their
`RoundResult` when `has_border_activity == true`.

Current bundle assumes option (a) — resolver takes `&[Partition]`. The
memory cost is one partition-copy per worker held on the coordinator
side, refreshed each round via `apply_deltas` (the coordinator
mirrors the worker's state). **Spec-critic please rule.**

**DC-B2 — resolver error semantics (TASK-0372).** If
`materialize_agent` returns `None` (agent vacated between detection
and resolution), should the resolver (i) panic (programmer error —
strict BSP ensures this cannot happen) or (ii) return a typed
`ResolverError::AgentMissing { bid, side }`? Current bundle defaults
to (i); spec-critic should confirm.

**DC-B3 — delta content for local reconnections (TASK-0373).** The
CON-CON / DUP-DUP cross-pattern reconnections re-target each consumed
agent's auxiliary ports to the OTHER agent's former auxiliary-port
targets. If those former targets are *local* to one partition (not
border wires), Worker A needs to perform a local `net.connect` between
two `AgentPort`s in its own partition — NOT a border delta. Options:
(a) extend `BorderDelta` with an optional `local_reconnections:
Vec<(PortRef, PortRef)>` field;
(b) ship a new `LocalReconnection` delta type alongside
`BorderDelta`;
(c) coordinator expresses everything via `BorderDelta` by converting
the local reconnection to a "virtual border" with both endpoints on
the same worker (awkward but type-preserving).

**Spec-critic please rule.** This cascades to TASK-0373 and 2.26-C.

**DC-B4 — BSP race tolerance (TASK-0373).** Strict BSP should prevent
a worker from consuming a border-adjacent agent locally before the
coordinator resolves the border redex, because the worker signalled
`has_border_activity = true` and the coordinator resolves before
dispatching the next `RoundStart`. But the current pipeline has a
subtle window: what if the worker's LOCAL reduction consumed the
agent in round N, the worker reported `has_border_activity = false`,
but a CON-DUP commutation in round N+1 re-populated that side with a
new principal-port agent before the coordinator dispatched round N+1?
The window depends on how 2.26-C orders operations. **Spec-critic
please rule** — may require a SPEC-19 §3.5 invariant amendment.

**DC-B5 — new agent ID allocation (TASK-0374).** Who picks the
`AgentId`s for the 4 new CON-DUP commutation agents? See TASK-0374
Notes for the three options. Current bundle leaves this unresolved —
TASK-0374 flags it as a precondition. **Spec-critic please rule.**
This may require a SPEC-19 design amendment or a new sub-protocol
under 2.26-C.

**DC-B6 — CON-ERA / DUP-ERA border preservation (TASK-0374).** When a
CON-ERA or DUP-ERA resolution happens and the consumed CON/DUP had an
auxiliary port connected to another FreePort (existing border), does
the resolver treat that as "existing border, new endpoint" (update via
`apply_deltas`) or "new border" (via `add_border_states`)? The
bundle's current plan treats this as an `apply_deltas` update — the
border still exists, just re-pointed to the new ERA's principal port.
**Spec-critic confirm.**

**DC-B7 — `BorderResolution.resolved_borders` worker attribution
(TASK-0375).** The `package_resolutions` function needs to fold
resolved-border IDs into the correct per-worker bucket for R23
semantics (worker must remove the `FreePort` entry). Options:
(a) embed `(bid, worker_a, worker_b)` triples in
`BorderResolution.resolved_borders`;
(b) pass the `BorderGraph` (post-mutation) to `package_resolutions`
so it can look up owners — but the border is already removed from
`graph.borders`, so a snapshot-before-removal is needed;
(c) embed the mapping inside `BorderResolution.worker_deltas` (both
workers appear; filter `resolved_borders` by their appearance). **Spec-
critic please rule.** Cascades to TASK-0373 struct shape.

**DC-B8 — pure-core guard scope (TASK-0377).** Should TASK-0377's
programmatic import-guard test apply only to `border_resolver.rs`
(current plan, minimal 2.26-B scope) or be extended to every file
under `merge/`? **Spec-critic please rule.**

**DC-B9 — pure-core guard granularity (TASK-0377).** Should the
forbidden-imports list include `use crate::coordinator` and
`use crate::worker` to cover transitive-leak scenarios? **Spec-critic
please rule.**

## Task graph (DAG)

```
                TASK-0372 (S, ~80)
                       │
           ┌───────────┴───────────┐
           ▼                       ▼
      TASK-0373 (M, ~130)     TASK-0377 (XS, ~30)
           │                       ▲
           ▼                       │
      TASK-0374 (M, ~150)          │
           │                       │
           ▼                       │
      TASK-0375 (S, ~80)           │
           │                       │
           ▼                       │
      TASK-0376 (M, ~130) ─────────┘
```

| ID   | Title | Spec Reqs | Size | LoC est. | Depends |
|------|-------|-----------|------|----------|---------|
| 0372 | `border_resolver.rs` skeleton + `materialize_agent` helper + module wiring | R13 scaffolding, R14 precondition, R19 | S | ~80 | none |
| 0373 | Annihilation + void dispatch — CON-CON, DUP-DUP, ERA-ERA | R13, R14 (Anni/Void), R15 part 2 | M | ~130 | 0372 |
| 0374 | Commutation + asymmetric erasure — CON-DUP, CON-ERA, DUP-ERA | R13, R14 (Comm/Eras), R15 parts 2-3 | M | ~150 | 0373 |
| 0375 | `RoundStartDispatch` + `package_resolutions` — group deltas by worker | R15 part 1 | S | ~80 | 0373, 0374 |
| 0376 | Integration test — end-to-end resolver on 2-partition fixture | R13, R14, R15 parts 1-2 | M | ~130 | 0372..0375 |
| 0377 | Programmatic pure-core import-guard test | R19 | XS | ~30 | 0372..0376 |

**Total:** ~600 LoC of production/test code. Implementable in
topological order: 0372 → 0373 → 0374 → 0375 → 0376 → 0377. TASK-0377
has a lightweight dependency on the others (they must exist first so
there's a file to test), but the task itself is trivial and could run
in parallel with TASK-0376 at Stage 3.

## Dependencies on sibling bundles

**Blocks on 2.26-A (none hard):** the pure-core `RoundStartDispatch`
mirror does NOT compile-time depend on `Message::RoundStart`. So this
bundle can land BEFORE 2.26-A. Logically, the field names should
match; if 2.26-A decides on different field names during Stage 1.5
spec-critic, TASK-0375 gets a task-updater amendment.

**Blocks on 2.26-C (none hard — 2.26-C depends on THIS bundle):**
2.26-C's coordinator BSP loop needs `BorderResolution` +
`RoundStartDispatch` to exist. This bundle unblocks that work.

**Blocks on 2.26-D (none):** no shared files or types.

## Per-task feature/build compatibility

The bundle is feature-flag-neutral. None of the six tasks add a cargo
feature or change feature-gated code. Both `cargo test --workspace`
and `cargo test --workspace --features zero-copy` MUST stay GREEN at
every task boundary. Bundle adds tests on top of both baselines.

- **TASK-0372..0377:** all production code is unconditional (no
  `#[cfg]` outside the standard `#[cfg(test)]` inline-test pattern).

## Acceptance gate for the whole bundle

- All 6 tasks shipped GREEN through Stages 3-6.
- `cargo test --workspace` test count ≥ 690 lib baseline (CLAUDE.md
  hard floor); bundle adds ~20-30 lib tests on top per Stage 2
  estimate.
- `cargo test --workspace --features zero-copy` test count equally
  at baseline + bundle additions.
- `cargo clippy --workspace --all-targets -- -D warnings` clean both
  with and without `--features zero-copy`.
- `cargo fmt --check` clean.
- Release smoke `compute add 3 5 → 8` works (default features).
- Pure-core invariant (R19) verified programmatically via TASK-0377's
  in-source test (no dependence on external `grep`).
- No changes to `worker.rs`, `coordinator.rs`, `protocol/*`,
  `config.rs`, or `merge/grid.rs` (scope guard — owned by sibling
  sub-bundles).
- Integration test (TASK-0376) demonstrates resolver correctness for
  all 6 IC rules on 2-partition fixtures.
- D-003 in `docs/DEFERRED-WORK.md` marked RESOLVED once 2.26-C
  composes this bundle's output into `run_grid_delta`. **This
  bundle alone does NOT close D-003** — D-003's acceptance signal
  requires an end-to-end test where the coordinator dispatches deltas
  through the wire and workers apply them. That's 2.26-C's closing
  move. 2.26-B ships the pure-core half of the resolution path.

## Stage advancement

- **Stage 1 SPLITTING:** complete (this file + 6 TASK-NNNN.md files).
- **Stage 1.5 SPEC-CRITIC:** **complete** (2026-04-17). Verdict in
  `docs/spec-reviews/SPEC-19-section-3.3-2.26B-design-choices-2026-04-17.md`.
  All 9 DCs ruled; 7 task files need amendments; 3 cross-bundle
  memos required (2.26-A, 2.26-C, 2.26-D). Next action: dispatch
  task-updater to apply in-bundle amendments and emit cross-bundle
  memos.
- **Stage 2 TESTS:** **unblocked in principle** once task-updater
  finishes. Will dispatch `test-generator` with bundle spec =
  SPEC-19 §3.2 (R13-R15 parts 1-2) + §3.3 (item 2.26-B), task list
  = TASK-0372..0377 (post-amendment), deliverable =
  `docs/tests/TEST-SPEC-0372.md` … `TEST-SPEC-0377.md`.
- Orchestrator pause requested by parent: STOP after Stage 1.5
  verdict (now reached) and confirm before Stage 2 dispatch.
