# v2 Feature Matrix

**Last updated:** 2026-04-15 (rev3)
**Purpose:** Consolidated inventory of all v2 features with priority, complexity, estimates, and dependencies.
**Source:** `docs/ROADMAP.md` (items 2.1–2.42, excluding 2.18/2.19 ARCHIVED and 2.20 DONE)

---

## Status Legend

| Status | Meaning |
|--------|---------|
| DONE | Already implemented in v1 |
| ARCHIVED | Evaluated and rejected / superseded |
| PLANNED | Scheduled for v2 implementation |
| FUTURE | Deferred to v3+ / documented as future work |

## Complexity Legend

| Level | Meaning | Typical LoC | Typical Time |
|-------|---------|-------------|--------------|
| Trivial | Config/tuning, no new abstractions | < 150 | < 1 day |
| Low | Single module, clear insertion point | 150–300 | 1–3 days |
| Medium | Multiple files, new types/traits | 300–600 | 3–7 days |
| High | Cross-module, new abstractions, spec changes | 600–1200 | 1–3 weeks |
| Very High | Architectural change, formal argument update | 1200+ | 3+ weeks |

---

## Tier 1 — CRITICAL: Break-Even Path (c_o/c_r < 0.50)

These features directly attack the 80% distribution overhead and 20% transport overhead that prevent speedup. They are the reason v2 exists.

**Target:** Reduce c_o/c_r from 2.2 to < 0.50, achieving speedup > 1.0 with w=2.

| ID | Feature | Status | Spec | Complexity | Est. LoC | Est. Time | Impact | Prerequisites |
|----|---------|--------|------|------------|----------|-----------|--------|---------------|
| 2.22 | TCP Transport Tuning (NODELAY, buffers, keepalive) | **DONE** (commit c360fe5, 2026-04-15) | SPEC-17 | Trivial | ~100 | 2–4h | -5–15% transport time | None |
| 2.23 | Wire Format Compaction (bincode v2 + varint + LZ4) | IN PROGRESS (TASK-0343..0347) | SPEC-18 | Medium | ~450 | 2–3d | -30–50% wire size | None |
| 2.34 | Coordinator-Free Round (skip merge when no borders) | PLANNED | SPEC-19 | Low-Medium | ~300 | 2–3d | Skip rounds without border redexes | None (benefits from strict BSP, already shipped) |
| 2.24 | Zero-Copy Archive (rkyv on hot path) | PLANNED (DEFERRED-WORK.md D-002) | SPEC-18 | Medium-High | ~600 | 3–5d | -50–80% deserialize CPU | 2.23 (wire format migration) |
| 2.25 | Same-Host Fast Path (UDS / shared memory) | **DONE** (commit c360fe5, 2026-04-15) | SPEC-17 | Medium | ~500 | 4–6d | Eliminates TCP loopback overhead | None |
| 2.35 | Delta-Based Merge (BorderGraph, lightweight resolution) | **DONE** (R8-R19 pure-core shipped 2026-04-17; R13-R15 coordinator dispatch + R20-R36 wire deferred to 2.26) | SPEC-19 | High | ~800 | 1–2w | Eliminates merge memory peak | None (design with 2.26) |
| 2.26 | Delta-Only Protocol (stateful workers, border deltas) | PLANNED | SPEC-19 | Very High | ~1600 | 2–4w | -90% per-round wire cost | 2.34, 2.35 (shared BorderGraph concept) |

**Implementation order:** 2.22 → 2.23 → 2.34 → 2.24 → 2.25 → 2.35 → 2.26

**Projected result:** With 2.26+2.34+2.35 fully deployed: c_o/c_r ≈ 0.10, speedup w=2: **1.67x**, w=4: **2.86x**, w=8: **4.44x**.

---

## Tier 2 — IMPORTANT: Elastic Grid (Confluence Showcase)

These features demonstrate the TCC's central argument: strong confluence enables free redistribution of work. They are Relativist's theoretical differentiator.

