# Next Steps & Active Pipeline

> **CRITICAL LLM INSTRUCTION:** This file is for **ACTIVE and FUTURE work only**. Do NOT accumulate historical logs here. Once a task, bundle, or milestone is DONE, its record MUST be moved to `progress.md`.

## Active Pipeline State

**Maintained by:** sdd-pipeline agent (see `docs/WORKFLOWS.md`)

### Active Bundle — Tier 3 (Memory Efficiency) — D-010 NEXT (D-009 CLOSED)

**Authoritative plan:** `docs/plans/2026-04-24-tier-4-master-plan.md` §3.

### D-009: SPEC-22 Arena Management — ✅ CLOSED 2026-04-27

All 6 SDD stages shipped. Test floor advanced **1308 → 1464 default / 1351 → 1507 zero-copy** (+156 / +156). Commits:
- `01184f1` Phase A — 10 spec amendments A1..A10 (SPEC-01/02/03/04/05/18/19)
- `47d9bf2` Phase B — free-list core (Net.free_list, create/remove_agent, merge reconciliation)
- `d6411be` Phase C Wave 1+2 — SparseNet + conversions + I3' assertions
- `c36a999` Phase C Wave 3 — CI lint guards (R23/R31) + integration regression suite
- `0d7c4a8` Stage 6 Wave 1 — reviewer fixes (MF-001..002, SF-001..003)
- `d74d527` Stage 6 Wave 2 — QA CRITICAL fixes (validate_free_list on deserialize, border_entries_shadow population, merge dedupe, to_dense bound, inverted-range error)
- `bb1057f` Stage 6 Wave 3 — QA HIGH fixes (is_behaviorally_equal multiset/ordered, empty-partition id_range)
- `92145a0` Stage 6 Wave 4 — QA MEDIUM/LOW (CI regex tightening, AgentId overflow, SparseNet write_port guard)

**Audit artefacts:** `docs/reviews/REVIEW-PHASE-D009-spec22-arena-2026-04-27.md` (verdict ACCEPT_WITH_FIXES; 2 MF + 3 SF) and `docs/qa/QA-PHASE-D009-spec22-arena-2026-04-27.md` (5 CRITICAL + 3 HIGH + 6 MEDIUM/LOW).

**Deferred to follow-up:** QA-D009-001 (CompactSubnet wire format silently drops free_list) — requires SPEC-19 amendment. Tracked as `docs/backlog/TASK-0595-compactsubnet-free-list-followup.md`.

### D-010: Online / Streaming Graph Partitioning — NEXT

**Spec:** SPEC-21 §3 (streaming partition strategy trait).
**Prereqs:** SparseNet + free-list (delivered by D-009 ✅).
**Tasks:** TASK-0510..0554 + 0565/0567/0568/0575-0578/0588-0591 already split (Pre-DEV Wave 2 second half closed in `131ca26`); 49 TEST-SPECs already written.
**Stage:** 3 — DEV (Stages 1 SPLITTING and 2 TESTS are DONE; nothing to do at Stages 0-2).
**Test floor entering D-010:** 1464 default / 1507 zero-copy. v1 floor: 690.

**Next action:** invoke the **developer** agent for D-010 Phase B implementation. Pre-DEV split structure already in `docs/backlog/`; consult `BACKLOG.md` for the SPEC-21 task block.

---

### Strategic decision (2026-04-25, user directive)

**Implement Tier 2 + Tier 3 first; defer Tier 4 + Tier 5 decisions until after Tier 2+3 are shipped.**

| Tier | Status |
|------|--------|
| Tier 1 | ✅ DONE — frozen at `a431320` (D-005 Option A 12/12 G1 parity green) |
| **Tier 2** (Elastic Grid) | ✅ **D-006 CLOSED — Phase D Option A landed 2026-04-27 (commits df93908, 8dd6d1b, 7988573, fc680f5). Tag: v2.0-elastic-grid-detection-only** |
| **Tier 3** (Memory Efficiency) | ACTIVE — D-009 open 2026-04-27. Stage 3 DEV (amendments wave). |
| Tier 4 (UX/Deploy) | 🛑 DECISION DEFERRED |
| Tier 5 | 🛑 DECISION DEFERRED |

After Phase D closes: D-007/D-008 in the original plan are largely subsumed into the Phase C/D refactor work; reassess scope before opening D-007 separately. Tier 3 bundles (D-009..D-013) are ready to dispatch once D-006 is wrapped.

---

### Deferred to v2.1 (recorded 2026-04-27, post-audit)

