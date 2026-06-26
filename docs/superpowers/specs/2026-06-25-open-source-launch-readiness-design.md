# Design — Open-Source Launch Readiness for Relativist

**Date:** 2026-06-25
**Branch:** `chore/open-source-launch-readiness`
**Author:** Filipe Andrade Nascimento (with Claude Code, autonomous run)
**Status:** Approved (decisions captured below) — implementation in progress

---

## 1. Problem

The TCC article was graded 10 and publicly promises that Relativist is open-source. Filipe
will announce it on social media; strangers will then read, clone, fork, and try to reproduce
or extend it. The repository — after months of Spec-Driven Development (SDD) — is not in a
state that survives that scrutiny:

- **Heavy git tree:** 3,283 tracked files; `results/` alone is 1,873 files including ~500 MB+
  of binary `.bin` baseline outputs (89 MB files × 3 baseline snapshots) committed to history.
- **Documentation sprawl:** ~1,100 docs files, overwhelmingly SDD process artifacts
  (`backlog/archive` 425, `tests/archive` 402, `reviews/archive` 85, `spec-reviews/archive` 69…).
  Useful to the author's history; noise to a newcomer or to an LLM doing focused work.
- **Workflow shift:** SDD is being retired (too token-heavy, too much generated documentation).
  The new workflow is **RPI (Research → Plan → Implement)**, delivered as an agent pack in
  `.claude/rpi-pipeline/`. Documentation must be redesigned around RPI's context model.
- **Reproducibility vs. evolution tension:** `scripts/` and `results/` sit at the root. They
  must stay reproducible (the article depends on them) while the codebase keeps moving without
  tripping over frozen artifacts.
- **Open-source governance gaps:** no `SECURITY.md`, `CODE_OF_CONDUCT.md`, `GOVERNANCE.md`, or
  `NOTICE`; license is MIT (author worried about being "robbed"); CONTRIBUTING still teaches the
  retired SDD pipeline.

## 2. Goals / Non-Goals

**Goals**
1. Bring order: a clean, honest, browsable repository where SDD history is preserved but
   quarantined, and the live surface is lean.
2. Adopt RPI as the committed, standardized workflow + coding standards, retiring SDD agents.
3. Make article reproducibility explicit and self-contained in one reserved location.
4. Make "next steps" for the software explicit for curious readers / would-be contributors.
5. Resolve the license question (→ Apache-2.0) and fill governance gaps.
6. Produce a guide for the GitHub-side configuration the agent cannot perform from here.

**Non-Goals**
- No new experiments or benchmark runs; only already-frozen data is touched.
- No source-code behavior changes to `relativist-core` / `relativist-cli` (tests stay green;
  this is a hygiene + docs + governance + harness change, not a feature change).
- No git history rewrite (decided: safe reorganization only — see §3.2).
- No `git push` / PR creation in this run (outward-facing; left to the author with a guide).

## 3. Decisions (from author)

