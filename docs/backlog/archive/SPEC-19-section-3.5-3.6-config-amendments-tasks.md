# Bundle: SPEC-19 §3.5 + §3.6 — Invariant Amendments + Config Flag (item 2.26-D)

**Created:** 2026-04-17
**Owner:** task-splitter (orchestrated by sdd-pipeline, parent item 2.26)
**Parent bundle:** 2.26 (Delta-Only Protocol) — sub-bundle D of four
  (A = wire variants, B = coordinator dispatch, C = BSP loop, **D = config + amendments**).
**Stage:** 1 SPLITTING — complete; awaiting Stage 1.5 spec-critic review.
**Test baseline before bundle:** 968 default lib tests (the current v2-development
  baseline the orchestrator brief refers to; matches the 850 post-SPEC-18 baseline
  plus the 2.26-A/B/C tasks that will ship ahead of this sub-bundle — however 2.26-D
  is the SMALLEST bundle and is largely independent, so it may be shipped in parallel
  provided the convergence test in 2.26-C is landed separately).
**Hard floor (CLAUDE.md):** test count MUST NOT decrease.
**Estimated total LoC:** ~220 across 5 atomic tasks (each < 100 LoC). Sub-bundle D is
  the smallest of the four 2.26 sub-bundles because §3.5 is largely documentation and
  §3.6 adds a single new config field.

## Scope (in vs out)

**In scope (SPEC-19 §3.5 + §3.6, R38-R42):**

- **R38** — Amendment to G1 (Fundamental Property): the v1 formulation
  `reduce_all(net) ~ extract_result(run_grid(net, n))` is reformulated as
  `reduce_all(net) ~ extract_result(run_grid_delta(net, n))`. The full formal
  proof is explicitly deferred (Section 8, OQ-1 + DISC-011 + ARG-005). This
  sub-bundle captures the reformulation narratively in `docs/ROADMAP.md`
  (narrative amendment note linking to the spec). **No spec edit** — specs are
  read-only for task-splitter.
- **R39** — Amendment to D3 (Border Completeness): D3a/b/c/d reformulation.
  Operational behavior ships in 2.35 (BorderGraph), 2.26-B (BorderResolver),
  and 2.26-C (delta loop). This sub-bundle captures the narrative amendment
  note only.
- **R40** — Amendment to D6 (Protocol Termination): lenient vs strict delta
  mode, Global Normal Form, progress guarantee. Operational behavior ships
  in 2.26-C. This sub-bundle captures the narrative amendment note only.
- **R41** — `GridConfig.delta_mode: bool` field (default `false`). The sibling
  field `coordinator_free_rounds: bool` is **already shipped** in TASK-0350
  (bundle 2.34, SPEC-19 §3.1) — verified in `relativist-core/src/merge/types.rs:200`.
  Sub-bundle D adds **only** `delta_mode`.
- **R42** — `delta_mode = false` MUST preserve v1 backwards compatibility
  exactly. Zero behavioral regression against the 968 default baseline.

**Out of scope (separate sub-bundles or already shipped):**

- §3.2 `BorderGraph` — shipped in 2.35 (TASK-0374..0388).
- §3.3 coordinator dispatch (R13-R15) — sub-bundle 2.26-B.
- §3.3 stateful worker lifecycle (R20-R30) — sub-bundle 2.26-C (NOT D).
- §3.4 new wire variants (R31-R37) — sub-bundle 2.26-A.
- `GridConfig.coordinator_free_rounds` — already shipped (TASK-0350).
- `GridConfig.strict_bsp` — already shipped (pre-SPEC-19).
- Formal proofs of R38/R39/R40 — OQ-1, ARG-005 (TCC work, not Relativist
  implementation).
- Spec edits to `SPEC-01-invariantes.md` or `SPEC-19-delta-protocol.md` — specs
  are read-only to task-splitter. The spec R38-R40 text already exists in
  SPEC-19; this sub-bundle does not re-encode it.

