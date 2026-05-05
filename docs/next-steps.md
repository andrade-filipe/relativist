# Next Steps & Active Pipeline

> **CRITICAL LLM INSTRUCTION:** This file is for **ACTIVE and FUTURE work only**. Do NOT accumulate historical logs here. Once a task, bundle, or milestone is DONE, its record MUST be moved to `progress.md`.

## Active Pipeline State

**Maintained by:** sdd-pipeline agent (see `docs/WORKFLOWS.md`)

### ✅ D-012 Instrumentation Restore — CLOSED + EMPIRICALLY VALIDATED 2026-05-05

Bundle CLOSED. Full 6-stage SDD cycle landed in 7 commits (`56aad38..c439182`) on `v2-development`. Restored 3 instrumentation channels surfaced as RF-04, RF-05, RF-07 in the D-011 cold post-mortem + repaired the release-mode test build. Full narrative + path decisions in `docs/progress.md` 2026-05-05 entry.

**Empirical validation (`results/locked/v2_post_d012_baseline_2026-05-05/`):** Re-ran the full `bench_docker_v2.sh --no-resume` campaign from HEAD `e6ff6bb`. **All RF-04/05/07 closures verified in CSV output.** 32/32 distributed slots `all_correct=true`, 10/10 reps, `network_time_secs > 0` and `compute_time_secs > 0` on 100 % of TCP rows, `mips_mean > 0` on 100 % of TCP rows (range 0.002–1.261 MIPS). Wall-time within ±1 % of the previous post-D-011 run, confirming the instrumentation adds zero measurable overhead. **This is now the canonical v2 baseline going into v0.20.0-pre.1 LAN testing.**

**Test floor:** 1798 default / 1842 zero-copy / 1789 streaming-no-recycle / 1740 release. v1 floor=690 inviolable. All cargo gates green on all 4 profiles.

**Deferred (separate follow-up tasks):**
- **TASK-0620** — bench subcommand has no real TCP path (uses `Mode::Local` hardcode); architectural gap, unrelated to D-012 scope
- **TASK-0621** — `cfg(debug_assertions)` audit across the codebase (TASK-0617 fixed 4 instances; QA flagged broader manifestations)
- **D-012-FU-SEQ-MIPS** — sequential-mode `mips_mean` still 0.000 (only distributed-mode `total_interactions` is wired); cosmetic, doesn't affect distributed analysis
- **D-011-FU-CONDUP** — investigate `con_dup_expansion(N)` setup-time asymmetry between sequential and distributed paths (RF-02 floor effect)

**Updated post-mortem analysis:** `docs/analysis/D011-final-baseline-analysis-2026-05-04.md` (rev 2026-05-05) now references the canonical baseline and marks RF-04/05/07 as CLOSED. Verdict updated DEFENSIBLE (was DEFENSIBLE WITH CAVEATS).

### ✅ D-011 BLOCKER — Performance regression CLOSED + LOCKED 2026-05-05

The partition perf regression flagged in the v2_tcp_baseline rodada is **CLOSED + LOCKED**. Final empirical baseline frozen at `results/locked/v2_d011_final_baseline_2026-05-04/` (32 configs × 10 reps over Docker TcpLocalhost, sequential native baselines included; MANIFEST.md with full provenance + SHA-256). See `docs/progress.md` 2026-05-04 entry for the investigation arc and `docs/analysis/D011-final-baseline-analysis-2026-05-04.md` for the cold post-mortem analysis (red-flag catalog + thesis-impact assessment).

**Bench verification (TASK-0614, `0fd27c0`):** ep_con 5M w=2 local wall median v1 = 14.247 s vs HEAD post-fix = 15.775 s, ratio **1.11× (within noise floor)**. ~88% of the +83% regression closed.

**Deferred (separate follow-up tasks, NOT blocking):** QA F-003 (empty dense partition + AF-2 brittleness — pre-existing QA-D009-009 contract gap, unrelated to D-011); QA F-005 (`freeport_redirects` memory cost at scale — perf concern, not correctness).

---

### Active Bundle — Tier 3 (Memory Efficiency) — D-013 NEXT (D-009..D-012 CLOSED)

**Authoritative plan:** `docs/plans/archive/2026-04-24-tier-4-master-plan.md` §3 (archived 2026-05-05; the active queue tracking now lives in this `next-steps.md` file).

### D-013: SPEC-21 / SPEC-22 hardening follow-ups (was D-011 NEXT before BLOCKER 2026-05-04 redirect) — NEXT

**Scope:** clear the deferred audit items left behind by D-009 + D-010 closures so SPEC-21/22 are production-grade before further Tier 3 work. Inventory:

