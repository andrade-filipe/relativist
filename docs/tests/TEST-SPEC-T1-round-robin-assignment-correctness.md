# TEST-SPEC-T1: RoundRobinStreamingStrategy assignment correctness

**SPEC-21 §7.1 ID:** T1.
**Owning task:** TASK-0530.
**Parent spec:** SPEC-21 §3.1 R4, R7, R8; §7.1 T1.
**Type:** unit + property.
**Theory anchor:** ARG-002 Q2.

---

## Inputs / Fixtures

- 100 synthetic agents in 5 batches of 20 (monotone IDs 0..99); `num_workers = 4`.
- Empty connection_directives (T1 only tests assignment, not connection installation).

## Unit Tests

| ID | Test | Then |
|----|------|------|
| UT-T1-01 | Generate 100 agents in 5 batches of 20; verify each agent assigned to exactly one worker | `assigned.len() == 100`; no duplicates. |
| UT-T1-02 | Verify round-robin order: agent `n` → worker `n % 4` | for every (id, worker) pair: `worker == id % 4`. |
| UT-T1-03 | `finalize().per_worker_agent_counts == [25, 25, 25, 25]` | exact match. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | 100 agents, num_workers = 3 | counts `[34, 33, 33]` (round-robin remainder distribution). |
| EC-2 | 100 agents, num_workers = 1 | counts `[100]`. |
| EC-3 | 0 agents | counts `[0, 0, 0, 0]`. |

## Invariants asserted

- R4, R7, R8.
- C1 (Complete Agent Coverage).

## Determinism notes

Pure synchronous, no tokio, no RNG.

## Cross-test dependencies

- This T-test is the spec-catalog mirror of **TEST-SPEC-0530** (which contains the full unit + property test suite). Use 0530 as the implementation source; T1 is a documentation anchor in SPEC-21 §7.1.