## Reserved TASK-ID range

**0389-0393** (5 IDs, exclusive to this sub-bundle per the orchestrator brief).
Do NOT allocate outside this range.

## File territory for this sub-bundle

**Files this sub-bundle may touch:**
- `relativist-core/src/merge/types.rs` — add `GridConfig.delta_mode: bool` +
  `Default` impl update + unit tests.
- `relativist-core/src/config.rs` — add `--delta-mode` flag on `CoordinatorArgs`
  + `LocalArgs`; thread through `build_grid_config` / `build_grid_config_from_local`.
- `docs/ROADMAP.md` — append narrative amendment notes for G1/D3/D6 linking to
  SPEC-19 §3.5.

**Files FORBIDDEN for sub-bundle 2.26-D** (coordinate with parallel splitters):
- `relativist-core/src/worker.rs` — sub-bundle 2.26-C.
- `relativist-core/src/merge/grid.rs` — sub-bundle 2.26-C.
- `relativist-core/src/merge/border_graph.rs` — shipped in 2.35.
- `relativist-core/src/merge/border_resolver.rs` — sub-bundle 2.26-B.
- `relativist-core/src/protocol/*` — sub-bundle 2.26-A.
- `specs/*` — read-only for task-splitter (the R38-R40 spec text already exists;
  this sub-bundle only adds narrative ROADMAP notes linking back to the spec).

## Pre-split findings (verified against the working tree)

1. **`coordinator_free_rounds` is already in `GridConfig`.** Confirmed in
   `relativist-core/src/merge/types.rs:200` (field) and `:209-210` (Default
   impl). Shipped by TASK-0350 (bundle 2.34). Sub-bundle D does NOT re-add it;
   R41 is satisfied for that half of the struct already. Only `delta_mode`
   remains.
2. **`delta_mode` is NOT yet in `GridConfig`.** The only reference to
   `delta_mode` in `src/` is an aspirational docstring comment in
   `relativist-core/src/merge/border_graph.rs:55` (`//! - GridConfig.delta_mode
   flag and the run_grid_delta BSP loop`). No field, no plumbing.
3. **R42 interpretation.** "No existing caller, test, or benchmark MUST change
   behavior" — reading the spec text literally: behavior MUST NOT change; the
   addition of a new field to `GridConfig` with `Default` = `false` is a source
   change but not a behavior change. Adding a `--delta-mode` CLI flag that
   defaults to `false` preserves every existing `clap` parse; the flag absence
   on a command line gives the same `GridConfig` value as pre-bundle. This
   interpretation is flagged for spec-critic (see below) but drives the split.

## Task graph (DAG)

```
TASK-0389 (S, ~40)  ─┬─→ TASK-0390 (S, ~60)  ─┐
                     │                          │
                     └─→ TASK-0391 (S, ~40) ────┼─→ (ready)
                                                 │
TASK-0392 (S, ~50)  ───────────────────────────┘  (ROADMAP narrative; independent)
TASK-0393 (S, ~30)  ← depends on 0389              (doctest / usage example)
```

| ID | Title | Spec Reqs | Size | LoC est. | Depends |
|------|---------------------------------------------------------|-----------|------|----------|---------|
| 0389 | Add `GridConfig.delta_mode: bool` field + Default impl | R41p       | S    | ~40      | none    |
| 0390 | Thread `--delta-mode` CLI flag (coordinator + local)   | R41p, R42  | S    | ~60      | 0389    |
| 0391 | R42 regression: default run preserves 968-test baseline | R42       | S    | ~40      | 0389    |
| 0392 | ROADMAP amendment notes for G1 / D3 / D6 (narrative)   | R38-R40    | S    | ~50      | none    |
| 0393 | `delta_mode` docstring + doctest usage example         | R41p       | S    | ~30      | 0389    |

**Total:** ~220 LoC across 5 atomic tasks, each well under the <200 LoC ceiling.
No cycles. Implementable in topological order: 0389 → (0390 ‖ 0391 ‖ 0393) ‖ 0392.

