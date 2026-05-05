# Tiers 2 + 3 + 4 Master Plan — Relativist v2 Path to Next Benchmark

> **For agentic workers:** this is a **roadmap plan** that decomposes 14 bundles (D-006..D-019) across three tiers. Each bundle gets its own detailed TDD plan in `docs/plans/YYYY-MM-DD-d-NNN-<slug>.md` when started, following `superpowers:writing-plans` + `superpowers:subagent-driven-development`. This master plan is the source of ordering, gate decisions, and bundle scope.

**Goal:** Close the complete v2 backlog across Tiers 2 (Elastic Grid / Confluence Showcase), 3 (Memory Efficiency), and 4 (UX / Deployment) — 14 features total — before running the next benchmark campaign. After closure, prioritize Tier 5 features near the TCC delivery window.

**Architecture:** Three sequential phases. Phase 1 implements the Confluence Showcase (Tier 2, SPEC-20 already drafted) which is the most TCC-central visible artefact. Phase 2 ships memory efficiency (Tier 3, SPEC-21 + SPEC-22 drafted) enabling larger-than-RAM workloads. Phase 3 ships UX/deployment (Tier 4, no specs yet) which unlocks multi-machine benchmarking. Each bundle is one SDD cycle with stages scaled to bundle size; small bundles may collapse REVIEW+QA where justified (as D-004 did).

**Source of truth:** `V2-FEATURE-MATRIX.md` (primary). `ROADMAP.md` provides design text; matrix provides status.

**Tech stack per phase:**
- Phase 1: coordinator FSM extensions, `JoinRequest` / `LeaveRequest` protocol messages, retained-partition bookkeeping.
- Phase 2: streaming generator + partitioner traits, free-list `Vec<AgentId>` in `Net`, `HashMap`-backed sparse net alternative, chunked pipeline.
- Phase 3: `cargo-wix` + SignPath + winget/Homebrew/scoop manifests; Tailscale CLI shell-outs; `mdns-sd` crate; `rayon`; redex-count partition heuristic; SPEC-14 encoder amendment.

---

## 0. Scope alignment (revised per user directive 2026-04-24)

User clarified: **no tier skipping.** Sequence is Tier 1 (DONE) → Tier 2 → Tier 3 → Tier 4 → benchmark → Tier 5 priorities → final deliverable. Previous plan's "skip Tier 2/3" was a misreading; this revision corrects it.

### 0.1 Tier 1 status (sanity check)

All Tier 1 items are now DONE as of 2026-04-24, commit `a431320`:

| Matrix ID | Feature | Evidence |
|-----------|---------|----------|
| 2.22 | TCP Tuning | DONE (commit c360fe5, 2026-04-15) |
| 2.23 | Wire Format v2 | DONE (shipped as part of Tier 1 M1) |
| 2.34 | Coordinator-Free Round | DONE (shipped as part of Tier 1 M3) |
| 2.24 | Zero-Copy rkyv | DONE (D-002 resolved 2026-04-16) |
| 2.25 | Same-Host UDS | DONE (commit c360fe5, 2026-04-15) |
| 2.35 | Delta Merge / BorderGraph | DONE (R8-R19 shipped 2026-04-17) |
| 2.26 | Delta-Only Protocol | **NEW — DONE as of 2026-04-24** (D-005 Option A CLOSED commit `a431320`; G1 parity 12/12 green; D-003 + D-004 cascade-closed) |

**Conclusion:** break-even path is complete at the code level. Empirical break-even measurement (`ep_con 5M w=2`) is now runnable against v2 `run_grid_delta`.

### 0.2 Specs we inherit (Tier 2 / 3 drafted, never adversarially reviewed)

| Spec | Tier | Bundles it covers | Drafted | spec-critic reviewed? |
|------|------|-------------------|---------|------------------------|
| SPEC-20 | 2 | 2.1, 2.2, 2.3 | ✓ (598 lines) | **NO** — needs Round 1/2/3 before DEV |
| SPEC-21 | 3 | 2.27, 2.28, 2.30, 2.36 | ✓ (836 lines) | **NO** |
| SPEC-22 | 3 | 2.33, 2.32 | ✓ (616 lines) | **NO** |

