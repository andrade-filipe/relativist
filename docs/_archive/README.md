# `docs/_archive/` — Frozen historical record (read-only)

This directory preserves the **Spec-Driven Development (SDD)** process artifacts produced while
Relativist was built (roughly late-2025 → mid-2026). It is kept for provenance and honesty, not
for active use.

> **Status: READ-ONLY.** Nothing here describes the current workflow. Do not edit these files and
> do not treat them as authoritative for how the project works today. The active workflow is
> **RPI (Research → Plan → Implement)** — see [`../../CONTRIBUTING.md`](../../CONTRIBUTING.md) and
> [`../../.claude/agents/README.md`](../../.claude/agents/README.md).

## Why this exists

Relativist was developed with a heavyweight SDD + TDD pipeline (task-splitter → test-generator →
developer → reviewer → qa → refactor, with adversarial spec review). That pipeline produced a
large volume of per-bundle documentation. It served its purpose — the implementation is correct
and the article that depends on it was graded 10 — but it is too token-heavy and too noisy to
carry forward as live documentation. The project has since adopted RPI, where a disposable plan is
generated per task from a fresh scan of code + curated docs, then discarded after the docs are
updated.

Rather than delete the SDD history (which would erase the real story of how the system was built),
it is frozen here.

## What's inside

| Path | What it held |
|------|--------------|
| `backlog/` | Atomic task definitions per bundle (`TASK-NNNN-*.md`) + `BACKLOG.md` |
| `tests/` | Pre-implementation test specifications (`TEST-SPEC-*.md`) |
| `reviews/` | Code-review reports per bundle |
| `spec-reviews/` | Adversarial spec-review rounds (critic + defender) |
| `plans/` | Per-bundle implementation plans |
| `handoffs/` | Per-bundle dispatch handoffs |
| `briefings/` | Per-bundle research briefings |
| `qa/` | Adversarial QA bug reports per bundle |
| `sdd-agents/` | The 12 retired SDD Claude Code agent definitions (sdd-pipeline, developer, reviewer, qa, …) |
| `next-steps.md` | SDD pipeline state (superseded by RPI; future work now in [`../reference/next-steps.md`](../reference/next-steps.md)) |
| `progress.md` | SDD implementation history / shipped-bundle narrative |
| `WORKFLOWS.md` | The old unified 6-stage SDD + spec-review + git workflow doc |

## A note on links

Internal links and file paths inside these archived documents reflect the **pre-reorganization
repository layout** (e.g. `results/locked/...`, `scripts/...` at the repo root). After the
open-source launch reorganization (2026-06), frozen benchmark evidence and reproduction scripts
moved under [`../../reproduce_article/`](../../reproduce_article/). Those archived links are left
untouched on purpose — rewriting frozen history would defeat the point. If you follow an old link
and it 404s, check `reproduce_article/` for the relocated artifact.
