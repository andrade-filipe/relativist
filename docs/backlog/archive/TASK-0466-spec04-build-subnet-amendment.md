# TASK-0466: [SPEC-04 amendment A7] Extend §4.5 `build_subnet` — populate per-partition free-list + 4× sparse threshold

**Spec:** SPEC-22 §3.8 A7 (closes SC-003, SC-006, SC-009).
**Requirements:** A7 (formal SPEC-04 §4.5 amendment); enables SPEC-22 R10a, R22.
**Priority:** P0 (blocker for TASK-0481 build_subnet free-list population and TASK-0492 sparse-then-dense build_subnet).
**Status:** TODO
**Depends on:** TASK-0460 (I3'), TASK-0461 (R2 reuse), TASK-0462 (R10), TASK-0463 (R11), TASK-0464 (R12).
**Blocked by:** none
**Estimated complexity:** S (~30 LoC SPEC-04 next-revision diff; no production code)
**Bundle:** SPEC-22 Arena Management — predecessor-spec amendment cluster (Phase A).
**Tag:** `[SPEC-04 amendment]`

## Context

SPEC-04 §4.5 `build_subnet(net, worker_agents[i], sigma, border_entries[i])` produces a subnet for partition `i` using the dense `Net` representation; no free-list mention. The amendment requires:

1. `build_subnet` MUST populate the partition subnet's `free_list` with all `None` slots in `[partition.id_range.start, partition.id_range.end)` after live agents are placed (R10a — closes SC-006).
2. When the dense-arena threshold check fires (`id_range.end - id_range.start > 4 × live_agent_count`), `build_subnet` MUST use `SparseNet` internally and call `to_dense(Some(partition.id_range.clone()))` before returning (R22 — closes SC-009 M5 pathology at 100M agents).
3. The exposed signature MAY remain `Net` to preserve API stability; the sparse path is an implementation detail.

## Acceptance Criteria

- [ ] ESPECIALISTA EM SPECS lands the SPEC-04 next-revision diff amending §4.5 with the SPEC-22 §3.8 A7 *New text* verbatim.
- [ ] §4.5 explicitly states the per-partition free-list population requirement (R10a).
- [ ] §4.5 explicitly states the 4× threshold rule and its sparse-build fallback (R22).
- [ ] §4.5 cross-references SPEC-22 R10a, R22, R30 (sparse_build flag).
- [ ] §4.5 notes the `PartitionError::DenseAllocationExceedsThreshold` rejection at threshold when `sparse_build=false` is forced (R30).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `codigo/relativist/specs/SPEC-04-partitioning.md` | modify (by ESPECIALISTA EM SPECS only) | §4.5 `build_subnet` amended per SPEC-22 §3.8 A7. |

## Test Expectations (forward-ref)

TEST-SPEC-0466 — covered by:
- T9 (per-partition ID range compliance — TASK-0481).
- T14a (partition-scoped to_dense — TASK-0490).
- T16 (sparse build_subnet — TASK-0492).

## Invariants Touched

- D4 (ID Uniqueness After Distributed Reduction — partition free-list confined to `[id_range.start, id_range.end)`).
- M5-scale memory feasibility (closes the M5 800 MB pathology).

## Notes

- The exposed `Net` return type is preserved for API stability; sparse path is an internal implementation detail.
- This amendment is consumed by both TASK-0481 (R10a free-list population) and TASK-0492 (R22 sparse build).
- A4 (R11 clarification) is a prerequisite because A7's sparse path produces a `SparseNet` that is then converted to `Net` via `to_dense(Some(range))`; the resulting `Net` must satisfy the new R11 contract.

## DAG Links

- **Predecessors:** TASK-0460, TASK-0461, TASK-0462, TASK-0463, TASK-0464.
- **Successors:** TASK-0481 (build_subnet free-list per range), TASK-0492 (sparse-then-dense build_subnet at threshold).