## Per-task R42 compatibility argument

Each task individually preserves R42 (zero behavioral regression):

- **TASK-0389:** adds a `bool` field with `Default = false`. Every existing
  `GridConfig { ..GridConfig::default() }` literal gets `delta_mode: false`
  automatically. No call site reads the field (yet), so no behavioral change.
- **TASK-0390:** `#[arg(long, default_value_t = false)]` on `--delta-mode`.
  Absent flag → `false` → matches pre-bundle behavior. Every existing CLI test
  continues to pass.
- **TASK-0391:** parametric regression run asserting that `cargo test` counts
  are unchanged under default `delta_mode = false`; pure meta-test, no code
  changes in core.
- **TASK-0392:** docs-only, no code path change.
- **TASK-0393:** doctest is compile-time only for this spec (no runtime path
  yet — `delta_mode = true` has no observable behavior until 2.26-C lands the
  `run_grid_delta` loop); the doctest asserts the field is settable and
  documented.

## Spec ambiguities flagged for spec-critic (Stage 1.5)

Three items flagged; recommend spec-critic review before Stage 2 (TESTS) dispatch
so the answers feed directly into the test generator.

1. **AMB-D-1: `coordinator_free_rounds` dual-shipment.** R41 lists both
   `delta_mode` AND `coordinator_free_rounds` in the post-amendment struct.
   `coordinator_free_rounds` was shipped early by TASK-0350 (bundle 2.34)
   because §3.1 (coordinator-free round) is a mergeable subset of §3.6.
   Sub-bundle D does NOT re-add it. **Question for spec-critic:** is there a
   notional update needed to flag TASK-0350 as having satisfied half of R41
   early (e.g., a "partial satisfaction" note in the bundle index), or is the
   current state — field present, narrative note not yet written — already
   consistent? **Bundle D's current answer:** treat R41 as "half already
   shipped; sub-bundle D adds only the missing half (`delta_mode`)". If
   spec-critic disagrees, TASK-0389 is the place to add a defensive assertion
   or comment cross-referencing TASK-0350.

2. **AMB-D-2: R42 literal reading — "zero behavior changes" vs "zero source
   changes".** The spec says "No existing caller, test, or benchmark MUST
   change behavior when `delta_mode` is not explicitly set." The sub-bundle
   interprets this as *zero behavioral* regression while allowing source
   changes (new field, new CLI flag, new default value assignments at
   struct-literal sites). TASK-0391 operationalises this as: (a) no existing
   test needs modification; (b) `cargo test` count does not decrease; (c)
   default `compute add 3 5 → 8` smoke still works. **Question for spec-critic:**
   is this acceptable, or does R42 also forbid touching struct-literal sites
   (which would require `..GridConfig::default()` to be the only sanctioned
   update idiom)? **Bundle D's current answer:** "behavioral, not textual"
   interpretation, which is standard Rust practice for additive struct changes.

3. **AMB-D-3: R38/R39/R40 use "MUST" for reformulations with proofs deferred.**
   The spec writes "G1 MUST be reformulated as [new statement]" but also states
   "The full formal proof of the recoverability property is pending (see
   Section 8, OQ-1 + DISC-011 + ARG-005)." **Question for spec-critic:** does
   the MUST apply only to the *design* reformulation (no proof required here,
   satisfied by narrative documentation of the new formula — which sub-bundle
   D delivers via TASK-0392), or does it also require the proof to ship in this
   sub-bundle (which would be out of scope and misaligned with the Section 8
   deferral)? **Bundle D's current answer:** MUST applies only to the design
   reformulation — the explicit "proof pending" clause at R38 is read as a
   formal deferral, consistent with Section 8 OQ-1. TASK-0392 ships the
   narrative acknowledgement; the proof is a separate TCC deliverable
   (ARG-005).

