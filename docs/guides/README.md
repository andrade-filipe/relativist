---
title: Guides
summary: Ordered learning path from install to v2 features.
keywords: [guides, tutorial, learning path, getting started]
modules: []
specs: []
audience: [user]
status: guide
updated: 2026-06-26
---

# Guides

An ordered learning trail for Relativist — a distributed Interaction Combinator reducer (Lafont 1997) in Rust. Workers reduce partitions locally; a coordinator merges borders and iterates to normal form. No Rust knowledge required; basic terminal familiarity (Bash/PowerShell) is enough. **The guides are now in English.**

## learning-path

Read in order; each guide builds on the previous one.

| # | Guide | What you learn |
|---|-------|----------------|
| 1 | [Getting Started](getting-started.md) | Install `relativist` (script, Docker, source) and the 3 symbols + 6 interaction rules of IC |
| 2 | [First Reduction](first-reduction.md) | `generate` a net, `inspect` it, and `reduce` it to normal form |
| 3 | [Local Grid](local-grid.md) | In-process distribution with `local -w N` and the BSP reduction cycle |
| 4 | [Distributed TCP](distributed-tcp.md) | Run `coordinator` + `worker` across machines or containers over TCP |
| 5 | [Church Arithmetic](church-arithmetic.md) | Encode `add`/`mul` in IC via `compute`, with parallel workers |
| 6 | [v2 Features](v2-features.md) | Delta protocol, zero-copy, elastic grid, streaming, arena management |

## reference

- [CLI Reference](../reference/cli.md) — every flag and subcommand.
- [Docs Catalog](../README.md) — full documentation index.

## conventions

- **Bash examples.** On Windows use Git Bash or WSL2; PowerShell variants are marked `# Windows (PowerShell)`.
- **Self-contained blocks.** Each command is copy-paste runnable, assuming `relativist` is on `PATH`.
- **Spec pointers.** When formalization matters, guides link the matching `docs/specs/SPEC-XX-*.md` instead of duplicating it.
- **Language.** Guides are English; specs (`docs/specs/`) are English to match the literature.
