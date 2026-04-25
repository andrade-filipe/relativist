# TEST-SPEC-T9: Strategy independence (property test)

**SPEC-21 §7.3 ID:** T9.
**Owning task:** TASK-0531 + TASK-0530 + TASK-0554.
**Parent spec:** SPEC-21 §7.3 T9.
**Type:** property.
**Theory anchor:** ARG-002 (partition quality affects performance only).

---

## Inputs / Fixtures

- Both strategies: `RoundRobinStreamingStrategy` and `FennelStreamingStrategy { alpha: 1.0 }`.
- A common benchmark and chunk_size.

## Property tests

| ID | Property | Generator | Assertion |
|----|----------|-----------|-----------|
| PT-T9-01 | Result is invariant under strategy | proptest: fixed `(benchmark, size, chunk_size, num_workers)`, varying strategy | `is_behaviorally_equal(round_robin_result, fennel_result) == true` AND both equal sequential baseline. |
| PT-T9-02 | Interaction count is invariant under strategy | same generator | counts equal across both strategies. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | A topology where FENNEL produces dramatically different partitions vs RoundRobin | correctness still holds; only performance differs. |
| EC-2 | FENNEL not implemented (Q3 calibration shows alpha=1.0 inadequate) | this T-test reduces to RoundRobin-only; PT-T9-01 trivializes; the property test runner MUST detect FENNEL-feature-gating and skip gracefully. |

## Invariants asserted

- Partition quality affects performance but never correctness.

## Determinism notes

Proptest with stable seed. RoundRobin is deterministic by construction (TEST-SPEC-0530 PT); FENNEL requires deterministic tiebreak (TEST-SPEC-0531 UT-0531-05).

## Cross-test dependencies

- **TEST-SPEC-0530** (RoundRobin) and **TEST-SPEC-0531** (FENNEL) — strategy implementations.
- **TEST-SPEC-0554** (orchestrator) — invokes the strategies.
- **TEST-SPEC-T8** (chunk size independence) — sibling; T9 fixes chunk_size and varies strategy.
