---
title: Documentation catalog
summary: The keyword-searchable entry point to all Relativist docs, with by-category and by-module cross-indexes.
keywords: [catalog, documentation, index, navigation, docs, keyword search, by module, frontmatter]
modules: []
specs: []
audience: [user, contributor, llm, researcher]
status: reference
updated: 2026-06-26
---

# Relativist documentation — catalog

The single entry point to Relativist's documentation, built for fast retrieval by both humans and
LLMs. **Find what you need by keyword:** every doc below lists its `keywords`; `grep` this file (or
any doc's frontmatter) for a term and follow the link. Each doc also carries YAML frontmatter
(`title`, `summary`, `keywords`, `modules`, `specs`, `audience`, `status`) so an agent can rank
relevance without opening the file.

> **Source-of-truth order:** the **code** (`relativist-core`, `relativist-cli`) is the living truth;
> the **[specs](specs/README.md)** define intent and invariants; **[`_archive/`](_archive/README.md)**
> holds frozen history. When docs and code disagree, the code wins — fix the doc.

## Start here

| If you want to… | Read |
|-----------------|------|
| Understand the model | [theory/interaction-combinators](theory/interaction-combinators.md) |
| Understand the system | [architecture/overview](architecture/overview.md) |
| Map code → docs | [architecture/modules](architecture/modules.md) |
| Install & run | [guides/getting-started](guides/getting-started.md) → [guides/README](guides/README.md) |
| Look up a command | [reference/cli](reference/cli.md) |
| Know what's next | [reference/next-steps](reference/next-steps.md) |
| Reproduce the paper | [../reproduce_article/README.md](../reproduce_article/README.md) |
| Contribute | [../CONTRIBUTING.md](../CONTRIBUTING.md) · [../CODING_STANDARDS.md](../CODING_STANDARDS.md) |

## Catalog

### Theory

| Doc | Summary | Keywords |
|-----|---------|----------|
| [theory/interaction-combinators](theory/interaction-combinators.md) | Lafont's 3 symbols, 6 interaction rules, strong confluence. | interaction combinators, CON, DUP, ERA, rules, redex, confluence, normal form |
| [theory/invariants](theory/invariants.md) | The correctness contract: T / D / I layers + the global G1 property. | invariant, T1–T7, D1–D6, I1–I5, G1, fundamental property, determinism |
| [theory-bridge](theory-bridge.md) | Index of the TCC arguments/discussions/references the specs cite. | theory bridge, ARG-001, confluence, citations |

### Architecture

| Doc | Summary | Keywords |
|-----|---------|----------|
| [architecture/overview](architecture/overview.md) | BSP model, pure-core/async-infra layering, dependency direction, FSMs. | architecture, BSP, layer, dependency direction, coordinator, worker, FSM |
| [architecture/modules](architecture/modules.md) | Every module + subcommand → spec → doc → key API (the code↔doc bridge). | module, code map, net, reduction, partition, merge, protocol, subcommand |

### Guides (learning path)

| Doc | Summary | Keywords |
|-----|---------|----------|
| [guides/getting-started](guides/getting-started.md) | Install the binary; the minimal IC vocabulary. | install, build, docker, release, active pair |
| [guides/first-reduction](guides/first-reduction.md) | generate → inspect → reduce a net sequentially. | generate, inspect, reduce, normal form |
| [guides/local-grid](guides/local-grid.md) | Full BSP cycle across N in-process workers; confirm G1. | local, bsp, workers, strict-bsp, G1 |
| [guides/distributed-tcp](guides/distributed-tcp.md) | Real coordinator + workers over TCP; bind, token auth. | distributed, tcp, coordinator, worker, token |
| [guides/church-arithmetic](guides/church-arithmetic.md) | Encode → reduce → decode arithmetic via `compute`. | church numerals, compute, encode, decode |
| [guides/v2-features](guides/v2-features.md) | Delta, zero-copy, elastic, streaming, arena — one section each. | delta protocol, zero-copy, elastic grid, streaming, arena |
| [demos/horner-g1-demonstration](demos/horner-g1-demonstration.md) | Horner's method as a concrete distributed-equals-sequential G1 witness. | horner, G1, codec, distributed, determinism |

### Reference

| Doc | Summary | Keywords |
|-----|---------|----------|
| [reference/cli](reference/cli.md) | All 13 subcommands, flags, examples (authored from code). | cli, subcommand, flag, coordinator, compute, bench, transport |
| [reference/file-formats](reference/file-formats.md) | `.bin` (bincode v2), `.ic` text DSL, `.json`; load/save APIs. | file format, .bin, .ic, bincode, text DSL |
| [reference/troubleshooting](reference/troubleshooting.md) | Symptom→fix for install, run, Docker, TCP, memory, build. | troubleshooting, error, windows, docker, memory |
| [reference/next-steps](reference/next-steps.md) | The critical path to break-even + Phase 3 LAN milestone. | next steps, break-even, delta protocol, phase 3 lan |
| [specs/](specs/README.md) | Index of the 28 formal specifications (SPEC-00..27). | spec, invariant, requirement, formal |

### Operations

| Doc | Summary | Keywords |
|-----|---------|----------|
| [operations/docker](operations/docker.md) | Build the image; coordinator/worker + bench via Docker Compose. | docker, docker-compose, scaling, container |
| [operations/security-observability](operations/security-observability.md) | 3-tier security (token, TLS) + tracing/metrics/health. | security, tls, token, tracing, metrics, prometheus |
| [MAINTAINER-GITHUB-SETUP](MAINTAINER-GITHUB-SETUP.md) | GitHub web-UI/`gh` steps to do before announcing. | maintainer, github, branch protection, launch |

### Benchmarks & results

| Doc | Summary | Keywords |
|-----|---------|----------|
| [benchmarks/README](benchmarks/README.md) | Entry point: `relativist bench` flags + phase/campaign links. | benchmarks, bench, csv, profiles |
| [benchmarks/phase-1-local](benchmarks/phase-1-local.md) | Run the in-process baseline campaign. | phase 1, local, in-process, baseline |
| [benchmarks/phase-2-docker](benchmarks/phase-2-docker.md) | Run the BSP protocol over TCP loopback in Docker. | phase 2, docker, tcp localhost |
| [benchmarks/phase-3-lan](benchmarks/phase-3-lan.md) | Pending cross-machine LAN campaign (true network overhead). | phase 3, lan, network, pending |
| [benchmarks/limitations](benchmarks/limitations.md) | The seven L-items with status and v2 pointers. | limitations, L1–L7, break-even |
| [benchmarks/DATA-COLLECTION-PLAN](benchmarks/DATA-COLLECTION-PLAN.md) | Experimental matrix, CSV schemas, statistics. | data collection, csv schema, statistics |
| [benchmarks/campaigns/v1-local-baseline](benchmarks/campaigns/v1-local-baseline.md) | Reproduce the frozen v0.10.0-bench snapshot. | v1 baseline, frozen, reproduce |
| [benchmarks/campaigns/stress-curve](benchmarks/campaigns/stress-curve.md) | Methodology for the N=10⁴–10⁹ scaling-wall campaign. | stress curve, scaling wall, oom |
| [benchmarks/d011-perf-fix-verification](benchmarks/d011-perf-fix-verification-2026-05-04.md) | Timing verification of the D-011 perf fix. | d-011, perf fix, timing |
| [analysis/D011-final-baseline-analysis](analysis/D011-final-baseline-analysis-2026-05-04.md) | Adversarial post-mortem of the canonical v2 baseline. | post-mortem, baseline, c_o/c_r, verdict |

### Roadmap

| Doc | Summary | Keywords |
|-----|---------|----------|
| [roadmap](roadmap.md) | v2+ feature rationale + break-even analysis §2.40. | roadmap, v2, break-even, elastic grid, delta protocol |

### Design records & history

- [superpowers/specs/](superpowers/specs/) — dated design records (open-source launch, this docs regeneration).
- [_archive/](_archive/README.md) — frozen SDD process history, research surveys (PESQ), historical benchmark findings, superseded design docs. Read-only; paths inside reflect older layouts.

## Cross-index: by code module

Reading `src/<module>`? Here are its docs (see [architecture/modules](architecture/modules.md) for the full map):

| Module | Primary docs |
|--------|--------------|
| `net` | [theory/interaction-combinators](theory/interaction-combinators.md), [reference/file-formats](reference/file-formats.md) |
| `reduction` | [theory/interaction-combinators](theory/interaction-combinators.md), [theory/invariants](theory/invariants.md) |
| `partition` | [architecture/overview](architecture/overview.md), [guides/v2-features](guides/v2-features.md) |
| `merge` | [architecture/overview](architecture/overview.md), [guides/local-grid](guides/local-grid.md), [guides/v2-features](guides/v2-features.md) |
| `protocol` | [guides/distributed-tcp](guides/distributed-tcp.md), [operations/docker](operations/docker.md), [guides/v2-features](guides/v2-features.md) |
| `security` | [operations/security-observability](operations/security-observability.md) |
| `observability` | [operations/security-observability](operations/security-observability.md) |
| `coordinator` / `worker` | [architecture/overview](architecture/overview.md), [guides/distributed-tcp](guides/distributed-tcp.md) |
| `encoding` | [guides/church-arithmetic](guides/church-arithmetic.md), [demos/horner-g1-demonstration](demos/horner-g1-demonstration.md) |
| `io` | [reference/file-formats](reference/file-formats.md) |
| `config` / `commands` | [reference/cli](reference/cli.md) |
| `bench` | [benchmarks/README](benchmarks/README.md) |

## Maintaining this catalog

Add or move a doc → update this file and the doc's frontmatter. The
[`doc-curator`](../.claude/skills/doc-curator/SKILL.md) skill is the writing standard; the
[`doc-catalog`](../.claude/agents/doc-catalog.md) agent validates that every live doc has
frontmatter and appears here.
