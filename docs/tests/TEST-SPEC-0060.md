# TEST-SPEC-0060: Define GridMetrics struct

**Task:** TASK-0060
**Spec:** SPEC-05 (R34, R35, R35a)
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Default values are correct
Create `GridMetrics::default()`. Assert: `rounds == 0`, `total_interactions == 0`, `total_interactions_by_rule == [0; 6]`, `converged == false`, `total_time == Duration::ZERO`.

### T2: All Vec fields default to empty
Create `GridMetrics::default()`. Assert all `Vec` fields are empty: `local_interactions_per_round.is_empty()`, `border_interactions_per_round.is_empty()`, `border_redexes_per_round.is_empty()`, `agents_per_round.is_empty()`, `partition_time_per_round.is_empty()`, `compute_time_per_round.is_empty()`, `merge_time_per_round.is_empty()`, `border_reduce_time_per_round.is_empty()`, `index_rebuild_time_per_round.is_empty()`, `worker_stats_per_round.is_empty()`.

### T3: Fields are writable
Create `GridMetrics::default()`, set `rounds = 5`, `total_interactions = 42`, `converged = true`. Assert values after mutation.

### T4: Derives Debug and Clone
Create `GridMetrics::default()`, call `format!("{:?}", metrics)` to verify Debug. Call `metrics.clone()` to verify Clone.

### T5: total_interactions_by_rule has 6 elements
Create `GridMetrics::default()`. Assert `total_interactions_by_rule.len() == 6`. Set each index [0..6] to a distinct value (e.g., `[10, 20, 30, 40, 50, 60]`) and assert round-trip.

## Edge Cases

### E1: Push to Vec fields and verify
Create `GridMetrics::default()`. Push `Duration::from_millis(100)` to `merge_time_per_round`. Push `Duration::from_millis(50)` to `border_reduce_time_per_round`. Assert lengths are 1 and values match. This confirms the structural separation of merge timing from border resolution timing (SC-009).

### E2: worker_stats_per_round nested structure
Create `GridMetrics::default()`. Push an empty `Vec<WorkerRoundStats>` to `worker_stats_per_round`. Then push a Vec with one entry. Assert `worker_stats_per_round.len() == 2` and `worker_stats_per_round[0].is_empty()` and `worker_stats_per_round[1].len() == 1`.

### E3: Module compiles
`cargo check` passes with the `GridMetrics` struct in `src/merge.rs`.
