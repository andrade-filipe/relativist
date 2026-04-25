# Next Steps & Active Pipeline

> **CRITICAL LLM INSTRUCTION:** This file is for **ACTIVE and FUTURE work only**. Do NOT accumulate historical logs here. Once a task, bundle, or milestone is DONE, its record MUST be moved to `progress.md`.

## Active Pipeline State

**Maintained by:** sdd-pipeline agent (see `docs/WORKFLOWS.md`)

### Active Bundle

**v2 Pre-DEV Spec Pipeline (Waves 1–5, no code)** — opened 2026-04-24 by user directive: replicate the v1 success pattern by driving every pending v2 spec through Stage 0 (Spec Review) + Stage 1 (TASK-SPLITTER) + Stage 2 (TEST-GENERATOR) BEFORE any DEV. When this bundle closes, the backlog will hold ~80–150 new TODO tasks each with a TEST-SPEC ready, so Stage 3 (DEV) can run continuously without design stalls.

**Authoritative plan:** `C:\Users\Filipe\.claude\plans\kind-shimmying-harbor.md` (read this first; do not duplicate its contents here).

**Waves (per-spec workflow: PESQUISADOR coherence brief → spec-critic Round 1 → especialista-specs Round 2 → [Round 3 if needed] → task-splitter → test-generator → tracking + commit):**

| Wave | Spec(s) | Tier / Milestone | Status |
|------|---------|------------------|--------|
| 1 | SPEC-20 Elastic Grid | Tier 2 / M2 | **DONE 2026-04-24 (pending commit)** — Stage 0: `Reviewed v2`, closure log `docs/spec-reviews/SPEC-REVIEW-20-round-3-2026-04-24.md` (10 NFs closed). Stage 1: 36 tasks TASK-0410..0455 in `docs/backlog/`, BACKLOG.md SPEC-20 section + coverage matrix added (~9.6k LoC est). Stage 2: 62 TEST-SPECs (33 EG-U / 10 EG-I / 6 EG-P / 3 EG-B / 10 plumbing) + INDEX in `docs/tests/`. Forward-refs logged for SPEC-02 A7, SPEC-04 A3/A4, SPEC-19 A8. After commit, move row to progress.md and trigger Wave 2. |
| 2 | SPEC-22 Arena Mgmt → SPEC-21 Streaming Generation | Tier 3 / M5 | NOT STARTED |
| 3 | SPEC-25 Recipe Gen → SPEC-27 R26-R28 (deferred D-001 — plan tasks/tests now per user directive) | Tier 5 / M10+M7 | NOT STARTED |
| 4 | SPEC-26 §3.2-§3.6 GUI App (workspace §3.1 already shipped) | Tier 5 / M11 | NOT STARTED |
| 5 | SPEC-23 Compact Memory → SPEC-24 WAN Deployment | Tier 5 / M9 | NOT STARTED |

**Out of scope this bundle:** any DEV (`src/` edits), any v1 changes, the article (`tcc_pt_br.tex`), and Tier 4 features without SPEC ID (2.7, 2.8, 2.11, 2.17, 2.37, 2.38).

**Test counts unchanged during this bundle:** baseline 1181 default / 1224 `--features zero-copy` (this is documentation-only work).

**On Wave close:** sdd-pipeline appends a closure entry to `progress.md` (date, spec, qty tasks, qty test-specs, NFs closed, commit hash) and removes the wave row from this table. When the last wave closes, this Active Bundle entry is replaced with whichever DEV bundle the user selects next.

---

## Deferred Work & Blockers (from DEFERRED-WORK.md)

### D-001 — SPEC-27 R26 / R27 / R28 (RecipeEncoder integration with SPEC-25)

