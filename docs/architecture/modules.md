---
title: Module map (code ↔ docs)
summary: Every source module and CLI subcommand mapped to its spec, its canonical doc, and its key public types.
keywords: [module, code map, source, net, reduction, partition, merge, protocol, coordinator, worker, encoding, io, security, observability, bench, config, commands, error, subcommand, public API]
modules: [net, reduction, partition, merge, protocol, coordinator, worker, config, commands, security, observability, io, encoding, bench, error]
specs: [SPEC-02, SPEC-03, SPEC-04, SPEC-05, SPEC-06, SPEC-07, SPEC-10, SPEC-11, SPEC-12, SPEC-13, SPEC-14]
audience: [contributor, llm]
status: reference
updated: 2026-06-26
---

# Module map (code ↔ docs)

The bridge between `src/` and the documentation. Reading code in a module? This table points to
the spec that defines it and the doc that explains it. Writing docs? It tells you which modules a
claim touches. Code comments cite bare `SPEC-NN` IDs (in [`docs/specs/`](../specs/)); this table
resolves those to prose docs.

Crates: **`relativist-core`** (the library — core + infrastructure) and **`relativist-cli`** (the
binary — `main.rs` dispatch). Dependency direction is inviolable
([overview](overview.md#dependency-direction-inviolable)).

## Core layer (pure, no async/I/O)

| Module | Purpose | Spec | Canonical doc | Key public API |
|--------|---------|------|---------------|----------------|
| `net` | IC net data structures: `Symbol`, `Agent`, `PortRef`, `Net`, CRUD | SPEC-02 | [theory/interaction-combinators](../theory/interaction-combinators.md), [reference/file-formats](../reference/file-formats.md) | `Symbol`, `Agent`, `PortRef`, `Net`, `AgentId` |
| `reduction` | The six interaction rules, dispatch, reduction loop | SPEC-03 | [theory/interaction-combinators](../theory/interaction-combinators.md) | `reduce_all`, `reduce_step`, `reduce_n` |
| `partition` | Split a net into K disjoint partitions; border markers; ID ranges; streaming | SPEC-04, SPEC-21 | [architecture/overview](overview.md), [guides/v2-features](../guides/v2-features.md) | `split`, `PartitionPlan`, `PartitionStrategy`, `generate_and_partition_chunked` |
| `merge` | Recombine reduced partitions; resolve border redexes; BSP grid loop; `BorderGraph` | SPEC-05, SPEC-19 | [architecture/overview](overview.md), [guides/v2-features](../guides/v2-features.md) | `merge`, `run_grid`, `BorderGraph`, `GridConfig`, `GridMetrics` |
| `encoding` | Church numerals, arithmetic combinators, Encoder/Decoder/Codec registry | SPEC-14, SPEC-27 | [guides/church-arithmetic](../guides/church-arithmetic.md), [demos/horner-g1-demonstration](../demos/horner-g1-demonstration.md) | `encode_nat`, `decode_nat`, `build_add/mul/exp`, `Codec`, `HornerCodec`, `EncoderRegistry` |
| `io` | `.bin`/`.ic`/`.json` load/save, example net generators, summaries | SPEC-12 | [reference/file-formats](../reference/file-formats.md) | `load_net_from_file`, `save_net_to_file`, `generate`, `net_summary` |
| `config` | clap CLI definitions → runtime config | SPEC-07, SPEC-13 | [reference/cli](../reference/cli.md) | `Cli`, `Command`, `*Args` structs |
| `observability` | tracing init, log format, metrics, health endpoints (feature-gated) | SPEC-11 | [operations/security-observability](../operations/security-observability.md) | `init_tracing`, `LogFormat`, `ObservabilityConfig` |
| `error` | Centralized error types; transient vs fatal; exit codes | SPEC-13 | [architecture/overview](overview.md) | `RelativistError` + per-module error enums |

## Infrastructure layer (async, tokio, network)

| Module | Purpose | Spec | Canonical doc | Key public API |
|--------|---------|------|---------------|----------------|
| `protocol` | Wire protocol: messages, length+CRC32 framing, transports, bincode v2 | SPEC-06, SPEC-18, SPEC-17 | [operations/docker](../operations/docker.md), [guides/v2-features](../guides/v2-features.md) | `Message`, `Transport`, `Frame`, `NodeConfig`, `TransportConfig` |
| `security` | 3-tier model: token auth, TLS 1.3 / mTLS | SPEC-10, SPEC-24 | [operations/security-observability](../operations/security-observability.md) | `SecurityTier`, `AuthToken`, `detect_tier` |
| `coordinator` | Coordinator FSM: round orchestration, membership, dispatch/collect/merge | SPEC-13, SPEC-19, SPEC-20 | [architecture/overview](overview.md) | `CoordinatorState/Event/Action`, `transition` |
| `worker` | Worker FSM: receive partition, `reduce_all`, return; daemon; delta reporting | SPEC-13, SPEC-16, SPEC-19 | [architecture/overview](overview.md) | `WorkerState/Event/Action` |
| `bench` | Parametric benchmark suite, execution modes, CSV output, stress-curve | SPEC-09 | [benchmarks/README](../benchmarks/README.md) | `Benchmark`, `BenchmarkId`, `run_benchmark_suite`, `Mode` |
| `commands` | Entry point per subcommand, dispatched from `main.rs` | SPEC-07, SPEC-13 | [reference/cli](../reference/cli.md) | `run_*_command` functions |

## CLI subcommands → behavior

The binary exposes 13 subcommands (full flags in [reference/cli](../reference/cli.md)):

| Subcommand | Purpose | Spec |
|------------|---------|------|
| `coordinator` | Run the grid coordinator (partition, dispatch, merge over TCP) | SPEC-07, SPEC-13 |
| `worker` | Connect to a coordinator and reduce assigned partitions | SPEC-07, SPEC-16 |
| `local` | In-process K-worker BSP simulation (no network) | SPEC-05, SPEC-13 |
| `reduce` | Pure sequential `reduce_all` on a net | SPEC-03 |
| `inspect` | Print net summary (agent counts, redexes, normal-form status) | SPEC-12 |
| `generate` | Generate an example workload net to a file | SPEC-12 |
| `compute` | Encode → reduce → decode arithmetic (Church / Horner codecs) | SPEC-14, SPEC-27 |
| `bench` | Run the benchmark suite, emit CSV | SPEC-09 |
| `validate` | Data-quality checks on benchmark CSV output | — |
| `update` | Self-update from GitHub releases | SPEC-15 |
| `completions` | Emit shell completion scripts | SPEC-15 |
| `encoders` (`codecs`) | List registered encoders/codecs | SPEC-27 |
| `decode` | Decode a `.bin` normal form back to JSON via a named codec | SPEC-27 |

## How to keep this in sync

When you add or move a module, spec, or subcommand, update this table and the catalog
([docs/README.md](../README.md)). The [`doc-curator`](../../.claude/skills/doc-curator/SKILL.md)
skill describes the standard; the [`doc-catalog`](../../.claude/agents/doc-catalog.md) agent
validates it.
