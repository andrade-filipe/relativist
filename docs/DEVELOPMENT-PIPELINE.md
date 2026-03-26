# Relativist — Development Pipeline

**Last updated:** 2026-03-26
**Status:** Approved — apply to every spec implementation phase

---

## Overview

Relativist follows a strict **Spec-Driven, TDD** development pipeline. Each spec (SPEC-02 through SPEC-12) goes through the same 7-stage pipeline. No stage can be skipped. The pipeline ensures clean context, precise tasks, comprehensive tests, and multi-perspective code review.

---

## Pipeline Stages

```
┌─────────────────────────────────────────────────────────────────────┐
│                    PER-SPEC IMPLEMENTATION CYCLE                    │
│                                                                     │
│   Stage 1          Stage 2          Stage 3          Stage 4        │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐     │
│  │   TASK   │───▶│   TEST   │───▶│   DEV    │───▶│  CLEAN   │     │
│  │ SPLITTER │    │GENERATOR │    │(TDD impl)│    │  CODE    │     │
│  └──────────┘    └──────────┘    └──────────┘    └──────────┘     │
│                                                       │             │
│   Stage 7          Stage 6          Stage 5           │             │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐        │             │
│  │   DEV    │◀───│    QA    │◀───│  ARCH    │◀───────┘             │
│  │(refactor)│    │(bug hunt)│    │ REVIEW   │                      │
│  └──────────┘    └──────────┘    └──────────┘                      │
│       │                                                             │
│       ▼                                                             │
│  ┌──────────┐                                                       │
│  │  TESTS   │ ── All green? ── YES ──▶ DONE ✓                     │
│  │  PASS?   │                                                       │
│  └──────────┘ ── NO ──▶ Fix and re-run                             │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Stage Details

### Stage 1: Task Splitter (PO)

**Agent:** `task-splitter`
**Input:** Spec file (e.g., `specs/SPEC-02-net-representation.md`)
**Output:** Task files in `docs/backlog/`, updated `docs/backlog/BACKLOG.md`

**What happens:**
1. Reads the entire spec
2. Breaks every MUST/SHOULD requirement into atomic tasks (< 200 LoC each)
3. Defines explicit dependencies between tasks (DAG, no cycles)
4. Specifies exact files to create/modify and type signatures
5. Creates TASK-XXXX.md files with acceptance criteria

**Exit criteria:**
- Every MUST requirement maps to at least one task
- No task exceeds 200 estimated LoC
- Dependency graph is a DAG
- BACKLOG.md index is up to date

---

### Stage 2: Test Generator

**Agent:** `test-generator`
**Input:** Task file (TASK-XXXX.md) + parent spec
**Output:** Test specification in `docs/tests/TEST-SPEC-XXXX.md`

**What happens:**
1. Reads the task's acceptance criteria and spec requirements
2. Designs unit tests with exact inputs and expected outputs
3. Designs property tests for applicable invariants (SPEC-01)
4. Catalogs edge cases (at least 2 per happy path)
5. Writes test spec — NOT code, but precise enough for the developer to implement

**Exit criteria:**
- Every acceptance criterion has at least one test
- At least 2 edge cases documented per behavior
- Property tests defined for all applicable invariants
- Concrete input/output values, not vague descriptions

---

### Stage 3: Developer (TDD Implementation)

**Agent:** `developer`
**Input:** Task file + test specification + (optionally) review feedback
**Output:** Production code in `src/`, test code in `src/` and `tests/`

**What happens:**
1. **RED:** Writes all tests from the test spec → tests compile but FAIL
2. **GREEN:** Writes minimum production code → tests PASS
3. **REFACTOR:** Cleans up obvious issues → tests still PASS
4. Runs `cargo test` for the module → ALL GREEN

**Exit criteria:**
- All specified tests are implemented and passing
- Production code satisfies the task's acceptance criteria
- `cargo clippy -- -D warnings` passes
- `cargo fmt --check` passes

---

### Stage 4: Code Cleaner Review

**Agent:** `code-cleaner`
**Input:** Code produced in Stage 3
**Output:** Structured review (MF/SF/NTH issues with before/after examples)

**What happens:**
1. Reviews code for Clean Code principles (naming, function size, abstraction levels)
2. Reviews for SOLID principles (SRP, OCP, DIP especially)
3. Reviews for Rust idioms (ownership, iterators, error handling)
4. Classifies issues as Must-Fix, Should-Fix, or Nice-to-Have
5. Provides concrete before/after refactoring examples

**Exit criteria:**
- Review document produced with categorized issues
- All Must-Fix issues have concrete fix examples

---

### Stage 5: Architecture Review

**Agent:** `code-reviewer`
**Input:** Code from Stage 3 + code-cleaner review
**Output:** Structured architecture review (AI/PR/SN issues)

**What happens:**
1. Verifies module boundaries match SPEC-13
2. Verifies dependency direction (core → infra, never reverse)
3. Verifies design patterns (FSM, Transport trait, newtypes, error enums)
4. Checks spec compliance: every MUST requirement implemented correctly
5. Flags anti-patterns (god struct, primitive obsession, leaky abstraction)

**Exit criteria:**
- Architecture review document produced
- Spec compliance matrix completed
- All Architectural Issues have concrete fix examples

---

### Stage 6: QA Review (Bug Hunt)

**Agent:** `qa`
**Input:** Code from Stage 3 + previous reviews
**Output:** Bug reports, edge case catalog, test coverage gaps

**What happens:**
1. Hunts for panics: every `.unwrap()`, `[]` access, `as` cast, division
2. Hunts for logic errors: off-by-one, boundary conditions, state machine gaps
3. Hunts for concurrency issues: race conditions, deadlocks, event ordering
4. Hunts for IC-specific bugs: invariant violations, ID collisions, border port errors
5. Catalogs untested edge cases with concrete inputs and expected behaviors

**Exit criteria:**
- Bug report produced with severity classification
- Edge case catalog with concrete reproduction steps
- Test coverage gaps identified

---

### Stage 7: Developer Refactoring

**Agent:** `developer`
**Input:** All reviews from Stages 4, 5, 6
**Output:** Refactored code with all tests still passing

**What happens:**
1. Reads ALL reviews (code-cleaner, code-reviewer, QA)
2. Addresses all Must-Fix issues first
3. Addresses Should-Fix issues
4. Adds tests for edge cases identified by QA
5. Runs `cargo test` → ALL GREEN
6. Runs `cargo clippy -- -D warnings` → CLEAN

**Exit criteria:**
- All Must-Fix issues from all reviewers addressed
- All new edge case tests passing
- No regressions (all previously passing tests still pass)
- `cargo test && cargo clippy -- -D warnings && cargo fmt --check` all green

---

## Per-Task vs Per-Spec

The pipeline runs **per task**, not per spec:

```
SPEC-02 decomposed into:
  TASK-0001 → Stages 1-7 → DONE
  TASK-0002 → Stages 1-7 → DONE
  TASK-0003 → Stages 1-7 → DONE
  ...
