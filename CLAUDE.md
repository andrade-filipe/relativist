# CLAUDE.md — Relativist

## Overview

Relativist is a distributed Interaction Combinator reducer for Grid Computing, written in Rust. It implements Lafont's 3 symbols and 6 interaction rules with BSP (Bulk Synchronous Parallel) reduction across networked workers via TCP.

**Repository:** github.com/andrade-filipe/relativist
**Part of:** TCC — Interaction Combinators for Grid Computing (UNIT, 2026)

## Current State

- **v1:** frozen on branch `v1-feature-complete` (tag `v0.10.0-bench`). DO NOT modify.
- **v2:** active development on branch `v2-development`.
- **Tests:** 690 passing (`cargo test`). This number must never decrease.
- **Specs:** 17 specs (SPEC-00 through SPEC-16) in `specs/`
- **Benchmarks:** 4490 executions, 0 correctness failures (Phase 1 + Phase 2 frozen)

## Build & Test

```bash
cargo test                        # run all tests (expect 690+ passing)
cargo clippy -- -D warnings       # lint (must be clean)
cargo fmt --check                 # formatting (must pass)
cargo build --release             # release build
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

Pipeline state tracked in `docs/pipeline-state.md`. Invoke the `sdd-pipeline` agent to see current state and next action.

## Agent System

| Agent | Role | Writes to |
|-------|------|-----------|
| **sdd-pipeline** | Orchestrator — reads state, tells you what to do next | `docs/pipeline-state.md` |
| pesquisador | Context curator — researches specs/docs/code, produces focused briefings | `docs/briefings/` |
| task-splitter | Break spec into atomic tasks (<200 LoC each) | `docs/backlog/` |
| test-generator | Write test specifications (NOT code) | `docs/tests/` |
| developer | TDD implementation — **ONLY agent that writes code** | `src/`, `tests/` |
| reviewer | Code quality + architecture review | review output |
| qa | Adversarial bug hunting | bug reports |
| spec-critic | Adversarial spec review (before implementation) | `docs/spec-reviews/` |
| task-updater | Align tasks after spec revision | `docs/backlog/` |

## Key Files

- `specs/` — all 17 formal specifications (ENGLISH only)
- `docs/progress.md` — implementation state
- `docs/pipeline-state.md` — current pipeline stage (maintained by sdd-pipeline)
- `docs/backlog/BACKLOG.md` — all tasks with status
- `docs/ROADMAP.md` — v2+ features, break-even analysis (section 2.40)
- `docs/DEVELOPMENT-PIPELINE.md` — pipeline definition
- `results/locked/v1_local_baseline/` — frozen benchmark data (DO NOT modify)

## v2 Development Rules

1. All work on `v2-development` branch (or feature branches from it)
2. Every change must pass all 690 existing v1 tests — zero regression
3. New features follow ROADMAP.md priorities
4. Every change follows the 6-stage SDD pipeline
5. Specs MUST be written before implementation (Theory -> Specs -> Code)
6. Specs are ALWAYS in English; code is ALWAYS in English
