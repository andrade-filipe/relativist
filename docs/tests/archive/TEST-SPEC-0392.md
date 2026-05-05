# TEST-SPEC-0392: ROADMAP narrative amendment notes for G1 / D3 / D6 (R38, R39, R40) — DOC-ONLY

**Task:** TASK-0392
**Spec:** SPEC-19 §3.5 — R38 (G1 reformulation, **proof pending**), R39 (D3 reformulation, **proof pending**), R40 (D6 reformulation, **operational — no §8 deferral**).
**Amendment log ref:** `docs/spec-reviews/SPEC-19-section-3.5-3.6-2.26D-design-choices-2026-04-17.md` (AMB-D-3 — surgical narrative split: R38/R39 marked "proof pending"; R40 marked "operationally complete after 2.26-C"). The split is the load-bearing amendment for this TEST-SPEC.
**Generated:** 2026-04-17
**Baseline before this task:** post TEST-SPEC-0391 — cumulative 975 default lib / 1015 `--features zero-copy`.
**Cumulative target after this task:** **+0** new `#[test]` fns — 975 default lib / 1015 `--features zero-copy` (UNCHANGED).

---

## Scope note

TASK-0392 is a **DOCUMENTATION-ONLY** task. It appends one subsection (~50 lines of Markdown) to `docs/ROADMAP.md` inside or immediately after the existing 2.26 section. It MAKES NO SOURCE CHANGES, ADDS NO TESTS, AND TOUCHES NO SPECS.

This TEST-SPEC therefore contains **zero `#[test]` fns**. Its purpose is to:

1. **Codify the "no runtime test" decision** explicitly so that Stage 4 reviewer and Stage 5 QA do not ask "where are the tests?" — there are intentionally none.
2. **Define documentation-quality acceptance checks** the developer (Stage 3) and reviewer (Stage 4) MUST run instead of `cargo test`. These are mechanical Markdown / cross-reference checks, not Rust-test assertions.
3. **Lock the AMB-D-3 narrative split** as a per-paragraph review checklist — R38/R39 paragraphs MUST carry "proof pending" framing; R40 paragraph MUST carry "operationally complete after 2.26-C" framing and an inline operational termination argument; no proof deferral phrasing on R40.
4. **Cross-reference the cross-bundle coupling** (R40 PARTIAL after 2.26-D → COMPLETE after 2.26-C) as a documentation-level coupling that mirrors the AMB-D-1 pattern (R41 partial after 2.34 → complete after 2.26-D).

**Why no `#[ignore]` stub for R38/R39?**
The natural reflex is to add an `#[ignore = "TODO(ARG-005): pending formal proof of G1 recoverability"]` test that future-someone enables when ARG-005 lands. This TEST-SPEC explicitly REJECTS that pattern for R38/R39 because:

- ARG-005 is a **TCC theoretical work item** (a written formal proof in the article), NOT a Relativist test artefact. There is nothing to "enable" in `cargo test` when ARG-005 ships — the proof lives in `discussoes/argumentos/ARG-005-*.md` and ultimately in `artigo/tcc_pt_br.tex`, not in Rust source.
- A theatrical ignored stub would mislead future maintainers into thinking ARG-005 has a Relativist test hook. The honest signal is the ROADMAP narrative + the SPEC-19 §3.5 / §8 cross-reference.
- The spec-critic AMB-D-3 verdict explicitly endorses "narrative documentation satisfies the MUST" for R38/R39 — no test layer is required.

**Why no `#[ignore]` stub for R40?**
R40's operational discharge ships in sub-bundle 2.26-C's BSP loop. The convergence test that operationally discharges R40 is a 2.26-C TEST-SPEC deliverable (joint check: per-worker `has_border_activity == false` + `BorderGraph.is_empty()` + zero local redexes), NOT a 2.26-D ignored stub. Adding a 2.26-D `#[ignore]` placeholder would duplicate-track the same operational obligation in two places. The single source of truth is the 2.26-C TEST-SPEC.