| Source | ID | Severity | Title |
|--------|----|----------|-------|
| QA-D009 | QA-D009-001 | CRITICAL | CompactSubnet wire format silently drops free_list (requires SPEC-19 amendment via ESPECIALISTA EM SPECS) — pre-tracked as `docs/backlog/TASK-0595-compactsubnet-free-list-followup.md` |
| QA-D010 | QA-D010-009 (residual) | HIGH | Final `GridConfig.max_pending_lifetime` integration through `generate_and_partition_chunked_with_delta` callers (Stage 6 wired the new `_with_lifetime` wrapper end-to-end but legacy callers still pass `u32::MAX`) |
| QA-D010 | QA-D010-010 | MEDIUM | `worker_a/b` placeholder semantics |
| QA-D010 | QA-D010-011 | MEDIUM | `streaming-no-recycle` bypasses debug_asserts on free-list integrity |
| QA-D010 | QA-D010-012 | MEDIUM | Vacuous IT-0591 coverage — strengthen invariants |
| QA-D010 | QA-D010-013 | MEDIUM | Parallel state representations `Pull*`/`PullCoordinatorState` — collapse |
| QA-D010 | QA-D010-014 | MEDIUM | `debug_assertions` ABI drift between debug/release builds (counter fields gated) |
| QA-D010 | QA-D010-016 | LOW | LIFO non-protected stalemate edge case |

**Prereqs:** none — all directly actionable. SPEC-19 amendment work for QA-D009-001 is the only item that must route through ESPECIALISTA EM SPECS first. Add to inventory: TASK-0620 (bench subcommand real-TCP path) and TASK-0621 (`cfg(debug_assertions)` audit) deferred from D-012 closure.

**Next action:** invoke **task-splitter** to break the inventory above into atomic tasks (`TASK-0619+`), then run the standard Stages 2→6.

**Test floor entering D-013:** **1798 default / 1842 zero-copy / 1789 streaming-no-recycle / 1740 release**. v1 floor: 690.

### D-010: SPEC-21 Streaming Generation — ✅ CLOSED 2026-04-30

All 6 SDD stages shipped. Test floor advanced **1464 → 1683 default / 1507 → 1726 zero-copy** (+219 / +219); new build profile `streaming-no-recycle` introduced at 1680. Phase F (production) commits `2f751a4..61e86a1` (9 waves) on top of Phase A spec amendments. Audit + Stage 6 REFACTOR commits (this bundle's closure):

- `f61dffb` QA-D010-001 — `streaming_active` flag separation + R37b disjunction gate
- `95a34f5` MF-001 — `merge::generate_and_partition_chunked_with_delta` wrapper wires `extend_with_chunk_borders` from production
- `6c383f1` MF-002 — `enter/exit_streaming_mode` helpers wired from worker FSM
- `7fca43e` QA-D010-002 — `RequestWork.worker_id` validated against authenticated connection identity
- `ec81eb3` QA-D010-003 — `default_chunked_iter` propagates FreePorts (T6 isomorphism preserved)
- `d4865c0` QA-D010-004 — `border_id_counter` skips reserved FreePort interface ids (cross-batch HashSet)
- `58d75cb` QA-D010-005..007 — strategy input validation: `try_new` constructors, `allocate_batch` bounds, FENNEL `f64::total_cmp` + non-finite alpha rejection
- `5a54111` QA-D010-008 + QA-D010-009 (partial) — orchestrator returns `Err` instead of panic; `generate_and_partition_chunked_with_chunk_size_and_lifetime` wired end-to-end
- `38f8bd8` SF-001..SF-004 polish — propagate `recycle_under_delta` to `net.recycle_policy`; `Debug/Clone` derives on streaming strategies; stale comment cleanup; UT-0590-07 assertion on `free_list_pops_border` semantics

**Audit artefacts:** `docs/reviews/REVIEW-PHASE-D010-spec21-streaming-2026-04-28.md` (verdict ACCEPT_WITH_FIXES; 2 MF + 4 SF) and `docs/qa/QA-PHASE-D010-spec21-streaming-2026-04-28.md` (4 CRITICAL + 5 HIGH + 7 MEDIUM/LOW).

**Deferred to D-011:** QA-D010-009 (residual GridConfig threading through legacy callers) + QA-D010-010..016 (MEDIUM/LOW). See D-011 inventory above.

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

---

### Strategic decision (2026-04-25, user directive)

**Implement Tier 2 + Tier 3 first; defer Tier 4 + Tier 5 decisions until after Tier 2+3 are shipped.**

| Tier | Status |
|------|--------|
| Tier 1 | ✅ DONE — frozen at `a431320` (D-005 Option A 12/12 G1 parity green) |
| **Tier 2** (Elastic Grid) | ✅ **D-006 CLOSED — Phase D Option A landed 2026-04-27 (commits df93908, 8dd6d1b, 7988573, fc680f5). Tag: v2.0-elastic-grid-detection-only** |
| **Tier 3** (Memory Efficiency) | ACTIVE — D-009 ✅ CLOSED 2026-04-27, D-010 ✅ CLOSED 2026-04-30. D-011 hardening NEXT (consolidates QA-D009-001 + QA-D010-009..016 deferrals). |
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
| **M5** Memory Efficiency | SPEC-21, SPEC-22 | 2.33 → 2.28 → 2.27 → 2.30 | IN PROGRESS — D-009 + D-010 CLOSED; D-011 hardening NEXT (D-012/D-013 still PLANNED) | `ep_con 100M` runs on 2GB coordinator |
| **M10** Encoder/Decoder API | SPEC-27 | 2.41 | DONE | LambdaEncoder end-to-end, registry with 5 codecs |
