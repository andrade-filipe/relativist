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
2. Create a feature branch from `main`: `git checkout -b feature/your-feature`
3. Write tests for your changes (we use TDD — see `specs/SPEC-08-test-strategy.md`)
4. Ensure all tests pass: `cargo test`
5. Ensure code is formatted: `cargo fmt`
6. Ensure no lint warnings: `cargo clippy`
7. Commit with clear messages following conventional commits
8. Open a Pull Request against `main`

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

## Specs and Invariants

The project is built around formal invariants (see `specs/SPEC-01-invariantes.md`):

- **T1-T7**: Theoretical invariants from Interaction Combinator theory
- **D1-D4**: Distribution invariants for correct partitioned reduction
- **I1-I5**: Implementation invariants for data structure correctness
- **G1**: The fundamental property: `reduce_all(net) == run_grid(net, n)`

Every change must preserve these invariants. Tests verify them.

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