All three must go through a spec-critic round before Stage 1 (SPLIT). Tier 4 has no specs at all — requires **both** authoring and review.

---

## 1. Executive summary

| Phase | Tier | Bundles | LoC (est) | Specs to author | Specs to review |
|-------|------|---------|-----------|-----------------|------------------|
| 1 | 2 (Elastic Grid) | D-006, D-007, D-008 | ~1300 | 0 | SPEC-20 (one pass; all 3 bundles share it) |
| 2 | 3 (Memory Efficiency) | D-009, D-010, D-011, D-012, D-013 | ~2250 | 0 | SPEC-21 (shared 2.27/2.28/2.30/2.36) + SPEC-22 (shared 2.33/2.32) |
| 3 | 4 (UX / Deployment) | D-014, D-015, D-016, D-017, D-018, D-019 | ~1600 + CI | 4-6 (SPEC-28..SPEC-33 or amendments) | same (authored-then-reviewed) |
| — | — | **14 bundles total** | **~5150 LoC + CI** | **4-6 new specs** | **3 review passes (Tier 2/3) + per-bundle (Tier 4)** |

**Estimated focused effort:** 25-50 working days. Variance driven by SignPath external wait (D-015), proptest flakes (D-013 sparse net, D-017 rayon), and spec-critic rounds on authoring-heavy Tier 4 specs.

---

## 2. Phase 1 — Tier 2: Elastic Grid (Confluence Showcase)

Three features, one spec (SPEC-20). Demonstrates the TCC's central argument (ARG-001 P1: strong confluence enables free work redistribution) at runtime.

### Bundle D-006 — 2.1 Coordinator as Worker (Hybrid Node)

| Field | Value |
|-------|-------|
| **Matrix ID** | 2.1 |
| **Spec** | SPEC-20 §3.1 (R1-R7) — needs spec-critic |
| **Scope** | Coordinator keeps one partition for itself (`worker_id=0`), reduces locally during the same BSP round in which remote workers reduce. Effective worker count `K_eff = K + 1`. |
| **Files expected** | MODIFY `relativist-core/src/merge/grid.rs` (hybrid branch in `run_grid` + `run_grid_delta`); MODIFY `relativist-core/src/config.rs` (`--hybrid-coordinator` flag, default `true`); MODIFY `relativist-core/src/coordinator.rs` if present or binary entry. |
| **LoC estimate** | ~250 production + ~80 test |
| **SDD cycle** | Stage 0 spec-critic on SPEC-20 §3.1 · full 6-stage pipeline for DEV (shared spec review across D-006/007/008 counts once) |
| **Acceptance gate** | Single-machine hybrid run reduces `ep_annihilation_con 1M` in <= time of `K=2` pure-remote run; `cargo test --workspace --lib` ≥ 1181 default / 1224 zero-copy preserved; G1 parity invariant holds with K_eff rather than K. |
| **TCC relevance** | VERY HIGH — simplest confluence demo; also eliminates coordinator idle time |
| **Risk** | LOW-MEDIUM — touches the core grid loop; must not regress G1 parity |

### Bundle D-007 — 2.2 Dynamic Worker Joining

| Field | Value |
|-------|-------|
| **Matrix ID** | 2.2 |
| **Spec** | SPEC-20 §3.2 — needs spec-critic (shared pass with D-006) |
| **Scope** | New workers connect between BSP rounds; accepted during Join Window (end of merge → start of dispatch). New workers are assigned partitions from the next round forward. `ActiveWorkerSet` grows; split strategy adjusts to new K. |
| **Files expected** | MODIFY `relativist-core/src/protocol/messages.rs` (`JoinRequest`, `JoinAck`); MODIFY coordinator FSM to accept join during window; MODIFY `run_grid_delta_inner` loop entry to re-query active set each round. |
| **LoC estimate** | ~500 production + ~150 test (including multi-round join proptest) |
| **SDD cycle** | full 6-stage pipeline |
| **Acceptance gate** | Integration test: start with K=2, join a third worker mid-run, verify result matches K=3 single-dispatch run via `canonicalize_net` + `metrics.total_interactions`. |
| **TCC relevance** | HIGH — direct demonstration of confluence |
| **Risk** | MEDIUM — FSM extension across async + pure layers |

### Bundle D-008 — 2.3 Dynamic Worker Departure

