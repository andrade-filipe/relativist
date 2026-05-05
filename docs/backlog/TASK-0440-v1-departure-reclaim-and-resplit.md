# TASK-0440: v1-mode departure reclaim + deferred re-`split` (R24a/b-v1, R24c, R24d, R25, R26, R30)

**Spec:** SPEC-20 §3.3.4 R24a (catastrophic departure, retained_initial), R24b (after successful round, retained_last_acked), R24c (D3-elastic: no in-round mixed-merge), R24d (border_id rebase via A3), R25 (re-partition for K_eff_new), R26 (multiple simultaneous departures), R30 (ID uniqueness via `remap_partition_ids`).
**Requirements:** R24a-v1, R24b-v1, R24c, R24d, R25, R26, R30.
**Priority:** P0 (v1 departure correctness).
**Status:** TODO
**Depends on:** TASK-0410 (`Net::union`), TASK-0411 (`remap_partition_ids`, `allocate_border_ids`), TASK-0439 (retained-state), TASK-0438 (detection), TASK-0436 (FSM).
**Blocked by:** TASK-0411, TASK-0439.
**Estimated complexity:** M (~180-250 LoC production + ~150 LoC tests) — *if exceeds 200 LoC, split into 0440a (single-departure) / 0440b (multi-departure R26)*.
**Bundle:** SPEC-20 Elastic Grid — Phase 2.3 Dynamic Departure.

## Context

v1 reclaim: the coordinator does NOT `merge()` a mixed set within this round (D3-elastic R24c). Instead it merges the K_eff_old - D survivor results normally, then in the join window performs: (i) `remap_partition_ids` (A4) on each reclaimed partition to a fresh `IdRange` (disjoint from survivors); (ii) `Net::union` (A7) to concatenate merged_net ∪ reclaimed_partitions; (iii) `allocate_border_ids` (A3) for fresh border_id ranges; (iv) `split(unioned_net, K_eff_new)`. Multiple simultaneous departures (R26) are handled in ONE cycle.

## Acceptance Criteria

- [ ] `WaitingForResults × PhaseTimeout/ConnLost/Urgent` → action `ReclaimPartition(w, auto)` which:
  - Selects `retained_last_acked[w]` if present (R24b), else `retained_initial[w]` (R24a) (R23d priority).
  - Emits `RemoveWorker(w)`, `LogDeparture`.
- [ ] Survivor results merged normally via `merge()` (K_eff_old - D partitions).
- [ ] Transition to `AcceptingMembershipChanges` (doubles as departure-resolution window).
- [ ] On `MembershipWindowClosed`:
  - For each reclaimed partition: `remap_partition_ids(p, new_range)` using fresh IdRange from `compute_id_ranges(K_eff_new)`.
  - `allocate_border_ids(count)` for the reclaimed partition's fresh border_ids (R24d); discard its old border_ids; rebuild `free_port_index`.
  - `merged_net = merge_output.union(reclaimed_1).union(reclaimed_2)...` (Net::union, A7).
  - `partitions = split(merged_net, K_eff_new)` where `K_eff_new = K_eff_old - D`.
  - Dispatch via `AssignPartition`.
- [ ] R26: multiple departures in the same window → collectively handled in ONE re-`split` cycle. No sequential repartitions.
- [ ] At-least-once semantics: some reductions may be performed twice (R29); correctness preserved by ARG-006 (CLOSED for v1 per R29a).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/protocol/coordinator.rs` | modify | `ReclaimPartition` action handler; deferred re-split logic. |
| `relativist-core/src/partition/departure_recovery.rs` *(new)* | create | Re-split + remap + union + allocate_border_ids orchestrator. |

## Test Expectations (forward-ref)

- EG-U7 `test_departure_reclaim_initial` (R23a-b, R24a).
- EG-U7a `test_departure_reclaim_border_id_rebase` (R24d, SC-021).
- EG-U7b `test_departure_reclaim_last_acked_v1` (R23c-v1, R24b).
- EG-U8 `test_departure_multiple_workers_v1` (R26).
- EG-U10a `test_graceful_leave_urgent_v1` (R22b, SC-008).
- EG-U12 `test_id_ranges_no_collision_after_repartition` (R13, R30, R11a).
- EG-I3 `test_elastic_departure_correctness_v1` (ARG-006 empirical).
- EG-I5a `test_condup_cascades_with_retained_redispatch` (R24c, SC-005, SC-015).
- EG-I5b `test_emergent_borders_across_retained_evolved` (R24d, SC-021).

## Invariants Touched

- D3 (Border Completeness) — PRESERVED via D3-elastic (R24c + R24d).
- D4 (ID Uniqueness) — PRESERVED via `remap_partition_ids` + disjoint `compute_id_ranges`.
- D5 (Exclusive Ownership) — PRESERVED: reclaimed snapshots never live concurrently with live copies.
- G1 — PRESERVED for v1 via ARG-006 (R29a).

## Notes

- **Splitting guidance**: if production diff exceeds 200 LoC, split into:
  - `TASK-0440a` — single-departure path (R24a, R24b, R24d).
  - `TASK-0440b` — multiple-departure orchestration (R26).
- **At-least-once**: documented in R29 and defensively via ARG-006 P12 (mixed-trace recoverability).

## DAG Links

- **Predecessors:** TASK-0410, TASK-0411, TASK-0439, TASK-0438.
- **Successors:** TASK-0442 (R26a edge case D==K_eff), TASK-0443 (delta departure parallel), EG-I3, EG-I5a, EG-I5b.
