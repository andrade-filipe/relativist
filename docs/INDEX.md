# Documentation Index

> **542 documents** organized by purpose. This index helps navigate the project's documentation.

---

## For Users

| Document | Description |
|----------|-------------|
| [USAGE_GUIDE.md](../USAGE_GUIDE.md) | Complete command reference — every subcommand, every flag, end-to-end pipelines |
| [README.md](../README.md) | Project overview, quick start, key results |

## For Contributors

| Document | Description |
|----------|-------------|
| [CONTRIBUTING.md](../CONTRIBUTING.md) | Development guidelines, code style, PR workflow |
| [GIT-WORKFLOW.md](GIT-WORKFLOW.md) | Branch and commit conventions (GitHub Flow) |
| [DEVELOPMENT-PIPELINE.md](DEVELOPMENT-PIPELINE.md) | 7-stage development pipeline |
| [SPEC-REVIEW-PIPELINE.md](SPEC-REVIEW-PIPELINE.md) | Adversarial spec review process |

## Formal Specifications (17)

All specs live in [`specs/`](../specs/). See the [README specs table](../README.md#specs) for the full list with requirement counts.

| Range | Scope |
|-------|-------|
| SPEC-00 to SPEC-05 | Core theory: glossary, invariants, net representation, reduction, partition, merge |
| SPEC-06 to SPEC-09 | Infrastructure: wire protocol, deployment, test strategy, benchmarks |
| SPEC-10 to SPEC-14 | Cross-cutting: security, observability, user I/O, architecture, arithmetic encoding |
| SPEC-15 to SPEC-16 | Operations: distribution & packaging, worker daemon mode |

## Benchmark Results

| Document | Description |
|----------|-------------|
| [V1-LOCAL-BASELINE-SUMMARY.md](V1-LOCAL-BASELINE-SUMMARY.md) | Phase 1 + Phase 2 key results and analysis |
| [PHASE1-FINDINGS.md](PHASE1-FINDINGS.md) | Phase 1 detailed findings (in-process, 3800 reps) |
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

## Internal (Historical)

These directories contain detailed records of the development process:

| Directory | Files | Description |
|-----------|-------|-------------|
| [backlog/](backlog/) | 207 | Task definitions across 11 implementation phases |
| [reviews/](reviews/) | 60 | Code review documents per task |
| [spec-reviews/](spec-reviews/) | 45 | Adversarial spec review rounds (critic + defender + impact) |
| [tests/](tests/) | 154 | Pre-implementation test specifications |
