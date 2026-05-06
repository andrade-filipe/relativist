# Progress & History — Relativist Software Implementation

> **CRITICAL LLM INSTRUCTION:** This file is for **PAST/COMPLETED work only**. Active work, current pipeline state, and future milestones live exclusively in `next-steps.md`. Per-bundle deep narratives have been moved to `docs/{qa,reviews,spec-reviews,plans,handoffs,briefings}/archive/` and to per-bundle commits — this file is a **navigable index**, not the full text.

**Last updated:** 2026-05-05 (v0.20.0-pre release published; v2-development merged into main; CI green for the first time since 2026-04-27).

---

## TL;DR — bundle history at a glance

| Bundle | Closed | Tag (if any) | Scope | Test floor (default) | Per-bundle archive |
|--------|--------|--------------|-------|----------------------|---------------------|
| Local Benchmark Phase | 2026-04-11 | `v0.10.0-bench` | v1 frozen baseline (Phase 1 + 2, 4600 runs, 0 failures) | 690 | `results/locked/v1_local_baseline/` |
| SPEC-19 §3.3 Refactor | 2026-04-23 | — | Worker R23/R26 completion + G1 parity ITs | 1138 | commit `08722e0` |
| D-004 Coordinator Round-N+2 Finalizer | 2026-04-23 | — | Plumbing-only scope; SKIP_ASYMMETRIC=true retained | 1146 | TASK-0398/0399 |
| D-005 SPEC-19 §3.4 Shape A Worker-Side Wiring | 2026-04-24 | — | mint-then-wire ordering; PROTOCOL_VERSION 2→3; G1 parity 12/12 | 1181 | `docs/{reviews,qa}/archive/REVIEW-D-005-* + QA-D-005-*` |
| Pre-DEV Wave 1 (SPEC-20 Elastic Grid) | 2026-04-24 | — | Stages 0+1+2 (spec → tasks → test-specs); 36 TASKs + 62 TEST-SPECs | 1181 | `docs/spec-reviews/archive/SPEC-REVIEW-20-round-3-*` |
| Pre-DEV Wave 2a (SPEC-22 Arena) | 2026-04-25 | — | Stages 0+1+2 with §3.8 amendments A1..A10 | 1181 | `docs/spec-reviews/archive/SPEC-REVIEW-22-round-{1,2}-*` |
| Pre-DEV Wave 2b (SPEC-21 Streaming) | 2026-04-25 | — | Stages 0+1+2 with 8 cross-spec amendments | 1181 | `docs/spec-reviews/archive/SPEC-REVIEW-21-round-{1,2}-*` |
| D-006 SPEC-20 Hybrid Coordinator (Option A) | 2026-04-27 | `v2-llm-experiment-archive` | LLM Stage-3 audit found 14 CRIT + 23 HIGH; 4-phase REFACTOR shipped on top | 1308 | `docs/{reviews,qa}/archive/REVIEW+QA-PHASE-{B,C,D,E}-elastic-*` |
| D-009 SPEC-22 Arena Management | 2026-04-27 | — | Free-list LIFO + SparseNet; QA found 5 CRIT + 3 HIGH all closed | 1464 | `docs/{reviews,qa}/archive/*-D009-*` + `docs/spec-reviews/archive/CLOSURE-D009-*` |
| D-010 SPEC-21 Streaming Generation | 2026-04-30 | — | Pull-dispatch FSM, R10b/c recycling, PROTOCOL_VERSION 5→6, `streaming-no-recycle` profile | 1683 | `docs/{reviews,qa}/archive/*-D010-*` + `docs/spec-reviews/archive/CLOSURE-D010-*` |
| D-011 BLOCKER (partition perf regression) | 2026-05-04 | — | SPEC-22 v2.4 metric correction + 2 latent dense-path bugs (Bug 1 + Bug 2) + AF-2/AF-3 guards; **scientific finding:** apparent regression masked correctness bugs v1 never detected | 1784 | `docs/qa/archive/QA-D011-*` + `docs/spec-reviews/archive/SPEC-22-amendment-2026-05-04-*` |
| D-012 Instrumentation Restore | 2026-05-05 | — | Restored `network_time_secs` / `compute_time_secs` / `mips_mean` (RF-04/05/07 from D-011 post-mortem) + unblocked `cargo test --release` | 1798 | `docs/{reviews,qa}/archive/REVIEW+QA-D012-*` |
| v0.20.0-pre release + main merge | 2026-05-05 | `v0.20.0-pre` | Bench re-run with restored instrumentation; canonical baseline locked; 8 CI fixes; main promoted from v1 to v2 | 1798 / 1842 / 1789 / 1740 | `docs/handoffs/archive/2026-05-05-D012-*` + this file |

