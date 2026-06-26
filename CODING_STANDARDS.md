# Coding Standards — Relativist

These are the rules for any code that lands in `relativist-core/` or `relativist-cli/`. They
apply to humans and to AI agents equally. They are intentionally short; the goal is a codebase a
newcomer can read and trust.

> **Workflow:** Relativist now uses **RPI (Research → Plan → Implement)**, not the older
> Spec-Driven Development pipeline. See [`.claude/agents/README.md`](.claude/agents/README.md) and
> [`CONTRIBUTING.md`](CONTRIBUTING.md). These standards are the "how the code must look" half;
> RPI is the "how the work flows" half.

## Non-negotiables (CI enforces these)

All three must pass before any commit, with **zero regressions** in the test count:

```bash
cargo test                                  # all tests green
cargo clippy --all-features -- -D warnings  # no warnings
cargo fmt --check                           # formatted
```

The v1 frozen test floor (690, on `v1-feature-complete`) must never drop. Adding code means
adding tests; the test count goes up, never silently down.

## Rust rules

- **No `unwrap()` / `expect()` / `panic!` in production code.** Use `?` and typed errors. Tests
  may `unwrap()` freely.
- **No `unsafe` without a `// SAFETY:` comment** that justifies why it is sound.
- **No `println!` / `eprintln!` in library/production paths.** Use `tracing` macros
  (`tracing::info!`, `debug!`, …). CLI user-facing output is the only exception, and stays in the
  `relativist-cli` presentation layer.
- **Errors:** `thiserror`-derived enums, not `anyhow`, in library crates. One error type per
  module boundary; convert at boundaries with `#[from]`.
- **Visibility:** prefer `pub(crate)`; make something `pub` only when it is genuinely public API.
- **IDs are newtypes:** `struct AgentId(u32)`, never bare integers, so the type system catches
  mix-ups.
- **Derives on public data types:** `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]`
  where it makes sense.
- **Comment the IC concepts.** Interaction Combinator semantics (principal vs. auxiliary ports,
  active pairs, the six rules, confluence) are counter-intuitive to most programmers. Where code
  encodes a rule or invariant, say which one in a comment.

## Architecture rules

The core layer is **pure**: `net/`, `reduction/`, `partition/`, `merge/` contain **no async, no
tokio, no I/O**. Networking and orchestration live above them. The dependency direction is
one-way and inviolable:

```
net  <-  reduction  <-  partition  <-  merge  <-  protocol  <-  coordinator/worker
```

A lower layer must never import a higher one. If you find yourself wanting `net` to know about
TCP, the design is wrong — push the dependency upward instead.

## Tests

- Co-locate unit tests with the code (`#[cfg(test)] mod tests`); integration tests in `tests/`.
- Property-based tests (proptest) guard the algebraic invariants (confluence, structural
  equality modulo renaming). Regressions are checked in under `proptest-regressions/`.
- The fundamental property `reduce_all(net) ≅ run_grid(net, n)` (graph isomorphism) is the
  contract every distribution change must preserve. If a change can affect it, add/extend a test
  that asserts it.

## Commits

- Conventional Commits (`feat:`, `fix:`, `refactor:`, `docs:`, `chore:`, `test:`).
- One logical change per commit; keep the three checks green at each commit.
- Specs and code are always written in **English**.

## Specs as reference (not gospel)

The 28 formal specs in [`specs/`](specs/) document the design and the invariants (SPEC-01:
T1–T7, D1–D6, I1–I5, G1). Under RPI they are **reference**, not a process gate: read them to
understand *why* the system is shaped the way it is, cite them when a change touches an invariant,
but you no longer have to author a new spec before every change. New theory still flows
Theory → design note → code.
