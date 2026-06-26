# TEST-SPEC-T13: Short stream (fewer chunks than workers)

**SPEC-21 §7.5 ID:** T13.
**Owning task:** TASK-0514 + future TASK-0581 (out of scope wave 1).
**Parent spec:** SPEC-21 §3.6 R35; §7.5 T13.
**Type:** integration.
**Theory anchor:** ARG-001 P1 (confluence — result independent of worker count).

---

## Inputs / Fixtures

- `ep_annihilation(10)`, K=4, chunk_size=100. Total agents = 20 (`2 * 10`); `chunk_size = 100 > 20` so only 1 chunk total.

## Unit Tests

| ID | Test | Then |
|----|------|------|
| UT-T13-01 | Run with Pull dispatch | only 1 worker receives the single chunk; the other 3 receive `NoMoreWork` immediately. |
| UT-T13-02 | Result correctness | `is_behaviorally_equal(short_stream_result, reduce_all(make_net(ep_annihilation(10))))`. |
| UT-T13-03 | Verify which worker received the chunk | the deterministic-ordering rule (lowest WorkerId first to send RequestWork wins) → WorkerId(0) receives the chunk. |
| UT-T13-04 | Workers 1, 2, 3 receive `NoMoreWork` | each emits exactly one RequestWork, gets NoMoreWork, terminates. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | `total_agents = 0` | every worker gets NoMoreWork on first request; pipeline returns trivial empty result. |
| EC-2 | `total_agents = chunk_size` exactly | one chunk, K-1 workers idle. |

## Invariants asserted

- R35 (short-stream handling).
- P1 (confluence — result independent of worker count).

## Determinism notes

`#[tokio::test(flavor = "current_thread")]`. The test asserts deterministic worker assignment (UT-T13-03); this requires controlled message ordering.

## Cross-test dependencies

- **TEST-SPEC-T11** (pull baseline).
- **TEST-SPEC-0514** (FSM amendment EC-3 — deterministic ordering).
