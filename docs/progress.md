# Progress — Relativist Software Implementation

**Last updated:** 2026-04-24 (D-005 Option A CLOSED — **12/12 G1 parity gate GREEN**, both iteration orders, both feature configs)
**Updated by:** **D-005 Option A Stages 4-6 CLOSED**. Stage 4 REVIEW (`docs/reviews/REVIEW-D-005-2026-04-24.md`) pinpointed root cause H5 ("coordinator never tells workers about promoted borders") with code citations; refuted H1-H4 from the orchestration plan. Stage 5 QA (`docs/qa/QA-D-005-2026-04-24.md`) enumerated 3 CRITICAL + 3 HIGH + 3 MEDIUM + 4 LOW findings and a 12-UT test matrix. Stage 6 REFACTOR addressed all 6 CRITICAL/HIGH items: CRIT-1 changed `BorderGraph::register_minted_agents` signature to return `Result<Vec<(u32, BorderState)>, GridError>` enumerating promoted borders (F-C1); CRIT-2 added `apply_promoted_borders_to_cache` + `promoted_borders_to_per_worker_new_borders` helpers with strict worker-a/side-a / worker-b/side-b routing and self-sentinel guards (F-C2 + F-H6); CRIT-3 plumbed per-worker `new_borders` into the next round's `RoundStartDispatch` via in-band append (F-C3); F-H7 added `debug_assert!` on cross-arena target injection in `apply_border_deltas_to_partition`; F-H8 added `check_delta_convergence_post_resolve` + `every_border_has_inert_remote` helpers that treat principal-port borders with a Lafont-concrete remote as inert, plus a `promotion_forces_next_round` short-circuit guard that defers convergence one round so workers observe promoted wires before final merge; F-H8 tail extends `partition.border_id_end` when a promoted border arrives out-of-range so `rebuild_free_port_index` keeps the entry; F-H6 tail adds `promoted_intra_worker_lafont_wires` helper that emits intra-worker Lafont promotions as direct `local_reconnections` and evicts the spurious border_id from `border_graph.borders` so merge does not attempt to restore a one-sided wire. 12 new UTs per QA matrix (UT-D005-01..12) landed across `border_graph.rs`, `grid.rs`, `grid_delta_integration_tests.rs`. Test counts: **1181 lib default** / **1224 lib `--features zero-copy`** (+13 / +13 over D-005 Stages 0-3 baseline, crossing the QA thresholds ≥1180 / ≥1223). UT-0385-08 green on both iteration orders (`[false, true]` and `[true, false]`). Clippy + fmt clean both configs. D-003 + D-004 + D-005 now CLOSED — cascade dependency cleared.

**Previous update (2026-04-23):** D-005 Option A Stages 0-3 SHIPPED — 11/12 G1 parity gate green; 1 asymmetric case open for Stage 4+
**Updated by:** **D-005 Option A — Worker-side application of `CommutationBatch.local_wiring` for minted agents (production, wire-level) — Stages 0-3 DONE**. Three-round adversarial spec review (R1 BLOCK 12 findings → R2 BLOCK 5 NFs → R3 SIGN-OFF) landed the SPEC-19 §3.4 Shape A amendment: `PendingCommutation { request_id, target_symbols: Vec<Symbol>, local_wiring: Vec<LocalWiringHint> }`, new `ProtocolError::MalformedLocalWiring { request_id, reason }` with `MalformedLocalWiringReason` enum (7 cases), `PROTOCOL_VERSION` bump 2→3, R23a clauses 1-6 (mint-then-wire ordering + HashSet pre-pass), R24.1.6a/b/c echo semantics, R48a stray slot-marker guard, R48b empty-wiring legality. Four tasks decomposed (TASK-0400 wire structs, TASK-0401 resolver-to-wire transport via `commutation_batch_to_pending`, TASK-0402 worker mint-then-wire in `apply_pending_commutation`, TASK-0403 `LocalDeltaDispatch` forwarding + `SKIP_ASYMMETRIC=false` flip). Test counts: **1168 lib default** / **1211 lib `--features zero-copy`** (+22 / +25 over D-004 baseline). Clippy + fmt clean both feature configs.

