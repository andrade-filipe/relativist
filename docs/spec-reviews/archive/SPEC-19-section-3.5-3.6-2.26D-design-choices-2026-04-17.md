# SPEC-19 §3.5 + §3.6 — Design Choices Verdict (Stage 1.5 spec-critic)

**Date:** 2026-04-17
**Reviewer:** spec-critic (adversarial)
**Bundle:** SPEC-19 §3.5 + §3.6 (item 2.26-D) — Invariant Amendments (R38-R40) + Config (R41-R42)
**Predecessors consulted:**
  SPEC-19 §3.1 R3-R4 (coordinator-free rounds — the field that already
  ships half of R41),
  SPEC-19 §3.5 R38-R40 (the three MUST-reformulations under review),
  SPEC-19 §3.6 R41-R44 (the config block — R41/R42 in bundle, R43/R44
  out of bundle but referenced),
  SPEC-19 §8 Open Questions (OQ-1 → DISC-011 → ARG-005 deferral chain),
  SPEC-01 invariants G1, D3, D6 (the v1 formulations being amended),
  SPEC-05 §4.1 (the `GridConfig` struct R41 extends),
  SPEC-18 §3.5 R36/R37 (the `use_zero_copy` flag — CLI precedent for
  R41/R42 plumbing).
**Source consulted:**
  `relativist-core/src/merge/types.rs` L180-213 (`coordinator_free_rounds`
  field + Default impl — already shipped by TASK-0350 in bundle 2.34),
  `relativist-core/src/merge/types.rs` L248-259 (the existing
  `grid_config_default_disables_coordinator_free_rounds` guard test — the
  template the bundle mirrors for `delta_mode`),
  `relativist-core/src/merge/border_graph.rs` L55 (the aspirational
  docstring mention of `delta_mode` — confirms no runtime consumer yet),
  `relativist-core/src/config.rs` L1-50 (CLI layout, `CoordinatorArgs` /
  `LocalArgs` location confirmed).
