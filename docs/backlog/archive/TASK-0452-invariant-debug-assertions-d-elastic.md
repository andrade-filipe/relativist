# TASK-0452: Invariant defense — debug assertions for D3-elastic, D4-elastic, D5, R31 atomic refresh

**Spec:** SPEC-20 §3.7 invariant preservation (R39); D3-elastic (R24c), D4-elastic (R11a), D5 (R31 atomic refresh), G1-elastic-departure (R39 sub-clauses, gated by ARG-006 v1 / ARG-006 delta-conservative; CONDITIONAL on ARG-005 for delta-optimized).
**Requirements:** R39 (T1-T7, D1-D6, I1-I5, G1 invariant preservation defense), R24c (D3-elastic), R24d (border_id rebase), R11a (D4-elastic), R31 (D5 atomic refresh).
**Priority:** P1 (defensive; catches regressions during development).
**Status:** TODO
**Depends on:** TASK-0439 (retained state), TASK-0440 (v1 reclaim), TASK-0443 (delta reclaim), TASK-0420 (WorkerId + partition_index).
**Blocked by:** TASK-0440.
**Estimated complexity:** S (~60-90 LoC production + ~60 LoC tests)
**Bundle:** SPEC-20 Elastic Grid — Phase 3 Invariant Defense.

## Context

SPEC-20 §3.7 claims PRESERVED for D1, D2, D3 (via D3-elastic R24c), D4 (via D4-elastic R11a), D5, D6, G1 (v1 and delta-conservative via ARG-006; delta-optimized CONDITIONAL on ARG-005). This task adds `debug_assert!`s at the critical code points so invariant violations panic in debug builds.

## Acceptance Criteria

- [ ] **D3-elastic defense (R24c)**: at the entry of `merge()`, `debug_assert!` that the input partition set is UNIFORM (all came from the same round's dispatch — no reclaimed partition slipped in). Mechanism: `Partition` carries a `provenance_round: u32` field OR the merge caller asserts collective provenance via a sentinel.
- [ ] **D4-elastic defense (R11a)**: at `compute_round_id_ranges`, `debug_assert!` that `K_eff == active.len() + (1 if hybrid)` and that every consumed index is in `[0, K_eff)` (dense).
- [ ] **D5 atomic refresh (R31)**: at the `retained_last_acked[w]_round_n → round_n+1` replacement point, `debug_assert!` that the new dispatch has been fully transmitted to all surviving members of `W_active` BEFORE the release.
- [ ] **Border_id rebase (R24d)**: at `Net::union` of reclaimed + survivors, `debug_assert!` no border_id range overlap between the two sides.
- [ ] Release builds strip assertions (standard Rust `debug_assert!` behavior).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/merge/mod.rs` | modify | `merge()` input provenance assertion. |
| `relativist-core/src/partition/id_ranges.rs` | modify | `compute_round_id_ranges` assertion. |
| `relativist-core/src/protocol/retained.rs` | modify | Atomic refresh assertion. |
| `relativist-core/src/partition/departure_recovery.rs` | modify | border_id disjointness assertion. |

## Test Expectations (forward-ref)

- Property test EG-P3 `prop_id_ranges_disjoint_after_repartition` exercises D4-elastic.
- EG-U13 exercises R31 atomic refresh.
- EG-U7a exercises R24d border_id rebase.

## Invariants Touched

- D3, D4, D5 — all directly defended.

## Notes

- **Debug-only assertions**: in release builds these compile to nothing; zero runtime cost for production.

## DAG Links

- **Predecessors:** TASK-0439, TASK-0440, TASK-0443, TASK-0420.
- **Successors:** EG-P3, EG-U7a, EG-U13.