| Field | Value |
|-------|-------|
| **Matrix ID** | 2.3 |
| **Spec** | SPEC-20 §3.3 — needs spec-critic (shared pass) |
| **Scope** | Graceful (`LeaveRequest`) + timeout-detected departure; retained-partition re-dispatch to remaining workers; invariant preservation under partition loss. |
| **Files expected** | MODIFY `relativist-core/src/protocol/messages.rs` (`LeaveRequest`, `LeaveAck`); MODIFY coordinator to maintain retained partitions on dispatch; MODIFY `collect_timeout` handler to trigger re-dispatch. |
| **LoC estimate** | ~600 production + ~200 test |
| **SDD cycle** | full 6-stage + extra QA probe for "departure mid-dispatch + join in same window" race |
| **Acceptance gate** | Integration test: start K=3, mid-run one worker disconnects; remaining 2 workers complete the reduction with canonically-equivalent output to K=3 non-disruptive run. |
| **TCC relevance** | HIGH — the most adversarial confluence demo |
| **Risk** | MEDIUM-HIGH — retained partitions + timeout FSM + re-dispatch correctness |
| **Prerequisites** | D-007 (join machinery composes with leave machinery) |

---

## 3. Phase 2 — Tier 3: Memory Efficiency

Five features, two specs. Enables larger-than-RAM workloads.

### Bundle D-009 — 2.33 Arena Recycling (free-list variant)

| Field | Value |
|-------|-------|
| **Matrix ID** | 2.33 |
| **Spec** | SPEC-22 §1 / §3 (Arena Recycling section) — needs spec-critic |
| **Scope** | Free-list `Vec<AgentId>` in `Net`; `create_agent` pops from free-list first; `remove_agent` pushes to free-list. Relaxation of SPEC-01 I3 (monotonicity) is already drafted in SPEC-22. |
| **Files expected** | MODIFY `relativist-core/src/net/core.rs` (`create_agent`, `remove_agent`, free-list field); MODIFY `relativist-core/src/net/types.rs` if needed; UTs for slot reuse invariants. |
| **LoC estimate** | ~150 production + ~80 test |
| **SDD cycle** | small-scale: Stage 0 spec-critic on SPEC-22 (shared with D-013) · Stage 1 split into 2 tasks · Stage 2 tests · Stage 3 DEV · Stage 4 REVIEW · Stage 5 QA may collapse with REVIEW given scope |
| **Acceptance gate** | After 1M reduction steps on `ep_con`, `net.agents.len()` is bounded (not monotonic growth); canonicalized output unchanged; `cargo test --workspace --lib` preserved. |
| **TCC relevance** | MEDIUM — enables longer reductions without OOM |
| **Risk** | LOW |

### Bundle D-010 — 2.28 Online/Streaming Graph Partitioning

| Field | Value |
|-------|-------|
| **Matrix ID** | 2.28 |
| **Spec** | SPEC-21 §3 (streaming partition strategy trait) — needs spec-critic (shared pass with D-011/012/013 on SPEC-21) |
| **Scope** | `StreamingPartitionStrategy` trait; round-robin MVP; ingest `AgentBatch`, assign per-agent `worker_id` without global view. |
| **Files expected** | NEW `relativist-core/src/partition/streaming.rs`; MODIFY `relativist-core/src/partition/mod.rs`; MODIFY `relativist-core/src/config.rs` (`--streaming-partition`). |
| **LoC estimate** | ~400 production + ~100 test |
| **SDD cycle** | full 6-stage |
| **Acceptance gate** | Output partition assignment agrees with offline `split()` on equivalent inputs (same round-robin mapping); C1-C3 (from ARG-002) hold incrementally per batch. |
| **TCC relevance** | MEDIUM-HIGH — enables Tier 3 memory story |
| **Risk** | MEDIUM — forward-reference handling across batches |

### Bundle D-011 — 2.27 Streaming Net Generation

