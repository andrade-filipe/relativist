# Bundle: SPEC-19 §3.2 — BorderGraph and Delta-Based Merge (item 2.35)

**Created:** 2026-04-17
**Owner:** task-splitter (orchestrated by sdd-pipeline)
**Stage:** 1 SPLITTING — complete (this file + 6 TASK-NNNN.md files).
  **Stage 1.5 SPEC-CRITIC pending** (optional, see "Spec ambiguities flagged"
  below). **Stage 2 TESTS blocked on critic verdict.**
**Test baseline before bundle:** 905 lib default + 945 lib `--features zero-copy`
  + 4 integration (post-SPEC-18 §3.5 ship per `docs/pipeline-state.md`).
**Hard floor (CLAUDE.md):** 905 lib default tests post-bundle, 945 lib
  `--features zero-copy` tests post-bundle. Both must hold; bundle adds tests
  on top, never below.
**Estimated total LoC:** ~400 across 6 atomic tasks (each ≤120 LoC of
  production code; matches the V2-FEATURE-MATRIX estimate for R8-R19 only).
**Tier 1 break-even path (V2-FEATURE-MATRIX):** confirmed next after item 2.24
  (`2.22 → 2.23 → 2.34 → 2.24 → 2.25 → 2.35 → 2.26`). This bundle covers item
  **2.35 data-structure half**; the loop-and-protocol half (R13-R15 callers,
  R20-R30) ships under item 2.26 in a later bundle.
**Closes (partial):** the coordinator-side data-structure precondition for
  ROADMAP item 2.35. Full close of item 2.35 is gated on item 2.26's
  delta-BSP loop, which calls into the primitives shipped here.

## Scope (in vs out)

**In scope (SPEC-19 §3.2, R8-R19 — BorderGraph data structure only):**
- R8 — `BorderGraph` struct lives in `relativist-core/src/merge/border_graph.rs`.
- R9 — `BorderState { border_id, side_a, side_b, worker_a, worker_b, is_redex }`.
- R10 — `BorderGraph::from_partition_plan(plan: &PartitionPlan) -> Self`.
- R11 — `BorderGraph::apply_deltas(worker_id, deltas)` updates side endpoints,
  recomputes `is_redex`, maintains incremental `active_redexes` set.
- R12 — `BorderGraph::detect_border_redexes() -> Vec<(u32, &BorderState)>` from
  the incremental set.
- R16 — `BorderGraph::remove_border(border_id)` for annihilation cleanup
  (CON-CON, DUP-DUP, ERA-ERA consume both sides; the wire ceases to exist).
- R17 — `apply_deltas` recognises the `DISCONNECTED` (`PortRef::FreePort(u32::MAX)`)
  sentinel from a worker erasure delta and removes the border when both sides
  disconnect (per spec §4.2 pseudocode).
- R18 — incremental `active_redexes: HashSet<u32>` updated inside
  `apply_deltas` so `detect_border_redexes` is O(|active_redexes|), not O(B).
- R19 — pure-core layer: no `tokio`, no `async`, no I/O, no `protocol::*`
  imports. `border_graph.rs` only depends on `crate::net::*`,
  `crate::partition::*`, and `std`.
- **R15 part 3 (primitive only):** `BorderGraph::add_border_states(states)` to
  let a future coordinator add new border entries when CON-DUP commutation
  produces 4 new agents whose auxiliary ports inherit FreePort connections.
  The actual call from the coordinator is **out of scope** (item 2.26).

**Hard scope boundaries (out of scope — task-splitter MUST NOT generate tasks
for these):**
- R13 — coordinator request/response loop that asks workers for the two
  agents involved in a border redex and runs `interact_*` locally. Ships
  under item 2.26.