**Precedent consulted:**
  `docs/spec-reviews/SPEC-19-section-3.2-design-choices-2026-04-17.md`
  (format + verdict style; the "Option B — invariant at the boundary"
  pattern applies to AMB-D-3's narrative deferral),
  `docs/spec-reviews/SPEC-19-section-3.4-design-choices-2026-04-17.md`
  (feature-gate-as-opt-in ruling — analogous default-preserves-v1
  reasoning for R42),
  `docs/spec-reviews/SPEC-18-section-3.5-design-choices-2026-04-16.md`
  (the `use_zero_copy` plumbing — the near-identical precedent for R42).

---

## Overall Assessment

The SPLITTING bundle is sound. All three ambiguities resolve cleanly
against the existing spec text and the existing codebase **with zero
blocking spec amendments required**. AMB-D-1 is already de-facto
resolved by the working tree (`coordinator_free_rounds` shipped in
2.34; the bundle correctly adds only `delta_mode`). AMB-D-2 is the
obvious behavioral reading consistent with Rust idiom and the
`use_zero_copy` precedent. AMB-D-3 is the trickiest — R38 and R39
both carry explicit "pending formal proof" language (so narrative
documentation satisfies the MUST for those two), but **R40 does NOT**
contain any "pending proof" clause — its "Progress guarantee" bullet
is a self-contained operational argument (finite interaction budget
T7 ⇒ termination). R40's MUST is therefore satisfied by the
*operational* behaviour that ships in sub-bundle 2.26-C (BSP loop),
not merely by narrative documentation. This is a meaningful
distinction the task-splitter blurred; I flag it below and require
TASK-0392 to mark R40 differently.

**Verdict:** APPROVED WITH AMENDMENTS to **task files only** (no spec
edits). Stage 2 TESTS unblocked once TASK-0392's narrative
separates R38/R39 (proof-deferred, narrative suffices) from R40
(operational guarantee satisfied by 2.26-C, narrative documents the
design), and the bundle index records this distinction. AMB-D-1 and
AMB-D-2 require no task-file amendment — the tasks already implement
the ruled interpretation.

---

## Verdicts (3 ambiguities)

### AMB-D-1: `coordinator_free_rounds` partial-early-ship

**PICK:** **Option A** — treat R41 as "half already shipped early
(TASK-0350, bundle 2.34, 2026-04-16); sub-bundle D adds only the
missing `delta_mode` half; R41 is complete after 2.26-D ships with
the field set already in place". **Do NOT re-ship
`coordinator_free_rounds`** as a no-op refactor.

**WHY:**

1. **"Don't break what ships"** is the correct default. TASK-0350
   landed `coordinator_free_rounds` on 2026-04-16 as a mergeable
   subset of SPEC-19 §3.1 R4. The field is live in
   `relativist-core/src/merge/types.rs:200` with a working `Default`
   impl (L210) and a guard test
   (`grid_config_default_disables_coordinator_free_rounds`, L253)
   already defending R42 for that half. Re-touching the field would
   be a pure churn edit — zero behavioral change, risk of a merge
   conflict with whatever the developer staged in 2.34, and a
   gratuitous diff for reviewers.

2. **R41 is a compound requirement, not an atomic one.** The prose
   lists two fields (`delta_mode`, `coordinator_free_rounds`) with
   independent semantics (§3.6 R44 makes this explicit: they are
   MAY-independent opt-ins). A compound requirement is satisfied
   iff each component is satisfied. TASK-0350 satisfied half.
   2.26-D satisfies the other half. The whole is satisfied on
   2.26-D landing — no "atomicity" requirement in the spec forces
   a single-bundle ship.

3. **Spec §3.1 R4 explicitly pre-authorises early shipment.**
   §3.1 lists the coordinator-free round as a standalone
   optimisation deliverable — the spec itself treats it as
   decoupleable from the full delta protocol. The early-ship is
   *structurally* endorsed, not a convenience hack.

4. **Precedent.** SPEC-18 §3.5 R36 (`use_zero_copy`) was
   likewise a single-field addition added via a single task — the
   bundle did not reconstruct the entire `GridConfig` struct. The
   pattern for opt-in flag extensions is additive, per-field.

**Counter-option (B — re-ship as no-op refactor) rejected:**
(a) violates "no gratuitous edits"; (b) introduces a merge hazard
on the v2-development branch if any unshipped 2.26-A/B/C patch
touches the struct; (c) no reviewer value added because the field
state is trivially inspectable. The task-splitter's instinct was
correct.

**Task-file amendments required:** NONE. TASK-0389 already
appends `delta_mode` after `strict_bsp` and before
`coordinator_free_rounds` (its Acceptance Criteria line 1), and
its Default-impl update preserves the existing
`coordinator_free_rounds: false` entry. The bundle index (line
83-87) already records the partial-early-ship fact. No change
needed.

---

### AMB-D-2: R42 literal reading — behavioral vs source

**PICK:** **Option A** — R42 is a **behavioral** invariance
requirement, not a source-diff zero requirement. Additive source
changes (new struct field with `Default = false`, new
`#[arg(long, default_value_t = false)]` CLI flag, new
`..GridConfig::default()` spreads at existing construction sites)
are permitted because they do not alter the observable behaviour
of any existing caller, test, or benchmark when `delta_mode` is not
explicitly set.

**WHY:**

1. **Spec verb choice.** R42 reads "No existing caller, test, or
   benchmark MUST change *behavior*" (emphasis added). The spec
   does not say "No existing file MUST change" or "No existing
   test source MUST be modified". "Behavior" is the observable
   effect — output, exit code, metrics, side effects — not the
   textual form of the source that produces it.

2. **Option B makes R42 unsatisfiable.** Adding a new field to a
   Rust struct with any construction-site literal elsewhere in the
   codebase is a source change *by definition* (the literal must
   either name the new field or use `..Default::default()`
   spread). Interpreting R42 as source-diff-zero would forbid
   shipping the feature at all — a reading that contradicts R41's
   MUST to add the field. Specs do not contradict themselves;
   therefore R42 is not that reading.

3. **Rust idiom and existing precedent.** The
   `coordinator_free_rounds` ship (TASK-0350) added its field to
   the struct, the `Default` impl, and every relevant
   construction site — and the v1 test count did not decrease.
   That ship satisfied R42 for its half. The
   `use_zero_copy` plumbing (SPEC-18 §3.5, TASK-0358) follows
   the same pattern: opt-in `#[arg(long, default_value_t =
   false)]`, `Default = false`, threaded through
   `build_grid_config`. That ship also cleared an analogous
   "no behavioral regression" clause. Both precedents enshrine
   the behavioral reading.

4. **Operational test.** TASK-0391's regression is a smoke that
   runs `church_add(2, 3)` through `run_grid` with
   `delta_mode = false` and asserts bit-identical decoded result
   and `total_interactions` against a known v1 baseline. A
   behavioral regression (a bit flip, an extra round, a lost
   interaction) would fail that smoke immediately. A source-only
   textual change cannot fail that smoke — which is exactly the
   correct signal.

**Counter-option (B — source-diff zero) rejected:**
contradicts R41, contradicts Rust struct-extension idiom,
contradicts the TASK-0350 and TASK-0358 precedents, and would
render R42 unsatisfiable. The task-splitter's behavioral
interpretation is correct.

**Task-file amendments required:** NONE. TASK-0389 (Notes §2:
"adding a field to a struct is a source change; R42 forbids
*behavioral* regression"), TASK-0390 (Acceptance Criteria line 4:
"absent `--delta-mode` on the CLI → `args.delta_mode = false`"),
TASK-0391 (entire smoke-test design), and TASK-0393 (doctest
asserts `!cfg.delta_mode` for `Default`) already encode Option A.
No change needed. TASK-0391's Notes §3 "If spec-critic rules for
the strict textual reading…" branch can be deleted once the
developer reads this verdict; I mark it closed rather than
require an edit.

---

### AMB-D-3: R38/R39/R40 "MUST reformulate" — narrative vs proof

**PICK:** **Option C** — the MUST applies to the *design
reformulation* uniformly for R38/R39/R40 (narrative documentation
of the amended statement satisfies it in all three cases). The
deferral language is **NOT uniform**, however:

- **R38 carries an explicit "proof pending" clause** (line 223:
  "The full formal proof of the recoverability property is
  pending (see Section 8, Open Questions: DISC-011 + ARG-005).
  This spec defines the design; the theoretical proof is a
  separate deliverable.") — the `(MUST for the reformulation;
  proof pending)` parenthetical at line 225 makes the
  bifurcation explicit in the spec itself.
- **R39 also carries "pending formal proof" language** (line
  234: "…is the core correctness claim of the delta protocol and
  is pending formal proof (Section 8).") — narrative documentation
  suffices for the reformulation; the proof is deferred to ARG-005.
- **R40 does NOT carry any "pending proof" clause.** Its fourth
  bullet ("Progress guarantee") is a self-contained operational
  argument: each round consumes ≥ 1 interaction from the finite
  T7 budget ⇒ the protocol terminates. This is a short inline
  proof, not a deferral. R40's MUST is therefore satisfied by
  (a) narrative documentation of the reformulated bound
  (R_delta_lenient = 1 in the common case; R_delta_strict ≤ N),
  AND (b) the *operational* termination behaviour shipping in
  sub-bundle 2.26-C (the BSP loop that exits on Global Normal
  Form). Sub-bundle 2.26-D delivers only the narrative half.

**WHY:**

1. **Spec reading is surgical.** R38 and R39 both explicitly
   forward the proof obligation to Section 8 (DISC-011 / ARG-005).
   R40 does not. Treating them uniformly as "all three proof-
   deferred" would mis-represent R40's actual status — R40 has an
   in-spec operational termination argument and a concrete
   termination path (2.26-C's delta BSP loop with Global Normal
   Form check). The appearance of "MUST" + absence of "pending"
   language in R40 means the MUST is expected to be discharged
   operationally, not deferred indefinitely.

2. **Spec-critic role is to read closely.** The task-splitter's
   unified Option A reading ("MUST applies only to design
   reformulation for all three") is 2/3 correct. The remaining
   third — R40 — requires the operational behaviour of 2.26-C to
   fully discharge it; 2.26-D's narrative is a necessary but
   not sufficient half. Fortunately, 2.26-C is already scoped
   to ship the delta BSP loop, so no *new* cross-bundle work is
   created by this ruling — it only re-labels the ROADMAP
   narrative for R40.

3. **Option B (MUST = narrative + proof in bundle) rejected.**
   The proofs for R38/R39 are explicitly deferred by the spec
   itself to Section 8 and the TCC work stream (ARG-005). A
   sub-bundle 2.26-D obligation to ship those proofs would
   contradict the in-spec deferral and would also misplace the
   work (TCC theoretical argument, not Relativist implementation).

4. **Precedent.** SPEC-01's G1 ship (v1) did not carry a formal
   confluence proof either — it cited the Lafont 1997 result and
   documented the operational test coverage (`run_grid` smoke).
   The pattern "MUST for the statement; proof cited/deferred" is
   a recurring spec idiom; R38/R39 follow it; R40 deviates by
   inlining its own operational argument instead.

**Counter-option (A uniform — MUST only applies to narrative for
all three) rejected:** it under-specifies R40's status by
ignoring the absence of "pending proof" language and by ignoring
the in-spec operational termination argument. Option A risks
letting R40 ship as "documented, proof deferred forever" when
the spec actually calls for an *implemented* progress guarantee.

**Counter-option (B uniform — MUST requires proof for all three)
rejected:** contradicts R38/R39's explicit Section 8 deferral
and misplaces TCC theoretical work into implementation bundles.

**Task-file amendments required:** ONE. TASK-0392 (ROADMAP
narrative) currently treats all three amendments uniformly under
"proof deferred to Section 8". The task-updater must split the
narrative block so that:

- **G1 (R38)** and **D3 (R39)** paragraphs include the phrase
  "formal proof deferred to Section 8 / ARG-005" (as currently
  drafted). Operational guarantee for G1 is the 2.26-C
  convergence test vs `reduce_all` reference; for D3 it is the
  `BorderGraph.detect_border_redexes()` oracle shipped in 2.35.
- **D6 (R40)** paragraph does NOT say "proof deferred". Instead
  it says: "Termination proof is operational: each round
  consumes ≥ 1 interaction from the finite budget T7, so the
  protocol terminates in ≤ N rounds strict / 1 round lenient.
  This is the full spec statement at §3.5 R40; no Section 8
  deferral." The R40 paragraph cross-references sub-bundle
  2.26-C's delta BSP loop as the operational discharge of the
  MUST.

The task-splitter's proposed subsection skeleton in TASK-0392
(lines 86-124) needs this surgical edit on the D6 block. All
other skeleton paragraphs stand as written. This is a task-file
amendment only — **no spec edit, no code change**.

---

## Cross-Bundle Coupling

One cross-bundle coupling flagged by AMB-D-3 ruling:

- **R40 operational discharge requires sub-bundle 2.26-C.** The
  narrative in TASK-0392 documents the *design*; the *MUST* is
  fully discharged only when 2.26-C's delta BSP loop ships with
  the Global Normal Form termination check (joint:
  per-worker `has_border_activity == false` from TASK-0348 +
  `BorderGraph.is_empty()` from 2.35 + zero local redexes).
  Sub-bundle 2.26-D may ship and close independently, but the
  bundle index MUST record that R40 is **PARTIAL after 2.26-D,
  COMPLETE after 2.26-C**. This mirrors the AMB-D-1 pattern
  (R41 partial after 2.34, complete after 2.26-D). The
  task-updater should add this status line to the bundle index
  (after the current "Stage 1.5 SPEC-CRITIC" line at file-
  bottom).

No other cross-bundle coupling surfaced. AMB-D-1 is already
closed (2.34 shipped); AMB-D-2 is self-contained within 2.26-D.

---

## Summary Table

| AMB | Verdict | Rationale one-liner | Task-file amendments |
|-----|---------|---------------------|----------------------|
| D-1 | A | `coordinator_free_rounds` shipped early (TASK-0350); don't re-ship; treat R41 as compound (half + half). | None |
| D-2 | A | R42 is behavioral invariance, not source-diff zero; additive struct/CLI changes are idiomatic and regression-tested. | None (TASK-0391 Notes §3 conditional branch can be struck) |
| D-3 | C | MUST applies to design reformulation uniformly; proof-deferral is NOT uniform — R38/R39 defer to §8, R40 discharges operationally in 2.26-C. | TASK-0392 narrative split (D6 block must mark "operational, no §8 deferral"); bundle index must record R40 PARTIAL → COMPLETE after 2.26-C |

**Stage 2 TESTS status:** UNBLOCKED. The ruling on AMB-D-3
requires the task-updater to touch TASK-0392 (narrative wording
for D6) and the bundle index (R40 partial/complete status line)
before the test-generator dispatches, but the type-change tasks
(TASK-0389/0390/0391/0393) are unaffected and may proceed in
parallel.