**Out of scope for this TEST-SPEC:**
- Field/CLI/smoke regression for `delta_mode` → TEST-SPEC-0389/0390/0391.
- `delta_mode` docstring polish + doctest → TEST-SPEC-0393.
- Spec edits to `SPEC-01-invariantes.md` or `SPEC-19-delta-protocol.md` — out of scope per task spec (specs are read-only; SPEC-19 §3.5 is the canonical amendment location per AMB-D-3).
- 2.26-C convergence test that operationally discharges R40 — separate sub-bundle.
- ARG-005 / DISC-011 / OQ-1 formal proof artefacts — TCC §8 work stream, not Relativist.

---

## Documentation target file paths

- `docs/ROADMAP.md` — the SOLE target file. One subsection appended (~50 lines), placed inside or immediately after the existing 2.26 section (around line 551 per task spec).
- **NO source files modified.**
- **NO test files created.**
- **NO spec files edited.**

---

## Documentation deliverables (in lieu of `#[test]` fns)

This task ships ONE atomic deliverable — a single Markdown subsection inserted into ROADMAP.md. The structural requirements below stand in for "test cases" since the conventional Stage 2 → Stage 3 contract (TEST-SPEC defines testable units; DEV implements + makes them pass) does not apply to a doc-only task. Stage 3 DEV reads this TEST-SPEC, writes the subsection, and Stage 4 REVIEWER runs the mechanical checks listed under §Acceptance gate.

### DOC-0392-01: Subsection header + framing paragraph

**Location:** `docs/ROADMAP.md`, immediately inside or after the existing 2.26 section block. Heading depth `####` to nest under the 2.26 `###` (or whatever existing depth — match local convention).

**Required heading text (exact string):**

```markdown
#### 2.26 Invariant Amendments (SPEC-19 §3.5 — TASK-0392)
```

**Required framing paragraph (verbatim — this is the AMB-D-3 "no SPEC-01 edit needed" disclaimer):**

```markdown
This is narrative documentation of the SPEC-19 amendments. The formal text
lives in SPEC-19 §3.5; the formal proofs are TCC work items (OQ-1, ARG-005,
DISC-011). No SPEC-01 edit is required — SPEC-19 §3.5 is the canonical
amendment location per the spec-critic ruling on AMB-D-3.
```

**Acceptance:** subsection title and framing paragraph appear verbatim. Stage 4 reviewer greps for the exact string `"No SPEC-01 edit is required"` to verify.

---

### DOC-0392-02: G1 (R38) paragraph — PROOF PENDING (AMB-D-3 Group 1)

**Required content:**

- One sentence stating the amendment: G1 reformulated from `reduce_all(net) ~ extract_result(run_grid(net, n))` to `reduce_all(net) ~ extract_result(run_grid_delta(net, n))`, where `extract_result` = Final State Collection (R27-R29) followed by `merge()`.
- A cross-reference to SPEC-19 §3.5 R38 (exact label, not a hyperlink — the file is in the same repo).
- Implementation touchpoint(s): the convergence test in sub-bundle 2.26-C; the `BorderGraph.detect_border_redexes()` oracle shipped in 2.35.
- **Required AMB-D-3 marker:** the paragraph MUST contain the literal phrase "**PROOF PENDING**" (bold) and the literal phrase "Section 8 / ARG-005" (or "Section 8, OQ-1 → DISC-011 → ARG-005").

**Required NEGATIVE check:** the G1 paragraph MUST NOT claim the proof is operationally complete; the formal proof obligation is a TCC §8 deliverable, not a Relativist runtime artefact.

**Acceptance:** Stage 4 reviewer greps the paragraph for `**PROOF PENDING**` and for one of the §8 reference strings; both MUST appear within the G1 paragraph block.

---

### DOC-0392-03: D3 (R39) paragraph — PROOF PENDING (AMB-D-3 Group 1)

**Required content:**