| Field | Value |
|-------|-------|
| **Matrix ID** | 2.27 |
| **Spec** | SPEC-21 §4 |
| **Scope** | Producer-consumer pattern: generators emit `AgentBatch` as they go, instead of constructing full net. MVP limited to `ep_annihilation` generator. |
| **Files expected** | NEW `relativist-core/src/io/streaming.rs`; MODIFY existing generator APIs to implement the new trait; NEW bench entry for streaming gen. |
| **LoC estimate** | ~500 production + ~150 test |
| **SDD cycle** | full 6-stage |
| **Acceptance gate** | Streaming `ep_annihilation(5M)` peak coordinator memory <= 1 GB (vs current ~3-5 GB equivalent full-construct baseline). |
| **TCC relevance** | MEDIUM-HIGH — direct enablement of the scalability narrative |
| **Risk** | MEDIUM — memory measurement precision; per-batch invariant preservation |
| **Prerequisites** | D-010 (streaming partitioner consumes streaming generator output) |

### Bundle D-012 — 2.30 Chunked Generation + Incremental Partitioning

| Field | Value |
|-------|-------|
| **Matrix ID** | 2.30 |
| **Spec** | SPEC-21 §5 (integration) |
| **Scope** | End-to-end pipeline: `generate chunk -> partition chunk -> dispatch chunk -> repeat`. Composes D-010 and D-011 under a single orchestration loop. |
| **Files expected** | MODIFY coordinator dispatch loop to consume streaming generator + streaming partitioner; NEW integration test of end-to-end chunked pipeline. |
| **LoC estimate** | ~600 production + ~200 test |
| **SDD cycle** | full 6-stage |
| **Acceptance gate** | `ep_annihilation(10M) w=4` via chunked pipeline produces the same canonicalized Normal Form as the non-chunked baseline (invariant under confluence); peak coordinator memory scales with `chunk_size`, not `total_agents`. |
| **TCC relevance** | HIGH — this is the memory story's flagship demo |
| **Risk** | MEDIUM-HIGH — chunk-boundary edge cases |
| **Prerequisites** | D-010 + D-011 |

### Bundle D-013 — 2.32 Sparse Net Representation

| Field | Value |
|-------|-------|
| **Matrix ID** | 2.32 |
| **Spec** | SPEC-22 §4 — needs spec-critic (shared pass with D-009) |
| **Scope** | Alternative `Net` representation backed by `HashMap<AgentId, Agent>` + `HashMap<(AgentId, PortId), PortRef>`. Independent of arena recycling; useful for construction/partition phases. |
| **Files expected** | NEW `relativist-core/src/net/sparse.rs` implementing the Net interface; feature flag `sparse-net` default OFF; UTs mirroring `Net` trait requirements. |
| **LoC estimate** | ~600 production + ~200 test |
| **SDD cycle** | full 6-stage; **extra proptest** comparing dense vs sparse on randomized inputs |
| **Acceptance gate** | On `ep_construct(5M)` before any reduction, sparse representation uses < 30% of dense peak memory; reduction results canonically equivalent. |
| **TCC relevance** | MEDIUM — orthogonal memory-optimization axis |
| **Risk** | MEDIUM — two parallel representations risk code duplication |
| **Independent of** | D-009/010/011/012 (runs as last of Phase 2) |

---

## 4. Phase 3 — Tier 4: UX & Deployment

Six features, **no prior specs** — each bundle pays a spec-authoring cost. Unlocks multi-machine benchmarking.

### Bundle D-014 — 2.37 Tailscale Mesh VPN Integration

| Field | Value |
|-------|-------|
| **Matrix ID** | 2.37 |
| **Spec** | **NEW SPEC-28** (or SPEC-17 amendment) — author from `ROADMAP.md §2.37` design text |
| **Scope** | `--bind tailscale`, `--advertise-dns <name>`, `--discover tailscale`; `tailscale status --json` parsing; USAGE_GUIDE quickstart. |
| **Files expected** | NEW `relativist-core/src/protocol/tailscale.rs`; MODIFY `config.rs` (new flags); MODIFY CLI; DOCS `USAGE_GUIDE.md`. |
| **LoC estimate** | ~300 production + ~50 test + ~100 docs |
| **SDD cycle** | SPEC authoring + critic · full 6-stage DEV |
| **Acceptance gate** | Coordinator binds to Tailscale IPv4 when installed; CLI `--help` renders without Tailscale; graceful `tailscale`-missing error; integration smoke test uses a fixture JSON. |
| **TCC relevance** | HIGH — unblocks multi-machine Phase 3 LAN benchmark across NATs |
| **Risk** | LOW-MEDIUM — subprocess coupling + JSON shape drift |
| **Subsumes** | partial D-016 on tailnet via MagicDNS |

