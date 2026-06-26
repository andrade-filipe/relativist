# Documentation Index

Navigation for the **live** documentation. Frozen Spec-Driven-Development history
lives under [`_archive/`](_archive/README.md) and is intentionally not indexed here.

---

## Start here

| Goal | Document |
|------|----------|
| Project overview + 3-minute quick start | [../README.md](../README.md) |
| Use Relativist step by step | [guides/](guides/README.md) — 10-step learning path |
| Look up a command or flag | [reference/cli.md](reference/cli.md) |
| Understand the invariants (G1, D3, D6) | [reference/invariants.md](reference/invariants.md) |
| Debug an issue | [reference/troubleshooting.md](reference/troubleshooting.md) |
| Reproduce the paper's numbers | [../reproduce_article/README.md](../reproduce_article/README.md) |
| What the software should do next | [reference/next-steps.md](reference/next-steps.md) |

## Contributing & workflow

| Document | Description |
|----------|-------------|
| [../CONTRIBUTING.md](../CONTRIBUTING.md) | How to contribute; the RPI workflow |
| [../CODING_STANDARDS.md](../CODING_STANDARDS.md) | The code rules CI enforces |
| [../.claude/agents/README.md](../.claude/agents/README.md) | RPI agents (researcher / planner / implementer) |
| [../GOVERNANCE.md](../GOVERNANCE.md) | How decisions are made |
| [../CODE_OF_CONDUCT.md](../CODE_OF_CONDUCT.md) | Community standards |
| [../SECURITY.md](../SECURITY.md) | Reporting vulnerabilities |

## User guides

The numbered learning path in [`guides/`](guides/README.md). v2 feature guides:

| Guide | Spec |
|-------|------|
| [guides/06-delta-protocol.md](guides/06-delta-protocol.md) | SPEC-19 delta protocol |
| [guides/07-zero-copy.md](guides/07-zero-copy.md) | SPEC-18 zero-copy wire format |
| [guides/08-elastic-grid.md](guides/08-elastic-grid.md) | SPEC-20 elastic grid |
| [guides/09-streaming-generation.md](guides/09-streaming-generation.md) | SPEC-21 streaming generation |
| [guides/10-arena-management.md](guides/10-arena-management.md) | SPEC-22 arena management |

## Reference

| Document | Description |
|----------|-------------|
| [reference/cli.md](reference/cli.md) | Every subcommand and flag |
| [reference/file-formats.md](reference/file-formats.md) | `.bin` and `.ic` formats |
| [reference/invariants.md](reference/invariants.md) | G1/D3/D6 + amendments |
| [reference/troubleshooting.md](reference/troubleshooting.md) | Common errors, Windows/Docker notes |
| [reference/next-steps.md](reference/next-steps.md) | Roadmap headline + concrete next milestones |

## Formal specifications

All 28 specs live in [`../specs/`](../specs/) (English). Under the RPI workflow they
are **reference**, not a per-change gate.

| Range | Scope |
|-------|-------|
| SPEC-00 to SPEC-05 | Core theory: glossary, invariants, net representation, reduction, partition, merge |
| SPEC-06 to SPEC-09 | Infrastructure: wire protocol, deployment, test strategy, benchmarks |
| SPEC-10 to SPEC-14 | Cross-cutting: security, observability, user I/O, architecture, encoding |
| SPEC-15 to SPEC-16 | Operations: distribution & packaging, worker daemon mode |
| SPEC-17 onward | v2: transport, wire format v2, delta protocol, elastic grid, streaming, arena, compact memory, WAN, recipe gen, GUI, encoder API |

## Benchmarks & results

| Document | Description |
|----------|-------------|
| [benchmarks/README.md](benchmarks/README.md) | Bench suite overview, flags, 13 benchmarks |
| [benchmarks/phase-1-local.md](benchmarks/phase-1-local.md) | Phase 1 (in-process) workflow |
| [benchmarks/phase-2-docker.md](benchmarks/phase-2-docker.md) | Phase 2 (Docker TcpLocalhost) workflow |
| [benchmarks/phase-3-lan.md](benchmarks/phase-3-lan.md) | Phase 3 (real LAN) — pending |
| [benchmarks/limitations.md](benchmarks/limitations.md) | L1–L7 known limitations |
| [benchmarks/historical_v1/](benchmarks/historical_v1/) | v1 Phase 1+2 findings & analysis |
| [analysis/D011-final-baseline-analysis-2026-05-04.md](analysis/D011-final-baseline-analysis-2026-05-04.md) | Post-mortem of the v2 canonical baseline |

Frozen data + SHA-256 checksums: [`../reproduce_article/results/locked/`](../reproduce_article/results/locked/).

## Roadmap & research

| Document | Description |
|----------|-------------|
| [ROADMAP.md](ROADMAP.md) | v2+ feature roadmap + break-even analysis (§2.40) |
| [reference/next-steps.md](reference/next-steps.md) | Distilled next milestones for contributors |
| [pesquisa/INDICE.md](pesquisa/INDICE.md) | Research library (24 notes, 7 categories — PT-BR) |
| [theory-bridge.md](theory-bridge.md) | Bridge between IC theory and the implementation |

## Historical archive

[`_archive/`](_archive/README.md) holds the frozen SDD process record (backlog,
test specs, reviews, spec-reviews, plans, handoffs, briefings, QA, the retired SDD
agents, and the old `progress.md` / `next-steps.md` / `WORKFLOWS.md`). Read-only;
paths inside reflect the pre-reorganization layout.
