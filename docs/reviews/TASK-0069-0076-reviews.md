# TASK-0069/0070/0071/0072/0073/0075/0076/0230 Reviews: Grid Loop + Module Wiring

## Stage 4-6: Combined Review — PASS

### TASK-0069 (run_grid skeleton)
- Correct signature: `run_grid(net: Net, config: &GridConfig, strategy: &dyn PartitionStrategy) -> (Net, GridMetrics)`
- Step [0]: drain_stale_redexes + empty check for initial Normal Form (SC-011)
- Step [1]: max_rounds check at loop top (R29)
- Step [7]: drain + empty check for termination (SC-002, aligned with SPEC-13)
- total_time recorded via Instant::now()

### TASK-0070 (Phase 1 + Phase 2)
- split() called with current_net by value
- For each partition: agents_before, reduce_all, rebuild_free_port_index, agents_after
- WorkerRoundStats populated with all 6 fields (SC-001)
- interactions_by_rule used directly from ReductionStats (no mapping needed, OQ-4 resolved)
- local_by_rule accumulated across workers
- Timing: partition_time, compute_time per round

### TASK-0071 (Phase 3 + Phase 4)
- merge(plan) by value -> (merged_net, border_redex_count) (SC-003)
- reduce_all on merged net for border resolution (R15-R18)
- Metrics: merge_time, border_reduce_time, border_interactions, border_redexes
- Per-rule accumulation: total_interactions_by_rule += local_by_rule + border_by_rule (SC-004)
- total_interactions = local + border per round

### TASK-0072 (n==1 optimization)
- run_single_worker() skips split/merge entirely (R26)
- Direct reduce_all on the net
- Correct metrics population (1 round, 0 border redexes)
- max_rounds=0 handled correctly

### TASK-0073 (count_live_agents)
- Already implemented as Net::count_live_agents() in Phase 1 (TASK-0231)
- OBSOLETED — no separate helper needed

### TASK-0075 (Fundamental Property G1)
- 8 G1 tests covering all 6 interaction rules + multi-agent + empty net
- For each: reduce_all(net.clone()) vs run_grid(net, 2), compare agent count + total interactions
- CON-DUP across partition boundary tested (emergent border redex scenario)
- 4-worker test included
- All tests pass: G1 holds empirically

### TASK-0076 (module exports)
- merge/mod.rs: exports merge, run_grid, drain_stale_redexes, rebuild_free_port_index,
  GridConfig, GridMetrics, WorkerRoundStats
- All public types accessible via `crate::merge::*`

### TASK-0230 (verify_no_redexes_full_scan) — partial
- Implemented as debug-only function in grid.rs
- O(A) scan for active pairs not in queue
- Called at termination check in debug mode (R41)

### ID Range Fix (compute_id_ranges)
- Changed from dividing u32::MAX to compact ranges based on net size
- chunk = max(100_000, base_next_id * 10) per worker
- Last worker extends to u32::MAX as safety margin
- Prevents OOM from billion-entry sparse arrays during local reduction
- 7 updated tests verify disjointness, contiguity, min chunk guarantee

### Tests
- 17 new tests (grid loop + G1)
- 341 total tests. Clippy clean, fmt clean
