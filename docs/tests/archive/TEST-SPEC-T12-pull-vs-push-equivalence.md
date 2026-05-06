# TEST-SPEC-T12: Pull vs push equivalence

**SPEC-21 §7.5 ID:** T12.
**Owning task:** TASK-0514 + TASK-0511.
**Parent spec:** SPEC-21 §3.6 R30-R32; §7.5 T12.
**Type:** integration.
**Theory anchor:** ARG-001 G1.

---

## Inputs / Fixtures

- `ep_annihilation_con(100)`, K=4, chunk_size=20.
- Both `DispatchMode::Push` and `DispatchMode::Pull` configurations.

## Unit Tests

| ID | Test | Then |
|----|------|------|
| UT-T12-01 | Run with Push; record merged result and interaction count | both stored. |
| UT-T12-02 | Run with Pull; record same | both stored. |
| UT-T12-03 | Compare results | `is_behaviorally_equal(push, pull) == true`. |
| UT-T12-04 | Compare interaction counts | `interactions_push == interactions_pull` (SPEC-01 T7). |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | `DispatchMode::Auto` | router picks one; result equivalent to whichever path was chosen. |

## Invariants asserted

- G1 (full-cycle equivalence under both dispatch modes).

## Determinism notes

`#[tokio::test(flavor = "current_thread")]`. Both runs use the same RNG seed (if any) and same fixture.

## Cross-test dependencies

- **TEST-SPEC-T11** (pull-protocol baseline).
- **TEST-SPEC-0514** (FSM amendment).
- **TEST-SPEC-0491** (is_behaviorally_equal).
