# TEST-SPEC-T7: End-to-end reduction equivalence

**SPEC-21 §7.2 ID:** T7.
**Owning task:** TASK-0542 + TASK-0554 + run_grid integration.
**Parent spec:** SPEC-21 §7.2 T7; SPEC-01 T7 (interaction count parity); ARG-001 G1.
**Type:** integration.
**Theory anchor:** ARG-001 G1 (full-cycle equivalence).

---

## Inputs / Fixtures

- `dual_tree(8)` with both pipelines (sequential vs streaming).
- `ep_annihilation_pure(50)` with both pipelines.
- The interaction-counter instrumentation (SPEC-01 T7).

## Unit Tests

| ID | Test | Then |
|----|------|------|
| UT-T7-01 | (a) `reduce_all(make_net(dual_tree(8)))` (sequential baseline); (b) `run_grid` with streaming pipeline on the same generator | `is_behaviorally_equal(result_a, result_b) == true`. |
| UT-T7-02 | Compare interaction counts | `interactions_a == interactions_b` (SPEC-01 T7: count parity). |
| UT-T7-03 | Same comparison for `ep_annihilation_pure(50)` | both equalities hold. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | A net with no redexes (already in normal form) | both paths return immediately; 0 interactions; results equal. |
| EC-2 | A net with all redexes ERA-ERA (annihilations only) | per-rule counts equal; total count equal. |

## Invariants asserted

- G1 (full-cycle equivalence).
- SPEC-01 T7 (interaction count parity).

## Determinism notes

`run_grid` requires a controlled tokio runtime: `#[tokio::test(flavor = "current_thread")]`. The BSP barrier is single-threaded by construction.

## Cross-test dependencies

- **TEST-SPEC-0542** (dual_tree streaming override) — UT-0542-06 IS this T-test for dual_tree.
- **TEST-SPEC-T6** (streaming vs batch isomorphism) — partition+merge level; T7 extends to full reduction.
- **TEST-SPEC-0491** (is_behaviorally_equal).
