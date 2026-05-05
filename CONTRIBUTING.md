# Contributing to Relativist

Thank you for your interest in contributing to Relativist! This document provides guidelines for contributing to the project.

## How to Contribute

### Reporting Bugs

1. Check the [existing issues](../../issues) to see if the bug has already been reported
2. If not, open a new issue using the bug report template
3. Include: steps to reproduce, expected behavior, actual behavior, environment details

### Suggesting Features

1. Open an issue using the feature request template
2. Describe the use case and why the feature would be valuable
3. Be specific about the expected behavior

### Submitting Code

1. Fork the repository
2. Create a feature branch from the active development branch (`v2-development`, not `main`): `git checkout -b feature/your-feature v2-development`
3. Follow the **6-stage SDD pipeline** below before any code lands.
4. Ensure all tests pass on every profile (see "Test floor by profile" below).
5. Ensure code is formatted: `cargo fmt`
6. Ensure no lint warnings: `cargo clippy --all-features -- -D warnings`
7. Commit with clear messages following [conventional commits](https://www.conventionalcommits.org/):
   - Format: `type(scope): description` (e.g., `feat(partition): add topology-aware strategy`)
   - Types: `feat`, `fix`, `refactor`, `test`, `docs`, `perf`, `ci`
   - Scopes: module names (`net`, `reduction`, `partition`, `merge`, `protocol`, `bench`, etc.)
8. Open a Pull Request against `v2-development` (PRs into `main` come from the `v2-development -> main` integration cadence).

### 6-stage SDD pipeline (Spec-Driven Development + TDD)

Every feature in v2 follows the pipeline below. **No stage can be skipped.** The pipeline state is tracked in `docs/next-steps.md` (active) and historical entries land in `docs/progress.md` once the bundle ships.

```
1. SPLITTING  (task-splitter)   — break the spec into atomic tasks (<200 LoC each)
2. TESTS      (test-generator)  — write test specifications (SPEC-style, NOT code yet)
3. DEV        (developer)       — TDD: RED -> GREEN -> REFACTOR; the only stage that writes Rust
4. REVIEW     (reviewer)        — code quality + architecture review
5. QA         (qa)              — adversarial bug hunting, edge cases
6. REFACTOR   (developer)       — apply review/QA fixes; verify all profiles pass
```

The `sdd-pipeline` agent is the orchestrator — invoke it to see the current stage and next action. Specs MUST be written before implementation (Theory -> Specs -> Code) and ALWAYS in English; the v1 implementation is frozen on `v1-feature-complete` and v2 work happens on `v2-development`.

For full detail (when each agent runs, how `docs/spec-reviews/` Round 1/2/3 interact, the git workflow, and the progress.md/next-steps.md split) see [`docs/WORKFLOWS.md`](docs/WORKFLOWS.md).

### Test floor by profile

`cargo test` must pass on every feature profile before a PR can land. The floor counts below are the minimum the v2-development branch ships with as of D-012 (2026-05-05); no PR may regress any of them.

| Profile                                              | Minimum tests passing | What it covers                       |
|------------------------------------------------------|-----------------------|--------------------------------------|
| `cargo test`                                         | **1798**              | Default features                     |
| `cargo test --features zero-copy`                    | **1842**              | Adds rkyv archive tests              |
| `cargo test --features streaming-no-recycle`         | **1789**              | SPEC-21 R37b compile-time gate       |
| `cargo test --release`                               | **1740**              | Release-build behavior               |
| `cargo test` on `v1-feature-complete` (inviolable)   | **690**               | Frozen v1; never modified            |

## Development Setup

### Prerequisites

- Rust (latest stable, via [rustup](https://rustup.rs/))
- Docker (optional, for distributed testing)

### Building

```bash
cargo build
cargo test
cargo clippy
cargo fmt --check
```

### Running

```bash
# Local mode (simulated distribution)
cargo run -- local --workers 4 --net examples/ep_annihilation.bin

# Distributed mode
cargo run -- coordinator --workers 4 --port 9000 --net examples/ep_annihilation.bin
cargo run -- worker --coordinator localhost:9000
```

## Architecture

Relativist follows a **Spec Driven Development** approach. Before writing code, read the relevant spec in `specs/`:

| Module | Spec | Description |
|--------|------|-------------|
| Net types | SPEC-02 | Agent, Port, Wire, Net representation |
| Reduction | SPEC-03 | 6 interaction rules, reduce_all loop |
| Partition | SPEC-04 | Split, merge, FreePort, boundary handling |
| Grid cycle | SPEC-05 | Coordinator loop, border redex resolution |
| Protocol | SPEC-06 | TCP wire protocol, message framing |
| Deployment | SPEC-07 | CLI, configuration, lifecycle |
| Testing | SPEC-08 | Test strategy, 103 specified tests |
| Benchmarks | SPEC-09 | 9 benchmarks, metrics, methodology |

## Code Style

- Follow Rust idioms and conventions
- Use `rustfmt` defaults
- No `unsafe` without explicit justification and review
- All public APIs must have doc comments
- Prefer explicitness over cleverness

## Performance Expectations

Changes to core modules (`net`, `reduction`, `partition`, `merge`) must not regress benchmark performance by more than 5%. Run `cargo run --release -- bench` before and after your changes to verify.

## Review Process

- Bug fixes: response within 48 hours
- Features: response within 1 week
- All PRs require passing CI (`cargo test` on every profile listed above, `cargo clippy --all-features -- -D warnings`, `cargo fmt --check`)
- Changes to specs require adversarial review (see [`docs/WORKFLOWS.md`](docs/WORKFLOWS.md) for the 3-round spec review pipeline: critic Round 1 -> defender Round 2 -> closure Round 3)

## Specs and Invariants

The project is built around formal invariants (see `specs/SPEC-01-invariantes.md`):

- **T1-T7**: Theoretical invariants from Interaction Combinator theory
- **D1-D4**: Distribution invariants for correct partitioned reduction
- **I1-I5**: Implementation invariants for data structure correctness
- **G1**: The fundamental property: `reduce_all(net) == run_grid(net, n)`

Every change must preserve these invariants. Tests verify them.

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
