# Handoff — SPEC-21 Wave 2 (second half), Round 2 (especialista-em-specs)

**Status:** READY TO DISPATCH
**Saved:** 2026-04-25
**Active bundle:** v2 Pre-DEV Spec Pipeline (Waves 1–5, no code) — Wave 1 closed in `ec680e4`; Wave 2 first half (SPEC-22) closed in `b66f758` + `bbd976e`; this handoff drives the second half (SPEC-21).
**Master plan (reference):** `C:\Users\Filipe\.claude\plans\kind-shimmying-harbor.md`

> **Per memory policy** (`feedback_especialista_specs_dispatch.md`): `especialista-em-specs` runs from the TCC root session, not the relativist subdir. Paste the §3 prompt verbatim into the TCC root Claude Code session.

## 1. State of SPEC-21

- Spec: `codigo/relativist/specs/SPEC-21-streaming-generation.md`, currently `Status: Draft`.
- Coherence brief (pesquisador): `codigo/relativist/docs/briefings/SPEC-21-coherence-brief-2026-04-25.md` (308L).
- spec-critic Round 1: `codigo/relativist/docs/spec-reviews/SPEC-REVIEW-21-round-1-2026-04-25.md` (459L). **Verdict: BLOCK — MAJOR REVISION REQUIRED.** 24 findings: 2 CRITICAL / 7 HIGH / 10 MEDIUM / 5 LOW.
- Top-3 concerns (verbatim from review):
  1. **SC-001 (CRITICAL):** No §3.8 Amendments block; five cross-spec amendments (SPEC-06/07/09/13/04) buried in body prose, blocking task-splitter Phase A.
  2. **SC-006 (HIGH):** `PartitionAccumulator` (§4.9) wraps a dense `Net`, recreating the M5 dense-arena inflation that SPEC-22 R22 just fixed via `SparseNet`.
  3. **SC-007 (HIGH):** G1 threat — SPEC-22 free-list slot recycling × SPEC-21 streaming border references can collide unless SPEC-22 R10b protected tombstones are invoked.
- All 5 pesquisador-predicted findings CONFIRMED + sharpened. 5 fresh adversarial angles opened by spec-critic (SC-008 Benchmark default-impl, SC-018 §4.8 implicit SPEC-04 amendment, SC-019 G1 termination under pull dispatch, SC-021 chunks_processed gap, SC-022/SC-023 WorkerId/PortId origins).

## 2. Closure log + spec status expectations

- **Closure log:** `codigo/relativist/docs/spec-reviews/SPEC-REVIEW-21-round-2-YYYY-MM-DD.md` (replace YYYY-MM-DD with run date). Use `SPEC-REVIEW-22-round-2-2026-04-25.md` as the structural template (it just closed a similar BLOCK→Reviewed v2 pass).
- **End-state SPEC-21 status field:**
  - Preferred: `Reviewed v2` if all 2 CRITICAL + 7 HIGH close inline, MEDIUMs/LOWs addressed, no fresh structural finding emerges (matches SPEC-22 path).
  - Fallback: `Draft — Round 2 (closure landed; pending spec-critic Round 3 review)` only if a CRITICAL/HIGH closure forces structural re-scope.

## 3. Agent prompt (paste verbatim into TCC root Claude Code)