- R14 — coordinator-side `interact_*` invocation wiring (the import path
  exists; the call site is item 2.26's territory).
- R15 parts 1 and 2 — coordinator-side delta dispatch back to workers, and
  the call into `BorderGraph::remove_border` / `add_border_states` from the
  resolution path. Ships under item 2.26.
- R20-R36 — delta BSP loop (`run_grid_delta`), wire protocol extensions
  (`InitialPartition`, `RoundStart`, `RoundResult`, `FinalStateRequest`,
  `FinalStateResult`), `delta_mode` config flag. Ships under item 2.26.
- Any change to `protocol/`, `coordinator.rs`, `worker.rs`, or `merge/grid.rs`.
  This bundle adds a single new file and a single line to `merge/mod.rs`.

## Pure-core layer compliance (R19 — load-bearing)

Per CLAUDE.md (Relativist) module-structure rules, the dependency direction is
`net` <- `reduction` <- `partition` <- `merge` <- `protocol`, and the core
layer (`net/`, `reduction/`, `partition/`, `merge/`) has zero `async`, zero
`tokio`, zero I/O. `border_graph.rs` lives in `merge/` and inherits this
contract. The grep guard for the bundle:

```
grep -E '^use\s+(tokio|crate::protocol|async_trait)' \
  relativist-core/src/merge/border_graph.rs
# expected: zero matches
```

TASK-0360..0365 each individually preserve this property; TASK-0365's module
doc explicitly records the contract so that any future coordinator-side
patch (item 2.26) does NOT regress it by tempting an `async fn` into this
file.

## Wire compatibility

This bundle adds **no** wire-format changes. The `BorderDelta` and
`BorderState` types referenced from SPEC-19 §4 already live in the spec's
type sketch and will be lifted into source code by TASK-0360 (struct only;
no serde derives needed yet — wire encoding is item 2.26's job). v1 protocol
path unchanged; v2 hot path unchanged.

## Design choices flagged for spec-critic — RULED 2026-04-17

**Spec-critic verdict:** `docs/spec-reviews/SPEC-19-section-3.2-design-choices-2026-04-17.md`

Four implementation choices were flagged at SPLITTING. Spec-critic ruled
on all four; verdicts below. Three are amendments to task files (no spec
edits). DC-1 confirmed the task-splitter recommendation; DC-2 confirmed
with a doc-comment mandate; DC-3 and DC-4 OVERRODE the task-splitter
recommendations.

**DC-1 — `DISCONNECTED` sentinel value.** Spec R17 reads "the worker reports
a delta `(border_id, DISCONNECTED)` or `(border_id, None)`". The codebase
already exposes `PortRef::FreePort(u32::MAX)` as the canonical `DISCONNECTED`
const (`crate::net::types::DISCONNECTED`, used widely in `reduction/rules.rs`).
TASK-0362 assumes this sentinel rather than introducing an `Option<PortRef>`.
**Spec-critic DC-1 ruled:** APPROVED. Use `PortRef` with the existing
`DISCONNECTED` const; spec §4.1 line 346-350 already picks this and
TASK-0344's compact wire encoding gives DISCONNECTED a 1-byte form.
No task amendment required.

**DC-2 — `worker_borders` field (spec §4.1).** The spec sketch declares
`worker_borders: Vec<Vec<u32>>` (per-worker border-id list) but only uses it
implicitly during construction. R11 (`apply_deltas`) is keyed on
`(border_id, worker_id)` ownership lookup which the `BorderState.worker_a /
worker_b` fields already cover. **Spec-critic DC-2 ruled:** APPROVED with
doc-comment mandate. Ship the field now (spec mandates the struct shape;
co-seeding in `from_partition_plan` is O(B) free); mark it `pub(crate)`
and document it as "consumed by item 2.26 coordinator dispatch (R23)" so
no future reviewer prunes it as dead code. TASK-0360 amendment: replace
the `worker_borders` field doc with the explicit consumer pointer.

**DC-3 — `detect_border_redexes` return shape.** Spec §3.2 R12 prescribes
`Vec<(u32, BorderState)>` (owned `BorderState`); spec §4.2 pseudocode
prescribes `Vec<(u32, &BorderState)>` (borrowed). **Spec-critic DC-3
ruled:** OVERRIDE the task-splitter recommendation. Ship owned
`Vec<(u32, BorderState)>` per §3.2 R12 normative text. The §4.2
pseudocode is non-normative and contradicts the §4.3 coordinator loop's
mutable-borrow pattern (`detect → for state in vec { resolve(&mut graph, state) }`
is impossible if `state` borrows from `graph`). `BorderState` is ~32 bytes;
per-round clone cost is negligible (~32 KB at 1k active redexes/round).
TASK-0363 amendment: signature change + one `.clone()` in the body +
note rewrite.

**DC-4 — `add_border_states` signature.** Spec R15 part 3 prescribes the
*existence* of the primitive but not the exact signature. **Spec-critic
DC-4 ruled:** OVERRIDE the task-splitter recommendation (Option A —
caller pre-builds `BorderState`). Ship Option B with a new public
`AddBorderEntry` input struct (5 connectivity fields, no `is_redex`);
the graph recomputes `is_redex` via `is_principal_pair` to enforce R9's
invariant `state.is_redex == is_principal_pair(side_a, side_b)` at the
primitive boundary. Caller cannot break the invariant under Option B
because the caller does not write the bit. `from_partition_plan` (TASK-0361)
already follows this pattern (computes `is_redex` itself, does not trust
caller input). TASK-0364 amendments: new `AddBorderEntry` struct, signature
change, body change, one new test (`add_border_states_enforces_is_redex_invariant`)
bringing the task's test count from 10 to 11. TASK-0365 amendment: one
doc-bullet wording update.

All four amendments are documentation/test-shape changes inside `docs/backlog/`;
no spec edits required, no source-code changes outside what each task
already produces. Stage 2 TESTS is unblocked once task-updater applies the
amendments listed in the spec-review file.

## Task graph (DAG)

```
                       TASK-0360 (S, ~40)
                              │
                ┌─────────────┼─────────────┐
                ▼             ▼             ▼
           TASK-0361      TASK-0364      TASK-0365
           (M, ~120)      (S, ~50)       (S, ~50)
                │             ▲
                ▼             │
           TASK-0362  ────────┘
           (M, ~100)
                │
                ▼
           TASK-0363 (S, ~40)
```

| ID   | Title | Spec Reqs | Size | LoC est. | Depends |
|------|-------|-----------|------|----------|---------|
| 0360 | `border_graph.rs` skeleton: `BorderState` struct + `is_principal_pair` helper + module wiring | R8, R9, R19 | S | ~40  | none           |
| 0361 | `BorderGraph::from_partition_plan` + `find_border_owner` helper                                | R10        | M | ~120 | 0360           |
| 0362 | `BorderGraph::apply_deltas` (incremental redex set + DISCONNECTED handling)                    | R11, R17, R18 | M | ~100 | 0361           |
| 0363 | `BorderGraph::detect_border_redexes` + read-only accessors (`len`, `has_no_redexes`, `iter`)   | R12, R18   | S | ~40  | 0362           |
| 0364 | `BorderGraph::remove_border` (annihilation) + `add_border_states` (CON-DUP expansion primitive)| R15(p3), R16 | S | ~50  | 0361           |
| 0365 | Module-level `//!` doc + R13/R14/R15-callsite scaffolding notes (no code) + R19 invariant note | R13, R14, R15(p1,p2), R19 | S | ~50  | 0360           |

**Total:** ~400 LoC of production code, all ≤120 LoC per task. No cycles.
Implementable in topological order: 0360 → {0361, 0364, 0365} → 0362 → 0363,
with 0364 also depending on 0361 (it borrows the `is_principal_pair` helper
+ the `worker_borders` field set up by `from_partition_plan`).

## Per-task feature/build compatibility

The bundle is feature-flag-neutral. None of the six tasks add a cargo feature
or change feature-gated code. Both `cargo test --workspace` (905 lib + 4
integration baseline) and `cargo test --workspace --features zero-copy`
(945 lib + 4 integration baseline) MUST stay GREEN at every task boundary.
Bundle adds tests on top of both baselines (test counts target pinned by
TEST-SPEC-0360..0365 in Stage 2).

- **TASK-0360..0365:** all production code is unconditional (no `#[cfg]`),
  all tests use plain `#[test]` (no `#[tokio::test]` — pure-core, no async).

## Acceptance gate for the whole bundle

- All 6 tasks shipped GREEN through Stages 3-6.
- `cargo test --workspace` test count ≥ 905 lib + 4 integration (CLAUDE.md
  hard floor); bundle adds ~25 lib tests on top per Stage 2 estimate.
- `cargo test --workspace --features zero-copy` test count ≥ 945 lib + 4
  integration (zero-copy hard floor); same +~25 from this bundle.
- `cargo clippy --workspace --all-targets -- -D warnings` clean both with
  and without `--features zero-copy`.
- `cargo fmt --check` clean.
- Release smoke `compute add 3 5 → 8` works (default features).
- `grep -E '^use\s+(tokio|crate::protocol|async_trait)'
  relativist-core/src/merge/border_graph.rs` returns zero matches (R19).
- No changes to `merge/grid.rs`, `coordinator.rs`, `worker.rs`, or any file
  under `protocol/` (scope guard — coordinator integration is item 2.26).

## Stage advancement

- **Stage 1 SPLITTING:** complete (this file + 6 TASK-NNNN.md files).
- **Stage 1.5 SPEC-CRITIC:** **complete (2026-04-17)** —
  `docs/spec-reviews/SPEC-19-section-3.2-design-choices-2026-04-17.md`.
  Four DCs ruled: DC-1 confirmed (no amendment), DC-2 confirmed with doc
  mandate (TASK-0360 amendment), DC-3 OVERRIDDEN to owned return (TASK-0363
  amendment), DC-4 OVERRIDDEN to graph-recomputes-is_redex (TASK-0364 +
  TASK-0365 amendments). Two non-DC observations also flagged: (1) TASK-0360
  uses `AgentId::new(id)` but `AgentId` is a type alias — fix at the same
  time as the DC-2 doc-comment edit; (2) `is_principal_pair` already exists
  at `merge/helpers.rs` line 18 — TASK-0360 should re-export rather than
  redefine.
- **Stage 2 TESTS:** **unblocked once task-updater applies the amendments
  listed in the spec-review.** Will dispatch `test-generator` with bundle
  spec = SPEC-19 §3.2 (R8-R19), task list = TASK-0360..0365, deliverable =
  `docs/tests/TEST-SPEC-0360.md` … `TEST-SPEC-0365.md`. Test contracts
  MUST encode the four DC verdicts (DC-1 sentinel = `crate::net::DISCONNECTED`,
  DC-2 `worker_borders: Vec<Vec<u32>>` shipped, DC-3 owned
  `Vec<(u32, BorderState)>` return, DC-4 `Vec<AddBorderEntry>` input with
  graph-side `is_redex` recomputation).
- Orchestrator pause requested by parent: STOP after Stage 1 and confirm
  before Stage 1.5 (or skip-to-Stage-2) dispatch.
