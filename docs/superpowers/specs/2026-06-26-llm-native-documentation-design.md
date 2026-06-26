# Design — LLM-Native Documentation for Relativist

**Date:** 2026-06-26
**Branch:** `docs/llm-native-documentation`
**Status:** Approved (decisions below) — implementation in progress

---

## 1. Problem & goal

Relativist's documentation works for humans but is not optimized for the way it is now used:
an **LLM-driven RPI workflow** (Research → Plan → Implement) that needs to extract *exactly the
right context, fast*. Today the docs are sprawling (≈70 live files + 1,109 archived), mixed
PT-BR/English, redundant across clusters, and the formal `specs/` + `reproduce_article/` sit at
the repo root cluttering first contact.

**Goal:** a single coherent, English, **catalogued** documentation system where:
- every doc is compact, precise, non-redundant, and keyword-searchable;
- a machine-readable **catalog** lets an LLM (or `grep`) find the right doc from a keyword;
- the **code is connected to the docs** (module → spec → doc), so an agent reading `src/` lands
  on the canonical explanation;
- the living source of truth is the **code**, then the **specs**, then the archived history.

## 2. Decisions (from author)

| # | Decision | Choice |
|---|----------|--------|
| 1 | Placement | Move `specs/` → `docs/specs/`. **`reproduce_article/` stays at root** (runnable scripts + 500 MB data, wired into 5 test path-guards). |
| 2 | Language | **English canonical** for the new docs. PT-BR originals preserved in `docs/_archive/`. |
| 3 | Shape | **Catalogued structured set** — small focused docs, each with YAML frontmatter, fronted by ONE machine-readable catalog (`docs/README.md`). Not a monolith. |
| 4 | Code↔doc depth | **Light** — module→doc catalog + rustdoc pointers + fix the 8 stale source refs. Keep `SPEC-NN` IDs (1,634 refs untouched). |

## 3. The cataloging technique (the core deliverable)

**Per-doc YAML frontmatter** — every live doc starts with:

```yaml
---
title: Reduction engine
summary: The six interaction rules, dispatch, and the reduce_all/reduce_n loop.
keywords: [reduction, interaction rules, annihilation, commutation, erasure, redex, reduce_all]
modules: [reduction]          # code modules this doc explains (the code↔doc link)
specs: [SPEC-03, SPEC-01]     # related formal specs
audience: [contributor, llm]
status: reference             # reference | guide | draft
updated: 2026-06-26
---
```

**The catalog — `docs/README.md`** — the single entry point ("documentação única"). It is a
generated-style table of every live doc: `title · summary · keywords · path`, grouped by
category, plus two cross-indexes: **by module** (find the doc for `partition/`) and **by keyword**.
A keyword `grep docs/README.md` lands the reader on the exact file. This is what makes retrieval
fast for both LLMs and humans.

**Stable anchors & naming** — predictable `##`/`###` headings and kebab-case filenames so deep
links and keyword scoping are reliable.

## 4. Target structure

Familiar names kept; two new top-level concept dirs added. English unless noted.

```
docs/
├── README.md                 # THE CATALOG + cross-indexes (entry point; replaces INDEX.md)
├── specs/                    # moved from root — 28 formal specs (reference-grade, keep IDs) + specs/README.md index
├── architecture/
│   ├── overview.md           # BSP model, layers, dependency direction, coordinator/worker FSMs (SPEC-13 + PESQ-024)
│   └── modules.md            # module → spec → doc table (THE code↔doc bridge; 14 modules + 13 CLI subcommands)
├── theory/
│   ├── interaction-combinators.md  # 3 symbols, 6 rules, strong confluence (SPEC-00 + SPEC-03)
│   └── invariants.md         # T1–T7, D1–D6, I1–I5, G1 — the correctness contract (SPEC-01; English rewrite of reference/invariants)
├── guides/                   # compact ENGLISH how-to (translated/condensed from the 11 PT-BR guides)
├── reference/
│   ├── cli.md                # all 13 subcommands + flags (English, authored from code)
│   ├── file-formats.md       # .bin / .ic / .json (English)
│   ├── invariants.md         # -> redirect/merged into theory/invariants.md
│   ├── troubleshooting.md    # compact English
│   └── next-steps.md         # keep (already English)
├── benchmarks/               # keep methodology + limitations + break-even; archive historical findings
├── operations/
│   ├── docker.md             # from DOCKER.md
│   └── security-observability.md  # tiers + tracing/metrics (compact, from SPEC-10/11)
├── roadmap.md                # from roadmap.md (good as-is; light touch)
├── theory-bridge.md          # keep (TCC link)
└── _archive/                 # + research surveys, historical_v1, superseded campaigns, design docs
```

## 5. Disposition (from the live-docs audit)

