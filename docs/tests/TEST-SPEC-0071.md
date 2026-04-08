# TEST-SPEC-0071: Implement run_grid Phase 3 (merge + resolve borders + metrics)

**Task:** TASK-0071
**Spec:** SPEC-05 (R15, R16, R17, R18, R19, R31, R33, R38, R39)
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: merge_time_per_round recorded
Call `run_grid` with `num_workers: 2`, `max_rounds: Some(1)` on a net with border wires. Assert `metrics.merge_time_per_round.len() == 1` and `metrics.merge_time_per_round[0] > Duration::ZERO`.

### T2: border_redexes_per_round reflects actual border redexes
Create a net where the partition boundary crosses an active pair (both principal ports become border endpoints). After 1 round, assert `metrics.border_redexes_per_round[0] >= 1`.

### T3: border_reduce_time_per_round recorded (SC-009)
Call `run_grid` with `max_rounds: Some(1)`. Assert `metrics.border_reduce_time_per_round.len() == 1`.

### T4: border_interactions_per_round counts post-merge interactions
Create a net with a border redex. After merge + reduce_all, assert `metrics.border_interactions_per_round[0] >= 1`.

### T5: total_interactions accumulates local + border
Create a net with both local and border redexes. After `run_grid`, assert `metrics.total_interactions == sum(local_interactions_per_round) + sum(border_interactions_per_round)`.

### T6: total_interactions_by_rule accumulates per-rule (SC-004)
Create a net with a CON-CON local redex and a CON-DUP border redex. After `run_grid`, assert `metrics.total_interactions_by_rule[0] >= 1` (CON-CON) and `metrics.total_interactions_by_rule[1] >= 1` (CON-DUP).

### T7: current_net updated after merge
After `run_grid` with `max_rounds: Some(1)`, the returned `Net` should be the merged+reduced result, not the original input.

### T8: Full run_grid end-to-end (no more todo!())
Create a net with 4 agents and 2 redexes. Call `run_grid` with `num_workers: 2`, `max_rounds: None`. Assert `metrics.converged == true` and the returned net is in Normal Form (empty redex queue).

### T9: rounds counter incremented at end of round
Call `run_grid` with `max_rounds: Some(2)`. Assert `metrics.rounds == 2` (or fewer if converged earlier). Assert `metrics.rounds == metrics.local_interactions_per_round.len()`.

## Edge Cases

### E1: No border redexes after merge
Create a net where the boundary crosses only auxiliary-port wires (no principal-principal pairs). After merge, assert `metrics.border_redexes_per_round[0] == 0` and `metrics.border_interactions_per_round[0] == 0`.

### E2: Cascading border redexes
Create a net where resolving a border CON-DUP commutation creates new redexes that are then resolved in the same post-merge `reduce_all`. Assert `metrics.border_interactions_per_round[0] > 1` (the cascade produced multiple interactions).

### E3: Multiple rounds with decreasing activity
Run a net that takes 3 rounds to converge. Assert `metrics.local_interactions_per_round` is non-increasing across rounds (activity decreases as the net approaches Normal Form). This is a soft expectation, not a strict invariant.