### Bundle D-015 — 2.38 Installation & Distribution UX

| Field | Value |
|-------|-------|
| **Matrix ID** | 2.38 |
| **Spec** | **NEW SPEC-29 or SPEC-15 amendment** |
| **Scope** | SignPath application; `cargo-wix` MSI; winget / Homebrew / scoop manifests; USAGE_GUIDE install quickstart. |
| **Files expected** | NEW `wix/main.wxs` (generated); MODIFY `.github/workflows/release.yml`; NEW `packaging/{winget,scoop,homebrew}/*`; DOCS. |
| **LoC estimate** | ~250 lines YAML/manifest + ~50 Rust + docs |
| **SDD cycle** | SPEC amendment · 1-2 task split · DEV · CI smoke · REVIEW (QA may collapse given config-scope) |
| **Acceptance gate** | Tagged release publishes signed `.exe` + `.msi` + manifests; SmartScreen suppressed on signed MSI; release notes auto-populate. |
| **TCC relevance** | MEDIUM — enables demos and onboarding for Phase 3 LAN participants |
| **Risk** | MEDIUM — SignPath approval is external and slow |
| **Action NOW (before bundle starts):** submit SignPath application early so approval arrives by the time we reach D-015 |

### Bundle D-016 — 2.8 Automatic Node Discovery (mDNS/DNS-SD)

| Field | Value |
|-------|-------|
| **Matrix ID** | 2.8 |
| **Spec** | **NEW SPEC-30 or amendment to SPEC-28** (Tailscale spec may host mDNS as alternate backend) |
| **Scope re-evaluation after D-014** | Decide full (~300 LoC via `mdns-sd`) vs minimal (LAN broadcast fallback ~100 LoC) vs drop. |
| **Files expected** | NEW `relativist-core/src/protocol/discovery.rs`; CLI flag `--discover mdns`; worker auto-connect. |
| **LoC estimate** | ~100 / ~300 depending on scope |
| **SDD cycle** | SPEC authoring lite · full 6-stage (or collapsed for minimal scope) |
| **Acceptance gate** | Worker with `--discover mdns` finds coordinator advertising `_relativist._tcp.local` on same LAN within 5s. |
| **TCC relevance** | LOW (complementary to D-014) |
| **Risk** | LOW |

### Bundle D-017 — 2.11 Intelligent Partitioning (redex-aware)

| Field | Value |
|-------|-------|
| **Matrix ID** | 2.11 |
| **Spec** | **NEW SPEC-31** |
| **Scope** | Redex-aware partition strategy co-locating likely-to-interact agents in the same worker. Strategy trait + greedy implementation. |
| **Files expected** | NEW `relativist-core/src/partition/strategies/mod.rs`; NEW `relativist-core/src/partition/strategies/redex_aware.rs`; CLI flag `--partition-strategy redex-aware`. |
| **LoC estimate** | ~400 production + ~100 test |
| **SDD cycle** | full (spec authoring + critic + 6 stages) |
| **Acceptance gate** | On `cascade_cross` / `dual_tree` benchmarks: ≥10% fewer border redexes per round vs round-robin on ≥ 1 workload × K combination; G1 parity unchanged. |
| **TCC relevance** | HIGH — sharpens c_o/c_r for the rerun benchmark |
| **Risk** | MEDIUM — empirical validation; may discover round-robin is already near-optimal |

### Bundle D-018 — 2.7 Intra-Worker Parallelism (rayon)

| Field | Value |
|-------|-------|
| **Matrix ID** | 2.7 |
| **Spec** | **NEW SPEC-32 OR SPEC-13 R49 amendment** |
| **Scope** | `reduce_all` uses rayon across N threads; per-thread id-range subdivision; race-free arena access. |
| **Files expected** | MODIFY `reduction/core.rs`; MODIFY `partition/mod.rs` (thread-local id ranges); CLI `--threads N`. |
| **LoC estimate** | ~400 production + ~150 test (proptest for parallel=serial) |
| **SDD cycle** | full + proptest emphasis |
| **Acceptance gate** | `--threads 4` wall-clock ≥ 1.5× faster than `--threads 1` on `ep_con 1M`; bit-identical canonicalized output across thread counts. |
| **TCC relevance** | HIGH — second axis of parallelism for the narrative |
| **Risk** | MEDIUM-HIGH — arena races, non-determinism hazard |
| **Composes with** | D-017 multiplicatively |

