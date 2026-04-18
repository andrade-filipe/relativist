# Documentation Index

> **542 documents** organized by purpose. This index helps navigate the project's documentation.

---

## User Documentation

Start here if you want to **use** Relativist.

| Document | Description |
|----------|-------------|
| [**guides/**](guides/README.md) | Step-by-step learning path (7 numbered guides, Portuguese-BR) |
| [reference/cli.md](reference/cli.md) | Complete CLI reference — every subcommand, every flag |
| [reference/file-formats.md](reference/file-formats.md) | `.bin` and `.ic` formats |
| [reference/invariants.md](reference/invariants.md) | G1/D3/D6 + amendments (SPEC-19 bundle 2.26) |
| [reference/troubleshooting.md](reference/troubleshooting.md) | Common errors, Windows notes, Docker pitfalls |
| [benchmarks/](benchmarks/README.md) | Benchmark suite overview + Phase 1/2/3 workflows |
| [../README.md](../README.md) | Project overview, 3-minute quick start, key results |

The legacy monolithic `USAGE_GUIDE.md` has been split into the files above. If you arrived here from an old link, see the redirect stub in the project root.

## For Contributors

| Document | Description |
|----------|-------------|
| [CONTRIBUTING.md](../CONTRIBUTING.md) | Development guidelines, code style, PR workflow |
| [GIT-WORKFLOW.md](GIT-WORKFLOW.md) | Branch and commit conventions (GitHub Flow) |
| [DEVELOPMENT-PIPELINE.md](DEVELOPMENT-PIPELINE.md) | 6-stage development pipeline |
| [SPEC-REVIEW-PIPELINE.md](SPEC-REVIEW-PIPELINE.md) | Adversarial spec review process |

## Formal Specifications

All specs live in [`specs/`](../specs/). See the [README specs table](../README.md#specs) for the full list with requirement counts.

| Range | Scope |
|-------|-------|
| SPEC-00 to SPEC-05 | Core theory: glossary, invariants, net representation, reduction, partition, merge |
| SPEC-06 to SPEC-09 | Infrastructure: wire protocol, deployment, test strategy, benchmarks |
| SPEC-10 to SPEC-14 | Cross-cutting: security, observability, user I/O, architecture, arithmetic encoding |
| SPEC-15 to SPEC-16 | Operations: distribution & packaging, worker daemon mode |
| SPEC-17 onwards | v2: transport abstraction, wire format v2 (SPEC-18), delta protocol (SPEC-19), elastic grid, streaming, arena, compact memory, WAN, recipe gen, GUI, encoder API |

Guides cover the user-facing v2 features: [06-delta-protocol](guides/06-delta-protocol.md) (SPEC-19), [07-zero-copy](guides/07-zero-copy.md) (SPEC-18).

## Benchmark Results

| Document | Description |
|----------|-------------|
| [benchmarks/README.md](benchmarks/README.md) | Bench suite overview, flags, 13 benchmarks |
| [benchmarks/phase-1-local.md](benchmarks/phase-1-local.md) | Phase 1 (in-process) workflow |
| [benchmarks/phase-2-docker.md](benchmarks/phase-2-docker.md) | Phase 2 (Docker TcpLocalhost) workflow |
| [benchmarks/phase-3-lan.md](benchmarks/phase-3-lan.md) | Phase 3 (real LAN) workflow — pending |
| [benchmarks/limitations.md](benchmarks/limitations.md) | L1-L7 known limitations with status |
| [benchmarks/campaigns/v1-local-baseline.md](benchmarks/campaigns/v1-local-baseline.md) | Frozen baseline reproduction |
| [benchmarks/campaigns/v1-stress.md](benchmarks/campaigns/v1-stress.md) | Stress campaign (sizes beyond baseline) |
| [benchmarks/campaigns/church-sum-of-squares.md](benchmarks/campaigns/church-sum-of-squares.md) | Arithmetic demo |
| [V1-LOCAL-BASELINE-SUMMARY.md](V1-LOCAL-BASELINE-SUMMARY.md) | Phase 1 + Phase 2 key results and analysis |
| [PHASE1-FINDINGS.md](PHASE1-FINDINGS.md) | Phase 1 detailed findings (in-process, 3800+ reps) |
| [PHASE2-FINDINGS.md](PHASE2-FINDINGS.md) | Phase 2 detailed findings (Docker/TCP, 400 reps) |
| [V1-LOCAL-BASELINE-ANALYSIS.md](V1-LOCAL-BASELINE-ANALYSIS.md) | Post-campaign analysis |
| [DATA-COLLECTION-PLAN.md](DATA-COLLECTION-PLAN.md) | Benchmark data collection strategy |
| [benchmark-relevance-analysis.md](benchmark-relevance-analysis.md) | Benchmark selection rationale |

Frozen data: [`results/locked/v1_local_baseline/`](../results/locked/v1_local_baseline/) — SHA-256 checksums in `manifest.md`.

## Research Library (24 notes)

Organized in 7 categories in [`pesquisa/`](pesquisa/). See [`pesquisa/INDICE.md`](pesquisa/INDICE.md) for navigation.

| Category | Topics |
|----------|--------|
| 01-grid-architectures | BOINC, Apache Ignite, Ray, Dask, HTCondor |
| 02-rust-frameworks | Hydro, Paladin, Constellation, others |
| 03-design-patterns | Coordinator-worker, work-stealing, BSP, state machines |
| 04-observability | OpenTelemetry, tracing ecosystem, Prometheus |
| 05-security | TLS/mTLS, token auth, security lessons |
| 06-testing | DST concepts, turmoil/madsim, property-based testing |
| 07-synthesis | Decision matrix, architecture recommendations |

## Project Tracking

| Document | Description |
|----------|-------------|
| [progress.md](progress.md) | Current implementation state and phase summary |
| [ROADMAP.md](ROADMAP.md) | v2+ roadmap (confluence-enabled features) |
| [V2-FEATURE-MATRIX.md](V2-FEATURE-MATRIX.md) | Consolidated v2 feature inventory with milestones M1-M10 |
| [DEFERRED-WORK.md](DEFERRED-WORK.md) | **Tracker of partially-shipped specs awaiting an unblocker** — check before closing any milestone |

## Internal (Historical)

These directories contain detailed records of the development process:

| Directory | Files | Description |
|-----------|-------|-------------|
| [backlog/](backlog/) | 207 | Task definitions across 11 implementation phases |
| [reviews/](reviews/) | 60 | Code review documents per task |
| [spec-reviews/](spec-reviews/) | 45 | Adversarial spec review rounds (critic + defender + impact) |
| [tests/](tests/) | 154 | Pre-implementation test specifications |