| Field | Value |
|-------|-------|
| **Source spec** | SPEC-27 (Encoder/Decoder Trait API) |
| **Requirements deferred** | R26, R27, R28 |
| **Shipped instead** | R24 + R25 only (trait definition + non-coupling guarantee) — TASK-0340 |
| **Unblocker** | SPEC-25 (Recipe-Based Distributed Generation) implementation = ROADMAP item 2.29 = **Milestone M7** |
| **Why deferred** | SPEC-25 is not yet implemented in code (no `GenerationRecipe` type, no `AssignRecipe` wire message, no `make_recipe()` calls in coordinator/worker). R26/R27/R28 are *integrations* with code that does not exist. Shipping them now would mean writing speculative integration code with no observable behavior. |
| **Estimated effort** | ~150 LoC (R26 + R27) + ~50 LoC (R28) = **~200 LoC**, 2-4 days, after SPEC-25 is in. |
| **Files to revisit** | `relativist-core/src/encoding/recipe.rs`, `relativist-core/src/encoding/registry.rs`, `relativist-core/src/protocol/messages.rs`, `relativist-core/src/coordinator.rs`, `relativist-core/src/worker.rs` |
| **Status** | OPEN — waiting on M7 |

### D-003 — SPEC-19 R13 / R14 / R15 parts 1-2 (coordinator-side border-redex resolution)

| Field | Value |
|-------|-------|
| **Requirements deferred** | **Asymmetric rules (CON-DUP, CON-ERA, DUP-ERA)** cross-partition G1 parity. |
| **Unblocker** | **D-005** — propagate `CommutationBatch.local_wiring` across the `PendingCommutation` wire contract (or equivalent LocalDeltaDispatch workaround). |
| **Status** | CLOSED 2026-04-24 via commit a431320 (D-005 Option A — G1 parity 12/12 green). |

---

## v2 Feature Matrix (Tactical Planning)

**Last updated:** 2026-04-15 (rev3)
**Source:** `docs/ROADMAP.md` (items 2.1–2.42, excluding 2.18/2.19 ARCHIVED and 2.20 DONE)

### Status Legend

| Status | Meaning |
|--------|---------|
| DONE | Already implemented in v1 |
| ARCHIVED | Evaluated and rejected / superseded |
| PLANNED | Scheduled for v2 implementation |
| FUTURE | Deferred to v3+ / documented as future work |

### Complexity Legend

| Level | Meaning | Typical LoC | Typical Time |
|-------|---------|-------------|--------------|
| Trivial | Config/tuning, no new abstractions | < 150 | < 1 day |
| Low | Single module, clear insertion point | 150–300 | 1–3 days |
| Medium | Multiple files, new types/traits | 300–600 | 3–7 days |
| High | Cross-module, new abstractions, spec changes | 600–1200 | 1–3 weeks |
| Very High | Architectural change, formal argument update | 1200+ | 3+ weeks |

---

### Tier 1 — CRITICAL: Break-Even Path (c_o/c_r < 0.50)

These features directly attack the 80% distribution overhead and 20% transport overhead that prevent speedup. They are the reason v2 exists.

**Target:** Reduce c_o/c_r from 2.2 to < 0.50, achieving speedup > 1.0 with w=2.

| ID | Feature | Status | Spec | Complexity | Est. LoC | Est. Time | Impact | Prerequisites |
|----|---------|--------|------|------------|----------|-----------|--------|---------------|
| 2.22 | TCP Transport Tuning (NODELAY, buffers, keepalive) | **DONE** (commit c360fe5, 2026-04-15) | SPEC-17 | Trivial | ~100 | 2–4h | -5–15% transport time | None |
| 2.23 | Wire Format Compaction (bincode v2 + varint + LZ4) | IN PROGRESS (TASK-0343..0347) | SPEC-18 | Medium | ~450 | 2–3d | -30–50% wire size | None |
| 2.34 | Coordinator-Free Round (skip merge when no borders) | **DONE** (shipped 2026-04-16) | SPEC-19 | Low-Medium | ~300 | 2–3d | Skip rounds without border redexes | None (benefits from strict BSP, already shipped) |
| 2.24 | Zero-Copy Archive (rkyv on hot path) | **DONE** (shipped 2026-04-16) | SPEC-18 | Medium-High | ~600 | 3–5d | -50–80% deserialize CPU | 2.23 (wire format migration) |
| 2.25 | Same-Host Fast Path (UDS / shared memory) | **DONE** (commit c360fe5, 2026-04-15) | SPEC-17 | Medium | ~500 | 4–6d | Eliminates TCP loopback overhead | None |
| 2.35 | Delta-Based Merge (BorderGraph, lightweight resolution) | **DONE** (shipped 2026-04-17) | SPEC-19 | High | ~800 | 1–2w | Eliminates merge memory peak | None (design with 2.26) |
| 2.26 | Delta-Only Protocol (stateful workers, border deltas) | **PARTIAL** (bundle A/B/C/D shipped 2026-04-18; refactor 2026-04-23 closed MF-001/MF-002 for symmetric rules CON-CON/DUP-DUP/ERA-ERA; D-004 coordinator-side plumbing shipped 2026-04-23 via TASK-0398+TASK-0399; asymmetric rules CON-DUP/CON-ERA/DUP-ERA flip now deferred to D-005 — worker-side `CommutationBatch.local_wiring` application) | SPEC-19 | Very High | ~1600 | 2–4w | -90% per-round wire cost | 2.34, 2.35 (shared BorderGraph concept) |