- One sentence stating the amendment: border-redex detection is incremental via `BorderGraph.detect_border_redexes()` (shipped in bundle 2.35, TASK-0374..0388); the v1 exhaustive scan in `merge::find_border_redexes` is retained for the `delta_mode = false` path.
- Cross-reference to SPEC-19 §3.5 R39.
- Implementation touchpoint: bundle 2.35 (BorderGraph) for the delta-path oracle; v1 `merge::find_border_redexes` for the disabled-path oracle.
- **Required AMB-D-3 marker:** the paragraph MUST contain the literal phrase "**PROOF PENDING**" (bold) and the literal phrase "Section 8" (referring to TCC §8 / ARG-005).
- The paragraph MUST state that equivalence between the two oracles (exhaustive v1 vs incremental delta) is the core correctness claim and is pending formal proof.

**Required NEGATIVE check:** D3 paragraph MUST NOT claim the equivalence is operationally proved by 2.35's tests; the operational tests in 2.35 cover the BorderGraph oracle's correctness *under the delta protocol's assumptions*, not its formal equivalence to v1's exhaustive scan. The two are distinct claims.

**Acceptance:** Stage 4 reviewer greps the paragraph for `**PROOF PENDING**`; greps for "Section 8"; reads the equivalence-claim sentence.

---

### DOC-0392-04: D6 (R40) paragraph — OPERATIONALLY COMPLETE AFTER 2.26-C (AMB-D-3 Group 2)

**Required content:**

- One sentence stating the amendment: termination is anchored on **Global Normal Form** — joint condition (a) every worker reports `local_redexes == 0` AND `has_border_activity == false` (TASK-0348), AND (b) `BorderGraph.is_empty()` (bundle 2.35).
- Cross-reference to SPEC-19 §3.5 R40.
- The lenient/strict mode bounds: `R_delta_lenient = 1` in the absence of cross-partition cascades; `R_delta_strict ≤ N` matches the v1 strict-mode bound.
- **Required INLINE OPERATIONAL ARGUMENT (verbatim or close paraphrase):** "Termination proof is operational: each round consumes ≥ 1 interaction from the finite budget T7, so the protocol terminates in ≤ N rounds strict / 1 round lenient. This is the full spec statement at §3.5 R40; no Section 8 deferral."
- **Required PARTIAL → COMPLETE status note:** "**Status: PARTIAL after 2.26-D, COMPLETE after 2.26-C.**" The 2.26-D sub-bundle ships only the configuration plumbing (TASK-0389/0390) and this narrative documentation (TASK-0392). The 2.26-C sub-bundle ships the delta BSP loop with the Global Normal Form termination check that operationally discharges the MUST.
- Cross-reference to sub-bundle 2.26-C's delta BSP loop as the operational discharge.

**Required NEGATIVE checks:**

- R40 paragraph MUST NOT contain the phrases "proof deferred", "proof pending", "Section 8 deferral", or any equivalent forwarding to TCC §8. R40's spec text contains no such deferral language; the AMB-D-3 ruling explicitly forbids importing R38/R39's deferral framing into R40.
- R40 paragraph MUST NOT contain "**PROOF PENDING**" — that marker is reserved for R38/R39.

**Acceptance:** Stage 4 reviewer:
1. Greps the paragraph for "**PARTIAL after 2.26-D, COMPLETE after 2.26-C**" (exact bold-marker match).
2. Greps for "no Section 8 deferral" (case-sensitive substring) — MUST be present.
3. Greps for "PROOF PENDING" anywhere in the R40 paragraph — MUST be ABSENT.
4. Reads the Progress-guarantee sentence to confirm the "≤ N rounds strict / 1 round lenient" bound is stated.

---

### DOC-0392-05: Tail "Configuration mechanism" paragraph

**Required content:** A short closing paragraph explaining how the new `delta_mode` config field (TASK-0389) and `--delta-mode` CLI flag (TASK-0390) toggle between the v1 path and the amended path. Default is `false` (R42).

