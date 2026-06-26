# TASK-0611 — SPEC-22 R22+R30 amendment: effective_arena_size threshold metric (D-011 BLOCKER 2026-05-04)

**Phase:** D-011 BLOCKER 2026-05-04 — Stage 1 (SPEC amendment)
**Bundle:** D-011 — Tier 3 Hardening + Bench Enablement
**Status:** CLOSED (paperwork only — landed 2026-05-04 in commits `e941273`, `972ce47`, `5cca7e6`)
**Priority:** P0 (BLOCKER prerequisite — unblocks TASK-0612 implementation)
**Spec:** SPEC-22 v2.4 §3.4 R22, R30, +R22a (new). Cross-reference checks against SPEC-04 §A4 R10a and SPEC-19 §3.6 R41 performed by especialista-specs (no normative drift found).
**Origin:** D-011 BLOCKER 2026-05-04 — partition perf regression isolated to commit `d6411be` (D-009 Phase C Wave 1+2). Root cause: `id_range_size > 4 × live_count` metric routed every healthy workload through SPARSE branch (+83% wall on `ep_con 5M w=2`).
**Estimated complexity:** N/A (already shipped — paperwork closure only)

---

## Context

This task records the SPEC-22 amendment that already landed and unblocks the implementation tasks (TASK-0612, TASK-0613, TASK-0614). The original SPEC-22 R22 / R30 wording used `id_range > 4 × live_agent_count` as the threshold to route partition construction between dense and sparse paths. This metric measured the PLANNING range from `compute_id_ranges` (which yields chunks of `max(100_000, base_next_id × 10)` per worker), and was decoupled from the actual `Vec<Option<Agent>>` size that dense `build_subnet` allocates (`max_live_id + 1`, see `partition/helpers.rs:301-303`).

For 5M-agent workloads with `w=2`, `compute_id_ranges` yields per-worker chunks of ~100M IDs, so `id_range_size / live_count ≈ 20`. The threshold `> 4` was tripped on 100% of healthy partitions, routing them through the SPARSE (HashMap-based) path which is empirically 5–7× slower than the DENSE path. Bisect transcript: `docs/next-steps.md` BLOCKER 2026-05-04 (lines 79–155 historical, 9–35 current).

The amended spec replaces the metric with `effective_arena_size := max(live_agents.iter().map(|a| a.id).max().unwrap_or(0) as u64 + 1, 1)`, which exactly matches what dense `build_subnet` would actually allocate. The `4 ×` constant is preserved (no calibration drift); only the metric the constant multiplies is corrected. M5 pathology (recycled-id fragmentation under delta mode, the original motivation of R22/R30) is still detected, since it manifests as `max_live_id ≫ live_count` and trips the same constant — captured normatively in new R22a.

## Dependencies

- None upstream. This task IS the upstream dependency for TASK-0612.

## Files in scope

| File | Change |
|------|--------|
| `specs/SPEC-22-arena-management.md` | R22 metric formula amended; R30 error-payload field rename specified; R22a added (M5 pathology still caught under new metric); §3.4 worked example re-derived. Status field updated to `Reviewed v2.4`. |
| `specs/SPEC-04-partitioning.md` | Cross-reference grep performed; no normative changes required (SPEC-04 §A4 R10a refers to the metric symbolically without inlining the formula). |
| `specs/SPEC-19-*.md` | Cross-reference grep performed; no normative changes required. |
| `docs/spec-reviews/SPEC-22-amendment-2026-05-04-d011-blocker.md` | Closure log written by especialista-specs after Round 2 (commit `5cca7e6`). |

## Files explicitly OUT of scope

- `relativist-core/src/error.rs` — error variant rename is part of TASK-0612 (implementation atom).
- `relativist-core/src/partition/helpers.rs` — body rewrite is part of TASK-0612.
- `compute_id_ranges` chunk_size formula — explicitly rejected as Option A in plan §2 (separate concern; would mask the bug rather than fix it).
- Tests in `docs/tests/` (TEST-SPEC-T16, TEST-SPEC-0484, TEST-SPEC-0492, TEST-SPEC-0552) — historical record, drift by design.

## Acceptance criteria

1. SPEC-22 status field reads `Reviewed v2.4` (verified — commit `5cca7e6`).
2. SPEC-22 R22 normative text uses `effective_arena_size > 4 × live_agent_count` (no remaining occurrences of `id_range > 4 × live_agent_count` in normative text).
3. SPEC-22 R22a is present and references Phase D-009 SC-009 closure rationale.
4. Closure log file `docs/spec-reviews/SPEC-22-amendment-2026-05-04-d011-blocker.md` exists.
5. spec-critic Round 1 verdict on file (commit `972ce47`) and Round 2 closure log on file (commit `5cca7e6`).

## Test floor delta

**0** — spec-only amendment. Test-floor changes are deferred to TASK-0612 (net 0) and TASK-0613 (+1 default).

## Notes

- This task entry is **paperwork hygiene** so the BACKLOG reflects the full Stage-1 → Stage-6 chain for the BLOCKER. The actual amendment work was performed by `especialista-specs` from the TCC root session per project policy (memory: ESPECIALISTA EM SPECS dispatch policy).
- Authoritative plan: `docs/plans/2026-05-04-d011-partition-perf-fix.md` §3 (Handoff Brief).
- Active BLOCKER block: `docs/next-steps.md` lines 9–35 (Stage 1 closed banner).
- Historical BLOCKER detail (Stage 0 root-cause investigation): `docs/next-steps.md` lines 37–155.
- Round 2 review attack surfaces resolved by especialista-specs:
  - Determinism (T7) — `worker_agents.iter().max()` is order-independent on `&[u32]`; T7 holds.
  - Empty-partition boundary — `unwrap_or(0)` and `effective_arena_size = 1` is the safe convention; spelled out normatively.
  - Wire compatibility — none; the metric is a build-time decision, not a wire-format field.
- Downstream chain: TASK-0612 (implementation) and TASK-0613 (regression witness, TDD-RED) consume this spec; TASK-0614 (bench verification) closes the BLOCKER.