| ID | Feature | Status | Spec | Complexity | Est. LoC | Est. Time | Prerequisites |
|----|---------|--------|------|------------|----------|-----------|---------------|
| 2.1 | Coordinator as Worker (hybrid node) | PLANNED | SPEC-20 | Low | ~200 | 2–3d | None |
| 2.2 | Dynamic Worker Joining (between BSP rounds) | PLANNED | SPEC-20 | Medium | ~500 | 1w | None |
| 2.3 | Dynamic Worker Departure (timeout + re-dispatch) | PLANNED | SPEC-20 | Medium-High | ~600 | 1–2w | 2.2 |

**Implementation order:** 2.1 → 2.2 → 2.3

**Confluence dependency:** All three require P1 (strong confluence) to be correct. P1 guarantees that redistributing work at any point produces the same result.

---

## Tier 3 — USEFUL: Memory Efficiency

Enable Relativist to handle nets larger than available RAM. Important for scalability demonstrations.

| ID | Feature | Status | Spec | Complexity | Est. LoC | Est. Time | Prerequisites |
|----|---------|--------|------|------------|----------|-----------|---------------|
| 2.33 | Arena Recycling (free-list variant) | PLANNED | SPEC-22 | Low | ~150 | 1d | None |
| 2.28 | Online/Streaming Graph Partitioning | PLANNED | SPEC-21 | Medium | ~400 | 3–5d | None |
| 2.27 | Streaming Net Generation (producer-consumer) | PLANNED | SPEC-21 | Medium | ~500 | 3–5d | 2.28 |
| 2.30 | Chunked Generation + Incremental Partitioning | PLANNED | SPEC-21 | Medium | ~600 | 3–5d | 2.27, 2.28 |
| 2.32 | Sparse Net Representation (HashMap-backed) | PLANNED | SPEC-22 | Medium | ~600 | 3–5d | None |

**Implementation order:** 2.33 → 2.28 → 2.27 → 2.30 (2.32 independent)

**MVP for memory-bounded operation:** 2.28 (round-robin, ~80 LoC) + 2.27 (ep_annihilation only, ~100 LoC) + 2.30 (chunked loop, ~150 LoC) = ~330 LoC total.

---

## Tier 4 — COMPLEMENTARY: UX and Deployment

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

## Tier 5 — COMPLEX: Advanced Features for v2

Complex features that require significant effort but are important for v2's value proposition. These extend Relativist beyond LAN-only academic benchmarks into a production-capable distributed IC reducer.

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

**Implementation order:** 2.15 → 2.29 → 2.36 (memory/gen track) | 2.21 → 2.21.1 (WAN track) | 2.39 (GUI track) | 2.41 (encoder track, needs SPEC-26 R1-R7 first) | 2.42 (pending decision)

**Key dependencies:**
- 2.21 requires SPEC-17 Transport trait (M1) to be complete — WAN builds on the transport abstraction
- 2.36 requires 2.27+2.28 (M5) — pull model is orchestration on top of streaming generation
- 2.29 is fully independent — recipe-based gen coexists with streaming gen
- 2.39 requires workspace restructure (`relativist-core` library extraction)

---

## Tier 6 — FUTURE: Research / v3+

Features that are research projects in their own right or require months of work. Documented in the TCC as "future work."