**Required cross-references:**
- TASK-0389 (field + Default impl).
- TASK-0390 (CLI flag).
- R42 (default polarity).

**Acceptance:** Stage 4 reviewer verifies the tail paragraph mentions all three.

---

## Coverage mapping

| Requirement | Covered by |
|---|---|
| R38 — G1 reformulation narrative documented | DOC-0392-02 (G1 paragraph) |
| R38 — proof pending → Section 8 / ARG-005 | DOC-0392-02 ("**PROOF PENDING**" + "Section 8" markers) |
| R39 — D3 reformulation narrative documented | DOC-0392-03 (D3 paragraph) |
| R39 — proof pending → Section 8 | DOC-0392-03 ("**PROOF PENDING**" + "Section 8" markers) |
| R40 — D6 reformulation narrative documented | DOC-0392-04 (D6 paragraph) |
| R40 — operational discharge in 2.26-C, no §8 deferral (AMB-D-3 Group 2) | DOC-0392-04 (PARTIAL → COMPLETE marker + "no Section 8 deferral" string + Progress-guarantee sentence) |
| R41 — config mechanism narrative | DOC-0392-05 (tail paragraph cross-referencing TASK-0389/0390) |
| R42 — default polarity narrative | DOC-0392-05 (tail paragraph) |
| AMB-D-3 verdict — narrative split between R38/R39 (proof pending) and R40 (operationally complete) | DOC-0392-02/03 vs DOC-0392-04 |
| Cross-bundle coupling — R40 PARTIAL after 2.26-D → COMPLETE after 2.26-C | DOC-0392-04 (status marker) |

**Proof scaffolding note (no test layer).** This TEST-SPEC adds NO `#[test]` fns and NO `#[ignore]` stubs. The R38/R39 "proof pending" status discharges via narrative documentation (per spec-critic AMB-D-3 ruling); the R40 status discharges via narrative documentation HERE plus operational behaviour in the 2.26-C BSP loop. Future maintainers searching for proof-status tracking should read:

1. `docs/ROADMAP.md` §2.26 Invariant Amendments — the narrative landed by TASK-0392 (this task).
2. `discussoes/argumentos/ARG-005-*.md` (when it lands) — the formal proof artefact for R38/R39.
3. Sub-bundle 2.26-C's TEST-SPECs — the operational discharge of R40.

NOT this TEST-SPEC. NOT `relativist-core/src/`. NOT `cargo test`.

---

## Adversarial angles (QA candidates for Stage 5)

| # | Scenario | Why dangerous |
|---|----------|---------------|
| QA-0392-A | R40 paragraph copy-pasted from R38/R39 template; accidentally contains "PROOF PENDING" or "Section 8 deferral" | Reviewer grep for `PROOF PENDING` in the R40 paragraph block fires. AMB-D-3 ruling violated. |
| QA-0392-B | R38/R39 paragraphs OMIT the "PROOF PENDING" marker | Reviewer grep MISSES the marker — R38/R39 silently mislabelled as operational. AMB-D-3 violated in the opposite direction. |
| QA-0392-C | R40 paragraph omits the "PARTIAL after 2.26-D, COMPLETE after 2.26-C" status note | Future readers cannot trace the cross-bundle coupling. Reviewer grep for the exact bold-marker fires. |
| QA-0392-D | Subsection placed at wrong heading depth (e.g., `###` instead of `####`) | Markdown rendering breaks the 2.26 section's outline; visual review catches this on `cargo doc` or the GitHub preview. Stage 4 reviewer renders the file. |
| QA-0392-E | Subsection placed outside the 2.26 section (e.g., appended at file end) | Misplaces narrative; future readers of 2.26 miss the amendment context. Stage 4 reviewer reads the section structure. |
| QA-0392-F | TASK-0392 author also edits `SPEC-01-invariantes.md` or `SPEC-19-delta-protocol.md` to "back-reference" the ROADMAP note | Out-of-scope spec edit. Forbidden by AMB-D-3 ruling and by the task-splitter agent's territory. Stage 4 reviewer runs `git diff specs/` and asserts no changes. |
| QA-0392-G | TASK-0392 author also adds a Rust `#[test] #[ignore]` stub for ARG-005 | Out of scope per this TEST-SPEC's "Why no `#[ignore]` stub" decision. Stage 4 reviewer asserts test count delta is exactly 0. |
| QA-0392-H | Narrative duplicates SPEC-19 §3.5 prose verbatim instead of cross-referencing | Anti-pattern (per task spec §Notes "narrative notes do NOT restate the spec"). Reviewer reads the paragraph for length — > 200 words per amendment is a smell. |
| QA-0392-I | "Progress guarantee" sentence in R40 misstates the bound (e.g., says "≤ 1 round strict") | Misrepresents R40; reviewer cross-checks against SPEC-19 §3.5 R40 fourth bullet. |

