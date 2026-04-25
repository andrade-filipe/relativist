# TEST-SPEC-T4: Empty pending store assertion (R19 negative path)

**SPEC-21 §7.1 ID:** T4.
**Owning task:** TASK-0554.
**Parent spec:** SPEC-21 §3.3 R19; §7.1 T4.
**Type:** unit.
**Theory anchor:** ARG-002 Q5/C2 (complete wire coverage).

---

## Inputs / Fixtures

- A `MalformedGenerator` that emits a batch with `Pending { target_agent: AgentId(999) }` and never produces agent 999.

## Unit Tests

| ID | Test | Then |
|----|------|------|
| UT-T4-01 | Run pipeline with the malformed generator | `Err(PipelineError::UnresolvedPending { agent_id: 999, .. })` (R19 violation detected). |
| UT-T4-02 | Error message identifies the orphan agent_id | message contains "999" or equivalent diagnostic anchor. |
| UT-T4-03 | Verify the pipeline does NOT silently produce a partial result | function MUST return Err, not Ok with missing wires. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Multiple unresolved Pending entries | error reports the first one OR aggregates (impl choice; test asserts the documented behavior). |
| EC-2 | All Pending eventually resolved (control case) | no error; pipeline completes successfully. |

## Invariants asserted

- R19 (empty-pending-store post-stream).
- C2 (the negative path: detection of incomplete wire coverage).

## Determinism notes

Pure synchronous. The malformed generator is deterministic (fixed-seed).

## Cross-test dependencies

- **TEST-SPEC-0554** (orchestrator) — UT-0554-04 implements this T-test.
- **TEST-SPEC-T3** (positive path: forward references resolved) — sibling.