- **KEEP/REWRITE→English (CORE):** guides (11, translate+compact), reference/{cli,file-formats,
  troubleshooting}, benchmarks/{README,limitations,phase-1/2/3,DATA-COLLECTION-PLAN,campaigns/
  {v1-local-baseline,stress-curve}}, analysis/D011, demos (keep PT-BR, add English summaries to
  catalog), ROADMAP, theory-bridge, next-steps. NEW: architecture/*, theory/*.
- **ARCHIVE → `docs/_archive/`:** `pesquisa/` PESQ-001..022 (landscape/framework/pattern/obs/
  security/testing surveys — not Relativist-specific); keep PESQ-023/024 (synthesis) as source
  for `architecture/overview.md` then archive the raw notes; `benchmarks/historical_v1/*`;
  `benchmark-relevance-analysis.md`; `campaigns/{v1-stress,church-sum-of-squares}`; `pipeline.md`
  (covered by guides); `superpowers/specs/` design docs for unshipped features (keep the
  open-source-launch + this design; keep horner-method-explainer as theory source then archive).
- **Principle:** a doc stays live only if a *current user, contributor, or LLM doing RPI* needs
  it. Research provenance and process history go to `_archive` (already the convention).

## 6. Code ↔ doc connection (light)

- **Fix the 8 stale source refs** the code map found:
  - `docs/next-steps.md` → `docs/_archive/next-steps.md` (the BLOCKER-bearing historical file) in
    `error.rs:137`, `partition/helpers.rs:448,483`, `partition/streaming.rs:811`,
    `protocol/coordinator.rs:1312,1383`.
  - `merge/grid.rs:846` — reword the `task-splitter` (retired SDD agent) comment.
  - `bench/suite.rs:493` — `scripts/bench_docker_v2.sh` → `reproduce_article/scripts/...`.
- **`docs/architecture/modules.md`** is the bridge: a table mapping each of the 14 modules + 13
  CLI subcommands → its SPEC-NN → its canonical doc. From any module name an agent finds the doc.
- **`lib.rs` / `main.rs` `//!`**: add one line pointing to `docs/architecture/modules.md` as the
  map. No mass rustdoc rewrite (the 98 module `//!` blocks already exist and stay).

## 7. Tooling (agents/skills)

After surveying community examples via `/npx-skills-search` and `/npx-skillfish-search`, create a
small committed set under `.claude/` that helps keep docs LLM-grade:
- a **`doc-curator`** skill — the standard for writing/maintaining a Relativist doc: frontmatter
  schema, compactness rules (no redundancy, keyword-first, stable anchors), and the rule that the
  catalog must be updated when a doc is added/moved.
- a **`doc-catalog`** agent (or skill) — regenerates/validates `docs/README.md` from frontmatter
  and flags docs missing frontmatter or absent from the catalog.
These plug into the RPI loop's "update the living docs" step.

## 8. Phases (execution order)

1. **Reorg:** `git mv specs → docs/specs`; fix 28 md `specs/` link refs + README/CLAUDE/
   CONTRIBUTING/INDEX; fix the 8 code stale refs. Verify build+test green.
2. **Archive sweep:** `git mv` the ARCHIVE-classified docs into `docs/_archive/`.
3. **Author core (English, frontmatter):** architecture/{overview,modules}, theory/{interaction-
   combinators,invariants}, reference/{cli,file-formats} rewrite, the catalog `docs/README.md`.
4. **Translate+compact guides** to English (parallelizable; each guide self-contained).
5. **Frontmatter pass:** add the YAML schema to every surviving live doc; build the catalog +
   cross-indexes.
6. **Tooling:** npx searches → `doc-curator` skill + `doc-catalog` agent under `.claude/`.
7. **Verify:** `cargo build`/`test` green; link sweep (live→live, live→archive, specs paths);
   catalog completeness (every live doc present + has frontmatter); update CLAUDE.md/CONTRIBUTING
   pointers; final commits.

## 9. Non-goals / risks

- **No code behavior change** beyond fixing 8 stale comment refs (tests stay green; comments don't
  affect tests, but I'll run the suite — last time a path-guard test caught a move).
- **No `reproduce_article/` move** (decision #1) — avoids re-breaking the 5 path-guards.
- **Risk:** translating 11 guides is the heavy part — parallelize with subagents, keep each
  compact; the catalog + architecture/theory core is authored centrally for coherence.
- **Risk:** moving `specs/` shifts relative links — same mechanical sweep discipline as the prior
  reorg (token-level, verify with grep, leave `docs/_archive/**` frozen).

## 9b. Grill refinements (resolved before execution)

- **Guides consolidate 11 → ~6:** v1 trail (getting-started, first-reduction, local-grid,
  distributed-tcp, church-arithmetic) + **one** `guides/v2-features.md` (delta, zero-copy,
  elastic, streaming, arena — one section each). Removes the 5-way v2 redundancy.
- **Single invariants doc:** `theory/invariants.md` canonical; `theory/invariants.md` dropped
  (links redirected).
- **Specs catalogued via `docs/specs/README.md` index** (id→title→module→status→keywords); no
  per-file frontmatter on the 28 specs (they already carry IDs + formal headers).
- **`docs/README.md` → `docs/README.md`** (the catalog; GitHub renders it for the `docs/` dir).
- **Demos:** translate `horner-g1-demonstration` → English (it demonstrates G1); archive
  `live_demo` + `horner_runbook` (PT-BR presentation logistics).
- **Frontmatter applies to the live narrative docs** (architecture, theory, guides, reference,
  benchmarks, operations, roadmap, theory-bridge), not to specs or archived files.

## 10. Verification

- `cargo build --release` + `cargo test` green (full suite, not filtered).
- `grep` sweeps: no live doc links into moved/empty paths; `specs/` path refs updated; 8 code refs
  fixed; no `reproduce_article/reproduce_article` style double-prefix.
- Catalog completeness: every file under `docs/` (excl. `_archive`) appears in `docs/README.md`
  and carries frontmatter.
- Language: no PT-BR left in the CORE set except intentionally-kept demos (flagged in catalog).