### Bundle D-019 — 2.17 Streaming Arithmetic Encoding

| Field | Value |
|-------|-------|
| **Matrix ID** | 2.17 |
| **Spec** | **SPEC-14 amendment** |
| **Scope** | Replace batch Church `add(S(x), y) = add(x, S(y))` with streaming `add(S(x), y) = S(add(x, y))` per Mackie & Sato. |
| **Files expected** | MODIFY `encoding/church.rs` (add, mul, exp streaming variants); tests in same file. |
| **LoC estimate** | ~200 production + ~100 test |
| **SDD cycle** | SPEC amendment · small 4-stage pipeline |
| **Acceptance gate** | `add_streaming(n, m) == n + m` and `mul_streaming(n, m) == n * m`; intermediate interaction count ≤ batch variant. |
| **TCC relevance** | LOW-MEDIUM |
| **Risk** | LOW |

---

## 5. Cross-bundle risks & mitigations

| Risk | Bundles affected | Mitigation |
|------|------------------|------------|
| SPEC-20/21/22 spec-critic reveals structural issues requiring re-author | D-006..D-013 | Start spec-critic EARLY (before any DEV in its tier); budget 2-3 rounds as was needed for SPEC-19 §3.4 |
| SignPath approval slow | D-015 | Submit SignPath application NOW (before D-006 starts). External wait runs in parallel with the 13 other bundles |
| Hybrid coordinator + dynamic join/leave FSM interaction | D-006 × D-007 × D-008 | Design D-007/008 with hybrid coordinator in mind from Stage 0; integration test matrix explicitly covers `hybrid=true + join + leave` |
| Streaming pipeline correctness under confluence | D-010 × D-011 × D-012 | Per-batch invariant preservation proptest; end-to-end equivalence vs non-streaming on `ep_annihilation(1M)` as golden reference |
| Rayon + arena data races | D-018 | Proptest: parallel result == serial result on 100+ random fixtures; `#[cfg(debug_assertions)]` arena guards |
| Redex-aware partitioning trades one overhead for another | D-017 | Acceptance gate requires measured border-redex reduction; fall back to round-robin default if no win |
| Tier 2 hybrid coordinator introduces non-deterministic race between self-partition reduce and collect | D-006 | Strict ordering: coordinator completes self-reduce BEFORE entering the collect phase; FSM invariant guarded by test |
| Scope creep per bundle (>600 LoC in DEV) | all | Strict ≤200 LoC tasks per SDD rule; re-split triggered at 600 LoC DEV |
| Ambient risk — deadline pressure near TCC defense | all | After Phase 2 closes, if TCC deadline looms, Tier 4 bundles can be parallelized or subset-shipped (e.g., ship D-014 + D-015 + D-017 + D-018, defer D-016 + D-019 to v3) |

---

## 6. Phase gates

**Gate 1 — Tier 2 complete (end of Phase 1):**
- D-006 + D-007 + D-008 all CLOSED
- Integration test: 3-worker hybrid run with mid-round join + mid-round leave produces canonically-equivalent result to static K=4 run
- `cargo test --workspace --lib` preserved

**Gate 2 — Tier 3 complete (end of Phase 2):**
- D-009..D-013 all CLOSED
- `ep_annihilation 10M w=4` via chunked streaming pipeline produces canonically-equivalent Normal Form to non-streaming baseline; peak memory bounded
- `cargo test --workspace --lib` preserved

**Gate 3 — Tier 4 complete (end of Phase 3) == Benchmark gate:**
- D-014..D-019 all CLOSED
- Tagged release with signed MSI + winget + Homebrew + Tailscale quickstart
- Phase 3 LAN benchmark environment set up on Tailscale tailnet
- `cargo test --workspace --lib` preserved

**Benchmark deliverable (after Gate 3):** multi-machine benchmark campaign, CSV data, article Section 5 update.

---

