# TASK-0049 + TASK-0052 + TASK-0220 Reviews: General split case

## Stage 4-6: Combined Review — PASS

### TASK-0049 (split orchestrator)
- Implements full 7-step split algorithm per SPEC-04 Section 4.5
- Steps: trivial check → allocate → group → classify → build subnets → ID ranges → root
- Uses all previously built helpers: classify_wires, build_subnet, compute_id_ranges

### TASK-0052 (FreePort index)
- Built inline from border_entries: `(bid, AgentPort(aid, pid))`
- Enables O(1) border lookup during merge

### TASK-0220 (root port propagation)
- R28: root goes to partition containing the root agent
- Other partitions get None
- Handles None root and FreePort root correctly

### Tests
- 10 new general split tests (G1-G10): agent distribution, border map, FreePort index,
  ID range disjointness, redex filtering, C1 coverage, worker IDs, root propagation,
  empty net, more workers than agents
- 291 total tests. Clippy clean, fmt clean