| ID | Feature | Status | Complexity | Est. Time | Reason for Deferral |
|----|---------|--------|------------|-----------|---------------------|
| 2.4 | Distributed Partitioning (tree topology) | FUTURE | Very High | 2–3 months | Research project: hierarchical coordination, O(n²) border interactions |
| 2.5 | Coordinator Election (Raft/Paxos) | FUTURE | High | 1–2 months | Consensus protocols are a separate domain |
| 2.6 | Work Stealing | FUTURE | High | 1–2 months | Incompatible with BSP barrier synchronization |
| 2.9 | Fault Tolerance (checkpointing + persistence) | FUTURE | High | 1–2 months | Requires persistence layer, Byzantine tolerance out of scope |
| 2.10 | Multi-Tenancy and Job Queuing | FUTURE | Medium-High | 1 month | Infrastructure complexity beyond TCC scope |
| 2.12 | GPU Workers (HVM2-style) | FUTURE | Very High | 3+ months | Heterogeneous compute, CUDA/OpenCL |
| 2.13 | Visualization (Graphviz, live dashboard) | FUTURE | Medium | 2–3w | Nice-to-have, not correctness/performance |
| 2.14 | WASM Target (browser-based IC reduction) | FUTURE | Medium | 2–3w | Educational, not critical for TCC |
| 2.16 | Streaming Reduction Mode (async, no barriers) | FUTURE | Very High | 2–3 months | Requires rewrite of G1 formal argument |
| 2.31 | Memory-Mapped Net (mmap, out-of-core) | FUTURE | Medium-High | 2–3w | Platform-specific complexity |

---

## Completed / Archived

| ID | Feature | Status | Notes |
|----|---------|--------|-------|
| 2.18 | Native Numeric Types (NUM/OPE/SWI) | ARCHIVED | Different goal from HVM. Relativist focuses on net partitioning/reduction; domain-specific operations are defined by encoders/decoders, not native agent types. |
| 2.19 | Protocol Payload Chunking | ARCHIVED | Superseded by 2.20 (CompactSubnet) + 1 GiB cap raise |
| 2.20 | Compact Subnet Encoding | DONE | Shipped as `CompactSubnet` in `src/partition/compact.rs` |

---

## Spec Mapping

### New Specs

| Spec | Title | Features | Tier | Milestone | Status |
|------|-------|----------|------|-----------|--------|
| SPEC-17 | Transport Abstraction & Tuning | 2.22, 2.25 | 1 | M1 | Created |
| SPEC-18 | Wire Format v2 | 2.23, 2.24 | 1 | M1 | Created |
| SPEC-19 | Delta Protocol & Stateful Workers | 2.26, 2.34, 2.35 | 1 | M3, M4 | Created |
| SPEC-20 | Elastic Grid | 2.1, 2.2, 2.3 | 2 | M2 | Created |
| SPEC-21 | Streaming Generation & Partitioning | 2.27, 2.28, 2.30, 2.36 | 3, 5 | M5, M7 | Created (2.36 amend done) |
| SPEC-22 | Arena Management & Memory Efficiency | 2.33, 2.32 | 3 | M5 | Created |
| SPEC-23 | Compact Memory Representation | 2.15 | 5 | M7 | Created |
| SPEC-24 | WAN Deployment and Security | 2.21, 2.21.1 | 5 | M8 | Created |
| SPEC-25 | Recipe-Based Distributed Generation | 2.29 | 5 | M7 | Created |
| SPEC-26 | GUI Application | 2.39 | 5 | M9 | Created |
| SPEC-27 | Encoder/Decoder Trait API | 2.41 | 5 | M10 | Phases 1-5 DONE, Phase 6 partial (R24+R25 only; R26-R28 deferred to M7, see DEFERRED-WORK.md D-001) |

### Existing Spec Amendments

| Spec | Amendment | Reason |
|------|-----------|--------|
| SPEC-01 | New invariants for delta protocol | G1 delta-aware decomposition |
| SPEC-02 | Compact memory types | u64 Agent, u32 PortRef (SPEC-23) |
| SPEC-04 | Streaming partitioning API | New `StreamingPartitionStrategy` trait |
| SPEC-05 | Delta merge, coordinator-free rounds | BorderGraph, convergence check |
| SPEC-06 | Wire format v2, new message variants | bincode v2, rkyv archive variants |
| SPEC-07 | WAN deployment config | NAT, TLS, discovery (SPEC-24) |
| SPEC-10 | Strong authentication | mTLS/OAuth2, replaces plaintext tokens (SPEC-24) |
| SPEC-13 | Transport trait, module boundaries v2 | New abstraction layer |

---

## Milestones

