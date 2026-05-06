# Relativist Implementation Backlog

**Last updated:** 2026-05-05 (post-cleanup: all closed-bundle TASKs archived; active queue is empty).

**Status:** ZERO active TASKs. The full inventory of D-005..D-012 atomic tasks (TASK-0001..TASK-0618 with intentional gaps) is preserved at `archive/`. The next bundle (D-013) will repopulate this file once `task-splitter` runs against the inventory in `docs/next-steps.md`.

**Pipeline:** See `../WORKFLOWS.md` (§1 Development Pipeline) for the 6-stage SDD process.

---

## Active

| ID | Title | Priority | Status | Depends | Complexity | Bundle |
|----|-------|----------|--------|---------|------------|--------|
| _(none)_ | | | | | | |

When a new bundle (D-013+) opens, `task-splitter` populates this section with atomic tasks (~<200 LoC each); when the bundle closes those entries move to `archive/`.

---

## Cumulative bundles delivered (per `progress.md`)

| Bundle | TASKs | Tasks archive | Closure narrative |
|--------|-------|---------------|--------------------|
| Phase 1..11 (v1) | TASK-0001..TASK-0399 (~270 done) | `archive/` | `progress.md` "Local Benchmark Phase" |
| D-005 | TASK-0400..0403 (4) | `archive/` | `progress.md` D-005 entry |
| D-006 (SPEC-20 elastic, Option A) | TASK-0410..0455 (~46) | `archive/` | `progress.md` D-006 entry |
| D-009 (SPEC-22 arena) | TASK-0460..0500 (~36) | `archive/` | `progress.md` D-009 entry |
| D-010 (SPEC-21 streaming) | TASK-0510..0591 (~40) | `archive/` | `progress.md` D-010 entry |
| D-011 (BLOCKER perf regression) | TASK-0595..0614 (~10) | `archive/` | `progress.md` D-011 entry |
| D-012 (instrumentation restore) | TASK-0615..0618 (4) | `archive/` | `progress.md` D-012 entry |

**Total tasks shipped across bundles:** ~410 atomic tasks across SPEC-02..SPEC-22, all archived. Per-task definitions in `archive/TASK-NNNN-*.md`. Full per-bundle narratives in `progress.md`.

---

## How to repopulate this file (D-013+ workflow)

1. The next bundle's inventory lives in `docs/next-steps.md` (e.g., D-013 hardening backlog inherited from D-011).
2. Run `task-splitter` from the relativist subdir against the relevant SPEC + inventory items. The agent writes TASK files directly into `docs/backlog/` (NOT into `archive/`) and updates this file's "Active" section + per-spec coverage matrix.
3. When the bundle closes, the next housekeeping commit moves the TASK files into `docs/backlog/archive/` and clears the "Active" section.

This pattern keeps the **active backlog small enough to read at a glance** while preserving every historical task definition for audit/reproducibility.
