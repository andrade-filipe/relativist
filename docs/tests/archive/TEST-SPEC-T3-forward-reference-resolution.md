# TEST-SPEC-T3: Forward reference resolution

**SPEC-21 §7.1 ID:** T3.
**Owning task:** TASK-0542 + TASK-0553 + TASK-0554.
**Parent spec:** SPEC-21 §3.2 R14; §4.7 forward-reference resolution; §7.1 T3.
**Type:** integration.
**Theory anchor:** ARG-002 Q3 (bidirectional FreePort).

---

## Inputs / Fixtures

- Batch 1: contains a `Pending { from: (1, 0), target_agent: AgentId(50), target_port: PortId(0) }` directive.
- Batch 2: contains agent 50 (resolves the pending directive).

## Unit Tests

| ID | Test | Then |
|----|------|------|
| UT-T3-01 | Generate a batch with a Pending connection to agent ID 50; record `pending_store` size | `>= 1` after batch 1 processed. |
| UT-T3-02 | Generate a second batch containing agent 50; record `pending_store` size after | `== 0` (the entry was resolved). |
| UT-T3-03 | Verify the wire (internal or border) is correctly installed in the appropriate accumulator(s) | post-finalize, the wire appears: as an internal wire if both endpoints landed in the same worker, OR as a border wire (with allocated bid) otherwise. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | A Pending directive resolved within the same batch (target appears later in the same batch's agents) | resolved at end-of-batch; pending_store never grows. |
| EC-2 | Multiple Pending directives to the same target agent | all resolved when target arrives; install_connection called once per directive. |

## Invariants asserted

- R14 (forward references via Pending).
- C2 (Complete Wire Coverage) preserved.

## Determinism notes

Use `#[tokio::test(flavor = "current_thread")]` if any async path is exercised by the orchestrator.

## Cross-test dependencies

- **TEST-SPEC-0542** (dual_tree streaming override) — generates real Pending directives.
- **TEST-SPEC-0553** (install_connection helper) — handles classification.
- **TEST-SPEC-0554** (orchestrator) — coordinates resolution.