| Milestone | Specs | Features | Duration | Success Criterion |
|-----------|-------|----------|----------|-------------------|
| **M1** Transport Optimization | SPEC-17, SPEC-18 | 2.22 → 2.23 → 2.24 | 2–3w | tcp_localhost/seq ratio drops from 3.48x to ~1.5–2.0x |
| **M2** Elastic Grid Basics | SPEC-20 | 2.1 → 2.2 → 2.3 | 2–3w | Workers join/leave between rounds, 690+ tests green |
| **M3** Delta Foundation | SPEC-19 partial | 2.34 → 2.35 → 2.25 | 3–5w | Coordinator-free rounds observable on `cascade_cross` |
| **M4** Full Delta Protocol | SPEC-19 complete | 2.26 | 3–5w | `ep_con 5M w=2` speedup > 1.0 (BREAK-EVEN) |
| **M5** Memory Efficiency | SPEC-21, SPEC-22 | 2.33 → 2.28 → 2.27 → 2.30 | 2–3w | `ep_con 100M` runs on 2GB coordinator |
| **M6** Complementary | — | 2.37, 2.38, 2.7, ... | as time allows | UX improvements shipped |
| **M7** Advanced Memory & Generation | SPEC-23, SPEC-25, SPEC-21 amend | 2.15 → 2.29 → 2.36 | 4–6w | Compact memory reduces footprint 30%+; recipe gen for `ep_annihilation` works |
| **M8** WAN Deployment & Security | SPEC-24 | 2.21 → 2.21.1 | 4–6w | Workers connect via WAN with TLS; threat model documented |
| **M9** GUI Application | SPEC-26 | 2.39 | 5–8w | Tauri MVP: Dashboard + Generate + Reduce + Grid screens |
| **M10** Encoder/Decoder API | SPEC-27 | 2.41 | 2–3w | LambdaEncoder end-to-end, registry with 5 codecs, RecipeEncoder generalized — **NOTE:** RecipeEncoder full generalization (R26-R28) is held until M7 ships SPEC-25; M10 itself shipped only R24+R25. See DEFERRED-WORK.md D-001. |

---

## Dependency Graph (Tiers 1–5)

```
Tier 1–3 (unchanged)         Tier 5 additions              Cross-tier deps
─────────────────────       ──────────────────              ───────────────
2.22 (TCP tune)             2.15 (compact mem)  independent  M1 ──→ 2.21 (WAN needs transport)
2.34 (no-merge) ─┐         2.29 (recipe gen)   independent  M5 ──→ 2.36 (lazy needs streaming)
2.35 (delta)  ───┼→ 2.26   2.39 (GUI)          independent  SPEC-22 ──→ SPEC-23 (net/ types)
2.25 (UDS/SHM)   ┘
2.23 → 2.24 (wire)         2.21 → 2.21.1 (WAN → security)
2.1 → 2.2 → 2.3 (elastic)  2.27+2.28 → 2.36 (lazy gen)
2.28 → 2.27 → 2.30 (stream)
2.33 (free-list)
2.32 (sparse)
```

**Critical path to break-even:** 2.22 → 2.23 → 2.34 → 2.35 → 2.26
**Critical path to WAN:** M1 (SPEC-17 transport) → 2.21 → 2.21.1

---

## Review Gates

Every artifact passes explicit review before advancing. See plan file for full R1–R5 checklists.

| Gate | When | Reviewer | Key Check |
|------|------|----------|-----------|
| R1 | After briefings | User + orquestrador | P1-P6 correctly mapped, no invariant violations |
| R2 | After this matrix | User | All 38 features classified, dependencies correct |
| R3 | After each new spec | spec-critic | Requirements don't contradict SPEC-01–16, G1 preserved |
| R4 | After task splitting | User + sdd-pipeline | Tasks ≤ 200 LoC, DAG acyclic, no cross-milestone deps |
| R5 | After each milestone | qa + User | cargo test ≥ 690, clippy clean, G1 holds |
