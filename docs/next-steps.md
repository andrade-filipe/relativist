# Next Steps & Active Pipeline

> **CRITICAL LLM INSTRUCTION:** This file is for **ACTIVE and FUTURE work only**. Do NOT accumulate historical logs here. Once a task, bundle, or milestone is DONE, its record MUST be moved to `progress.md`.

## Active Pipeline State

**Maintained by:** sdd-pipeline agent (see `docs/WORKFLOWS.md`)

### Active Bundle — Tier 2 + Tier 3 DEV

**Opened:** 2026-04-25 (Tier 2 first; Tier 3 next; Tier 4+5 deferred per user directive).
**Authoritative plan:** `C:\Users\Filipe\.claude\plans\kind-shimmying-harbor.md` (master) + `docs/plans/2026-04-24-tier-4-master-plan.md` §2/§3.

### SPEC-20 Elastic Grid Bundle (D-006) — STATUS: 3 of 4 phases REFACTORED; Phase D outstanding

The Stage 3 DEV pass (TASK-0410..0455) was delegated to a non-Claude LLM 2026-04-25; commits `4fb77bc..a84cb37` (12 commits, archived at tag `v2-llm-experiment-archive`). The code compiled and tests passed (1256/1299) but the Stage 4+5 audit on 2026-04-27 found **14 CRITICAL + 23 HIGH bugs** (see `docs/reviews/AUDIT-SUMMARY-2026-04-27.md`).

**Stage 6 REFACTOR progress (2026-04-27):**

| Phase | Verdict | Refactor commit | Tests (default → final) | Status |
|-------|---------|-----------------|-------------------------|--------|
| **E** (Observability) | ACCEPT_WITH_FIXES | `434a242` | 1256 → 1273 | ✅ DONE |
| **B** (Foundations) | ACCEPT_WITH_FIXES | `df93908` | 1273 → 1282 | ✅ DONE |
| **C** (Joining) | REJECT_WITH_FIXES | `8dd6d1b` | 1282 → 1292 | ✅ DONE (BTreeMap migration deferred to Phase D rework) |
| **D** (Departure) | REJECT | — | (1292) | 🛑 **OUTSTANDING — Option A scope** |

**Phase D — next action (after rate-limit reset):**

Reviewer's recommended **Option A** (instead of full reclaim+reconstruct rework, which the reviewer estimated 5-7 days):

1. **Remove** the broken reclaim path (`materialize_reclaimed_partitions` + `reconstruct(border_graph, evolved_survivors, round_0_reclaimed)` block) from `protocol/coordinator.rs`. Delete `partition/departure_recovery.rs`.
2. **Enforce** `GridConfig.elastic_departure: bool = false` as default. When `true`, log a one-time warning and proceed as if `false`.
3. **Keep** detection helpers (`handle_connection_loss`, `handle_phase_timeout`), `RetainedStateRegistry` (already fixed by Phase E refactor's `register_initial`), `LeaveAck` send-before-close, `PROTOCOL_VERSION = 4`.
4. **Wire** `release_worker(wid)` at every departure path (QA-010 D — currently never called → unbounded retained-state growth).
5. **Defer to v2.1:** full `elastic_departure=true` reclaim + reconstruct path; `worker_streams: Vec → BTreeMap` migration (Phase B QA-006 / Phase C QA-002).
6. Single new commit: `refactor(elastic-grid): apply Option A — Phase D (TASK-0438..0443)`.

**Phase D consolidated brief is in the previous orchestrator dispatch transcript;** re-dispatching the developer agent on Phase D after rate-limit reset is the next concrete action.

**Test floor invariant:** ≥ 1292 default / 1335 zero-copy (current). v1 floor (690) inviolable.

**Bundle close-out gate:** after Phase D commits, run final `cargo test`/`clippy`/`fmt`, archive Phase A audit artifacts (`docs/reviews/REVIEW-TASK-0411..0414`, `docs/qa/QA-TASK-0411..0414`) under `archive/`, tag `v2.0-elastic-grid-detection-only`.

---

### Strategic decision (2026-04-25, user directive)

**Implement Tier 2 + Tier 3 first; defer Tier 4 + Tier 5 decisions until after Tier 2+3 are shipped.**

| Tier | Status |
|------|--------|
| Tier 1 | ✅ DONE — frozen at `a431320` (D-005 Option A 12/12 G1 parity green) |
| **Tier 2** (Elastic Grid) | ⏭ **IN PROGRESS — Phase D Option A pending** |
| **Tier 3** (Memory Efficiency) | ⏭ AFTER Tier 2 D-006 closes |
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
