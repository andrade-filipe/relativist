# Progress — Relativist Software Implementation

**Last updated:** 2026-03-26
**Updated by:** ORCHESTRATOR — Development pipeline defined. 6 agents created/updated. Backlog structure ready.

---

## Current State

| Phase | Status | Detail |
|-------|--------|--------|
| Specs (core) | 10/10 COMPLETE | SPEC-00 to SPEC-09 revised in English (v2, with DISCs + ARGs) |
| Specs (end-to-end) | 4/4 DRAFT v1 | SPEC-10 (Security), SPEC-11 (Observability), SPEC-12 (User I/O), SPEC-13 (System Architecture) — all Draft v1 |
| Research library | 24/24 COMPLETE | PESQ-001 to PESQ-024 in `docs/pesquisa/` |
| Open decisions | 8/8 RESOLVED | See PESQ-023 (Decision Matrix) |
| Open-source setup | IN PROGRESS | LICENCE, README, CONTRIBUTING created |
| Rust scaffolding | NOT STARTED | No Cargo.toml, no src/ |
| Docker | NOT STARTED | No Dockerfile |
| CI/CD | NOT STARTED | No .github/workflows/ |
| Development pipeline | DEFINED | 6 agents + DEVELOPMENT-PIPELINE.md + backlog structure |
| Task decomposition | NOT STARTED | Run task-splitter on SPEC-02 to begin |
| Implementation | NOT STARTED | All specs complete; ready for task decomposition |

---

## Resolved Decisions (from PESQ-023)

| # | Decision | Resolution |
|---|----------|-----------|
| D1 | Workspace structure | Single crate + feature flags |
| D2 | Error handling | thiserror |
| D3 | Feature flags | tls, metrics, otel |
| D4 | Security model | 3-tier (none / token / token+TLS) |
| D5 | Observability | tracing + prometheus-client + OTel (optional) |
| D6 | Testing strategy | proptest + in-memory grid + Transport trait |
| D7 | Module structure | 10 modules, core/infra split |
| D8 | Programming model | BSP (Bulk Synchronous Parallel) |

---

## Next Steps (in order)

### 1. Write end-to-end specs
All research is complete. Each spec has its primary PESQ inputs identified:

- [x] **SPEC-13: System Architecture** — Draft v1 complete. 50 requirements (R1-R50). Consumes PESQ-010/012/013/023/024.
- [x] **SPEC-10: Security** — Draft v1 complete. 34 requirements (R1-R34). Consumes PESQ-005/017/018/019/023 (D4).
- [x] **SPEC-11: Observability** — Draft v1 complete. 37 requirements (R1-R37). Consumes PESQ-003/014/015/016/023 (D5).
- [x] **SPEC-12: User I/O & Examples** — Draft v1 complete. 52 requirements (R1-R52), 9 test requirements (T1-T9). Consumes PESQ-002/024, AC-005.

### 2. Infrastructure setup

#### Rust scaffolding
- [ ] Cargo.toml with metadata and initial dependencies
- [ ] src/lib.rs with module declarations
- [ ] src/main.rs with clap CLI skeleton
- [ ] tests/ directory structure
- [ ] benches/ directory

#### Docker
- [ ] Dockerfile (multi-stage: rust:slim build → debian:slim runtime)
- [ ] docker-compose.yml (coordinator + N workers)
- [ ] .dockerignore

#### CI/CD
- [ ] .github/workflows/ci.yml (fmt, clippy, test, build)
- [ ] .github/workflows/docker.yml (build + push image)
- [ ] .github/ISSUE_TEMPLATE/bug.md
- [ ] .github/ISSUE_TEMPLATE/feature.md
- [ ] .github/PULL_REQUEST_TEMPLATE.md

### 3. Repository migration
- [ ] Create github.com/[user]/relativist
- [ ] Migrate specs/ to new repo
- [ ] Set up CI/CD in new repo
- [ ] Reference from TCC repo (submodule or link)

---

## Implementation Order (after preparation)

Once all specs are finalized and infrastructure is ready:

| Phase | Specs | Modules | Tests |
|-------|-------|---------|-------|
| 1. Core types | SPEC-02 | net/ | N1-N17 |
| 2. Reduction | SPEC-03 | reduction/ | RE1-RE21 |
| 3. Partition | SPEC-04 | partition/ | P1-P21 |
| 4. Grid cycle | SPEC-05 | merge/ | I1-I11 |
| 5. Protocol | SPEC-06 | protocol/ | (protocol tests) |
| 6. CLI + Config | SPEC-07 | config/, main.rs | (CLI tests) |
| 7. Security | SPEC-10 | security/ | (security tests) |
| 8. Observability | SPEC-11 | observability/ | (metrics tests) |
| 9. I/O + Examples | SPEC-12 | io/ | (I/O tests) |
| 10. Benchmarks | SPEC-09 | benches/ | F1-F5 |

---

## Roadmap (NOT in v1)

Documented for future work, not implementation scope:

- Multi-tenancy / job queuing
- Automatic node discovery (Consul/etcd/mDNS)
- Full fault tolerance (checkpoint, re-dispatch, replication)
- Visualization (Graphviz export, live progress)
- Dynamic plugin system (custom partition strategies without recompilation)
- GPU worker support (heterogeneous compute)
- Kubernetes / Helm charts
- WASM target for browser-based reduction
- Load balancing / intelligent scheduling
- Full DST with Turmoil (PESQ-021)
- rayon intra-worker parallelism (PESQ-011)
- mTLS mutual authentication (PESQ-017)
- Coordinator high-availability (PESQ-010)

---

## Decisions Log

| Date | Decision | Context |
|------|----------|---------|
| 2026-03-24 | MIT License | Same as Haskell prototype |
| 2026-03-24 | Separate repository | Better for open-source visibility, contributions, CI/CD |
| 2026-03-24 | Specs in English | Code in English, LLMs work better, closer to original papers |
| 2026-03-24 | TIER 1 gaps in v1 | Security, observability, user I/O are basic requirements, not optional |
| 2026-03-24 | System Architecture spec first | SPEC-13 must come before SPEC-10, 11, 12 |
| 2026-03-26 | Single crate + feature flags | PESQ-023 D1: simpler for single developer |
| 2026-03-26 | thiserror for errors | PESQ-023 D2: typed errors needed for transient/fatal classification |
| 2026-03-26 | BSP programming model | PESQ-012: exact mapping to IC grid cycle |
| 2026-03-26 | 3-tier security model | PESQ-023 D4: dev (none), private (token), production (token+TLS) |
| 2026-03-26 | tracing + prometheus-client | PESQ-023 D5: ecosystem standard for Rust observability |
| 2026-03-26 | Transport trait abstraction | PESQ-020/021: enables in-memory testing + future DST |
| 2026-03-26 | proptest for invariants | PESQ-022: P1, P2, P3, T1-T7, D1-D6 verified by property tests |
