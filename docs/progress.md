# Progress & History — Relativist Software Implementation

> **CRITICAL LLM INSTRUCTION:** This file is for **PAST/COMPLETED work only**. Active work, current pipeline state, and future milestones live exclusively in `next-steps.md`.

**Last updated:** 2026-04-25 (Phase B COMPLETED in commit `21cbfb4`. All 12 foundational hybrid-coordinator tasks implemented and verified across 4 waves. Phase C Joining is now ACTIVE.)
**Updated by:** **Phase B Final Closure (TASK-0415 through 0426).**
- **Wave 1:** `GridConfig` extension, `PROTOCOL_VERSION 4`, `compute_round_id_ranges`, `TimerKind`.
- **Wave 2:** CLI flags, `Message` elastic variants, `WorkerId 0` reservation, `SoloReducing` loop.
- **Wave 3:** `JoinRequest` handshake branch, `tokio::select!` 4-arm event loop.
- **Wave 4:** In-process self-worker spawn, strict BSP uniformity (R3c).
Test counts: 1256 default / 1299 zero-copy. v1 floor (690) preserved.
**Updated by:** **Pre-DEV Spec Pipeline closed across Waves 1+2 (Tier 2 + Tier 3); user directive 2026-04-25 to defer Tier 4+5 until after Tier 2+3 are shipped.** Wave 1 (SPEC-20 Elastic Grid, Tier 2 / M2): Stage 0+1+2 done in `ec680e4`. Wave 2 first half (SPEC-22 Arena Management, Tier 3 / M5): Stage 0 in `b66f758`, Stages 1+2 in `bbd976e` — 36 tasks `TASK-0460..0500` + 48 TEST-SPECs. Wave 2 second half (SPEC-21 Streaming Generation, Tier 3 / M5): Stage 0 in `508e595`, Stages 1+2 in `131ca26` — 36 tasks `TASK-0510..0554+0565/0567/0568/0575-0578/0588-0591` + 49 TEST-SPECs (35 plumbing + 14 spec-catalog T1..T14). Defensive PROTOCOL_VERSION pattern propagated across SPEC-20, SPEC-22, SPEC-21 (TEST-SPEC-0476/0511/0575/0576). Cumulative across Waves 1+2: **108 atomic tasks, 159 TEST-SPECs, 4490 benchmarks frozen at v1 baseline.** TCC-root deferred items: SC-013 (DISC-012 stale tag), SC-020 (FENNEL/LDG REF-NNN registration). 5 non-blocking gaps in SPEC-21 Stage 2 (IT-0577 exact-count, IT-0588 cross-spec block, IT-0578 wall-clock, UT-0568 SC-020 gate, PREVIOUS_LIVE_VERSION mechanism unification). Test counts unchanged across the entire Pre-DEV bundle: 1181 default / 1224 zero-copy. **Next phase: Tier 2 DEV starting bundle D-006 (SPEC-20 §3.1 hybrid coordinator).**

