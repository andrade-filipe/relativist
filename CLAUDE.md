# CLAUDE.md — Relativist

## Overview

Relativist is a distributed Interaction Combinator reducer for Grid Computing, written in Rust. It implements Lafont's 3 symbols and 6 interaction rules with BSP (Bulk Synchronous Parallel) reduction across networked workers via TCP.

**Repository:** github.com/andrade-filipe/relativist
**Part of:** TCC — Interaction Combinators for Grid Computing (UNIT, 2026)

## Current State

- **v1:** frozen on branch `v1-feature-complete` (tag `v0.10.0-bench`). DO NOT modify.
- **v2:** active development on branch `develop`.
- **Tests (post-D-012, 2026-05-05):**
  - `cargo test`: 1798 default
  - `cargo test --features zero-copy`: 1842
  - `cargo test --features streaming-no-recycle`: 1789
  - `cargo test --release`: 1740 (compiles and runs after TASK-0617 + D-012 REFACTOR)
  - v1 inviolable floor: 690 (frozen on `v1-feature-complete`).
- **Specs:** 28 specs (SPEC-00 through SPEC-27) in `docs/specs/`. v1 implements SPEC-00..16; v2 adds SPEC-17..27 (transport abstraction, wire format v2, delta protocol, elastic grid, streaming, arena, compact memory, WAN, recipe gen, GUI, encoder API).
- **Benchmarks:** 4490 executions, 0 correctness failures (Phase 1 + Phase 2 frozen at v1 baseline)

## Build & Test

```bash
cargo test                                    # run all tests (1798+ on develop; 690 floor on v1-feature-complete)
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

> Full coding rules: [`CODING_STANDARDS.md`](CODING_STANDARDS.md).

## Development Workflow — RPI (Research → Plan → Implement)

Relativist uses **RPI**, not the older Spec-Driven Development pipeline. RPI keeps context small
and attention high: each phase runs in a **fresh context** and hands off through a disposable file
on disk.

```
1. RESEARCH   (researcher)   — map code + docs            -> docs/rpi/RESEARCH.md
2. PLAN       (planner)      — surgical, testable plan     -> docs/rpi/PLAN.md
3. IMPLEMENT  (implementer)  — edit + test + verify green  -> src/, tests/
   then update the living docs (spec/reference/ROADMAP as needed) and repeat
```

`docs/rpi/RESEARCH.md` and `docs/rpi/PLAN.md` are disposable (gitignored, overwritten each cycle).
Agent definitions and the full loop: [`.claude/agents/README.md`](.claude/agents/README.md).

The retired SDD pipeline (12 agents: sdd-pipeline, task-splitter, test-generator, developer,
reviewer, qa, spec-critic, especialista-specs, task-updater, cicd, opensource, pesquisador) is
frozen, read-only, under [`docs/_archive/sdd-agents/`](docs/_archive/sdd-agents/).

## Agent System (RPI)

| Agent | Phase | Writes to |
|-------|-------|-----------|
| [`researcher`](.claude/agents/researcher.md) | Research | `docs/rpi/RESEARCH.md` |
| [`planner`](.claude/agents/planner.md) | Plan | `docs/rpi/PLAN.md` |
| [`implementer`](.claude/agents/implementer.md) | Implement — **ONLY agent that writes code** | `src/`, `tests/` |

## Key Files

- `CODING_STANDARDS.md` — the code rules CI enforces
- `.claude/agents/README.md` — the RPI workflow
- `docs/specs/` — 28 formal specifications (ENGLISH only); reference under RPI, not a per-change gate
- `docs/README.md` — master documentation index (entry point for navigation)
- `docs/roadmap.md` — v2+ features, break-even analysis (section 2.40)
- `docs/reference/next-steps.md` — what the software should do next (for contributors)
- `reproduce_article/` — frozen benchmark evidence + reproduction scripts (DO NOT modify the data)
- `docs/_archive/` — frozen SDD history (read-only): backlog, tests, reviews, progress, WORKFLOWS, …

## v2 Development Rules

1. All work on `develop` branch (or feature branches from it)
2. Every change must pass all 690 v1 tests (floor) plus the current v2 baseline — zero regression
3. New features follow roadmap.md priorities
4. Every change follows the RPI loop (Research → Plan → Implement → update docs)
5. Theory → design → code; cite a spec/invariant when a change touches one (no mandatory new spec per change)
6. Specs and code are ALWAYS in English
