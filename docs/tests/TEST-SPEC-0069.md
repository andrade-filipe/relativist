# TEST-SPEC-0069: Implement run_grid function skeleton with termination logic

**Task:** TASK-0069
**Spec:** SPEC-05 (R24, R27, R28, R29, R30)
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Net already in Normal Form - immediate convergence
Create a `Net` with 2 agents and no redexes (empty queue, no active pairs). Call `run_grid(net, &config, &strategy)` with `num_workers: 2`, `max_rounds: None`. Assert `metrics.converged == true`, `metrics.rounds == 0`, and the returned net is unchanged.

### T2: max_rounds = Some(0) - immediate termination
Create a `Net` with active redexes in the queue. Call `run_grid` with `max_rounds: Some(0)`. Assert `metrics.converged == false` and `metrics.rounds == 0`.

### T3: total_time is populated
Create any valid `Net`. Call `run_grid`. Assert `metrics.total_time > Duration::ZERO` (or `>= Duration::ZERO` for nets already in NF that return instantly).

### T4: agents_per_round recorded at start of each round
Create a `Net` with 4 agents and active redexes. Call `run_grid` with `max_rounds: Some(2)`. Assert `metrics.agents_per_round.len() == metrics.rounds as usize` and `metrics.agents_per_round[0] == 4` (or the actual live agent count).

### T5: Rounds counter increments correctly
Call `run_grid` with `max_rounds: Some(3)` on a net that does not converge within 3 rounds. Assert `metrics.rounds == 3`.

### T6: Return type is (Net, GridMetrics)
Verify the function signature returns `(Net, GridMetrics)`. The Net is the final state (Normal Form or partial). GridMetrics contains all accumulated data.

## Edge Cases

### E1: Empty net (no agents, no redexes)
Create an empty `Net`. Call `run_grid`. Assert `metrics.converged == true`, `metrics.rounds == 0`. An empty net is trivially in Normal Form.

### E2: max_rounds = Some(1) with convergence on first round
Create a `Net` with a single CON-CON active pair (annihilates in 1 interaction). Call `run_grid` with `num_workers: 2`, `max_rounds: Some(1)`. Assert `metrics.converged == true` and `metrics.rounds == 1`.

### E3: Module compiles
`cargo check` passes with the `run_grid` skeleton in `src/merge.rs`. Phase placeholder `todo!()` calls are acceptable at this stage.