**Implementation order:** 2.22 → 2.23 → 2.34 → 2.24 → 2.25 → 2.35 → 2.26

**Projected result:** With 2.26+2.34+2.35 fully deployed: c_o/c_r ≈ 0.10, speedup w=2: **1.67x**, w=4: **2.86x**, w=8: **4.44x**.

---

### Tier 2 — IMPORTANT: Elastic Grid (Confluence Showcase)

These features demonstrate the TCC's central argument: strong confluence enables free redistribution of work. They are Relativist's theoretical differentiator.

| ID | Feature | Status | Spec | Complexity | Est. LoC | Est. Time | Prerequisites |
|----|---------|--------|------|------------|----------|-----------|---------------|
| 2.1 | Coordinator as Worker (hybrid node) | PLANNED | SPEC-20 | Low | ~200 | 2–3d | None |
| 2.2 | Dynamic Worker Joining (between BSP rounds) | PLANNED | SPEC-20 | Medium | ~500 | 1w | None |
| 2.3 | Dynamic Worker Departure (timeout + re-dispatch) | PLANNED | SPEC-20 | Medium-High | ~600 | 1–2w | 2.2 |

**Implementation order:** 2.1 → 2.2 → 2.3

---

### Tier 3 — USEFUL: Memory Efficiency

Enable Relativist to handle nets larger than available RAM. Important for scalability demonstrations.

| ID | Feature | Status | Spec | Complexity | Est. LoC | Est. Time | Prerequisites |
|----|---------|--------|------|------------|----------|-----------|---------------|
| 2.33 | Arena Recycling (free-list variant) | PLANNED | SPEC-22 | Low | ~150 | 1d | None |
| 2.28 | Online/Streaming Graph Partitioning | PLANNED | SPEC-21 | Medium | ~400 | 3–5d | None |
| 2.27 | Streaming Net Generation (producer-consumer) | PLANNED | SPEC-21 | Medium | ~500 | 3–5d | 2.28 |
| 2.30 | Chunked Generation + Incremental Partitioning | PLANNED | SPEC-21 | Medium | ~600 | 3–5d | 2.27, 2.28 |
| 2.32 | Sparse Net Representation (HashMap-backed) | PLANNED | SPEC-22 | Medium | ~600 | 3–5d | None |

---

### Tier 4 — COMPLEMENTARY: UX and Deployment

Improve user experience without affecting performance or correctness.

| ID | Feature | Status | Spec | Complexity | Est. LoC | Est. Time | Prerequisites |
|----|---------|--------|------|------------|----------|-----------|---------------|
| 2.37 | Tailscale Mesh VPN Integration | PLANNED | — | Low-Medium | ~300 | 2–3d | None |
| 2.38 | Installation UX (MSI, winget, Homebrew) | PLANNED | — | Low-Medium | CI config | 3–5d | None |
| 2.8 | Automatic Node Discovery (mDNS/DNS-SD) | PLANNED | — | Low-Medium | ~300 | 2–3d | None |
| 2.7 | Intra-Worker Parallelism (rayon) | PLANNED | — | Medium | ~400 | 3–5d | None |
| 2.11 | Intelligent Partitioning (redex-aware) | PLANNED | — | Medium | ~400 | 3–5d | None |
| 2.17 | Streaming Arithmetic Encoding | PLANNED | — | Low-Medium | ~200 | 2–3d | None |