| # | Decision | Choice |
|---|----------|--------|
| 1 | License | **Apache-2.0** (permissive + explicit patent grant & termination — protects author and contributors; still OSI-open so the article's claim holds). |
| 2 | Git history of big binaries | **Safe: reorganize, no rewrite.** Use `git mv` (rename reuses blobs — no new bloat) to relocate; `.gitignore` future regenerable outputs. No `filter-repo`, no force-push. |
| 3 | SDD artifact disposition | **In-repo `docs/_archive/`, read-only**, with a README marking it frozen/historical. Specs treated as curated reference (kept prominent — variant of option 3). |
| 4 | Harness scope | **Full:** author repo-native RPI agents, a `CODING_STANDARDS` doc, retire SDD agents to archive, and apply the open-source-readiness findings directly (the author's plan-approval is the human gate). |

## 4. Approach

Five workstreams, executed in dependency order. Each ends green (`cargo test`, `cargo clippy`,
`cargo fmt --check` unaffected; doc-only and move-only changes verified by build).

### 4.1 Repository hygiene & structure (the "bring order" core)

- **Stray root files:** remove build/junk that should never have been tracked or left around:
  `texput.log` (LaTeX log — already untracked, delete), `prompt.txt` (this task's prompt —
  untracked, delete at end). Confirm `.env` stays untracked (it is). `USAGE_GUIDE.md` is a
  legacy redirect stub → fold into docs and remove or keep as a one-line pointer.
- **`docs/_archive/`:** move SDD process directories under it, preserving structure:
  `backlog/`, `tests/`, `reviews/`, `spec-reviews/`, `plans/`, `handoffs/`, `briefings/`,
  `qa/`, plus `next-steps.md`, `progress.md`, `WORKFLOWS.md` (SDD pipeline doc). Add
  `docs/_archive/README.md` explaining provenance and read-only status. Use `git mv` so history
  follows the files and no blobs duplicate.
- **Keep live & curate:** `docs/guides/`, `docs/reference/`, `docs/benchmarks/`,
  `docs/analysis/`, `docs/pesquisa/` (research notes — valuable to readers), `roadmap.md`,
  `INDEX.md` (rewritten). `specs/` stays top-level as first-class design reference.
- **`.gitignore` hardening:** ensure regenerable outputs (`data/`, `logs/`, `target/`,
  `*.log`, `__pycache__/`, `results/*.bak`, `results/*.before_*`, `results/*.pre_*`) are
  ignored going forward. Remove the stale `results/*.bak`/`.before_*`/`.pre_*` working copies
  from tracking (they are editor/run backups, not evidence).

### 4.2 Article reproducibility — `reproduce_article/`

Adopt the author's proposed reserved folder (named **`reproduce_article/`**), clearly existing
"only because of the article". **Grilled decision:** the physical move is worth doing — blast
radius measured at ~18 live doc links + 14 script-internal path refs, all mechanical and
grep-verifiable. Contents:

- `reproduce_article/README.md` — what the article claims, which tables/figures map to which
  data, exact commands, environment notes, and the reproducibility table (env, toolchain,
  seeds, expected checksums).
- `reproduce_article/scripts/` — `git mv` of the locked/reproduction scripts from `scripts/`
  (`reproduce_local_baseline.sh`, `bench_phase*_locked.sh`, `bench_docker*.sh`, `horner_*`,
  `stress_curve.sh`, `plot_stress_curve.py`, `cv_triage.py`, `requirements-stress-curve.txt`).
  Each script is made **location-robust** (resolve repo root via `git rev-parse --show-toplevel`
  / `BASH_SOURCE`, then operate on root-relative paths) so the 14 internal `results/` / `data/`
  references keep working. `install.sh` stays as the one dev-facing script (root or `scripts/`).
- `reproduce_article/results/` — `git mv` of `results/locked/` (the frozen, checksummed
  evidence). The ~18 **live** doc references (README, INDEX, `benchmarks/*`, `analysis/*`,
  `guides/09–10`, `reference/file-formats`, `ROADMAP`) are updated to the new path. References
  inside `docs/_archive/**` are intentionally left as-is (frozen history; archive README states
  paths reflect the pre-reorg layout).
- The **60 loose junk files** at `results/` root (`*.bak`, `*.before_*`, `*.pre_*`, superseded
  loose CSVs) are dropped from tracking — not moved. Only frozen evidence migrates.
- A prominent top-level pointer in README so the path is discoverable.

`git mv` keeps blobs identical (no clone-size increase). The large `.bin` files remain in
history (decision #2) but now live under a clearly reserved, documented path. After the move,
`results/` at root no longer exists; new run outputs are gitignored (§4.1), so future code
evolution never re-clutters the root.

### 4.3 Workflow & harness standardization (RPI + coding standards)

- **Commit `.claude/` packs** already staged (`rpi-pipeline/`, `open-source-readiness/`,
  `skills/`) — these are the standardized, version-controlled harness the author wants tracked.
- **Repo-native RPI agents:** author `.claude/agents/researcher.md`, `planner.md`,
  `implementer.md` adapted from the pack (concrete Claude Code tool names; drop dangling
  external-"framework constant" references that don't resolve in this repo). Add a short
  `.claude/agents/README.md` describing the RPI loop and the Research→Plan→Implement handoff
  files (`RESEARCH.md` → `PLAN.md` → verified implementation, disposable per cycle).
- **`CODING_STANDARDS.md`** (root or `docs/`): distills the existing `relativist/CLAUDE.md`
  coding rules (no `unwrap()` in prod, no `unsafe` without `// SAFETY:`, `tracing` not
  `println!`, `thiserror`, newtype IDs, module dependency direction, all-green gate) into a
  contributor-facing standard, decoupled from SDD.
- **Update `relativist/CLAUDE.md`** to describe RPI as the active workflow and point SDD to the
  archive (without deleting the frozen historical context).

### 4.4 Open-source governance & docs (readiness findings → remediation)

Applying the four open-source-readiness dimensions directly:

- **Dim 1 — Docs:** rewrite `README.md` license/section pointers; rewrite `CONTRIBUTING.md`
  around RPI (remove the 6-stage SDD section); rewrite `docs/README.md` for the new layout; add
  `CODE_OF_CONDUCT.md` (Contributor Covenant 2.1) and `GOVERNANCE.md` (BDFL-style, honest:
  single maintainer + academic origin).
- **Dim 2 — License:** replace MIT `LICENCE` with Apache-2.0 `LICENSE` (+ keep a note in
  CHANGELOG about the relicense; author is sole copyright holder so relicensing is clean), add
  `NOTICE`, update all `License: MIT` references (README, Cargo.toml of both crates,
  CONTRIBUTING). Add SPDX headers policy to CODING_STANDARDS (lightweight; not mass-applied to
  every file in this pass unless cheap).
- **Dim 3 — Secrets & git hygiene:** confirm no secrets in tree/`.env.example` (verified clean);
  `.gitignore` hardening (4.1); document that `.env` is local-only. No history rewrite, so no
  secret-purge needed (none found).
- **Dim 4 — Security & CI:** add `SECURITY.md` (private vulnerability reporting via GitHub
  advisories + email); confirm issue/PR templates exist (they do) and align them with RPI; note
  `cargo audit`/Dependabot as a recommended CI addition in the GitHub guide.

### 4.5 Next-steps visibility

- Add `docs/reference/next-steps.md` (or `ROADMAP` "What's next" section) distilled from
  `roadmap.md` §2.40 break-even analysis and the article's future work: the headline being the
  **clean negative result** (c_o/c_r = 2.2, needs < 0.5) and the concrete path to break-even
  (delta protocol §2.26 + coordinator-free round §2.34 + delta merge §2.35), plus Phase 3 LAN
  as the next empirical milestone. Written for a curious outsider who might want to help.

### 4.6 GitHub configuration guide (point 6)

A `docs/MAINTAINER-GITHUB-SETUP.md` (and a copy surfaced in the final chat summary) covering
everything the agent cannot do from the CLI without pushing: branch protection on `main`,
required status checks, enabling Issues/Discussions, repository topics/description/social
preview, private security advisories + "Report a vulnerability", Dependabot/`cargo audit`,
release/tag settings, "About" sidebar, and the announce checklist.

## 5. Risks & Mitigations

- **`git mv` of 1,800+ files producing a noisy diff** → group moves into a few logical commits
  with clear messages; the rename detection keeps history intact.
- **Broken internal doc links after moves** → after restructuring, grep for links into moved
  paths (`docs/backlog`, `docs/tests`, `next-steps.md`, `LICENCE`, `USAGE_GUIDE.md`) and fix or
  redirect. README/INDEX rewritten wholesale.
- **Relicense correctness** → author is the sole copyright holder (LICENCE header confirms);
  Apache-2.0 relicense is legally clean. Record it in CHANGELOG.
- **Cargo metadata** → update `license = "Apache-2.0"` in both crate `Cargo.toml`s so
  crates.io/`cargo` metadata is consistent; run `cargo build` to confirm no breakage.
- **Tests** → no source changes; run `cargo test` once at the end to confirm the floor holds.

## 6. Verification

- `cargo build --release` and `cargo test` succeed (no source touched; sanity only).
- `cargo fmt --check` and `cargo clippy` unaffected.
- `git status` clean of stray junk; `git ls-files | wc -l` materially reduced (backups dropped).
- All moved-doc links resolve (grep sweep clean).
- New governance files present and internally consistent (license name matches everywhere).
- `reproduce_article/README.md` commands are accurate against the moved scripts/results.

## 7. Out of scope / deferred

- Git history rewrite / LFS migration (decision #2: not now).
- Mass SPDX header insertion across all source files (policy documented; bulk apply deferred).
- Actually running the multi-agent readiness audit as separate gated subagents (folded into
  this direct remediation, since the author pre-approved the plan).
- Pushing the branch / opening the PR / GitHub-side settings (author does this with the guide).