## 7. Tier 5 priorities (post-benchmark placeholder)

After Gate 3 + benchmark, the Tier 5 priority decision happens:

| Matrix ID | Feature | TCC relevance | LoC | Notes |
|-----------|---------|---------------|-----|-------|
| 2.15 | Compact Memory (HVM2-style) | MEDIUM | ~400 | SPEC-23 drafted |
| 2.29 | Recipe-Based Gen | LOW-MEDIUM | ~700 | SPEC-25 drafted; closes D-001 |
| 2.36 | Lazy Demand-Driven Gen | LOW | ~350 | SPEC-21 amendment; requires D-011+D-010 |
| 2.21 | WAN Deployment + Security | LOW for TCC | ~1850 | SPEC-24 drafted; large scope |
| 2.21.1 | End-to-End Security Analysis | LOW | ~300 | sub-bundle of 2.21 |
| 2.39 | GUI (Tauri) | HIGH for defense | ~3000+ | SPEC-26 drafted; major scope |
| 2.41 | Encoder/Decoder Registry | LOW | ~900 | SPEC-27 phase 6 completion (D-001 closure) |
| 2.42 | Label Support Extended ICs | PENDING | ~1000+ | architectural decision required |

**Recommended Tier 5 triage at Gate 3:** pick 1-2 high-TCC-impact items given remaining time. Most likely candidates: **2.39 GUI** (defense demo) + **2.29 Recipe Gen** (closes D-001 cleanly). Defer 2.21/2.21.1 (WAN) to v3 — the TCC thesis doesn't depend on it.

---

## 8. Execution decisions required before launch

1. **Confirm ordering:** `D-006 → D-007 → D-008 → D-009 → D-010 → D-011 → D-012 → D-013 → D-014 → D-015 → D-016 → D-017 → D-018 → D-019`, OR reorder.
2. **Confirm parallelism:** single-threaded bundle-by-bundle (safest), OR run SignPath application (D-015 external) in parallel with D-006 immediately, OR allow phase-internal parallelism later.
3. **Confirm git push** of D-005 commits (`a431320`, `1ea2c03`) to `origin/v2-development`.
4. **Decide SignPath timing:** start application now (recommended) or at D-015.
5. **Confirm D-016 scope pre-commit** (full/minimal/drop after D-014) OR defer decision.

---

## 9. Next action (after user confirms §8)

1. Orchestrator writes `docs/pipeline-state.md` with D-006 as Active Bundle, archives NEXT placeholder.
2. `git push origin v2-development` (if authorized).
3. Dispatch **`spec-critic`** (opus) on SPEC-20 entire document — adversarial review Round 1 covering all three Tier 2 features. Deliverable: `docs/spec-reviews/SPEC-REVIEW-20-round-1-2026-04-24.md`.
4. On spec-critic sign-off (expected 2-3 rounds), dispatch **`task-splitter`** for D-006 (coordinator-as-worker) scope.
5. In parallel (Task 3 onward), start SignPath application.
6. Author detailed TDD plan for D-006 in `docs/plans/2026-04-24-d-006-hybrid-coordinator-tdd.md` per `superpowers:writing-plans`.
7. Execute D-006 via `superpowers:subagent-driven-development` (fresh subagent per task, two-stage review).
8. On D-006 close, advance to D-007, repeat.

---

## Self-review

- **All 14 bundles have:** matrix ID, spec pointer, scope, files expected, LoC estimate, SDD cycle, acceptance gate, TCC relevance, risk — ✓
- **Inter-bundle dependencies named:** D-008 ← D-007; D-011 ← D-010; D-012 ← D-010+D-011; D-016 ← D-014; D-018 composes with D-017 — ✓
- **Gates measurable:** test counts preserved; canonicalized equivalence; memory bounds; wall-clock improvements; border-redex reductions — ✓
- **Spec authoring vs review correctly distinguished:** Tier 2 and Tier 3 inherit drafted specs (review only); Tier 4 needs full authoring (6 new specs / amendments) — ✓
- **SignPath external wait mitigated:** flagged to start in parallel at plan launch — ✓
- **Tier 5 placeholder with recommendation:** 2.39 GUI + 2.29 Recipe Gen as the focused pair — ✓

**Gaps user must confirm before execution:** 5 decisions in §8.
