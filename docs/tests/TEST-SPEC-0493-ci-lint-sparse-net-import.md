# TEST-SPEC-0493: CI lint forbidding `SparseNet` imports in `src/reduction/**` (R23 — closes SC-008)

**SPEC-22 §7 ID:** none direct (CI design constraint, not runtime); plus this plumbing file.
**Owning task:** TASK-0493.
**Parent spec:** SPEC-22 §3.2 R23.
**Type:** CI lint (manual smoke verification, not a permanent test).

---

## Inputs / Fixtures

- The repository's CI workflow file (`.github/workflows/*.yml`).
- A scratch branch with a deliberate violation: `use crate::net::sparse::SparseNet;` added to a file in `src/reduction/`.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0493-01 | `ci_grep_step_present` | the workflow YAML | grep / inspect | a step exists that runs `grep -r "use crate::net::sparse::SparseNet" src/reduction/` (or equivalent — including the patterns `use SparseNet`, `crate::net::sparse::SparseNet`, `net::sparse::SparseNet`). The step fails the build on match. |
| UT-0493-02 | `ci_smoke_violation_fails_build` | scratch branch with the violation | run CI | the lint step fails with a clear error message citing SPEC-22 R23. |
| UT-0493-03 | `ci_smoke_remove_violation_passes_build` | scratch branch with the violation removed | run CI | the lint step passes. |
| UT-0493-04 | `lint_runs_on_pr_and_push` | the workflow trigger surface | inspect | the lint runs on `pull_request` events to `v2-development` and `main`, and on `push` to those branches. |
| UT-0493-05 | `documentation_in_contributing_md` | `relativist-core/CONTRIBUTING.md` (or equivalent) | grep | a section documents the constraint and cites AC-006 / AC-001 perf rationale. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | A legitimate use of SparseNet in `src/reduction/sparse_export.rs` (if such a file exists) | The lint MAY include a bypass for that specific file. (Currently NONE expected per TASK-0493 acceptance.) |
| EC-2 | A doc-comment in `src/reduction/` that mentions "SparseNet" without an import | The grep pattern targets `use ` clauses specifically; pure doc mentions are not matched. (Refine the regex if false positives occur.) |
| EC-3 | A `cfg(test)` import of SparseNet in `src/reduction/tests.rs` | The lint may or may not exempt test code; document the chosen scope. |

## Invariants asserted

- R23 (DESIGN CONSTRAINT — CI-enforced; closes SC-008).
- AC-006 / AC-001 performance design constraint.

## ARG/DISC/REF citation

- AC-006 (HVM2 flat-array rationale).
- AC-001 (Haskell IC.Core `Map AgentId Agent` baseline — sparse representation has 5-10× constant factor; not for hot path).

## Determinism notes

CI lint is deterministic (grep over a fixed file set). The smoke verification (UT-0493-02 / UT-0493-03) is one-time manual; not an automated permanent test.

## Cross-test dependencies

- This task is the **cicd agent's** territory per the SPEC-22 R23 closure log; the task-splitter records the requirement, the cicd agent implements the lint at Stage 3 / Stage 5.
- TEST-SPEC-0498 (unsafe-free audit) shares the same CI lint infrastructure pattern.
