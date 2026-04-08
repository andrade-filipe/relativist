# TEST-SPEC-0070: Implement run_grid Phase 1 (split) and Phase 2 (local reduction)

**Task:** TASK-0070
**Spec:** SPEC-05 (R24, R25, R31, R32)
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Phase 1 calls split and records partition_time
Create a `Net` with 4 CON agents forming 2 active pairs. Call `run_grid` with `num_workers: 2`, `max_rounds: Some(1)`. Assert `metrics.partition_time_per_round.len() == 1` and `metrics.partition_time_per_round[0] > Duration::ZERO`.

### T2: Phase 2 reduces each partition locally
Create a `Net` with 4 CON agents: agents 0-1 form an active pair, agents 2-3 form another. Split into 2 partitions such that each partition contains one active pair. After `run_grid` with `max_rounds: Some(1)`, assert `metrics.local_interactions_per_round[0] >= 2` (at least 2 local interactions).

### T3: compute_time_per_round is recorded
Call `run_grid` with `max_rounds: Some(1)`. Assert `metrics.compute_time_per_round.len() == 1` and `metrics.compute_time_per_round[0] > Duration::ZERO`.

### T4: local_interactions_per_round sums worker interactions
Create a net where partition A has 1 redex and partition B has 2 redexes (all local). After 1 round, assert `metrics.local_interactions_per_round[0] == 3`.

### T5: WorkerRoundStats captures agents_before and agents_after
Create a `Net` with 4 agents, 2 per partition, each pair being an annihilation (CON-CON). After 1 round, assert `worker_stats_per_round[0][0].agents_before == 2` and `worker_stats_per_round[0][0].agents_after == 0` (agents consumed by annihilation).

### T6: WorkerRoundStats includes reduce_duration_secs
After `run_grid`, assert `worker_stats_per_round[0][0].reduce_duration_secs >= 0.0` for each worker.

### T7: WorkerRoundStats includes interactions_by_rule
After running a net with a CON-CON annihilation in partition 0, assert `worker_stats_per_round[0][0].interactions_by_rule[0] >= 1` (index 0 = CON-CON).

### T8: free_port_index rebuilt after local reduction
Create a partition with an agent connected to `FreePort(150)` (boundary). After local reduction rewires the connection, the rebuilt `free_port_index` should reflect the NEW endpoint. Verify by checking the merged result has the correct wiring.

## Edge Cases

### E1: Partition with no redexes
Create a net where one partition has active pairs and the other has none. Assert both partitions are processed. The empty-redex partition should have `local_redexes == 0` and `agents_before == agents_after` in its WorkerRoundStats.

### E2: All agents erased during local reduction
Create a partition with 2 ERA agents connected at principal ports (ERA-ERA void). After local reduction, both agents are consumed. Assert `agents_after == 0` in the WorkerRoundStats and the `free_port_index` is empty.

### E3: Multiple rounds accumulate correctly
Call `run_grid` with `max_rounds: Some(3)` on a net that requires 3 rounds. Assert `local_interactions_per_round.len() == 3`, `compute_time_per_round.len() == 3`, `worker_stats_per_round.len() == 3`.