```
Dispatch the especialista-em-specs agent with the prompt below. The agent
should run autonomously and return a closure log + summary.

Mode: Round 2 defender / closure for SPEC-21 (Streaming Generation).
spec-critic Round 1 verdict was BLOCK — MAJOR REVISION REQUIRED
(24 findings: 2 CRITICAL, 7 HIGH, 10 MEDIUM, 5 LOW). This is a fresh
Round 2 — no prior closure exists. The Round 1 verdict is BLOCK, so the
closure pass must be substantial: every CRITICAL and HIGH MUST be
addressed; MEDIUMs and LOWs MAY be deferred only with explicit in-spec
gating that surfaces the residual obligation.

INPUTS (read in this order):
1. codigo/relativist/specs/SPEC-21-streaming-generation.md — target spec
   (Status: Draft).
2. codigo/relativist/docs/spec-reviews/SPEC-REVIEW-21-round-1-2026-04-25.md
   — adversarial review (459 lines, 24 findings SC-001..SC-024 with
   verdict BLOCK; cross-spec consistency table enumerates every R-number
   amendment audit).
3. codigo/relativist/docs/briefings/SPEC-21-coherence-brief-2026-04-25.md
   — pesquisador's research-orientation brief (308L). Use as supplementary
   context only; spec-critic's review is the authoritative finding list.
4. codigo/relativist/docs/theory-bridge.md — canonical ARG/DISC/REF/AC index.
5. (Reference) Predecessor specs that SPEC-21 amends: SPEC-04 (R12 border-id
   allocation policy implicit per SC-018), SPEC-06 (R31 wire variants
   RequestWork/NoMoreWork), SPEC-07 (R24/R25/R34 GridConfig fields),
   SPEC-09 (R10 Benchmark trait amendment — 13 implementations affected),
   SPEC-13 (FSM amendments), SPEC-17 (transport layer), SPEC-18 (wire
   format v2 serde for new variants), SPEC-19 (R36 cross-reference),
   SPEC-22 (Arena Management — PartitionAccumulator interaction with
   free-list and R10b protected tombstones).
6. (Reference) codigo/relativist/specs/SPEC-22-arena-management.md §3.8 —
   the canonical "Amendments to Predecessor Specs" block pattern that
   SPEC-21 must adopt (just authored by SPEC-22 Round 2 closure).
7. (Reference) codigo/relativist/specs/SPEC-22-arena-management.md R22
   (SparseNet) and R10b/R10c (BorderGraph protected tombstones + recycle
   strategies) — SPEC-21 §4.9 PartitionAccumulator must adopt SparseNet
   to close SC-006; SPEC-21 streaming pipeline must reference R10b to
   close SC-007 G1 threat.

WORK TO DO:

  CRITICAL (2) — MUST close inline:
    SC-001  Add §3.8 Amendments block (canonical SPEC-22 pattern).
            Populate with A-amendments for SPEC-04 (R12 — implicit per
            SC-018 §4.8), SPEC-06 (R31 wire variants), SPEC-07 (R24/R25/R34
            GridConfig), SPEC-09 (R10 Benchmark trait — see SC-008 for
            default-impl decision), SPEC-13 (FSM amendments), and any
            other amendment surfaces the cross-spec audit table identifies.
    SC-002  Add to `Depends on:` frontmatter every predecessor spec that
            §3.8 amends: SPEC-06, SPEC-07, SPEC-09, SPEC-17, SPEC-18,
            SPEC-19, SPEC-22 (in addition to existing SPEC-01/02/04/05/13).

  HIGH (7) — MUST close inline (defer only with strong rationale + in-spec
  gating):
    SC-003  Add DISC-009 v2 to `Discussions consumed:` frontmatter — the
            primary taxonomy source for streaming generation per the bridge.
    SC-004  Resolve REF-015 streaming-level mismatch — bridge tags REF-015
            as "rule-level" (level 1); SPEC-21 cites for generation-protocol
            (level 3). Either justify the cross-level use inline OR remove
            the citation OR replace with a level-3-appropriate reference.
    SC-005  Predecessors-missing-from-Depends-on elevated (overlaps SC-002
            above; ensure both surfaces are reconciled).
    SC-006  PartitionAccumulator (§4.9) MUST adopt SPEC-22 SparseNet under
            the same `id_range > 4 × live_agent_count` threshold to avoid
            re-introducing the M5 dense-arena inflation pathology.
    SC-007  G1 threat closure: §4.x streaming pipeline MUST reference
            SPEC-22 R10b (protected tombstones) as the mitigation for
            slot-id stability across stream chunks, OR explicitly disable
            free-list recycling during the generation+accumulation phase
            via a feature gate. Pick one and document.
    SC-008  Benchmark trait amendment (R10/R11/R12) — pick a default-impl
            disposition: either provide a default impl in the trait
            (~30 LoC migration of 13 implementations) OR mandate explicit
            implementation by every benchmark (~520 LoC). Document the
            choice and the migration path.
    SC-009  PROTOCOL_VERSION disposition for R31 wire variants — third
            spec in the wave to touch the version constant (SPEC-20 3→4,
            SPEC-22 2→3). Use the same defensive language as SPEC-22
            TASK-0476 (assert PREVIOUS_LIVE_VERSION + 1, not a hardcoded
            integer). Document explicitly in §3.8 / §3.x.

  MEDIUM (10) and LOW (5): close where possible; defer with in-spec gating
  only when necessary. Read review §"MEDIUM" and §"LOW" inline.

  Items to file as TCC-root cleanup (NOT to fix from this Round 2):
    Tsourakakis 2014 (FENNEL) and Stanton & Kliot 2012 (LDG) cited but
    absent from theory-bridge — file as the SPEC-22 SC-013 analog. Do NOT
    edit theory-bridge.md (TCC-root territory). Acknowledge in §11 Change
    Log as TCC-root to-do.

  POLISH (frontmatter):
    Add AC-007 (HVM2 Reduction Engine), AC-010 (HVM4 WNF Evaluation),
    AC-014 (Bench methodology) to `Code analyses consumed:` per the brief's
    natural-anchor list. Add DISC-004 v2 to `Discussions consumed:`
    (currently cited only in body).

DELIVERABLES:
1. Edit codigo/relativist/specs/SPEC-21-streaming-generation.md applying
   every CRITICAL + HIGH closure plus all MEDIUM/LOW closures that don't
   require structural re-scope. Specifically:
   - Add §3.8 Amendments block (canonical SPEC-22 pattern) populated with
     A-amendments for SPEC-04 (R12), SPEC-06 (R31), SPEC-07 (R24/R25/R34),
     SPEC-09 (R10 — with default-impl disposition), SPEC-13, and any other
     amendment surfaces the cross-spec audit table identifies.
   - Update §4.9 PartitionAccumulator to adopt SparseNet (SC-006).
   - Add §4.x clause referencing SPEC-22 R10b for streaming border
     protection (SC-007), or explicitly gate streaming on free-list
     recycling disabled.
   - Update Benchmark trait amendment with default-impl disposition (SC-008).
   - Document PROTOCOL_VERSION disposition with defensive language (SC-009).
   - Update frontmatter `Depends on:` (+SPEC-06/07/09/17/18/19/22),
     `Discussions consumed:` (+DISC-004 v2, +DISC-009 v2),
     `Code analyses consumed:` (+AC-007/010/014).
   - Apply remaining HIGH/MEDIUM/LOW closures inline.
2. Add §11 Change Log section if absent, with "Round 2 — YYYY-MM-DD —
   NF closure pass" entry enumerating SC-001..SC-024 closures with diff
   summary.
3. Bump status field from "Draft" to "Reviewed v2" (preferred — analog of
   SPEC-22 path) OR "Draft — Round 2 (closure landed; pending spec-critic
   Round 3 review)" only if a CRITICAL/HIGH closure forced structural
   re-scope.
4. Write closure log at
   codigo/relativist/docs/spec-reviews/SPEC-REVIEW-21-round-2-YYYY-MM-DD.md
   following SPEC-REVIEW-22-round-2-2026-04-25.md structure (per-finding
   verdict CLOSED/DEFERRED/NOT_CLOSED, evidence, diff pointer; gate
   decision section).

CONSTRAINTS:
- Documentation only. NO src/ or tests/ edits. Test counts must remain
  1181 default / 1224 zero-copy.
- ARG/DISC/REF/AC identifiers must match theory-bridge.md exactly. Any
  cited ID not in the bridge is a hard block — surface it instead of
  inventing.
- Stay within "specs/" and "docs/spec-reviews/" — do NOT touch src/,
  tests/, docs/backlog/, docs/tests/, docs/briefings/, or
  docs/theory-bridge.md.
- Theory-bridge cleanup items (FENNEL/LDG REF registration, plus the
  SPEC-22 SC-013 DISC-012 stale tag still pending) are TCC-root cleanup;
  acknowledge in §11 but do NOT edit the bridge.
- Stage 1 (TASK-SPLITTER) and Stage 2 (TEST-GENERATOR) come AFTER your
  closure log lands.

When done, report: (a) closure log path, (b) status field new value, (c)
per-finding one-line summary (SC-001..SC-024: CLOSED/DEFERRED/NOT_CLOSED),
(d) any item escalated to spec-critic Round 3, (e) any new fresh finding
(NF-NNN) introduced by the revision.
```

