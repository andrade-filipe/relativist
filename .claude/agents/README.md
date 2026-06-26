# Relativist agents — the RPI workflow

Relativist is developed with **RPI: Research → Plan → Implement**. It replaced the heavier
Spec-Driven Development (SDD) pipeline used during v1/early-v2 (those 12 SDD agents are frozen,
read-only, under [`../../docs/_archive/sdd-agents/`](../../docs/_archive/sdd-agents/)).

RPI exists to keep the model's attention high and its context small. Each phase runs in a **fresh
context** and hands off to the next through a file on disk — so no phase carries the previous
phase's noise.

```
        goal
          │
          ▼
   ┌──────────────┐  writes   docs/rpi/RESEARCH.md
   │  researcher  │ ───────────────────────────────┐
   └──────────────┘   (read-only: map code + docs)  │
          ⟲ fresh context                            ▼
   ┌──────────────┐  reads RESEARCH.md, writes docs/rpi/PLAN.md
   │   planner    │ ───────────────────────────────┐
   └──────────────┘   (read-only: surgical plan)    │
          ⟲ fresh context                            ▼
   ┌──────────────┐  reads PLAN.md, edits code + tests, verifies
   │ implementer  │ ───────────────────────────────► green build
   └──────────────┘   (the only agent that writes code)
          │
          ▼
   update the living docs (specs/reference/ROADMAP as needed) → repeat
```

## The three agents

| Agent | Phase | Writes | Reads |
|-------|-------|--------|-------|
| [`researcher`](researcher.md) | Research | `docs/rpi/RESEARCH.md` | code, `specs/`, `docs/`, `docs/_archive/` |
| [`planner`](planner.md) | Plan | `docs/rpi/PLAN.md` | `docs/rpi/RESEARCH.md` |
| [`implementer`](implementer.md) | Implement | `src/`, `tests/` | `docs/rpi/PLAN.md`, `CODING_STANDARDS.md` |

## How to run a cycle

1. **Research** — invoke `researcher` with the high-level goal. It writes `docs/rpi/RESEARCH.md`.
2. **Reset context**, then invoke `planner`. It reads the research and writes `docs/rpi/PLAN.md`.
3. **Reset context**, then invoke `implementer`. It executes the plan and runs the full
   verification suite (`cargo test` / `clippy` / `fmt`, plus the `reduce_all ≅ run_grid` invariant
   where relevant).
4. **Update the living docs** for anything that changed (a spec invariant, a reference page, the
   ROADMAP), then start the next cycle.

`docs/rpi/RESEARCH.md` and `docs/rpi/PLAN.md` are **disposable** working artifacts — they are
gitignored and overwritten every cycle. They capture the current task, not project history.

## Why fresh contexts matter

A long single conversation degrades: rules get skipped, earlier mistakes get anchored on. By
serializing into three short, single-purpose contexts joined only by a written artifact, each
phase reasons cleanly over exactly what it needs — and the disposable plan keeps the repo's
permanent documentation lean.

## Companion harness

- [`../skills/`](../skills/) — reusable skills (license auditing, git guardrails, pre-commit,
  documentation, threat modeling) usable from any phase.
- [`../open-source-readiness/`](../open-source-readiness/) — the audit pack used to bring this
  repo to open-source launch quality.
- [`../rpi-pipeline/`](../rpi-pipeline/) — the upstream RPI pack these repo-native agents are
  adapted from.