**D-005 status after Stage 3:** **11/12 UT-0385-08 gate cases green**; one case failing: asymmetric **CON-DUP strict=false** — v1 produces the expected 4-agent commutation residue, v2 delta collects an empty net via `run_grid_delta_final_collect`. Structural issue localised between `apply_pending_commutation` (mint + wire correct per UT-0402-11) and `handle_final_state_request` / merge path. Symmetric rules (CON-CON, DUP-DUP, ERA-ERA) both strict modes green; remaining asymmetric cases (CON-ERA, DUP-ERA) blocked cascading behind CON-DUP failure. Requires Stage 4 (REVIEW) + Stage 5 (QA) + Stage 6 (REFACTOR) to surgically locate the drop point and restore 12/12.

**D-003 status:** still PARTIAL — symmetric-rule G1 parity verified; asymmetric flip pending D-005 Stage 4+.

**D-004 status after D-005 Stages 0-3:** coordinator-side plumbing remains complete (from 2026-04-23 earlier bundle); full closure gates on the D-005 CON-DUP asymmetric fix shipping.

**Next milestone:** D-005 Stage 4 REVIEW (unified code quality + architecture) with targeted scope on the v2 final-collect path (`merge/grid.rs::run_grid_delta_final_collect`, `merge/grid_delta_integration_tests.rs::dispatch_final_state_request`, `merge/core::merge`, `partition::cleanup_t1_violations`). Root cause hypothesis: minted agents land in worker partition but are dropped during merge or by T1 cleanup.

**Previous update (2026-04-23):** D-004 Coordinator-Side Round-N+2 Finalizer bundle CLOSED (plumbing-only scope). TASK-0398/0399 shipped; test counts 1146 / 1186; review verdict ALIGNED, 0 Must-Fix, 2 Should-Fix. `SKIP_ASYMMETRIC=true` retained pending D-005.

**Previous update (2026-04-23):** SPEC-19 §3.3 Refactor bundle CLOSED (commit `08722e0` on `v2-development`). Four MF/SF tasks shipped (TASK-0394 worker R23/R26 completion, TASK-0395 G1 parity integration tests, TASK-0396 R20 dispatcher fork, TASK-0397 R43 normalize) plus 3 adversarial QA probes. Test counts: 1138 lib default / 1178 lib `--features zero-copy`. D-003 PARTIALLY CLOSED; D-004 opened (closed today via this bundle, plumbing scope).

**Previous update (2026-04-11):** Local Benchmark Phase closed — tag `v0.10.0-bench` released. L2 (BSP multi-round) resolved architecturally via opt-in `strict_bsp` mode (SPEC-05 R30a) + `reduce_border_once` primitive. New benchmark `cascade_cross` (SPEC-09 R18) forces rounds > 1 under strict mode. Unified `v1_local_baseline` campaign: Phase 1 (11:39, 4200 reps, 12 lenient + 2 strict) + Phase 2 (43:42, 400 reps Docker/TcpLocalhost), **0 failures in 4600 runs**. SPEC-09 theoretical predictions confirmed empirically: `cascade_cross(N)` terminates in `N` rounds and `dual_tree(d)` in `d` rounds under strict BSP with `workers ≥ 2`. Frozen snapshot in `results/locked/v1_local_baseline/` with full provenance (`manifest.md` + `.gitattributes` fixing LF for sha256 reproducibility on Windows). 655+ tests (643 pre-existing + 12 new: T-strict-1..6, T-cc-1..5, T-equivalence-strict-lenient).

---

## Current State