---

## Acceptance gate

This task has NO `cargo test` increase obligation (test count delta = 0). The acceptance gate is mechanical Markdown / cross-reference checks:

1. `cargo test --workspace --lib` count: 975 → **975** (UNCHANGED). Hard floor preserved (test count MUST NOT decrease).
2. `cargo test --workspace --lib --features zero-copy` count: 1015 → **1015** (UNCHANGED).
3. `cargo build --workspace` clean (no source changes — should be a no-op build after `git pull`).
4. `cargo clippy --workspace --all-targets -- -D warnings` clean (no source changes).
5. `cargo fmt --check` clean (no Rust changes).
6. `git diff --stat relativist-core/src/` — empty (no source changes).
7. `git diff --stat specs/` — empty (no spec changes).
8. `git diff --stat docs/ROADMAP.md` — non-empty; the diff adds ~50 lines under the 2.26 section.
9. **Markdown render check:** open `docs/ROADMAP.md` in the GitHub preview (or `mdcat` / `glow` locally); verify:
   - The new `####` heading nests correctly under the existing 2.26 `###`.
   - All bold markers (`**PROOF PENDING**`, `**Status: PARTIAL after 2.26-D, COMPLETE after 2.26-C**`) render.
   - No broken Markdown (no half-open code fences, no malformed lists).
10. **Per-paragraph grep checks (the AMB-D-3 enforcement layer):**
    - G1 paragraph (DOC-0392-02): contains `**PROOF PENDING**` AND contains "Section 8" (or "ARG-005").
    - D3 paragraph (DOC-0392-03): contains `**PROOF PENDING**` AND contains "Section 8".
    - D6 paragraph (DOC-0392-04): contains `**PARTIAL after 2.26-D, COMPLETE after 2.26-C**` AND contains "no Section 8 deferral" AND DOES NOT contain "PROOF PENDING".
    - Framing paragraph (DOC-0392-01): contains "No SPEC-01 edit is required".
    - Tail paragraph (DOC-0392-05): cross-references TASK-0389, TASK-0390, R42.

---

## Out of scope (deferred to later TEST-SPECs in the bundle or future bundles)

- Runtime tests of `GridConfig.delta_mode` / `--delta-mode` / R42 smoke regression → TEST-SPEC-0389/0390/0391.
- `delta_mode` docstring polish + `GridConfig` doctest → TEST-SPEC-0393.
- 2.26-C delta BSP loop convergence test (operational discharge of R40) → sub-bundle 2.26-C TEST-SPEC.
- 2.26-C `BorderGraph.is_empty() && all_workers_idle` joint termination check → sub-bundle 2.26-C.
- ARG-005 formal proof of G1 recoverability (R38) → TCC §8 work stream, `discussoes/argumentos/ARG-005-*.md` (not a Relativist artefact).
- DISC-011 ping-pong of OQ-1 → TCC discussion stream (not a Relativist artefact).
- Spec edits to SPEC-01 or SPEC-19 — out of scope (specs are read-only for task-splitter and downstream agents; SPEC-19 §3.5 is the canonical amendment location).