**Tags shipped:** `v0.10.0-bench` (v1 freeze, frozen on `v1-feature-complete`), `v0.10.0-bench-stress` (stress campaign), `v0.10.1` (v1 trivial follow-up), `v2-llm-experiment-archive` (D-006 LLM-attempt before audit), `v0.20.0-pre` (first v2 pre-release, 2026-05-05).

**Test-floor evolution:** 690 (v1) → 1138 → 1146 → 1181 → 1308 → 1464 → 1683 → 1784 → 1798 default. zero-copy floor 1842, streaming-no-recycle 1789, release 1740 (post-D-012). v1 floor 690 inviolable on `v1-feature-complete`.

---

## Per-bundle narratives (condensed)

### Local Benchmark Phase (CLOSED 2026-04-11) — v1 frozen baseline

Tag `v0.10.0-bench` released. L2 (BSP multi-round) resolved architecturally via opt-in `strict_bsp` mode (SPEC-05 R30a). Unified `v1_local_baseline` campaign: Phase 1 (11:39, 4200 reps) + Phase 2 (43:42, 400 reps Docker/TcpLocalhost), **0 failures in 4600 runs**. SPEC-09 theoretical predictions confirmed: `cascade_cross(N) = N` rounds, `dual_tree(d) = d` rounds under strict + `workers ≥ 2`. 655 tests. Snapshot at `results/locked/v1_local_baseline/` with full provenance.

### SPEC-19 §3.3 Refactor (CLOSED 2026-04-23) — commit `08722e0`

Four MF/SF tasks shipped (TASK-0394..0397) plus 3 adversarial QA probes. Tests **1138 default / 1178 zero-copy**. D-003 partially closed; D-004 opened-and-closed (plumbing scope).

### D-005 SPEC-19 §3.4 Shape A Worker-Side Wiring (CLOSED 2026-04-24) — `a431320`

Three-round adversarial spec review (R1 BLOCK 12 → R2 BLOCK 5 → R3 SIGN-OFF) landed `PendingCommutation { request_id, target_symbols, local_wiring }`, `ProtocolError::MalformedLocalWiring` 7-case enum, PROTOCOL_VERSION 2→3, R23a clauses 1-6, R24.1.6a/b/c echo semantics. Stage 4 REVIEW pinpointed root cause H5 ("coordinator never tells workers about promoted borders"); Stage 5 QA enumerated 3 CRIT + 3 HIGH + 12-UT matrix; Stage 6 REFACTOR addressed all 6 CRIT/HIGH. **G1 parity gate 12/12 GREEN** on both iteration orders + both feature configs. Tests **1181 default / 1224 zero-copy**.

### Pre-DEV Wave 1 SPEC-20 Elastic Grid (CLOSED 2026-04-24) — `ec680e4`

Stage 0 closed all 10 Round-2 NF findings (NF-001..011); spec status `Reviewed v2`. Stage 1: 36 atomic tasks `TASK-0410..0455`. Stage 2: 62 TEST-SPECs (33 EG-U / 10 EG-I / 6 EG-P / 3 EG-B / 10 plumbing). Tests unchanged (doc-only). Closure log: `docs/spec-reviews/archive/SPEC-REVIEW-20-round-3-2026-04-24.md`.

### Pre-DEV Wave 2a SPEC-22 Arena Management (CLOSED 2026-04-25) — `b66f758` + `bbd976e`