**Previous update (2026-04-25):** v2 Pre-DEV Wave 2 / SPEC-22 Arena Management CLOSED (first half) in commits `b66f758` + `bbd976e`.
**Updated by:** **v2 Pre-DEV Spec Pipeline — Wave 2 / SPEC-22 Arena Management CLOSED (first half of Wave 2; SPEC-21 Streaming Generation still pending).** Stage 0 (Spec Review): pesquisador coherence brief (3 predicted findings, all confirmed); spec-critic Round 1 verdict BLOCK with 21 findings (4C/7H/6M/4L); especialista-em-specs Round 2 closed 20/21 inline (1 deferred to TCC-root cleanup, SC-013 theory-bridge stale tag); 0 NOT_CLOSED, 0 escalations to Round 3; spec status Draft → `Reviewed v2`; spec grew 617 → 880 lines (+43%); §3.8 Amendments block authored (A1–A10) covering SPEC-01 (I3'), SPEC-02 (R2/R10/R11/R12), SPEC-03, SPEC-04 (R10a/R22), SPEC-05 (R12), SPEC-18 (PROTOCOL_VERSION 2→3), SPEC-19 (BorderGraph + R10b strategies). Stage 1 (Task Splitter): 36 atomic tasks `TASK-0460..0500` partitioned across Phase A (10 predecessor amendments), Phase B (8 free-list core), Phase C (5 distributed integration), Phase D (8 SparseNet), Phase E (4 invariant audits), Phase F (1 regression gate); BACKLOG.md SPEC-22 section + coverage matrix; ~2.07k LoC prod / ~1.83k LoC tests estimated; all 10 amendment R-numbers verified against predecessor specs. Stage 2 (Test Generator): 48 TEST-SPECs (25 plumbing + 23 spec-catalog T-tests T1..T18 + T7a/T8a/T9a/T9b/T14a) + INDEX update; 32 unit / 13 integration / 3 benchmark / 2 CI lint; coverage complete. PROTOCOL_VERSION test (TEST-SPEC-0476) written defensively (`PREVIOUS_LIVE_VERSION + 1`, not hardcoded integer) to handle SPEC-20 vs SPEC-22 landing-order ambiguity. **Open question logged for ESPECIALISTA EM SPECS (non-blocking):** TASK-0476 PROTOCOL_VERSION sequencing (SPEC-20 3→4 vs SPEC-22 2→3) — defender's defensive language already in place; spec-level resolution deferred. Test counts unchanged: 1181 default / 1224 zero-copy. Commits: `b66f758` Stage 0 (7 files, +1511/-36), `bbd976e` Stages 1+2 (87 files, +5426/-5).

**Previous update (2026-04-24):** v2 Pre-DEV Wave 1 / SPEC-20 Elastic Grid CLOSED in commit `ec680e4` — Stage 0 + Stage 1 + Stage 2 done.
**Updated by:** **v2 Pre-DEV Spec Pipeline — Wave 1 / SPEC-20 Elastic Grid CLOSED.** Stage 0 (Spec Review): Round 3 NF closure pass by `especialista-em-specs` from TCC root closed all 10 remaining Round-2 CONDITIONAL_PASS findings (NF-001 §3.8 A7 Net::union, NF-003 R4-delta self-worker symmetry + EG-U4-delta-wire-symmetry test, NF-004 R38a metric audit, NF-005 §3.8 A8 reconstruct 3-arg, NF-006 A3/A4 error returns, NF-007 R26a D==K_eff edge case, NF-011 R23a wording + R31 memory bound corrected to `W_currently_active ∪ W_pending_reclaim`, plus LOW-severity NF-008/009/010); spec status bumped Draft Round 2 → `Reviewed v2`; closure log `docs/spec-reviews/SPEC-REVIEW-20-round-3-2026-04-24.md` (299L); spec grew 1179 → 1283 lines; nothing escalated to spec-critic Round 3. Stage 1 (Task Splitter): 36 atomic tasks `TASK-0410..0455` in `docs/backlog/` partitioned across Phase A (predecessor amendments A1..A8), Phase B (foundation: wire/config/hybrid), Phase C (joining), Phase D (departure), Phase E (observability/regression); BACKLOG.md SPEC-20 section + coverage matrix added; ~9.6k LoC estimated total. Stage 2 (Test Generator): 62 TEST-SPECs + 1 INDEX in `docs/tests/` (33 EG-U unit / 10 EG-I integration / 6 EG-P property / 3 EG-B benchmark / 10 plumbing); ARG-006 empirical-signature anchors on EG-I3, EG-I5a, EG-P2, EG-P5; no determinism escalations. Forward-refs logged for SPEC-02 §3.8 A7, SPEC-04 §3.8 A3/A4, SPEC-19 §3.8 A8 — non-blocking, picked up in those specs' next revisions. Test counts unchanged: 1181 default / 1224 zero-copy (documentation-only Wave; no `src/` or `tests/` edits). Commit `ec680e4`: 104 files, +6545/-30.

**Previous update (2026-04-24):** D-005 Option A CLOSED in commit `a431320` — **12/12 G1 parity gate GREEN**, both iteration orders, both feature configs.
**Updated by:** **D-005 Option A Stages 4-6 CLOSED**. Stage 4 REVIEW (`docs/reviews/REVIEW-D-005-2026-04-24.md`) pinpointed root cause H5 ("coordinator never tells workers about promoted borders") with code citations; refuted H1-H4 from the orchestration plan. Stage 5 QA (`docs/qa/QA-D-005-2026-04-24.md`) enumerated 3 CRITICAL + 3 HIGH + 3 MEDIUM + 4 LOW findings and a 12-UT test matrix. Stage 6 REFACTOR addressed all 6 CRITICAL/HIGH items: CRIT-1 changed `BorderGraph::register_minted_agents` signature to return `Result<Vec<(u32, BorderState)>, GridError>` enumerating promoted borders (F-C1); CRIT-2 added `apply_promoted_borders_to_cache` + `promoted_borders_to_per_worker_new_borders` helpers with strict worker-a/side-a / worker-b/side-b routing and self-sentinel guards (F-C2 + F-H6); CRIT-3 plumbed per-worker `new_borders` into the next round's `RoundStartDispatch` via in-band append (F-C3); F-H7 added `debug_assert!` on cross-arena target injection in `apply_border_deltas_to_partition`; F-H8 added `check_delta_convergence_post_resolve` + `every_border_has_inert_remote` helpers that treat principal-port borders with a Lafont-concrete remote as inert, plus a `promotion_forces_next_round` short-circuit guard that defers convergence one round so workers observe promoted wires before final merge; F-H8 tail extends `partition.border_id_end` when a promoted border arrives out-of-range so `rebuild_free_port_index` keeps the entry; F-H6 tail adds `promoted_intra_worker_lafont_wires` helper that emits intra-worker Lafont promotions as direct `local_reconnections` and evicts the spurious border_id from `border_graph.borders` so merge does not attempt to restore a one-sided wire. 12 new UTs per QA matrix (UT-D005-01..12) landed across `border_graph.rs`, `grid.rs`, `grid_delta_integration_tests.rs`. Test counts: **1181 lib default** / **1224 lib `--features zero-copy`** (+13 / +13 over D-005 Stages 0-3 baseline, crossing the QA thresholds ≥1180 / ≥1223). UT-0385-08 green on bo... [truncated]

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

## Current State (Milestones & Fases)

| Phase | Status | Detail |
|-------|--------|--------|
| Specs (core) | 10/10 COMPLETE | SPEC-00 to SPEC-09 revised in English (v2+; SPEC-02/03/04/06/08/09 at v3, SPEC-13 at Revised v2 via adversarial review) |
| Specs (end-to-end) | 5/5 REVISED | SPEC-10 (Security v2), SPEC-11 (Observability v2), SPEC-12 (User I/O v2), SPEC-13 (System Architecture v2), SPEC-14 (Encoding v2). SPEC-15 (Distribution) at Draft v2 |
| Research library | 24/24 COMPLETE | PESQ-001 to PESQ-024 in `docs/pesquisa/` |
| Open decisions | 8/8 RESOLVED | See PESQ-023 (Decision Matrix) |
| Open-source setup | COMPLETE | LICENCE, README, CONTRIBUTING, GitHub templates (.github/) |
| Rust scaffolding | COMPLETE | Cargo.toml, src/ (10 modules + CLI skeleton + error types), compiles clean |
| Docker | COMPLETE | Dockerfile (multi-stage: rust:slim build → debian:bookworm-slim runtime) |
| CI/CD | COMPLETE | .github/workflows/ci.yml (fmt, clippy, test, build release) |
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

## Archived Pipeline State History (Transferred from pipeline-state.md)

## Prior Bundle (archived — reference for traceability)

**Bundle:** CLOSED — D-005 Option A — Worker-side application of `CommutationBatch.local_wiring` for minted agents (production, wire-level). CLOSED 2026-04-24, commit `a431320`.
**Stage:** DONE. Six SDD stages completed in full (no stage collapsed):
  0. SPEC-CRITIC — 3 rounds (2026-04-23). R1: 12 findings, all closed. R2: 5 new findings, all closed. R3: SIGN-OFF (0 CRITICAL / 0 HIGH). 3 LOW NR3 findings (NR3-001/002/003) explicitly marked non-blocking and deferred — require a future spec-critic touch on SPEC-19 §3.3 R23a / §3.4 R37. Stage 0 artefacts: `docs/spec-reviews/SPEC-REVIEW-19-section-3.4-D-005-2026-04-23.md`, `docs/spec-reviews/SPEC-REVIEW-19-section-3.4-D-005-2026-04-23-REREVIEW.md`, `docs/spec-reviews/SPEC-REVIEW-19-section-3.4-D-005-2026-04-23-REREVIEW-R3.md`.
  1. SPLITTING — TASK-0400..0403 (strict linear DAG): wire struct rewrite, resolver-to-wire transport, worker mint-then-wire, LocalDeltaDispatch forwarding + gate flip.
  2. TESTS — 26 mandatory UTs + optional PTs across TEST-SPEC-0400..0403. Gate: UT-0385-08 12/12 green on 6 fixtures × 2 strict modes × 2 feature configs.
  3. DEV — 13 src files modified. Spec amendment landed (PROTOCOL_VERSION 2→3, Shape A PendingCommutation, MalformedLocalWiring enum). Test counts on ship: **1168 / 1211** (+22/+25 over D-004 baseline 1146/1186). Gate 11/12 green (1 CON-DUP asymmetric failure remained at Stage 3 close, resolved in Stage 6).
  4. REVIEW (diagnostic-first) — `docs/reviews/REVIEW-D-005-2026-04-24.md` / `docs/plans/2026-04-24-d-005-stage-4-6-con-dup-diagnosis.md`. Diagnosis plan produced for `run_grid_delta_final_collect` / `dispatch_final_state_request` / `merge::core::merge` / `cleanup_t1_violations`. QA NIT F-L3 (SPEC-19 R23b gap) surfaced — deferred to a future spec-critic round.
  5. QA — `docs/qa/QA-D-005-2026-04-24.md`. 13 findings total; all classified and resolved or deferred. NIT F-L3 (R23b gap) explicitly DEFERRED to next spec-critic pass. No unresolved CRITICAL or HIGH findings.
  6. REFACTOR — 12 new UTs added (UT-0385-08 coverage expansion). All Must-Fix and Should-Fix items applied. Gate: **UT-0385-08 12/12 green** on both iteration orders, both feature configs (default + `--features zero-copy`). Final test counts: **1181 default / 1224 zero-copy** (+13/+13 over Stage 3 ship). Clippy + fmt clean both configs.
**Branch:** `v2-development`
**Test baseline (start of D-005):** 1146 lib default / 1186 lib `--features zero-copy`.
**Test counts at close:** **1181** lib default (+35 total over D-004 baseline) / **1224** lib `--features zero-copy` (+38 total).
**Clippy:** clean both feature configs.
**fmt:** clean.
**Acceptance gate result:** PASS — UT-0385-08 12/12 green. `canonicalize(out_delta) == canonicalize(out_v1)` AND `metrics.total_interactions == metrics_v1.total_interactions` on all 6 fixtures × 2 strict modes.
**Cascade closures:** D-003 (symmetric rules G1 parity, fully verified via `SKIP_ASYMMETRIC=false`) and D-004 (coordinator-side round-N+2 finalizer, all steps including `SKIP_ASYMMETRIC` flip) cascade-closed upon D-005 gate passing. DEFERRED-WORK.md updated by HK-A in parallel.
**NR3 deferrals (non-blocking, require future spec-critic):**
  - NR3-001: prose edit `arity` → `pc.target_symbols.len()` in one R23a clause.
  - NR3-002: R37 wording sharpen — explicit mention of `ProtocolError::DeserializationFailed` vs `MalformedLocalWiring` dispatch boundary.
  - NR3-003: optional 8th enum case `MalformedLocalWiringReason::TargetSymbolsTooLong` (coordinator-side bound guard).
**QA NIT deferred:** F-L3 (SPEC-19 R23b gap — prose does not fully specify the wire validation ordering for zero-arity commutation case) — deferred to next spec-critic round on SPEC-19 §3.3.
**Previous bundle:** D-004 Coordinator-Side Round-N+2 Finalizer (2026-04-23).

---

**Bundle:** CLOSED — D-004 Coordinator-Side Round-N+2 Finalizer (plumbing-only scope) CLOSED as of 2026-04-23. `SKIP_ASYMMETRIC` flip gated on D-005.
**Stage:** DONE. Six SDD stages collapsed to 4 per reviewer endorsement (scope was pure-core plumbing + helpers + test-only T1 cleanup, no behavior change on symmetric rules):
  1. SPLITTING — TASK-0398 + TASK-0399 authored directly (Option B "cheap and equally formal" path per user directive).
  2. TESTS — TEST-SPEC-0398 + TEST-SPEC-0399 authored inline; UT-0398-01..08 cover encode/decode, enqueue, register, lenient duplicates, R48 stray, DC-B6 preserve-existing-border.
  3. DEV — TASK-0398 shipped inline (pure-core plumbing); TASK-0399 shipped inline (wire + revert SKIP_ASYMMETRIC).
  4. REVIEW — general-purpose reviewer agent (Opus-7), 2026-04-23 — ALIGNED, 0 Must-Fix, 2 Should-Fix (one docstring drift fixed inline; one release-mode overflow handling deferred to D-005 follow-up), several NITs, explicit endorsement of direct close without Stage 5/6.
  5. QA — SKIPPED per reviewer endorsement. UT-0398-01..08 cover the 4 critical Q-probes (R48 stray, duplicate-lenient, DC-B6 preserve, partial resolution).
  6. REFACTOR — SKIPPED (0 Must-Fix from REVIEW).
**Branch:** `v2-development`
**Test baseline (start of D-004):** 1138 lib default / 1178 lib `--features zero-copy`.
**Test counts at close:** **1146** lib default (+8: UT-0398-01..08) / **1186** lib `--features zero-copy` (+8, same set).
**Clippy:** clean both feature configs.
**fmt:** clean.
**D-003:** still PARTIALLY CLOSED — symmetric rules (CON-CON, DUP-DUP, ERA-ERA) G1 parity verified; asymmetric rules under `const SKIP_ASYMMETRIC: bool = true;` now pending D-005.
**D-004:** **PARTIALLY SHIPPED** (plumbing only) — `RoundResultPayload.minted_agents` extended, `BorderGraph::{enqueue_pending_borders, register_minted_agents}` implemented with R48 validation and DC-B6 preserve-existing-border path, `encode_request_id`/`decode_request_id` codec shared between resolver and LocalDeltaDispatch, `run_grid_delta_inner` wired, `package_resolutions_with_pending` exposes pending borders to coordinator. Step (5) `SKIP_ASYMMETRIC = false` flip blocked by D-005.
**D-005:** NEW — worker-side application of `CommutationBatch.local_wiring` for minted agents. Root cause: `PendingCommutation` wire message does not carry `local_wiring`, so workers mint agents but leave internal edges DISCONNECTED. Option B test-only workaround unblocks integration tests; Option A wire-level fix required for real LAN runs in delta mode. Blocks full D-003 AND full D-004 closure.
**Previous bundle:** SPEC-19 §3.3 Refactor (2026-04-23) — closed MF-001/MF-002/SF-001/SF-002; opened D-004.

---

## Resolved Deferrals (archive)

### D-002 — SPEC-18 R20-R27 (rkyv zero-copy archive path) — SHIPPED 2026-04-16

| Field | Value |
|-------|-------|
| **Source spec** | SPEC-18 (Wire Format v2) §3.5 |
| **Requirements shipped** | R20, R21, R22, R23, R24, R25, R26, R27 + tests T11-T14 |
| **Resolved** | 2026-04-16 |
| **Status** | SHIPPED — all 7 action steps from the original D-002 plan executed and verified; bundle entered Stage 4 REVIEW. |
