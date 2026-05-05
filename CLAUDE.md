# CLAUDE.md — Relativist

## Overview

Relativist is a distributed Interaction Combinator reducer for Grid Computing, written in Rust. It implements Lafont's 3 symbols and 6 interaction rules with BSP (Bulk Synchronous Parallel) reduction across networked workers via TCP.

**Repository:** github.com/andrade-filipe/relativist
**Part of:** TCC — Interaction Combinators for Grid Computing (UNIT, 2026)

## Current State

- **v1:** frozen on branch `v1-feature-complete` (tag `v0.10.0-bench`). DO NOT modify.
- **v2:** active development on branch `v2-development`.
- **Tests (post-D-012, 2026-05-05):**
  - `cargo test`: 1798 default
  - `cargo test --features zero-copy`: 1842
  - `cargo test --features streaming-no-recycle`: 1789
  - `cargo test --release`: 1740 (compiles and runs after TASK-0617 + D-012 REFACTOR)
  - v1 inviolable floor: 690 (frozen on `v1-feature-complete`).
- **Specs:** 28 specs (SPEC-00 through SPEC-27) in `specs/`. v1 implements SPEC-00..16; v2 adds SPEC-17..27 (transport abstraction, wire format v2, delta protocol, elastic grid, streaming, arena, compact memory, WAN, recipe gen, GUI, encoder API).
- **Benchmarks:** 4490 executions, 0 correctness failures (Phase 1 + Phase 2 frozen at v1 baseline)

## Build & Test

```bash
cargo test                                    # run all tests (1798+ on v2-development; 690 floor on v1-feature-complete)
cargo test --features zero-copy               # 1842+
cargo test --features streaming-no-recycle    # 1789+
cargo test --release                          # 1740+ (post-TASK-0617)
cargo clippy --all-features -- -D warnings    # lint (must be clean)
cargo fmt --check                             # formatting (must pass)
cargo build --release                         # release build
```

All three checks must pass before any commit.

## Module Structure (SPEC-13)

```
src/
  net/           # SPEC-02: Net, Agent, Wire, Port — pure, no async
  reduction/     # SPEC-03: 6 interaction rules, reduce_all — pure
  partition/     # SPEC-04: split, FreePort, border maps — pure
  merge/         # SPEC-05: merge, run_grid, BSP cycle — pure
  protocol/      # SPEC-06: TCP framing, wire protocol — async, tokio
  config.rs      # SPEC-07: CLI (clap), NodeConfig
  security/      # SPEC-10: auth tokens, TLS — feature-gated
  observability/  # SPEC-11: tracing, metrics, health — feature-gated
  io/            # SPEC-12: formats (binary, IC text), generators
  encoding/      # SPEC-14: Church numerals (add, mul)
  bench/         # SPEC-09: benchmark suite, profiles
  error.rs       # thiserror error types
  lib.rs, main.rs
```

**Dependency direction (inviolable):**
`net` <- `reduction` <- `partition` <- `merge` <- `protocol` <- coordinator/worker

Core layer (`net/`, `reduction/`, `partition/`, `merge/`) is pure — NO async, NO tokio, NO I/O.

## Coding Standards

- No `unwrap()` in production code — use `?` or explicit error handling
- No `unsafe` without `// SAFETY:` comment
- No `println!` — use `tracing` macros only
- `thiserror` for errors, not `anyhow`
- `pub(crate)` unless truly public API
- `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]` on public types
- Newtype pattern for IDs (`struct AgentId(u32)`)
- IC concepts in code need clear comments (counter-intuitive for programmers)

## Development Pipeline (SDD + TDD)

Every feature follows a 6-stage pipeline. **No stage can be skipped.**

```
1. SPLITTING  (task-splitter)   — break spec into atomic tasks
2. TESTS      (test-generator)  — write test specifications
3. DEV        (developer)       — TDD: RED -> GREEN -> REFACTOR
4. REVIEW     (reviewer)        — code quality + architecture review
5. QA         (qa)              — adversarial bug hunting
6. REFACTOR   (developer)       — apply fixes, verify all tests pass
```

Active pipeline state tracked in `docs/next-steps.md` (historical entries move to `docs/progress.md` after a bundle ships). Invoke the `sdd-pipeline` agent to see current state and next action.

## Agent System

| Agent | Role | Writes to |
|-------|------|-----------|
| **sdd-pipeline** | Orchestrator — reads state, tells you what to do next | `docs/next-steps.md` |
| pesquisador | Context curator — researches specs/docs/code, produces focused briefings | `docs/briefings/` |
| task-splitter | Break spec into atomic tasks (<200 LoC each) | `docs/backlog/` |
| test-generator | Write test specifications (NOT code) | `docs/tests/` |
| developer | TDD implementation — **ONLY agent that writes code** | `src/`, `tests/` |
| reviewer | Code quality + architecture review | review output |
| qa | Adversarial bug hunting | bug reports |
| spec-critic | Adversarial spec review Round 1/3 (before implementation) | `docs/spec-reviews/` |
| **especialista-specs** | Spec author and Round 2+ defender — ONLY agent that writes to `specs/` | `specs/`, `docs/spec-reviews/` (closure logs only) |
| task-updater | Align tasks after spec revision | `docs/backlog/` |
| cicd | CI/CD pipeline maintenance | `.github/workflows/`, `Dockerfile` |
| opensource | Open-source project hygiene | `README.md`, `LICENCE`, `.github/` |

## Key Files

- `specs/` — all 28 formal specifications (ENGLISH only)
- `docs/INDEX.md` — master documentation index (entry point for navigation)
- `docs/progress.md` — implementation history (PAST/COMPLETED only)
- `docs/next-steps.md` — active pipeline state and future work (maintained by sdd-pipeline)
- `docs/backlog/BACKLOG.md` — all tasks with status (completed task files in `docs/backlog/archive/`)
- `docs/ROADMAP.md` — v2+ features, break-even analysis (section 2.40)
- `docs/WORKFLOWS.md` — unified development, spec-review, and git workflows
- `results/locked/v1_local_baseline/` — frozen benchmark data (DO NOT modify)

## v2 Development Rules

1. All work on `v2-development` branch (or feature branches from it)
2. Every change must pass all 690 v1 tests (floor) plus the current v2 baseline (1181 default / 1224 zero-copy) — zero regression
3. New features follow ROADMAP.md priorities
4. Every change follows the 6-stage SDD pipeline
5. Specs MUST be written before implementation (Theory -> Specs -> Code)
6. Specs are ALWAYS in English; code is ALWAYS in English
