# TASK-0424: Strict-BSP uniformity for self-partition (R3c)

**Spec:** SPEC-20 §3.1 R3c (closes SC-002 sub-issue 4). Interaction with SPEC-05 R30a / SPEC-19 R40 strict_bsp.
**Requirements:** R3c.
**Priority:** P1 (semantics-critical under strict_bsp; unavoidable for correctness but non-blocking for most default runs).
**Status:** TODO
**Depends on:** TASK-0423 (self-worker exists), TASK-0436 (FSM transition table for merge path).
**Blocked by:** TASK-0423.
**Estimated complexity:** S (~30-50 LoC production + ~40 LoC tests)
**Bundle:** SPEC-20 Elastic Grid — Phase 2.1 Hybrid Coordinator.

## Context

Under `strict_bsp = true`, the post-merge / post-border-resolve `reduce_all` (v1) or in-round border resolution (delta R13) is skipped — border-derived redexes are deferred to the next round (SPEC-05 R30a; SPEC-19 R40). R3c mandates that the self-partition follows the *same* short-circuit rule: its border-origin redexes are NOT short-circuited but deferred identically to remote partitions. This uniformity follows by construction because the self-partition flows through the same merge / border-graph code path — no special branch exists.

This task is a *verification* task more than a coding task: it ensures no accidental short-circuit is introduced during hybrid-mode implementation.

## Acceptance Criteria

- [ ] Code review checklist added to `relativist-core/src/merge/`: any `hybrid_coordinator` branch in the merge / border-resolve code path is a red flag. The uniformity claim depends on NOT special-casing the self-partition.
- [ ] Add debug assertion: in `strict_bsp = true` hybrid mode, assert that the self-partition's border-origin redexes are in the next-round queue, not reduced in-round.
- [ ] Under `strict_bsp = false`, the default merge path's lenient post-merge `reduce_all` applies to the self-partition identically to any remote partition.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/merge/mod.rs` | modify | Add debug assertion at the strict_bsp branch; no functional change. |

## Test Expectations (forward-ref)

- EG-U17 `test_strict_bsp_self_partition_uniformity` (R3c).

## Invariants Touched

- D2, D3 — preserved by construction (no special-case branch for self).

## Notes

- **Developer intent**: if during hybrid implementation someone is tempted to write `if is_self_partition { … }` in the merge or border-resolve code, STOP and re-read R3c. Uniformity is the entire correctness argument.

## DAG Links

- **Predecessors:** TASK-0423.
- **Successors:** EG-U17 test generation.