| Phase | Status | Detail |
|-------|--------|--------|
| Specs (core) | 10/10 COMPLETE | SPEC-00 to SPEC-09 revised in English (v2+; SPEC-02/03/04/06/08/09 at v3, SPEC-13 at Revised v2 via adversarial review) |
| Specs (end-to-end) | 5/5 REVISED | SPEC-10 (Security v2), SPEC-11 (Observability v2), SPEC-12 (User I/O v2), SPEC-13 (System Architecture v2), SPEC-14 (Encoding v2). SPEC-15 (Distribution) at Draft v2 |
| Research library | 24/24 COMPLETE | PESQ-001 to PESQ-024 in `docs/pesquisa/` |
| Open decisions | 8/8 RESOLVED | See PESQ-023 (Decision Matrix) |
| Open-source setup | COMPLETE | LICENCE, README, CONTRIBUTING, GitHub templates (.github/) |
| Rust scaffolding | COMPLETE | Cargo.toml, src/ (10 modules + CLI skeleton + error types), compiles clean |
| Docker | COMPLETE | Dockerfile (multi-stage), docker-compose.yml, .dockerignore |
| CI/CD | COMPLETE | .github/workflows/ci.yml (fmt, clippy, test, build), docker.yml (tag push) |
| Git workflow | COMPLETE | docs/GIT-WORKFLOW.md (GitHub Flow, conventional commits, SSH auth) |
| Development pipeline | DEFINED | 6 agents + DEVELOPMENT-PIPELINE.md + backlog structure |
| Human Check | COMPLETE | Blocos 1-8 reviewed. All OQs resolved across SPEC-01, 02, 07 |
| Task decomposition | COMPLETE | 206 tasks across 11 phases (SPEC-02 through SPEC-14) |
| Phase 1: Core Types | COMPLETE | SPEC-02: 20/20 tasks done, v0.1.0 tagged |
| Phase 2: Reduction | COMPLETE | SPEC-03: 12/12 tasks done, v0.2.0 tagged |
| Phase 3: Partition | COMPLETE | SPEC-04: 13/17 P0 tasks done, v0.3.0 tagged (291 tests) |
| Phase 4: Merge & Grid | COMPLETE | SPEC-05: 18/18 tasks done, v0.4.0 tagged (341 tests) |
| Phase 5: Wire Protocol | COMPLETE | SPEC-06: 17/18 tasks done, v0.5.0 tagged (396 tests). TASK-0212 deferred (needs Transport trait from SPEC-13) |
| Phase 6: CLI & Config | COMPLETE | SPEC-07+SPEC-13: 17/20 tasks done (456 tests). 7 subcommands, FSMs, local mode end-to-end. TASK-0117/0118 deferred (P1/P2) |
| Phase 7: Security | COMPLETE | SPEC-10: 14/20 tasks done (490 tests). AuthToken, 3-tier security, Register handshake with token validation, TLS configs, SecurityConfig builder. TASK-0132/0133 (TLS handshake integration), 0134/0135 (conn limits/timeout), 0139 (integration tests) deferred (P1/P2) |
| Phase 8: Observability | COMPLETE | SPEC-11: 10/21 tasks done (497 default / 507 w/ metrics). LogFormat, ProcessRole, ObservabilityConfig, init_tracing (text+JSON+EnvFilter), CoordinatorMetrics, /health /ready /metrics endpoints, spawn_metrics_server. TASK-0145-0149 (#[instrument]/FSM logging), 0151/0153 (protocol+aggregation metrics), 0158/0159 (OTel), 0213/0214 deferred (P1/P2) |
| Phase 9: User I/O | COMPLETE | SPEC-12: 15/20 tasks done (534 tests). Binary/IC format load/save, text DSL parser+serializer, format dispatch, NetSummary, 6 generators (EP-annihilation, CON-CON, DUP-DUP, CON-DUP expansion, dual-tree, mixed-rules). TASK-0167 (JSON), 0176/0177 (tree_sum, Church generators), 0179 (integration tests) deferred (P1/P2) |
| Phase 10: Benchmarks | COMPLETE | SPEC-09: 12 benchmarks (EP-ERA/CON/DUP, CON-DUP, DualTree, TreeSum, TreeSumBalanced, MixedNet, ErasureProp, ChurchAdd, ChurchMul, **cascade_cross**), suite runner, CSV output, correctness verification |
| Phase 11: Encoding | COMPLETE | SPEC-14: Church numeral encode/decode, add, mul, exp (with known exp limitation) |
| Distribution | COMPLETE | SPEC-15: GitHub Releases (Windows .exe, Linux .tar.gz/.deb), Docker GHCR, install script, self-update command, shell completions. v0.9.0 |
| **Strict BSP (L2 resolved)** | **COMPLETE** | **SPEC-05 R30a opt-in `strict_bsp` mode in `run_grid` + `reduce_border_once` primitive. Default `strict_bsp=false` preserves all prior behavior (zero regression across 643+ tests). CLI flag `--strict-bsp` + 12 new tests (T-strict-1..6, T-cc-1..5, T-equivalence-strict-lenient). Empirically validated: `cascade_cross(N) = N` rounds and `dual_tree(d) = d` rounds under strict + `workers ≥ 2`** |
| **Local Benchmark Phase (v1_local_baseline)** | **FROZEN** | **Tag `v0.10.0-bench` (commit 787b195 moved via force-push). Phase 1: 11:39 wall-clock, 3800 lenient reps + 400 strict reps, 0 correctness failures. Phase 2: 43:42 wall-clock, 400 Docker reps, 0 failures, L6 configs unblocked (dual_tree=22 w=1, ep_annihilation_con=5M w∈{1,2,4} now within 1800s timeout). CV triage: 63 flagged / 63 keep / 0 rerun / 0 exclude. Snapshot in `results/locked/v1_local_baseline/` with `manifest.md`, `README.md`, `cv_triage.md`, `.gitattributes` (LF enforcement for cross-platform sha256), raw logs in `raw/phase{1,2}/`. Immutable reference for Phase 3 LAN subtraction.** |
| **v1 Freeze** | **FROZEN** | **Branch `v1-feature-complete` at tag `v0.10.0-bench` — inviolable floor of 690 passing tests. All v2 work happens on `v2-development` branch with zero-regression invariant on the v1 test surface.** |
| **v2 Tier 1 M1 — Transport Optimization** | **DONE** | ROADMAP 2.22 (TCP tuning, commit c360fe5), 2.25 (same-host UDS fast path, commit c360fe5), 2.23 (Wire Format v2 bincode v2+LZ4+compact PortRef+frame header v2+protocol version bump), 2.24 (rkyv zero-copy archive) all shipped. See `V2-FEATURE-MATRIX.md`. |
| **v2 Tier 1 M3 — Delta Foundation** | **DONE** | ROADMAP 2.34 (coordinator-free round merge avoidance, SPEC-19 §3.1), 2.35 (BorderGraph pure-core data structure, SPEC-19 §3.2) shipped 2026-04-16/17. SPEC-27 encoder/decoder API (6 phases) shipped as cross-cutting enabler. |
| **v2 Tier 1 M4 — Full Delta Protocol (item 2.26)** | **PARTIAL** | Bundle 2.26 A/B/C/D DEV shipped 2026-04-18 (commit `4abd70c`). SPEC-19 §3.3 Refactor bundle shipped 2026-04-23 (commit `08722e0`): MF-001 + MF-002 + SF-001 + SF-002. **Symmetric IC rules** (CON-CON, DUP-DUP, ERA-ERA) G1 parity with v1 `run_grid` empirically verified via `merge::grid_delta_integration_tests::ut_0385_06/07/08` under both strict modes. **D-004 coordinator-side round-N+2 finalizer plumbing shipped 2026-04-23** via TASK-0398 (pure-core plumbing) + TASK-0399 (integration wire): `RoundResultPayload.minted_agents`, `BorderGraph::{enqueue_pending_borders, register_minted_agents}` with R48 validation + DC-B6 preserve path, `encode_request_id`/`decode_request_id` codec, `run_grid_delta_inner` wiring, 8 UT-0398-01..08. **Asymmetric IC rules** flip still gated behind `const SKIP_ASYMMETRIC: bool = true` pending **D-005** (worker-side application of `CommutationBatch.local_wiring` — `PendingCommutation` wire message does not carry the wiring instructions, so minted agents arrive DISCONNECTED). |
| **Tests (current baseline)** | **GREEN** | `cargo test --workspace --lib` = 1146 passing / `--features zero-copy` = 1186 passing (+8/+8 from UT-0398-01..08). Zero regression on v1's 690 test floor. Clippy `-D warnings` clean both feature configs. fmt clean. |

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

### 2. Infrastructure setup (COMPLETE)

#### Rust scaffolding ✓
- [x] Cargo.toml with metadata and all dependencies (serde, bincode, clap, tokio, rayon, etc.)
- [x] src/lib.rs with 10 module declarations
- [x] src/main.rs with clap CLI skeleton (4 subcommands)
- [x] src/error.rs with RelError enum (thiserror)
- [x] 9 module stubs (net, reduction, partition, merge, protocol, config, security, observability, io)
- [x] benches/benchmarks.rs (criterion placeholder)

#### Docker ✓
- [x] Dockerfile (multi-stage: rust:slim build → debian:bookworm-slim runtime)
- [x] docker-compose.yml (coordinator + N workers via NUM_WORKERS)
- [x] .dockerignore

#### CI/CD ✓
- [x] .github/workflows/ci.yml (fmt, clippy, test, build release)
- [x] .github/workflows/docker.yml (build on tag push v*)
- [x] .github/ISSUE_TEMPLATE/bug.md
- [x] .github/ISSUE_TEMPLATE/feature.md
- [x] .github/PULL_REQUEST_TEMPLATE.md

#### Git workflow ✓
- [x] docs/GIT-WORKFLOW.md (GitHub Flow, conventional commits)
- [x] SSH authentication configured (git@github.com:andrade-filipe/relativist.git)

### 3. Repository setup (COMPLETE)
- [x] Created github.com/andrade-filipe/relativist
- [x] Specs, research, agents migrated to repo
- [x] CI/CD workflows in place
- [x] Referenced from TCC repo as submodule

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

Documented for future work, not implementation scope. See **[ROADMAP.md](ROADMAP.md)** for full details, including the elastic grid architecture (coordinator-as-worker, dynamic worker joining, distributed coordination) enabled by strong confluence.

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
| 2026-04-04 | Human Check complete (blocos 1-8) | All specs reviewed, OQs resolved in SPEC-01, 02, 07 |
| 2026-04-04 | Rust scaffolding complete | Cargo.toml, 10 modules, CLI skeleton, CI/CD, Docker |
| 2026-04-04 | HVM2-style compact repr → ROADMAP | Documented as v2 optimization (ROADMAP 2.15), not v1 scope |
| 2026-04-04 | Task decomposition started | Phases 1-5 (SPEC-02 to SPEC-06) being split in parallel |
| 2026-03-26 | proptest for invariants | PESQ-022: P1, P2, P3, T1-T7, D1-D6 verified by property tests |
| 2026-04-05 | SPEC-04 adversarial review + revision | Round 1 critic (14 issues: 1 CRITICAL, 4 HIGH, 4 MEDIUM, 5 LOW). Round 2 defender: 10 ACCEPTED, 4 PARTIALLY ACCEPTED, 0 NOT ADDRESSED. Status -> Revised v3. Key changes: SC-001 (split/SPEC-13 FSM reconciliation), SC-005 (border_id_start/end for FreePort disambiguation), SC-004 (Scenario 2 FreePort never deleted), R15a (new), R28 (root port propagation). |
| 2026-04-05 | SPEC-03 adversarial review + revision | Round 1 critic (12 issues: 0 CRITICAL, 2 HIGH, 4 MEDIUM, 6 LOW). Round 2 defender: 10 ACCEPTED, 2 PARTIALLY ACCEPTED, 0 NOT ADDRESSED. Status -> Revised v3. Key changes: R25 (self-referencing aux port guard + link helper), R26 (FreePort boundary sentinels), interact_* preconditions, R12 counter management, O(1) amortized, T2 invariant note. |
| 2026-04-05 | SPEC-08 adversarial review + revision | Round 1 critic (17 issues: 2 CRITICAL, 5 HIGH, 5 MEDIUM, 5 LOW). Round 2 defender: 14 ACCEPTED, 3 PARTIALLY ACCEPTED, 0 NOT ADDRESSED. Status -> Revised v3. Key changes: scope expanded to SPEC-01-14 (60 tests from SPEC-10-14 incorporated), global test label namespace (SEC-/OBS-/UIO-/ARCH-/ENC-), I1-I11 -> INT1-INT11, R22-R24 -> R24-R26, PB13-PB16 added (T2/T3/Profile B/C), directory structure for all 11 modules, isomorphism SHOULD target (500 agents/100ms), prop_assume! for budget exhaustion, E9 relative timing. |
| 2026-04-05 | SPEC-06 adversarial review + revision | Round 1 critic (17 issues: 2 CRITICAL, 3 HIGH, 4 MEDIUM, 8 LOW). Round 2 defender: 12 ACCEPTED, 5 PARTIALLY ACCEPTED, 0 NOT ADDRESSED. Status -> Revised v3. Key changes: SC-001 (Message enum extended to 7 variants with Register/RegisterAck/RegisterNack from SPEC-10), SC-002 (FSM R26-R28 demoted to historical, superseded by SPEC-13 R19-R25), SC-003/SC-004 (ProtocolError reconciled as canonical, Io->ConnectionLost, AuthFailed added, WorkerError/WorkerCountMismatch moved to CoordinatorError), SC-005 (NodeConfig: host+port replaced with bind:SocketAddr, NodeRole removed, default 127.0.0.1:9000), SC-006 (WorkerRoundStats 6-field), R2a (tier-dependent registration), R11 (explicit bincode config), R12 (transitive reachability), R17 (default bind), R30 (collect_timeout upgraded to MUST). |
| 2026-04-11 | v1 frozen for TCC at `v0.10.0-bench` | Locked baseline for the paper's Phase 1+2 results; streaming/memory features (ROADMAP 2.27-2.36) classified as v2 future work. All subsequent work on `v2-development`. |
| 2026-04-18 | SPEC-19 Delta-Only Protocol bundle 2.26 A/B/C/D DEV shipped | Four sub-bundles (2.26-A wire extensions, 2.26-B coordinator-side border-redex resolver, 2.26-C coordinator dispatch + BSP loop, 2.26-D worker lifecycle + GridConfig.delta_mode) shipped as commit `4abd70c`. Per user directive "full review at the end, when all features are implemented" — held for unified REVIEW rather than per-sub-bundle. |
| 2026-04-23 | REVIEW unified SPEC-19 §3.3 + §3.5 + §3.6 item 2.26 B/C/D | `docs/reviews/REVIEW-SPEC-19-section-3.3-3.5-3.6-item-2.26-BCD-2026-04-23.md`. Verdict: code-quality PASS WITH NOTES; architecture ALIGNED. 2 Must-Fix (MF-001 worker R23/R26, MF-002 G1 parity test) + 5 Should-Fix. R19 pure-core invariant, DC-B1..B9 compliance matrix, 13/15 Q-probes mapped to existing tests. Triggered the Refactor bundle closed same day. |
| 2026-04-23 | SPEC-19 §3.3 Refactor bundle CLOSED | Commit `08722e0`. Four TASKs (0394/0395/0396/0397) shipped through the 6-stage SDD pipeline with zero regression. +29 tests default / +29 zero-copy. Discovered MF-003 during TASK-0395 DEV (pure-core `RoundResultPayload` missing `minted_agents` field); tracked as **DEFERRED-WORK D-004**. D-003 PARTIALLY CLOSED — symmetric IC rules G1-parity-verified. Top-level submodule pin bumped in `5449ea7`. |
| 2026-04-23 | Option B "cheap and equally formal" authorship | For TASK/TEST-SPEC .md files, prefer direct authorship over `task-splitter`/`test-generator` sub-agent dispatch when templates exist and scope is defined by upstream artifacts. Preserved in user-memory `feedback_direct_doc_authorship.md`. Reserve sub-agent dispatch for REVIEW (opus), QA adversarial, or code-heavy integration DEV (e.g., TASK-0395 LocalDeltaDispatch). |
| 2026-04-23 | D-004 picked as next bundle | Tracker cross-check (DEFERRED-WORK.md + V2-FEATURE-MATRIX.md + pipeline-state.md + this progress.md) confirms D-004 is the single unblocker for full D-003 closure AND Passo 6 M1 exit measurement. Scope ~300-400 LoC across 2 atomic TASKs (TASK-0398 plumbing + TASK-0399 integration). |
