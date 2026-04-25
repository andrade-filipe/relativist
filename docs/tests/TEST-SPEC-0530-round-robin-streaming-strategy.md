# TEST-SPEC-0530: RoundRobinStreamingStrategy (MVP default)

**SPEC-21 §7 ID:** T1 (assignment correctness).
**Owning task:** TASK-0530.
**Parent spec:** SPEC-21 §3.1 R4, R7, R8, R9; §4.3 RoundRobin design.
**Type:** unit + property.
**Theory anchor:** ARG-002 Q2 (σ allocation function — round-robin is the simplest deterministic σ).

---

## Inputs / Fixtures

- 100 agents in 5 batches of 20; `num_workers = 4`.
- Synthetic `AgentBatch` instances with monotone IDs 0..99, all agents `Agent::new(CON)`, no connection_directives.
- A fresh `RoundRobinStreamingStrategy::new()`.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0530-01 | `assignment_round_robin_order` | 100 agents in 5 batches of 20, `num_workers = 4` | iterate batches; collect (agent_id, worker_id) pairs | for every (id, worker), `worker == (id % 4)`. |
| UT-0530-02 | `each_agent_assigned_exactly_once` | the same 100-agent run | union of all returned `Vec<WorkerId>` | `assigned_count == 100`; no duplicate (agent_id, worker_id) pairs. |
| UT-0530-03 | `finalize_per_worker_counts_match` | post-run | call `finalize()` and inspect `per_worker_agent_counts` | `[25, 25, 25, 25]` (100 agents / 4 workers). |
| UT-0530-04 | `determinism_repeated_invocation` | a fresh strategy and the same 100-agent batch sequence | run TWICE | both runs produce byte-identical `Vec<WorkerId>` outputs (R8). |
| UT-0530-05 | `c1_complete_coverage_cross_batch` | the 5-batch run | union of all assignments | covers every agent_id in 0..99 with no omissions and no duplicates (R7). |
| UT-0530-06 | `single_batch_run` | 1 batch of 20 agents | run | same `i % num_workers` mapping; no state corruption when only one batch is processed. |
| UT-0530-07 | `single_worker_all_to_zero` | 100 agents, `num_workers = 1` | run | every agent assigned to `WorkerId(0)`; finalize returns `[100]`. |
| UT-0530-08 | `more_workers_than_agents` | 5 agents, `num_workers = 10` | run | agents assigned to `WorkerId(0..5)`; workers 5..9 receive 0 agents; `finalize().per_worker_agent_counts == [1, 1, 1, 1, 1, 0, 0, 0, 0, 0]`. |
| UT-0530-09 | `r9_pure_core_grep_gate` | the impl source file | grep for `tokio::`, `async fn`, `.await` | NONE. |

## Property tests

| ID | Property | Generator | Assertion |
|----|----------|-----------|-----------|
| PT-0530-01 | C1 holds for any batch sequence | proptest: random batches with monotone IDs (1..1000 agents total), `num_workers ∈ 1..16` | union covers every emitted agent_id exactly once. |
| PT-0530-02 | Determinism holds for any input | same generator | running twice produces identical `Vec<WorkerId>`. |
| PT-0530-03 | Per-worker counts are within 1 of each other | same generator | `max(counts) - min(counts) <= 1` (round-robin balance property; same as ContiguousIdStrategy SPEC-04 R22). |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Empty batch (`agents.len() == 0`) | `allocate_batch` returns empty Vec; counter unchanged; `finalize` returns whatever was accumulated previously. |
| EC-2 | `num_workers = 0` | configuration error (caller's responsibility); the strategy MAY panic or return error per task acceptance. Test asserts the documented behavior. |
| EC-3 | A batch with `agents.len() = u32::MAX` (synthetic stress) | counter arithmetic MUST NOT overflow; the strategy uses `u64` internally OR documents the overflow as caller's responsibility. |

## Invariants asserted

- R4 (RoundRobin assignment formula).
- R7 (C1 cross-batch coverage).
- R8 (determinism).
- R9 (pure Core).
- C1 (Complete Agent Coverage) — preserved by counter-monotonic dispatch.
- T1 (RoundRobinStreamingStrategy assignment correctness — this TEST-SPEC IS T1).

## ARG/DISC/REF citation

- ARG-002 Q2 (σ allocation function).

## Determinism notes

Pure synchronous, no tokio, no RNG. Property tests use a seeded `proptest::test_runner::TestRunner` (default seed acceptable; seed MUST be stable for reproducibility).

## Cross-test dependencies

- TEST-SPEC-0524 (trait surface) — prerequisite (compile gate).
- TEST-SPEC-0531 (FENNEL) — sibling strategy; T9 strategy-independence test compares both.
- TEST-SPEC-T5 (streaming pipeline produces valid partitions) — uses RoundRobin as the default strategy.
- TEST-SPEC-T8 (chunk size independence) — uses RoundRobin.
