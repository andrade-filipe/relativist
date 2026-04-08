# TEST-SPEC-0044: Implement ContiguousIdStrategy

**Task:** TASK-0044
**Spec:** SPEC-04
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: 6 agents, 2 workers -- even split
Net with live agents at IDs `[0, 1, 2, 3, 4, 5]`, `num_workers=2`. Expected: agents `{0,1,2}` -> worker 0, agents `{3,4,5}` -> worker 1. Verify returned HashMap has 6 entries with these exact assignments.

### T2: 5 agents, 3 workers -- uneven split
Net with live agents at IDs `[0, 1, 2, 3, 4]`, `num_workers=3`. Expected chunks of size `ceil(5/3)=2`: `{0,1}` -> worker 0, `{2,3}` -> worker 1, `{4}` -> worker 2. Verify all 5 entries present.

### T3: 3 agents, 5 workers -- more workers than agents
Net with live agents at IDs `[0, 1, 2]`, `num_workers=5`. Expected: `{0}` -> worker 0, `{1}` -> worker 1, `{2}` -> worker 2. Workers 3 and 4 receive no agents. Verify HashMap has exactly 3 entries, all WorkerIds in `[0, 5)`.

### T4: 0 agents, 2 workers -- empty net
Net with no live agents, `num_workers=2`. Expected: empty HashMap.

### T5: Determinism -- two calls produce identical output
Call `allocate` twice on the same net with the same `num_workers`. Assert both returned HashMaps are identical (same keys, same values).

### T6: Every WorkerId is in range [0, num_workers)
Net with 10 agents, `num_workers=3`. Verify every value in the returned HashMap is `< 3`.

### T7: Every live agent has an entry
Net with live agents at IDs `[2, 5, 7]` (sparse), `num_workers=2`. Verify the HashMap contains keys `2`, `5`, and `7` (exactly 3 entries).

## Edge Cases

### E1: Sparse agent IDs
Net with live agents at IDs `[0, 10, 100]` (gaps between IDs), `num_workers=2`. Verify sorted-ascending split: `{0, 10}` -> worker 0, `{100}` -> worker 1.

### E2: Single agent, single worker
Net with 1 live agent at ID `0`, `num_workers=1`. Verify `{0: 0}`.

### E3: num_workers=1 assigns all to worker 0
Net with 5 agents, `num_workers=1`. All 5 agents must map to worker 0.
