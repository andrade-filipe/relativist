# TEST-SPEC-T2: AgentBatch construction (monotone IDs + directive classification)

**SPEC-21 §7.1 ID:** T2.
**Owning task:** TASK-0521 + TASK-0520.
**Parent spec:** SPEC-21 §4.1 AgentBatch; R14 (Pending support); §7.1 T2.
**Type:** unit.
**Theory anchor:** None direct.

---

## Inputs / Fixtures

- A 3-agent batch, base_agent_id=0; one Resolved directive, one Pending directive.
- A second batch base_agent_id=3 with 2 agents.

## Unit Tests

| ID | Test | Then |
|----|------|------|
| UT-T2-01 | Create batches with known agents and connections | constructs successfully; field-level inspection confirms inputs preserved. |
| UT-T2-02 | Agent IDs are monotonically increasing across batches | for batches B_k, B_{k+1}: `max(B_k.ids) < min(B_{k+1}.ids)`. |
| UT-T2-03 | Connection directives correctly classified | exactly N Resolved, exactly M Pending (matching constructor inputs). |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Empty batch in middle of stream | monotonicity skipped (None min/max); next batch's min still valid. |
| EC-2 | Batch with only Pending directives | UT-T2-03 counts: 0 Resolved, K Pending. |

## Invariants asserted

- §4.1 AgentBatch derive set.
- R14 (Pending entries).

## Determinism notes

Pure synchronous, no tokio.

## Cross-test dependencies

- Spec-catalog mirror of **TEST-SPEC-0521** (full unit suite) and **TEST-SPEC-0520** (ConnectionDirective enum). Use those for implementation.