## 4. After especialista-em-specs returns

1. Verify closure log exists and SPEC-21 §11 has the Round 2 row.
2. Decide whether spec-critic Round 3 is needed.
   - If status = `Reviewed v2`: skip Round 3, advance to Stage 1 (task-splitter).
   - If status = `Draft — Round 2 (...)`: dispatch spec-critic Round 3 here in the relativist session before Stage 1.
3. After Stage 0 closes: dispatch `task-splitter` on SPEC-21.
4. Then Stage 2: `test-generator`.
5. Tracking + commit (Conventional Commits, scope `SPEC-21`).
6. Update `docs/next-steps.md` Wave 2 row → fully closed.
7. **Wave 2 fully closed — advance to Wave 3** (SPEC-25 Recipe Gen → SPEC-27 R26-R28 deferred D-001).

## 5. Files NOT to touch in this round

- `src/`, `tests/` (no DEV until v2 Pre-DEV bundle closes)
- `docs/backlog/`, `docs/tests/` (Stages 1/2 territory)
- `docs/theory-bridge.md` (TCC-root maintained)
- `docs/briefings/SPEC-21-coherence-brief-2026-04-25.md` (research record)
- TCC root `discussoes/`, `biblioteca/`, `artigo/`

## 6. Quick-resume checklist

- [ ] `cd C:\Users\Filipe\Desktop\TCC_interaction_combinators_for_grid_computing` (TCC root)
- [ ] Open this handoff: `codigo/relativist/docs/handoffs/2026-04-25-SPEC-21-round-2-especialista-handoff.md`
- [ ] Dispatch `especialista-em-specs` with §3 prompt verbatim
- [ ] After agent returns, follow §4 steps in the relativist subdir session