None of the three blocks Stage 2 dispatch; recommend spec-critic ratify the
interpretations above (or amend) so TASK-0391's regression assertions and
TASK-0392's narrative wording match the ratified reading.

## Acceptance gate for the whole sub-bundle

- All 5 tasks shipped GREEN through Stages 3-6.
- Test count MUST NOT decrease from the pre-bundle baseline (CLAUDE.md hard
  floor). Expected post-bundle: pre-bundle + ~8 new unit tests (one Default
  assertion in TASK-0389, two CLI parse tests in TASK-0390, one regression
  meta-test in TASK-0391, one doctest in TASK-0393, ~3 for R42 defensive).
- Clippy clean (`cargo clippy --workspace --all-targets -- -D warnings`).
- fmt clean (`cargo fmt --check`).
- Release smoke `compute add 3 5 → 8` works with and without `--delta-mode`
  (both MUST produce identical output — R42, via the trivial fact that
  `delta_mode = true` is a no-op until 2.26-C lands the delta BSP loop).
- R42 regression test (TASK-0391) passes.

## Stage advancement

- **Stage 1 SPLITTING:** complete (this file + 5 TASK-NNNN.md files, IDs
  0389-0393).
- **Stage 1.5 SPEC-CRITIC:** **COMPLETE** (2026-04-17). Verdict recorded in
  `docs/spec-reviews/SPEC-19-section-3.5-3.6-2.26D-design-choices-2026-04-17.md`.
  Summary:
  - **AMB-D-1 → Option A:** `coordinator_free_rounds` early-ship stands
    (TASK-0350, bundle 2.34); R41 is compound and completes on 2.26-D
    landing `delta_mode`. No re-ship. No task-file amendment.
  - **AMB-D-2 → Option A:** R42 is a **behavioral** invariance requirement
    (not source-diff zero). Additive struct + CLI plumbing is R42-compliant
    provided no observable behaviour changes under default
    `delta_mode = false`. No task-file amendment; TASK-0391 Notes §3
    conditional on strict textual reading may be disregarded.
  - **AMB-D-3 → Option C (surgical):** MUST applies to design
    reformulation for all three; proof-deferral is **NOT uniform**.
    R38 and R39 carry explicit "pending formal proof" clauses (Section 8
    / ARG-005). R40 does NOT — its Progress guarantee bullet is a
    self-contained operational argument, so R40's MUST is fully
    discharged only when sub-bundle 2.26-C lands the delta BSP loop
    with Global Normal Form termination. **Task-file amendment
    required:** task-updater splits TASK-0392's ROADMAP narrative so
    the D6 paragraph marks R40 "operational, no §8 deferral;
    operationally discharged by 2.26-C" while G1/D3 paragraphs
    retain the "formal proof deferred to Section 8" language.
  - **Cross-bundle coupling:** R40 is **PARTIAL after 2.26-D, COMPLETE
    after 2.26-C** — same pattern as R41 (partial after 2.34,
    complete after 2.26-D).
- **Stage 2 TESTS:** ready to dispatch once task-updater applies the
  AMB-D-3 narrative split to TASK-0392. The type-change tasks
  (TASK-0389/0390/0391/0393) are unaffected by the spec-review and may
  proceed in parallel. Invoke `test-generator` with bundle spec =
  SPEC-19 §3.5 + §3.6 (R38-R42), task list = TASK-0389..0393,
  deliverable = `docs/tests/TEST-SPEC-0389.md` … `TEST-SPEC-0393.md`.
- Orchestrator pause requested by parent: STOP after Stage 1 and confirm with
  spec-critic before Stage 2 dispatch. **Confirmation delivered in the
  spec-review file above; task-updater next.**

## Spec-critic cross-reference

- Verdict file: `docs/spec-reviews/SPEC-19-section-3.5-3.6-2.26D-design-choices-2026-04-17.md`
- Date: 2026-04-17
- Net task-file amendments required: **1** (TASK-0392 narrative split — task-updater runs next)