---

### Tier 5 — COMPLEX: Advanced Features for v2

Complex features that require significant effort but are important for v2's value proposition.

| ID | Feature | Status | Spec | Complexity | Est. LoC | Est. Time | Prerequisites |
|----|---------|--------|------|------------|----------|-----------|---------------|
| 2.15 | Compact Memory Representation (u64 encoding) | PLANNED | SPEC-23 | Medium | ~400 | 2–3w | None (amends SPEC-02 net/ types) |
| 2.29 | Recipe-Based Distributed Generation | PLANNED | SPEC-25 | Medium-High | ~700 | 2–3w | None (independent of 2.27/2.28) |
| 2.36 | Lazy/Demand-Driven Generation (pull model) | PLANNED | SPEC-21 amend | Low-Medium | ~350 | 1w | 2.27, 2.28 (orchestration layer) |
| 2.21 | WAN/Internet Deployment (NAT, TLS, auth, reconnect) | PLANNED | SPEC-24 | High | ~1850 | 3–4w | 2.25 (Transport trait via SPEC-17) |
| 2.21.1 | End-to-End Security Analysis and Hardening | PLANNED | SPEC-24 | Medium | ~300 | 1–2w | 2.21 (security builds on WAN infra) |
| 2.39 | GUI Desktop Application (Tauri v2) | PLANNED | SPEC-26 | High | ~3000+ | 3–5w MVP | None (workspace restructure needed) |
| 2.41 | Encoder/Decoder API and Registry | PLANNED | SPEC-27 | Medium | ~900 | 2–3w | SPEC-26 R1-R7 (workspace restructure) |
| 2.42 | Label Support for Extended ICs | DECISION PENDING | — | High | ~1000+ | 2–3w | Architectural decision (IC puro vs estendido) |

---

## Milestones

| Milestone | Specs | Features | Duration | Success Criterion |
|-----------|-------|----------|----------|-------------------|
| **M1** Transport Optimization | SPEC-17, SPEC-18 | 2.22 → 2.23 → 2.24 | 2–3w | tcp_localhost/seq ratio drops from 3.48x to ~1.5–2.0x |
| **M2** Elastic Grid Basics | SPEC-20 | 2.1 → 2.2 → 2.3 | 2–3w | Workers join/leave between rounds, 690+ tests green |
| **M3** Delta Foundation | SPEC-19 partial | 2.34 → 2.35 → 2.25 | 3–5w | Coordinator-free rounds observable on `cascade_cross` |
| **M4** Full Delta Protocol | SPEC-19 complete | 2.26 | 3–5w | `ep_con 5M w=2` speedup > 1.0 (BREAK-EVEN) — **PARTIAL (2026-04-23):** symmetric-rule G1 parity verified (CON-CON/DUP-DUP/ERA-ERA); D-004 coordinator-side round-N+2 finalizer plumbing shipped 2026-04-23; full BREAK-EVEN benchmark now blocked on D-005 (worker-side `CommutationBatch.local_wiring` application) |
| **M5** Memory Efficiency | SPEC-21, SPEC-22 | 2.33 → 2.28 → 2.27 → 2.30 | 2–3w | `ep_con 100M` runs on 2GB coordinator |
| **M10** Encoder/Decoder API | SPEC-27 | 2.41 | 2–3w | LambdaEncoder end-to-end, registry with 5 codecs, RecipeEncoder generalized — **NOTE:** RecipeEncoder full generalization (R26-R28) is held until M7 ships SPEC-25; M10 itself shipped only R24+R25. See DEFERRED-WORK.md D-001. |