SPEC-02 COMPLETE
```

However, Stage 1 (task splitting) runs once per spec, producing all tasks upfront. Stages 2-7 run per task sequentially.

---

## Agent Roster

| Agent | Stage | Role | Writes Code? |
|-------|-------|------|-------------|
| `task-splitter` | 1 | Break specs into atomic tasks | No (docs only) |
| `test-generator` | 2 | Specify tests for TDD | No (docs only) |
| `developer` | 3, 7 | Write production + test code | **Yes (only agent)** |
| `code-cleaner` | 4 | Review: Clean Code, SOLID, Rust idioms | No (review only) |
| `code-reviewer` | 5 | Review: Architecture, patterns, spec compliance | No (review only) |
| `qa` | 6 | Review: Bugs, edge cases, adversarial testing | No (review only) |

Support agents (not in the per-task pipeline):
| Agent | Role |
|-------|------|
| `cicd` | GitHub Actions, Docker, CI pipeline |
| `opensource` | README, LICENCE, CONTRIBUTING, templates |

---

## Directory Structure

```
codigo/relativist/docs/
├── backlog/            ← Task Splitter output
│   ├── BACKLOG.md      ← Master index
│   ├── TASK-0001.md
│   ├── TASK-0002.md
│   └── ...
├── tests/              ← Test Generator output
│   ├── TEST-SPEC-0001.md
│   ├── TEST-SPEC-0002.md
│   └── ...
├── pesquisa/           ← Research library (complete)
├── progress.md         ← Overall progress
└── DEVELOPMENT-PIPELINE.md  ← This file
```

---

## Implementation Order

| Phase | Spec | Tasks (est.) | Description |
|-------|------|-------------|-------------|
| 1 | SPEC-02 | ~10 | Core types: Net, Agent, Wire, Port |
| 2 | SPEC-03 | ~12 | Reduction: 6 rules, redex detection |
| 3 | SPEC-04 | ~8 | Partitioning: split() |
| 4 | SPEC-05 | ~10 | Merge & grid cycle |
| 5 | SPEC-06 | ~12 | Wire protocol: messages, framing, Transport |
| 6 | SPEC-13 + SPEC-07 | ~8 | CLI, config, module wiring |
| 7 | SPEC-10 | ~6 | Security: token, TLS |
| 8 | SPEC-11 | ~8 | Observability: tracing, metrics |
| 9 | SPEC-12 | ~10 | User I/O: formats, generators |
| 10 | SPEC-09 | ~8 | Benchmarks |

Estimated total: ~90-100 tasks.

---

## Rules

1. **No task is implemented without a test spec.** (Stage 2 before Stage 3)
2. **No code merges without all three reviews.** (Stages 4, 5, 6 before Stage 7)
3. **Tests never decrease.** Refactoring cannot remove tests without replacing them.
4. **One task at a time.** Developer works on ONE task. Clean context, focused work.
5. **Specs are truth.** If code and spec disagree, the spec wins. Propose a spec change, don't silently deviate.
6. **Only the developer writes code.** All other agents produce documents/reviews.
7. **Reviews are actionable.** Every issue has a concrete example. No vague "could be better."
