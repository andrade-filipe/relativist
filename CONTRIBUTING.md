# Contributing to Relativist

Thanks for your interest in Relativist! This project is a research artifact (a
Computer Science thesis / TCC) that is now open source. Contributions — bug
reports, fixes, tests, docs, and well-scoped features — are welcome.

Please also read [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md),
[`CODING_STANDARDS.md`](CODING_STANDARDS.md), and [`GOVERNANCE.md`](GOVERNANCE.md).

## Ways to contribute

### Report a bug

1. Search [existing issues](../../issues) first.
2. Open a new issue with the bug template: steps to reproduce (a minimal
   `.bin`/`.ic` net or command line is ideal), expected vs. actual behavior, and
   your environment (OS, Rust version, feature flags).

### Suggest a feature

1. Open an issue with the feature template. Describe the use case and why it
   matters.
2. For anything that touches the model (the six interaction rules, the
   partition/merge protocol, the SPEC-01 invariants, or the `reduce_all ≅
   run_grid` contract), **discuss in the issue before coding** — these are
   weighed for research integrity, not just code quality (see `GOVERNANCE.md`).

### Submit code

1. Fork the repo.
2. Branch from the active development branch (`develop`, **not** `main`):
   `git checkout -b feature/your-feature develop`.
3. Develop using the **RPI workflow** (below).
4. Keep the three gates green (below) with **zero test regressions**.
5. Commit with [Conventional Commits](https://www.conventionalcommits.org/):
   `type(scope): description` — e.g. `feat(partition): add topology-aware
   strategy`. Types: `feat`, `fix`, `refactor`, `test`, `docs`, `perf`, `ci`,
   `chore`. Scopes are module names (`net`, `reduction`, `partition`, `merge`,
   `protocol`, `bench`, …).
6. Open a PR against `develop`. (`main` receives changes via the
   `develop → main` integration cadence.)

## The workflow: RPI (Research → Plan → Implement)

Relativist replaced its heavyweight Spec-Driven Development pipeline with **RPI**.
It is lighter and keeps changes focused. The retired SDD process is archived,
read-only, under [`docs/_archive/`](docs/_archive/).

```
1. RESEARCH   — map the affected code + the relevant specs/docs.        -> docs/rpi/RESEARCH.md
2. PLAN       — write a surgical, testable plan (incl. how you verify). -> docs/rpi/PLAN.md
3. IMPLEMENT  — make the change TDD-style; run all gates green.         -> src/, tests/
   then update any living docs (a spec invariant, a reference page, ROADMAP).
```

You can run this manually, or use the three Claude Code agents in
[`.claude/agents/`](.claude/agents/) (`researcher`, `planner`, `implementer`).
Either way: research before planning, plan before coding, verify before opening
the PR. `docs/rpi/RESEARCH.md` and `docs/rpi/PLAN.md` are disposable working
notes (gitignored), not deliverables.

## The three gates (CI enforces these)

Every PR must pass, with no regression in test counts:

```bash
cargo test                                   # all tests green
cargo clippy --all-features -- -D warnings   # no warnings
cargo fmt --check                            # formatted
```

The frozen v1 floor (**690** tests on `v1-feature-complete`) must never drop, and
the current `develop` baseline must not regress. Adding code means adding
tests — the count goes up, never silently down. Full rules live in
[`CODING_STANDARDS.md`](CODING_STANDARDS.md).

## Development setup

```bash
# Prerequisites: Rust (stable, via rustup); Docker optional for distributed tests
cargo build
cargo test
cargo clippy --all-features -- -D warnings
cargo fmt --check

# Run locally (simulated distribution)
cargo run --release -- local --workers 4 -i test.bin -o out.bin

# Distributed (two terminals)
cargo run --release -- coordinator --workers 2 --port 9000 -i test.bin -o out.bin
cargo run --release -- worker --coordinator localhost:9000
```

## Specs and invariants

The 28 specs in [`docs/specs/`](docs/specs/) document the design. Under RPI they are
**reference**, not a per-change gate — but they remain the source of truth for
the formal claims. The load-bearing invariants are in
[`docs/specs/SPEC-01-invariantes.md`](docs/specs/SPEC-01-invariantes.md):

- **T1–T7** — theoretical invariants from Interaction Combinator theory
- **D1–D6** — distribution invariants for correct partitioned reduction
- **I1–I5** — implementation invariants for data-structure correctness
- **G1** — the fundamental property: `reduce_all(net) ≅ run_grid(net, n)`

Any change that can affect these must keep the tests that verify them green, and
should say which invariant it touches.

## Performance

Changes to core modules (`net`, `reduction`, `partition`, `merge`) should not
regress benchmark performance meaningfully. The frozen evidence and the scripts
to reproduce it live in [`reproduce_article/`](reproduce_article/).

## Review

This is a solo-maintained project; reviews are best-effort. Bug fixes are looked
at sooner than features. All PRs need green CI (the three gates above on every
feature profile). Thank you for contributing!

## License of contributions

By contributing, you agree that your contributions are licensed under the
project's [Apache License 2.0](LICENSE).
