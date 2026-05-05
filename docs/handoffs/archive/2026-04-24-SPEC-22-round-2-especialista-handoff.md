# Handoff — SPEC-22 Wave 2, Round 2 (especialista-em-specs)

**Status:** READY TO DISPATCH
**Saved:** 2026-04-24
**Active bundle:** v2 Pre-DEV Spec Pipeline (Waves 1–5, no code) — Wave 1 closed in commit `ec680e4`; Wave 2 active.
**Master plan (reference):** `C:\Users\Filipe\.claude\plans\kind-shimmying-harbor.md`

> **Per memory policy** (`feedback_especialista_specs_dispatch.md`): `especialista-em-specs` runs from the TCC root session, not the relativist subdir. Paste the §3 prompt verbatim into the TCC root Claude Code session.

## 1. State of SPEC-22

- Spec: `codigo/relativist/specs/SPEC-22-arena-management.md`, currently `Status: Draft`.
- Coherence brief (pesquisador): `codigo/relativist/docs/briefings/SPEC-22-coherence-brief-2026-04-24.md` (280L).
- spec-critic Round 1: `codigo/relativist/docs/spec-reviews/SPEC-REVIEW-22-round-1-2026-04-24.md` (445L). **Verdict: BLOCK.** 21 findings: 4 CRITICAL / 7 HIGH / 6 MEDIUM / 4 LOW.
- Top-3 concerns:
  1. **SC-001 (CRITICAL):** §4.1 `Net` struct silently drops `freeport_redirects` (live code has 7 fields, §4.1 shows 6); defect propagates into §4.6 `to_dense()` body.
  2. **SC-002 (CRITICAL):** SPEC-22 amends SPEC-01 I3 but not SPEC-02 R2 ("never reused") or R10 (`next_id` "incremented by k"); leaves predecessors contradictory.
  3. **SC-005 (HIGH):** SPEC-19 `BorderGraph` stores `AgentPort(id, port)` references; SPEC-22 R10's per-worker ID-range constraint does not protect the coordinator from reading a recycled `AgentId` whose live agent is now a different `Symbol` — direct G1 threat under delta mode.
- Pesquisador-predicted findings all CONFIRMED + sharpened by spec-critic.
- Theory bridge audit: 3 cited IDs (REF-002, REF-003, REF-014) all resolve. Frontmatter missing AC-001/AC-006 citations. Bridge has stale "SPEC-22 (Job submission)" label on DISC-012 — TCC-root cleanup item, NOT SPEC-22's responsibility (SC-013).

## 2. Closure log + spec status expectations