| Item | Rationale | Original finding |
|------|-----------|------------------|
| Full `elastic_departure = true` reclaim + reconstruct | 5-7d estimated rework; v2.0 ships detection-only per reviewer Option A | MF-001..MF-009 D, QA-001..QA-005 D |
| `worker_streams: Vec → BTreeMap` migration | Position-as-identity bug only manifests with worker removal during runtime; Option A's v1-fallback semantics don't trigger it | QA-006 B, QA-002 C |
| `run_coordinator` decomposition into smaller `pub(crate) async fn`s | Not blocking; recommended for v2.1 maintainability | MF-006 C |
| Bounded `pending_connections_queue` size + parallel drain | Initial-window flush mitigates today; bound the queue at 256 with NACK overflow in v2.1 | QA-007/008/009 C |
| TASK-0443 reclaim counters wired into round loop | `retained_initial_reclaims_per_round` and `retained_last_acked_reclaims_per_round` are dead-on-arrival until reclaim path no longer early-returns | MF-004 E |
| SPEC-11 worker_id cardinality bound | Needs SPEC-11 amendment | QA-007 E |
| RetainedStateRegistry on-disk persistence | Not needed while `elastic_departure=false` is default | QA-001 D |
| TEST-SPEC-0450 UT-0450-01..13 full coverage | Phase E only added 28 regression tests; the full UT matrix is a v2.1 cleanup task | SF-003 E |
| Full R22a/R22b/R22c semantic divergence on `LeaveKind` | Option A uniformly removes the worker post-round; meaningful divergence requires the reclaim path | MF-005 D |
| `RetainedLastAcked::DeltaLight` real payload `(BorderGraph, RoundResult)` | Variant is unconstructible today; spec-correct payload lands with the v2.1 reclaim path | MF-007 D / QA-008 D |
| Concurrent `FuturesUnordered` recv-loop in collect phase | Sequential recv survives Option A because departure → run abort/SoloReducing on next iteration; concurrency adds value only once reclaim resumes the round | QA-009 D |
| Per-recv timeout in `accept_workers` | Slow-byte adversarial worker is a pre-departure DoS; bounded by `worker_connect_timeout` overall, but per-frame bound is v2.1 hardening | QA-015 D |

---

## Deferred Work & Blockers (legacy from DEFERRED-WORK.md)

### D-001 — SPEC-27 R26 / R27 / R28 (RecipeEncoder integration with SPEC-25)

| Field | Value |
|-------|-------|
| **Source spec** | SPEC-27 |
| **Requirements deferred** | R26, R27, R28 |
| **Unblocker** | SPEC-25 (Recipe-Based Distributed Generation) implementation = ROADMAP item 2.29 = **Milestone M7** |
| **Estimated effort** | ~200 LoC, 2-4 days, after SPEC-25 is in. |
| **Status** | OPEN — waiting on M7 |

### D-003 — SPEC-19 R13/R14/R15 parts 1-2 (asymmetric rules) — **CLOSED 2026-04-24** via commit `a431320` (D-005 Option A 12/12 G1 parity green)

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

### Tier 1 — CRITICAL: Break-Even Path (c_o/c_r < 0.50) — **DONE** (a431320)

### Tier 2 — IMPORTANT: Elastic Grid (Confluence Showcase)

| ID | Feature | Status | Spec |
|----|---------|--------|------|
| 2.1 | Coordinator as Worker | **DONE** (Phase B refactor `df93908`, 2026-04-27) | SPEC-20 |
| 2.2 | Dynamic Worker Joining | **DONE** (Phase C refactor `8dd6d1b`, 2026-04-27) | SPEC-20 |
| 2.3 | Dynamic Worker Departure | **PARTIAL** — detection + retained-state plumbing only (Option A); full reclaim deferred to v2.1 | SPEC-20 |

### Tier 3 — USEFUL: Memory Efficiency (D-009..D-013, all PLANNED, ready to dispatch)

| ID | Feature | Spec |
|----|---------|------|
| 2.33 | Arena Recycling (free-list) | SPEC-22 |
| 2.28 | Online/Streaming Graph Partitioning | SPEC-21 |
| 2.27 | Streaming Net Generation (producer-consumer) | SPEC-21 |
| 2.30 | Chunked Generation + Incremental Partitioning | SPEC-21 |
| 2.32 | Sparse Net Representation | SPEC-22 |

### Tiers 4 + 5 — DECISION DEFERRED (see Strategic decision above).

---

## Milestones

| Milestone | Specs | Features | Duration | Success Criterion |
|-----------|-------|----------|----------|-------------------|
| **M1** Transport Optimization | SPEC-17, SPEC-18 | 2.22 → 2.23 → 2.24 | DONE | tcp_localhost/seq ratio drops from 3.48x to ~1.5–2.0x |
| **M2** Elastic Grid Basics | SPEC-20 | 2.1 → 2.2 → 2.3 | **2.1+2.2 DONE; 2.3 detection-only (Option A)** | Workers join between rounds; departure detected + retained, full reclaim deferred to v2.1 |
| **M3** Delta Foundation | SPEC-19 partial | 2.34 → 2.35 → 2.25 | DONE | Coordinator-free rounds observable on `cascade_cross` |
| **M4** Full Delta Protocol | SPEC-19 complete | 2.26 | DONE | `ep_con 5M w=2` speedup > 1.0 (BREAK-EVEN) — verified 2026-04-24 commit `a431320` |
| **M5** Memory Efficiency | SPEC-21, SPEC-22 | 2.33 → 2.28 → 2.27 → 2.30 | NEXT — bundles D-009..D-013 ready to dispatch | `ep_con 100M` runs on 2GB coordinator |
| **M10** Encoder/Decoder API | SPEC-27 | 2.41 | DONE | LambdaEncoder end-to-end, registry with 5 codecs |
