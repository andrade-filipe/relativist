# CLAUDE.md — Relativist

## Overview

Relativist is a distributed Interaction Combinator reducer for Grid Computing, written in Rust. It implements Lafont's 3 symbols and 6 interaction rules with BSP (Bulk Synchronous Parallel) reduction across networked workers via TCP.

**Repository:** github.com/andrade-filipe/relativist
**Part of:** TCC — Interaction Combinators for Grid Computing (UNIT, 2026)

## Current State

- **v1:** frozen on branch `v1-feature-complete` (tag `v0.10.0-bench`). DO NOT modify.
- **v2:** active development on branch `develop`.
- **Branching:** GitFlow-lite — `main` (release) + `develop` (integration) + `v1-feature-complete` (frozen archive). PR-only, enforced by branch protection + the `branch-policy` check. See `CONTRIBUTING.md`.
- **Tests:** the **required CI gate** is `cargo test --lib` (~1700 library unit tests, incl. ~90 `run_grid`/G1 tests). The full `cargo test` (integration: example codecs, bench, e2e) is the **optional tier** (extended-tests.yml / local). v1 inviolable floor: **690** on `v1-feature-complete`. **TDD is mandatory** (test-first). Tiers: `docs/TESTING.md`.
- **Specs:** 28 specs (SPEC-00 through SPEC-27) in `docs/specs/`. v1 implements SPEC-00..16; v2 adds SPEC-17..27 (transport abstraction, wire format v2, delta protocol, elastic grid, streaming, arena, compact memory, WAN, recipe gen, GUI, encoder API).
- **Benchmarks:** frozen evidence in `reproduce_article/` (DO NOT modify).

## Build & Test

```bash
# Essential gate — what CI requires; run before every push:
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --lib                              # ~1700 library unit tests (engine + G1)

# Optional / full:
cargo test                                    # + integration tier (slow: Horner ~256s, bench, e2e)
cargo build --release
```

Toolchain pinned to **1.96.0** (`rust-toolchain.toml`) so local clippy/fmt match CI exactly.
Enable the local gate once: `git config core.hooksPath .githooks` (pre-push runs fmt+clippy+`--lib`).

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
  encoding/      # SPEC-14/27: Church numerals + Encoder/Decoder/Codec API (Horner = example codec)
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

- `CONTRIBUTING.md` — workflow (RPI + **mandatory TDD**), GitFlow-lite branch model, the gate
- `CODING_STANDARDS.md` — the code rules CI enforces
- `docs/TESTING.md` — the two test tiers (essential `--lib` gate vs optional integration)
- `.claude/agents/README.md` — the RPI workflow; `.claude/skills/doc-curator` + `.claude/agents/doc-catalog` keep docs LLM-grade; `.claude/skills/beck-tdd-pattern-family` for TDD
- `docs/README.md` — master documentation **catalog** (keyword-searchable entry point)
- `docs/specs/` — 28 formal specifications (ENGLISH only); reference under RPI, not a per-change gate
- `docs/roadmap.md` — v2+ features, break-even analysis (section 2.40)
- `docs/reference/next-steps.md` — what the software should do next (for contributors)
- `reproduce_article/` — frozen benchmark evidence + reproduction scripts (DO NOT modify the data)
- `docs/_archive/` — frozen SDD history (read-only): backlog, tests, reviews, progress, WORKFLOWS, …

## v2 Development Rules

1. Branch from `develop`; open a PR **into `develop`** (GitFlow-lite, enforced by `branch-policy`). Only `develop`/`release/*`/`hotfix/*` may target `main`.
2. **TDD: failing test first** (red → green → refactor). Core/engine/invariant behavior → a **library unit test** (the `cargo test --lib` gate). The 690 v1 floor never drops; the gate stays green (fmt + clippy + `--lib`).
3. New features follow roadmap.md priorities
4. Every change follows the RPI loop (Research → Plan → Implement → **update the living docs**, per `doc-curator`)
5. Theory → design → code; cite a spec/invariant when a change touches one (no mandatory new spec per change)
6. Specs and code are ALWAYS in English