- **Closure log:** `codigo/relativist/docs/spec-reviews/SPEC-REVIEW-22-round-2-YYYY-MM-DD.md` (replace YYYY-MM-DD with run date). Use `SPEC-REVIEW-20-round-2-2026-04-24.md` as the structural template.
- **End-state SPEC-22 status field:**
  - If all 4 CRITICAL + 7 HIGH findings close inline, MEDIUMs and LOWs addressed, and no fresh structural finding emerges: bump to `Reviewed v2` (matches SPEC-20 path). Round 3 spec-critic remains AVOIDABLE.
  - If any CRITICAL/HIGH closure judged structurally non-trivial (re-scope, new amendments needed beyond §3.8 surface): bump to `Draft — Round 2 (closure landed; pending spec-critic Round 3 review)` and explicitly request Round 3.
  - **NOT** `Reviewed v2 — Round 3 closure landed` (that was SPEC-20's pattern after a partial Round 3); SPEC-22 has no prior Round 3 entry.

## 3. Agent prompt (paste verbatim into especialista-em-specs)

```
Mode: Round 2 defender / closure for SPEC-22 (Arena Management). spec-critic
Round 1 verdict was BLOCK (21 findings: 4 CRITICAL, 7 HIGH, 6 MEDIUM, 4 LOW).
This is a fresh Round 2 — no prior closure exists. The Round 1 verdict is BLOCK,
not CONDITIONAL_PASS, so the closure pass must be substantial: every CRITICAL
and HIGH MUST be addressed; MEDIUMs and LOWs MAY be deferred only with explicit
in-spec gating that surfaces the residual obligation.

INPUTS (read in this order):
1. codigo/relativist/specs/SPEC-22-arena-management.md — target spec
   (Status: Draft).
2. codigo/relativist/docs/spec-reviews/SPEC-REVIEW-22-round-1-2026-04-24.md
   — adversarial review (445 lines, 21 findings SC-001..SC-021 with verdict
   BLOCK; cross-spec consistency table in §3 enumerates every R-number
   amendment audit).
3. codigo/relativist/docs/briefings/SPEC-22-coherence-brief-2026-04-24.md —
   pesquisador's research-orientation brief (280L). Use as supplementary
   context only; spec-critic's review is the authoritative finding list.
4. codigo/relativist/docs/theory-bridge.md — canonical ARG/DISC/REF/AC index.
5. (Reference) Predecessor specs that SPEC-22 amends or depends on:
   SPEC-01 (Invariants — I3 amendment), SPEC-02 (Net Representation — R2,
   R10, R11, R12), SPEC-03 (Reduction Engine — assert_next_id_valid),
   SPEC-04 (Partition — R10a/R22 build_subnet), SPEC-05 (Merge — R12 merge),
   SPEC-19 (Delta Protocol — BorderGraph slot-id stability), SPEC-18 (Wire
   Format v2 — PROTOCOL_VERSION versioning per SC-007).
6. (Format reference) codigo/relativist/specs/SPEC-20-elastic-grid.md §3.8 —
   the canonical "Amendments to Predecessor Specs" block pattern from Wave 1
   that SPEC-22 must adopt (the absence of this block was escalated to
   CRITICAL by spec-critic).

WORK TO DO:

  CRITICAL (4) — MUST close inline:
    SC-001  §4.1 Net struct missing freeport_redirects field; propagates into
            §4.6 to_dense() body. Either add the field to §4.1 explicitly OR
            state §4.1 shows only additive changes and explicitly preserves
            all 7 fields from SPEC-02 R12. Then audit §4.6 to_dense() body
            for the same defect.
    SC-002  Add §3.8 Amendments block (canonical pattern from SPEC-20 §3.8).
            Inside it, formally amend SPEC-02 R2 ("never reused") and SPEC-02
            R10 (`next_id` "incremented by k") to align with SPEC-22's I3'.
            Without these amendments, SPEC-01 ↔ SPEC-02 contradict each other
            after SPEC-22 ships.
    SC-003  Add SPEC-04 and SPEC-05 to the `Depends on:` frontmatter (R10a/R22
            amend build_subnet; R12 amends merge — the dependencies are
            load-bearing, not informational).
    SC-004  Promote the soft "no §3.8 block" finding: this is the structural
            container for SC-002, SC-003, and any new amendments. Without it
            the entire amendment surface is non-auditable. Spec-critic's
            cross-spec table (review §3) is the source of truth for what
            belongs in §3.8.

  HIGH (7) — MUST close inline (defer to MEDIUM only with strong rationale
  + in-spec gating):
    SC-005  BorderGraph AgentPort(id, port) recycling under delta mode —
            G1 threat. Either prohibit ID recycling for any Agent reachable
            from BorderGraph (per-worker ID range narrowed to "live agents
            only, never recycled across deltas") OR add an arena-local
            generation counter to AgentPort. Coordinate with SPEC-19's
            invariants.
    SC-006  Cross-spec audit table requirements — every R-number that names
            another spec must be lifted from review §3 into the §3.8 block.
    SC-007  R9 serde format change requires SPEC-18 PROTOCOL_VERSION bump.
            Add an A-amendment for SPEC-18 stating the version increment and
            the v3-vs-v4 worker rejection clause (mirrors SPEC-20 R37 pattern).
    SC-008  R21 round-trip ambiguity — "structurally equal modulo trailing
            None slots" must specify byte-equality OR behavioural-equality OR
            graph-isomorphism. Pick one and gate test EG-equivalent (or
            equivalent SPEC-22 test) on that.
    SC-009  R3 vs SPEC-02 R11 ("the next available ID") partial contradiction
            — SPEC-02 R11 says "the next available ID is monotonically
            increasing", SPEC-22 R3 reuses recycled IDs. Either amend SPEC-02
            R11 in §3.8 OR change R3 to allocate from a separate
            recycled-id-pool space distinct from the monotonic next_id.
    SC-010  SPEC-03 assert_next_id_valid + any rule-internal debug assertions
            need explicit I3'-compatibility audit. Add an A-amendment for
            SPEC-03 stating the new contract.
    SC-011  Memory bound at M5 milestone (100M agent target) — Q3 "tentatively
            no cap" was justified for 10M scale; at 100M the free-list is
            400 MB. Either commit to a cap with a graceful out-of-memory
            response OR re-justify the no-cap stance against the 100M target.

  MEDIUM (6) — close where possible; defer with in-spec gating only when
  necessary:
    [Read review §"MEDIUM" findings SC-012..SC-017 inline; closures are
    expected to be 1-paragraph each.]

  LOW (4):
    SC-013  Theory-bridge stale "SPEC-22 (Job submission)" tag on DISC-012
            — file as a TCC-root cleanup task; do NOT modify theory-bridge
            from this Round 2 closure (theory-bridge is TCC-root territory).
            Acknowledge in §11 Change Log.
    [Read review §"LOW" findings SC-018..SC-021 inline; closures are
    1-line each.]

  POLISH (frontmatter):
    Add AC-001, AC-006, AC-009, AC-011, AC-015 to `Code analyses consumed:`
    in SPEC-22 frontmatter. Verify each ID resolves in theory-bridge.md.

DELIVERABLES:
1. Edit codigo/relativist/specs/SPEC-22-arena-management.md applying every
   CRITICAL + HIGH closure plus all MEDIUM/LOW closures that don't require
   structural re-scope. Specifically:
   - Add a §3.8 Amendments block (canonical SPEC-20 pattern) populated with
     A-amendments for SPEC-01, SPEC-02 (R2, R10, R11), SPEC-03, SPEC-04
     (R10a, R22), SPEC-05 (R12), SPEC-18 (PROTOCOL_VERSION), and SPEC-19
     (if BorderGraph contract changes).
   - Update §4.1 Net struct definition (SC-001) and §4.6 to_dense() body.
   - Add SPEC-04 + SPEC-05 (and SPEC-18 + SPEC-19 if amended) to the
     `Depends on:` frontmatter. Add AC-001/006/009/011/015 to
     `Code analyses consumed:`.
   - Apply remaining HIGH/MEDIUM/LOW closures inline.
2. Add a §11 Change Log section if absent, with a "Round 2 — YYYY-MM-DD —
   NF closure pass" entry enumerating each finding closed (SC-001..SC-021)
   with diff summary.
3. Bump status field from "Draft" to "Reviewed v2" (preferred — analog of
   SPEC-20 path) OR "Draft — Round 2 (closure landed; pending spec-critic
   Round 3 review)" only if a CRITICAL/HIGH closure forced a structural
   re-scope or introduced a new amendment surface that requires fresh
   adversarial review.
4. Write closure log at
   codigo/relativist/docs/spec-reviews/SPEC-REVIEW-22-round-2-YYYY-MM-DD.md
   following SPEC-REVIEW-20-round-2-2026-04-24.md structure (per-finding:
   verdict CLOSED/DEFERRED/NOT_CLOSED, evidence, diff pointer; gate decision
   section).

CONSTRAINTS:
- Documentation only. NO src/ or tests/ edits. Test counts must remain
  1181 default / 1224 zero-copy.
- ARG/DISC/REF/AC identifiers must match theory-bridge.md exactly. Any cited
  ID not in the bridge is a hard block — surface it instead of inventing.
- Stay within "specs/" and "docs/spec-reviews/" — do NOT touch src/, tests/,
  docs/backlog/, docs/tests/, docs/briefings/, or docs/theory-bridge.md
  (last is TCC-root maintained).
- Theory-bridge stale DISC-012 label (SC-013) is TCC-root cleanup territory;
  acknowledge in §11 but do NOT edit the bridge.
- Stage 1 (TASK-SPLITTER) and Stage 2 (TEST-GENERATOR) come AFTER your
  closure log lands.

When done, report: (a) closure log path, (b) status field new value,
(c) per-finding one-line summary (SC-001..SC-021: CLOSED/DEFERRED with
gating/NOT_CLOSED), (d) any item escalated to spec-critic Round 3,
(e) any new fresh finding (NF-NNN) introduced by the revision that the
specialist surfaced.
```

## 4. After especialista-em-specs returns

1. Verify the closure log exists and SPEC-22 §11 Change Log has the Round 2 row.
2. Decide whether spec-critic Round 3 is needed.
   - If status = `Reviewed v2`: skip Round 3, advance to Stage 1 (task-splitter).
   - If status = `Draft — Round 2 (closure landed; pending spec-critic Round 3)`: dispatch spec-critic Round 3 here in the relativist session before Stage 1.
3. After Stage 0 fully closes: dispatch `task-splitter` on SPEC-22.
4. Then Stage 2: `test-generator` on SPEC-22 backlog.
5. Tracking + commit (Conventional Commits, scope `SPEC-22`).
6. Update `docs/next-steps.md` Wave 2 row.
7. Move on to SPEC-21 Streaming Generation (second half of Wave 2).

## 5. Files NOT to touch in this round

- `src/`, `tests/` (no DEV until v2 Pre-DEV bundle closes)
- `docs/backlog/`, `docs/tests/` (Stages 1/2 territory)
- `docs/theory-bridge.md` (TCC-root maintained)
- `docs/briefings/SPEC-22-coherence-brief-2026-04-24.md` (research record)
- TCC root `discussoes/`, `biblioteca/`, `artigo/`

## 6. Quick-resume checklist

- [ ] `cd C:\Users\Filipe\Desktop\TCC_interaction_combinators_for_grid_computing` (TCC root, NOT relativist subdir)
- [ ] Open this handoff: `codigo/relativist/docs/handoffs/2026-04-24-SPEC-22-round-2-especialista-handoff.md`
- [ ] Dispatch `especialista-em-specs` with §3 prompt verbatim
- [ ] After agent returns, follow §4 steps in the relativist subdir session
