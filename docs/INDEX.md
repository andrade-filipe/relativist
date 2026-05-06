# Documentation Index

> Live navigation. Per-bundle artefacts (qa, reviews, spec-reviews, plans, handoffs, backlog tasks, test-specs) for closed bundles live under each directory's `archive/` subfolder. This index points only to **active** documents.

---

## User Documentation

Start here if you want to **use** Relativist.

| Document | Description |
|----------|-------------|
| [**guides/**](guides/README.md) | Step-by-step learning path (10 numbered guides, Portuguese-BR) |
| [guides/08-elastic-grid.md](guides/08-elastic-grid.md) | SPEC-20 — hybrid coordinator + dynamic join/departure |
| [guides/09-streaming-generation.md](guides/09-streaming-generation.md) | SPEC-21 — chunked generation + streaming partitioning |
| [guides/10-arena-management.md](guides/10-arena-management.md) | SPEC-22 — free-list recycle + dense/sparse routing |
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
| [WORKFLOWS.md](WORKFLOWS.md) | Unified 6-stage development, spec review, and git pipelines |

## Formal Specifications

All specs live in [`specs/`](../specs/). See the [README specs table](../README.md#specs) for the full list with requirement counts.

| Range | Scope |
|-------|-------|
| SPEC-00 to SPEC-05 | Core theory: glossary, invariants, net representation, reduction, partition, merge |
| SPEC-06 to SPEC-09 | Infrastructure: wire protocol, deployment, test strategy, benchmarks |
| SPEC-10 to SPEC-14 | Cross-cutting: security, observability, user I/O, architecture, arithmetic encoding |
| SPEC-15 to SPEC-16 | Operations: distribution & packaging, worker daemon mode |
| SPEC-17 onwards | v2: transport abstraction, wire format v2 (SPEC-18), delta protocol (SPEC-19), elastic grid, streaming, arena, compact memory, WAN, recipe gen, GUI, encoder API |

Guides cover the user-facing v2 features: [06-delta-protocol](guides/06-delta-protocol.md) (SPEC-19), [07-zero-copy](guides/07-zero-copy.md) (SPEC-18), [08-elastic-grid](guides/08-elastic-grid.md) (SPEC-20), [09-streaming-generation](guides/09-streaming-generation.md) (SPEC-21), [10-arena-management](guides/10-arena-management.md) (SPEC-22).

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
| [benchmarks/historical_v1/V1-LOCAL-BASELINE-SUMMARY.md](benchmarks/historical_v1/V1-LOCAL-BASELINE-SUMMARY.md) | Phase 1 + Phase 2 key results and analysis |
| [benchmarks/historical_v1/PHASE1-FINDINGS.md](benchmarks/historical_v1/PHASE1-FINDINGS.md) | Phase 1 detailed findings (in-process, 3800+ reps) |
| [benchmarks/historical_v1/PHASE2-FINDINGS.md](benchmarks/historical_v1/PHASE2-FINDINGS.md) | Phase 2 detailed findings (Docker/TCP, 400 reps) |
| [benchmarks/historical_v1/V1-LOCAL-BASELINE-ANALYSIS.md](benchmarks/historical_v1/V1-LOCAL-BASELINE-ANALYSIS.md) | Post-campaign analysis |
| [benchmarks/DATA-COLLECTION-PLAN.md](benchmarks/DATA-COLLECTION-PLAN.md) | Benchmark data collection strategy |
| [benchmarks/benchmark-relevance-analysis.md](benchmarks/benchmark-relevance-analysis.md) | Benchmark selection rationale |

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
| [progress.md](progress.md) | Overall history and phase summary |
| [ROADMAP.md](ROADMAP.md) | v2+ roadmap (theoretical descriptions) |
| [next-steps.md](next-steps.md) | **Active planning, pipeline state & deferred work** |

## Benchmark Results (post-D-012)

| Document | Description |
|----------|-------------|
| [analysis/D011-final-baseline-analysis-2026-05-04.md](analysis/D011-final-baseline-analysis-2026-05-04.md) | Cold post-mortem analysis: 9 red flags + verdict + per-component decomposition |
| `results/locked/v2_post_d012_baseline_2026-05-05/` | **Canonical v2 baseline** — 32 distributed slots all_correct=true, mips/network/compute populated |
| `results/locked/v1_local_baseline/` | v1 frozen baseline (Phase 1 + Phase 2, 4600 reps, 0 failures) |
| [benchmarks/campaigns/stress-curve.md](benchmarks/campaigns/stress-curve.md) | **Stress Curve (v2)** — methodology for N up to 10⁹ campaign; locked dir produced by TASK-0708 |

## Internal (Historical / archived)

Per-bundle records (D-005..D-012) live under archive subfolders. Browse by directory:

| Directory | Active count | Archive | What lives there |
|-----------|--------------|---------|-------------------|
| [backlog/](backlog/) | 0 active TASKs | [archive/](backlog/archive/) (425 files) | Atomic task definitions per bundle |
| [tests/](tests/) | 0 active TEST-SPECs | [archive/](tests/archive/) (332 files) | Pre-implementation test specifications |
| [qa/](qa/) | 0 active | [archive/](qa/archive/) (16 files) | Adversarial QA bug reports per bundle |
| [reviews/](reviews/) | 0 active | [archive/](reviews/archive/) (85 files) | Code review reports per bundle |
| [spec-reviews/](spec-reviews/) | 0 active | [archive/](spec-reviews/archive/) (69 files) | Adversarial spec review rounds (critic + defender) |
| [plans/](plans/) | 0 active | [archive/](plans/archive/) (6 files) | Per-bundle implementation plans |
| [handoffs/](handoffs/) | 0 active | [archive/](handoffs/archive/) (5 files) | Per-bundle dispatch handoffs |
| [briefings/](briefings/) | 0 active | [archive/](briefings/archive/) (7 files) | Per-bundle research briefings |

When the next bundle (D-013+) opens, the relevant TASK / TEST-SPEC / QA / REVIEW files will appear in the active directories until the bundle closes, at which point they move to archive/. The `BACKLOG.md` and `tests/INDEX.md` still live in the active root.