Stage 0: spec-critic Round 1 BLOCK with 21 findings (4C/7H/6M/4L); especialista Round 2 closed 20/21 inline; spec Draft → `Reviewed v2`; +43% lines. §3.8 Amendments A1..A10 to SPEC-01 (I3'), -02, -03, -04, -05, -18 (PROTOCOL_VERSION 2→3), -19 (BorderGraph + R10b strategies). Stage 1: 36 atomic tasks `TASK-0460..0500`. Stage 2: 48 TEST-SPECs.

### Pre-DEV Wave 2b SPEC-21 Streaming Generation (CLOSED 2026-04-25) — `508e595` + `131ca26`

Same pattern. 36 TASKs `TASK-0510..0591`, 49 TEST-SPECs (35 plumbing + 14 spec-catalog T1..T14). Cumulative across Pre-DEV: **108 atomic tasks, 159 TEST-SPECs, 4490 benchmarks frozen at v1 baseline.** Tests unchanged (doc-only).

### D-006 SPEC-20 §3.1-3.3 Hybrid Coordinator (CLOSED 2026-04-27)

Stage-3 was the only LLM-delegation experiment of the project. 12 LLM-authored commits compiled and passed `cargo test` but Stage 4+5 audit found **14 CRITICAL + 23 HIGH bugs** — Phase B+E ACCEPT_WITH_FIXES, Phase C+D **REJECT**. Original LLM commits archived at tag `v2-llm-experiment-archive`. Stage 6 shipped per-phase: Phase E `434a242` (14 fixes), Phase B `df93908` (12), Phase C `8dd6d1b` (7), Phase D `7988573` (Option A — removed broken reclaim block, `elastic_departure: bool = false` default). Tests **1308 default / 1351 zero-copy**. Reclaim path deferred to v2.1. **Verdict on LLM experiment:** wall-clock saved at Stage 3 was consumed several times over by review+QA+refactor; future delegation limited to mechanical refactor passes only. See `docs/reviews/archive/AUDIT-SUMMARY-2026-04-27.md` for the full lessons-learned rubric.

### D-009 SPEC-22 Arena Management (CLOSED 2026-04-27)

Phase A `01184f1`: 10 amendments A1..A10. Phase B `47d9bf2`: free-list core (`Net.free_list: Vec<AgentId>`, LIFO push/pop). Phase C `d6411be` + `c36a999`: SparseNet (`HashMap`-backed) + R22/R30 sparse threshold + R27 debug-asserts + CI lint guards. Stage 4 REVIEW: 2 MF + 3 SF. Stage 5 QA found **5 CRITICAL** clustered on distributed seams (CompactSubnet drops free_list, `from_bytes` skipped validate, `border_entries_shadow` dead code, merge cross-partition dup, `to_dense` unbounded DoS). Stage 6 REFACTOR (4 waves) closed all CRITs. QA-D009-001 (CompactSubnet wire format) deferred → TASK-0595. Tests **1308 → 1464 default** (+156).

### D-010 SPEC-21 Streaming Generation (CLOSED 2026-04-30)

Phase A: 8 spec amendments A1..A8 across 7 predecessor specs. Phase F: 9 production waves (`2f751a4..61e86a1`) — GridConfig streaming fields, `RequestWork`/`NoMoreWork` wire variants (PROTOCOL_VERSION 5→6), coordinator + worker pull-dispatch FSM, R10b Strategy A streaming gate, R10b Strategy B `BorderClean` recycling, `streaming-no-recycle` cargo feature. Stage 4 REVIEW: 2 MF + 4 SF. Stage 5 QA found **4 CRITICAL + 5 HIGH** including QA-001 single-bit `is_in_delta_round` reuse, QA-002 RequestWork worker_id impersonation, QA-003 `default_chunked_iter` silently drops every FreePort (T6 violation). Stage 6 shipped 9 fix commits closing all 11 in-scope items. Tests **1464 → 1683 default / 1726 zero-copy** (+219/+219); new `streaming-no-recycle` profile at 1680. QA-D010-009..016 deferred to D-011. Bundle ships SPEC-21 R1..R37.

### D-011 BLOCKER — Partition perf regression (CLOSED 2026-05-04)

**The "serious bug" referenced in TCC-relevant scientific findings.** Investigation triggered by `ep_con 5M w=2` wall: v1 ~12s vs v2-HEAD ~22s (+83%). 7-point bisect isolated to `d6411be` (D-009 Phase C). Root cause: `partition::helpers::build_subnet_with_config` threshold `id_range > 4 × live_count` measured the planning range (`next_id × 10`) instead of actual arena memory (`max_live_id + 1`). Every healthy partition was mis-routed to SPARSE (~7s/partition) when DENSE would take ~1s. **Phase B of the fix surfaced 2 latent dense-path bugs** that had been masked since D-009: **Bug 1** (`helpers.rs:384`) — dense initialized `freeport_redirects: HashMap::new()` while sparse cloned them (broke delta-mode integration); **Bug 2** (`helpers.rs:382`) — dense returned `next_id = 0`, causing worker 0 to allocate fresh agents OUTSIDE its assigned `id_range` and INSIDE worker 1's pre-existing IDs → I1 violation post-merge. Stage 1: SPEC-22 v2.4 amendment (R22 metric → `effective_arena_size`). Stage 4 (third DEV cycle, `62de30f`): both bugs fixed + AF-2/AF-3 defensive guards. Stage 5 QA HIGH F-001 dormant in `PartitionAccumulator::finalize`. Stage 6 housekeeping `b1f9c10` closed all findings. **Bench verification (`0fd27c0`):** ratio **1.11× v1 (within noise)**. Tests **1683 → 1784 default**. **Scientific finding for TCC:** the formal invariant framework (SPEC-01 I1, SPEC-22 R10/D3) detected what the empirical test suite missed; v2 is now strictly more correct than v1, at residual ~10% wall-clock cost. Per-bundle artefacts in `docs/{qa,spec-reviews,plans,handoffs,benchmarks}/archive/`.

### D-012 Instrumentation Restore (CLOSED 2026-05-05)

Driven by D-011 cold post-mortem (`docs/analysis/D011-final-baseline-analysis-2026-05-04.md`) which catalogued 9 red flags. Three (RF-04 `network_time_secs = 0`, RF-05 `compute_time_secs = 0`, RF-07 `mips_mean = 0`) plus a `cargo test --release` blocker grouped into a single instrumentation-restore bundle. 4 atomic tasks (TASK-0615..0618). Stage 5 QA verdict **REJECT** found 2 CRITICAL: **QA-D012-001** SUM aggregation makes `compute_time > wall_clock` for parallel workers (drives `overhead_ratio` negative); **QA-D012-002** literal `mips_mean = 0.000` lived in `scripts/bench_docker_v2.sh:283` Python-in-bash hardcode — TASK-0618 witness was attached to the wrong layer entirely. Stage 6 REFACTOR `c439182`: SUM → MAX (BSP critical-path), bash hardcode patched, IT-0618-A4 added for end-to-end CSV witness. Tests **1798 default / 1842 zero-copy / 1789 streaming-no-recycle / 1740 release** (release was broken pre-D-012). Per-bundle artefacts in `docs/{reviews,qa}/archive/REVIEW+QA-D012-instrumentation-restore-2026-05-05.md`.

### v0.20.0-pre release (PUBLISHED 2026-05-05) — `v0.20.0-pre`

Bench re-run (`bench_docker_v2.sh --no-resume`) at HEAD `e6ff6bb` post-D-012; canonical baseline locked at `results/locked/v2_post_d012_baseline_2026-05-05/` with 32/32 distributed slots `all_correct=true`, 10/10 reps, instrumentation columns populated. Sample headline: `ep_500k w=1` round 0 wall = 0.460s = compute 0.10 + network 0.39 + merge 0.04 + ~0.03 framing — **47% network share on average across 320 TCP rounds, 30% fewer bytes/round vs v1**. CI rehab: 8 environment-aware fixes (clippy unused import in cfg(unix); test_buffer_sizes_applied SO_SNDBUF cap; IT-0606-04 + IT-0607-02 VmHWM saturation; IT-0609-03 `compose ps -a`; IT-0609-05 pre-generate input.bin; docker-bench-smoke renamed to break stuck registration; bench-smoke wall-clock budget 90s→180s). All 3 workflows (CI, Docker Smoke, Bench Smoke) green for the first time on v2-development since 2026-04-27. Cargo.toml `0.11.0 → 0.20.0-pre`. Annotated tag pushed; GitHub pre-release published. v2-development merged into main via `--no-ff` (commit `cb6fb91`). Pre-existing untracked v1-era files in `results/` left untouched (not in scope of this release). Phase 3 LAN tutorial `docs/benchmarks/phase-3-lan.md` covers axis 1 (bincode + delta) and axis 2 (zero-copy + delta).

---

## Resolved Decisions (from Pre-DEV PESQ-023 era)

| # | Decision | Resolution |
|---|----------|-----------|
| D1 | Workspace structure | Single crate (v1) → multi-crate `relativist-core` + `relativist-cli` (v2 via SPEC-26 §3.1) |
| D2 | Error handling | thiserror |
| D3 | Feature flags | tls, metrics, otel, zero-copy (v2), streaming-no-recycle (v2) |
| D4 | Security model | 3-tier (none / token / token+TLS) |
| D5 | Observability | tracing + prometheus-client + OTel (optional) |
| D6 | Testing strategy | proptest + in-memory grid + Transport trait |
| D7 | Module structure | 10 modules core/infra split (v1); SPEC-22 added arena layer (v2) |
| D8 | Programming model | BSP (Bulk Synchronous Parallel) |

---

## Where to find more detail

- **Active work + future milestones:** `docs/next-steps.md`
- **Per-bundle reviews/QA/spec-reviews:** `docs/{reviews,qa,spec-reviews}/archive/`
- **Per-bundle plans + handoffs:** `docs/{plans,handoffs}/archive/`
- **Per-bundle TASK definitions + TEST-SPECs:** `docs/{backlog,tests}/archive/`
- **Authoritative roadmap:** `docs/ROADMAP.md`
- **Pipeline definition:** `docs/WORKFLOWS.md`
- **D-011 cold post-mortem analysis:** `docs/analysis/D011-final-baseline-analysis-2026-05-04.md`
- **Frozen baselines:** `results/locked/{v1_local_baseline,v2_pre_fix_baseline_2026-05-04,v2_d011_final_baseline_2026-05-04,v2_post_d012_baseline_2026-05-05}/`
