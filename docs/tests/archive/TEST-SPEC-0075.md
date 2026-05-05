# TEST-SPEC-0075: Integration test - Fundamental Property G1 (run_grid equivalence)

**Task:** TASK-0075
**Spec:** SPEC-05 (R24, R30, R31, R33; SPEC-01 G1, T7)
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: CON-CON annihilation (simplest case)
Build a net with 2 CON agents connected at principal ports. Clone. `reduce_all` on A -> 0 agents, 1 interaction. `run_grid` on B with `num_workers: 2`. Assert Normal Forms are isomorphic. Assert `metrics.total_interactions == 1` (T7).

### T2: DUP-DUP annihilation
Build a net with 2 DUP agents connected at principal ports. Clone. Compare `reduce_all` vs `run_grid(num_workers: 2)`. Assert isomorphic Normal Forms and equal interaction counts.

### T3: ERA-ERA void
Build a net with 2 ERA agents connected at principal ports. Clone. Compare sequential vs grid. Assert both produce 0 live agents. Assert interaction counts equal.

### T4: CON-DUP commutation (creates 4 new agents)
Build a net with 1 CON and 1 DUP connected at principal ports, with auxiliary ports wired to FreePort stubs. Clone. `reduce_all` on A. `run_grid(num_workers: 2)` on B. Assert isomorphic results. Assert `metrics.total_interactions == seq_interactions`.

### T5: CON-ERA erasure
Build a net with 1 CON and 1 ERA connected at principal ports. Clone. Compare. Assert the CON's auxiliary ports are connected to new ERA agents (or erased). Assert interaction counts equal.

### T6: DUP-ERA erasure
Build a net with 1 DUP and 1 ERA connected at principal ports. Clone. Compare. Assert interaction counts equal.

### T7: Chain of 4+ agents requiring multiple reduction steps
Build a chain: CON(0) -> DUP(1) -> CON(2) -> ERA(3) with appropriate principal-port connections. Clone. Compare `reduce_all` vs `run_grid(num_workers: 2)`. Assert isomorphic Normal Forms.

### T8: Net requiring multiple grid rounds (border redexes generate new redexes)
Build a net where a CON-DUP commutation spans the partition boundary. The commutation creates 4 new agents, some of which form new active pairs. These require a second round. Clone. Compare. Assert isomorphic results and equal total interactions.

### T9: Worker count 4
Repeat T4 (CON-DUP commutation) with `num_workers: 4`. Assert G1 holds.

### T10: Worker count 2 and 4 produce same Normal Form
Build a net with 8 agents. Run `run_grid` with `num_workers: 2` and separately with `num_workers: 4`. Assert both Normal Forms are isomorphic to each other and to `reduce_all` result.

## Edge Cases

### E1: Net already in Normal Form
Build a net with 4 agents and no active pairs. Clone. `reduce_all` does nothing. `run_grid` returns immediately (0 rounds). Assert isomorphic (unchanged). Assert `metrics.total_interactions == 0`.

### E2: Single-interaction net with many workers
Build a net with 2 CON agents (1 active pair) and run `run_grid` with `num_workers: 4`. Most partitions will be empty. Assert G1 still holds.

### E3: Asymmetric partition load
Build a net where one partition gets 90% of the agents and the other gets 10%. Assert G1 holds regardless of load imbalance.
