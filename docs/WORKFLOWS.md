# Relativist — Unified Workflows

This document centralizes all development, review, and version control processes for the Relativist project.

---

## 1. Development Pipeline (SDD)

Relativist follows a strict **Spec-Driven, TDD** development pipeline. Each spec goes through the same **6-stage pipeline**. No stage can be skipped. The pipeline ensures clean context, precise tasks, comprehensive tests, and unified code review.

The `sdd-pipeline` orchestrator agent tracks the current state in `docs/next-steps.md` and tells you exactly which agent to invoke next. **Always invoke `sdd-pipeline` first** to know where you are and what to do.

### Pipeline Stages

```
┌──────────────────────────────────────────────────────────────────┐
│                   PER-SPEC IMPLEMENTATION CYCLE                   │
│                                                                    │
│  Stage 1          Stage 2          Stage 3          Stage 4       │
│ ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐    │
│ │   TASK   │───▶│   TEST   │───▶│   DEV    │───▶│ REVIEWER │    │
│ │ SPLITTER │    │GENERATOR │    │(TDD impl)│    │(quality +│    │
│ └──────────┘    └──────────┘    └──────────┘    │  arch)   │    │
│                                                  └──────────┘    │
│                                                       │           │
│  Stage 6                           Stage 5            │           │
│ ┌──────────┐                     ┌──────────┐         │           │
│ │   DEV    │◀────────────────────│    QA    │◀────────┘           │
│ │(refactor)│                     │(bug hunt)│                     │
│ └──────────┘                     └──────────┘                     │
│      │                                                            │
│      ▼                                                            │
│ ┌──────────┐                                                      │
│ │  TESTS   │ ── All green? ── YES ──▶ DONE ✓                    │
│ │  PASS?   │                                                      │
│ └──────────┘ ── NO ──▶ Fix and re-run                            │
└──────────────────────────────────────────────────────────────────┘
```

#### Skip Rules (v2)

- If REVIEW produces **0 Must-Fix** and architecture verdict is ALIGNED → auto-advance to QA
- If QA returns **CLEAN** (0 CRITICAL, 0 HIGH) → skip REFACTOR, go directly to DONE
- Tasks classified as **S** (small, <50 LoC) → reviewer uses single-pass lighter review

#### Stage Details

- **Stage 1: Task Splitter (PO)** - Breaks specs into atomic tasks (< 200 LoC).
- **Stage 2: Test Generator** - Designs unit and property tests based on task criteria.
- **Stage 3: Developer (TDD Implementation)** - Implements tests first, then production code.
- **Stage 4: Unified Review** - Code quality and architectural alignment check.
- **Stage 5: QA Review (Bug Hunt)** - Hunting for panics, logic errors, and concurrency issues.
- **Stage 6: Developer Refactoring** - Addresses all Must-Fix items from reviewer and QA.

---

## 2. Spec Review Pipeline

Adversarial review process for Relativist specifications. Each spec undergoes a structured 3-stage review before implementation begins.

**Goal:** Catch consistency issues and डिजाइन flaws BEFORE implementation.

### Spec Review Stages

1.  **Stage 1: Spec Critic (Adversarial)** - Reviews for consistency, testability, completeness, and invariant preservation.
2.  **Stage 2: Defender Response** - `especialista-specs` agent addresses critic findings (ACCEPTED, PARTIALLY ACCEPTED, or NOT ADDRESSED).
3.  **Stage 3: Task Updater** - Re-aligns the backlog in `docs/backlog/` with the revised spec.
4.  **Stage 4: Human Approval** - Final sign-off by the user.

---

## 3. Git Workflow

Relativist uses a simplified **GitHub Flow** model.

### Branch Strategy

- **`main`** — production branch. Always stable, compiles, and passes CI.
- **Feature branches** — one per backlog task: `feat/TASK-XXXX-short-description`.
- **Fix branches** — for bug fixes found during review: `fix/TASK-XXXX-description`.

### Workflow Per Task

1.  Create branch from `main`: `git checkout -b feat/TASK-XXXX-description`.
2.  Implement following the 6-stage SDD pipeline.
3.  Push and open PR against `main`. CI must pass (fmt, clippy, test, build).
4.  Merge via **squash-merge** (one commit per task on main) and delete feature branch.

### Commit Convention (Conventional Commits)

Format: `<type>(<scope>): <description> (TASK-XXXX)`
Example: `feat(net): add Symbol and Agent types (TASK-0001)`

---

## 4. Archiving Rules (v2)

To maintain a clean context window for LLMs and avoid token waste:
- **Per-Directory Archives:** Completed tasks, tests, and reviews MUST be moved to their local `archive/` subdirectories (e.g., `docs/backlog/archive/`) immediately after the bundle ships.
- **Next-Steps Discipline:** Active planning lives in `next-steps.md`. Historical summaries live in `progress.md`.
- **Ignore Rules:** All `archive/` and historical benchmark folders are ignored by LLM indexing tools.
