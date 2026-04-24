# TEST-SPEC-0072: Implement n==1 optimization in run_grid

**Task:** TASK-0072
**Spec:** SPEC-05 (R26)
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Single worker produces same result as reduce_all
Create a net with 4 CON agents forming 2 active pairs. Clone it. Run `reduce_all` on clone A. Run `run_grid` with `num_workers: 1` on clone B. Assert both result nets are structurally isomorphic.

### T2: Metrics converged is true
Call `run_grid` with `num_workers: 1` on a net with redexes. Assert `metrics.converged == true`.

### T3: total_interactions matches reduce_all count
Clone the net. `reduce_all` returns `ReductionStats` with `total_interactions = X`. `run_grid(num_workers: 1)` returns `metrics.total_interactions`. Assert both are equal.

### T4: total_interactions_by_rule populated (SC-004)
Create a net with a CON-DUP commutation. Call `run_grid` with `num_workers: 1`. Assert `metrics.total_interactions_by_rule` is not all zeros and the CON-DUP slot (index 1) is >= 1.

### T5: No partition-related metrics
Call `run_grid` with `num_workers: 1`. Assert `metrics.partition_time_per_round.is_empty()`, `metrics.merge_time_per_round.is_empty()`, `metrics.border_redexes_per_round.is_empty()`, `metrics.worker_stats_per_round.is_empty()`.

### T6: total_time is populated
Call `run_grid` with `num_workers: 1`. Assert `metrics.total_time > Duration::ZERO`.

## Edge Cases

### E1: Net already in Normal Form with num_workers == 1
Create a net with no redexes. Call `run_grid` with `num_workers: 1`. Assert `metrics.converged == true` and `metrics.total_interactions == 0`.

### E2: Single CON-CON annihilation
Create a net with exactly 1 CON-CON active pair. Call `run_grid` with `num_workers: 1`. Assert `metrics.total_interactions == 1` and the result net has 0 live agents (both consumed).

### E3: max_rounds ignored for n==1 path
Call `run_grid` with `num_workers: 1`, `max_rounds: Some(0)`. The n==1 optimization should bypass the round-limit check and reduce fully. Assert `metrics.converged == true`. (If the implementation checks max_rounds before the n==1 path, this test clarifies the expected behavior.)
