# TASK-0450: `GridMetrics` elastic fields + `WorkerRoundStats::is_coordinator_self` (R38, R38a, R38b)

**Spec:** SPEC-20 §3.6 R38 (7 new metric fields), R38a (additivity with SPEC-19 metrics, closes SC-019/NF-004 audit committed to record), R38b (is_coordinator_self field, closes SC-027).
**Requirements:** R38, R38a, R38b.
**Priority:** P1 (observability + ROADMAP break-even analysis prerequisite).
**Status:** TODO
**Depends on:** TASK-0420 (WorkerRoundStats extension).
**Blocked by:** none
**Estimated complexity:** S (~70-100 LoC production + ~60 LoC tests)
**Bundle:** SPEC-20 Elastic Grid — Phase 3 Observability.

## Context

R38 extends `GridMetrics` with 7 new fields: `workers_joined_per_round`, `workers_departed_per_round`, `effective_slots_per_round`, `partitions_redispatched_per_round`, `retained_initial_reclaims_per_round`, `retained_last_acked_reclaims_per_round`, `join_round_overhead_ms_per_round`. R38a documents the non-collision audit with SPEC-19's 8 R45 fields (prefix-disjoint: `workers_`, `effective_`, `partitions_`, `retained_`, `join_round_` vs SPEC-19's prefixes). R38b adds `is_coordinator_self: bool` on `WorkerRoundStats`.

## Acceptance Criteria

- [ ] Extend `GridMetrics` struct with 7 new fields (per R38 schema).
- [ ] Each field initialized to an empty `Vec` (growable per round).
- [ ] Populate fields at the correct lifecycle points:
  - `workers_joined_per_round` — at each `AcceptingMembershipChanges` exit.
  - `workers_departed_per_round` — at each `AcceptingMembershipChanges` exit.
  - `effective_slots_per_round` — at each `Partitioning` entry.
  - `partitions_redispatched_per_round` — at each re-split-on-departure.
  - `retained_initial_reclaims_per_round` — incremented on R24a reclaim.
  - `retained_last_acked_reclaims_per_round` — incremented on R24b reclaim.
  - `join_round_overhead_ms_per_round` — delta-mode rejoin round overhead.
- [ ] `WorkerRoundStats` has `is_coordinator_self: bool` (TASK-0420 already landed this; confirm).
- [ ] R38a audit is documented in a comment block with explicit field enumerations of SPEC-19 R45 and SPEC-20 R38, plus the by-prefix disjointness argument.
- [ ] Metrics serialize cleanly via serde + bincode.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/merge/metrics.rs` *(SPEC-05 GridMetrics site)* | modify | Add 7 fields; populate at lifecycle points. |

## Test Expectations (forward-ref)

- TEST-SPEC-0450: sentinel test asserts no field-name collision with SPEC-19 R45 (compile-time via `assert_fields_disjoint!` macro OR runtime enumeration).
- EG-B2 `bench_retention_memory_overhead` (R31, R23).
- EG-B3 `bench_join_round_overhead_delta` (R12a, R12-delta).

## Invariants Touched

- None.

## Notes

- **NF-004 closure**: the audit is committed to the written record per R38a. Any future field-name collision is a spec defect.

## DAG Links

- **Predecessors:** TASK-0420.
- **Successors:** EG-B1, EG-B2, EG-B3.
