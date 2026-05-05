# TASK-0477: `count_live_agents` MUST NOT count free-list entries

**Spec:** SPEC-22 §3.1 R11.
**Requirements:** R11 (`count_live_agents` does not change semantics; free-list does not affect the count).
**Priority:** P1 (defensive; current `count_live_agents` already iterates `agents.iter().flatten().count()` per SPEC-02 R16a, which naturally excludes `None` slots — but this task confirms and tests).
**Status:** TODO
**Depends on:** TASK-0473 (remove_agent populates free-list).
**Blocked by:** none
**Estimated complexity:** S (~10 LoC production audit + ~30 LoC tests)
**Bundle:** SPEC-22 Arena Management — Phase B (free-list core implementation).

## Context

R11 mandates: "`count_live_agents()` MUST NOT count free-list entries as live agents. The free-list does not change the semantics of `count_live_agents`; it only affects which `None` slots are available for reuse." The existing implementation (SPEC-02 R16a / TASK-0231) iterates `self.agents.iter().flatten().count()` — `flatten()` skips `None` slots automatically. Free-list entries correspond to `None` slots, so they are naturally excluded.

This task is a **confirmation + test surface** task: audit the existing implementation to ensure no future refactor breaks R11, and add explicit tests.

## Acceptance Criteria

- [ ] Audit `Net::count_live_agents` in `relativist-core/src/net/core.rs` and confirm it iterates `agents.iter().flatten()` (or equivalent that skips `None`). No code change unless the audit finds a defect.
- [ ] Add a comment citing SPEC-22 R11 above the function body.
- [ ] Test: build net, create 10 agents, remove 5, assert `count_live_agents() == 5` AND `free_list.len() == 5`. The free-list count and live count are independent.
- [ ] Test: build net, create 10 agents, remove all 10, assert `count_live_agents() == 0` AND `free_list.len() == 10`.
- [ ] Test (regression): all 1181 default tests continue to pass (count semantics preserved).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/net/core.rs` | modify (comment-only) | Add SPEC-22 R11 citation comment above `count_live_agents`. |

## Test Expectations (forward-ref)

TEST-SPEC-0477:
- `count_live_excludes_free_list_entries` (10 alive, 5 removed → 5 live, 5 free).
- `count_live_zero_after_full_removal_with_free_list` (all 10 → 0 live, 10 free).

## Invariants Touched

- R11 (consumed; defensive confirmation).
- Compatibility with TASK-0231 (count_live_agents implementation).

## Notes

- This task is mostly an audit + test-coverage task. Real semantics-change tasks are TASK-0472 / TASK-0473.

## DAG Links

- **Predecessors:** TASK-0473.
- **Successors:** TASK-0500 (regression gate).
