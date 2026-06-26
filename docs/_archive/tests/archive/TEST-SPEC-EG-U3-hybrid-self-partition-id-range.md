# TEST-SPEC EG-U3: hybrid self-partition ID range (R8)

**SPEC-20 §7.1 ID:** EG-U3
**Owning task(s):** TASK-0421 (id-range recomputation), TASK-0430 (orchestrator).
**Type:** unit.
**Test name:** `test_hybrid_self_partition_id_range`.

---

## Inputs / Fixtures

- A net (any size).
- `GridConfig { hybrid_coordinator: true, .. }`.
- K_eff = 4 (3 remote + 1 self).

## Expected behaviour

`compute_id_ranges(K_eff=4)` returns 4 contiguous, non-overlapping `IdRange` values indexed by `partition_index`. The self-partition has `partition_index = 0` and therefore receives `ranges[0]` — the FIRST range.

## Assertions

| # | Assertion |
|---|-----------|
| A1 | `let ranges = compute_id_ranges(4); assert_eq!(ranges.len(), 4);`. |
| A2 | `ranges[0].start == 0` (or whatever the SPEC-04 baseline minimum is). |
| A3 | `ranges[0]` is the range assigned to the self-partition (partition_index=0). |
| A4 | `ranges[i+1].start == ranges[i].end` (contiguous). |
| A5 | `ranges[0]` has the same length as `ranges[1..]` (uniform; SPEC-04 `compute_id_ranges` returns equal-sized ranges). |

## Edge / negative cases

- EC-1: K_eff = 1 (hybrid only, no remote) → `ranges.len() == 1`; self-partition gets the entire id space. Document upper bound.
- EC-2: K_eff change mid-run (e.g., from 4 to 3 due to a departure) → recompute; self-partition still gets `ranges[0]` of the new K_eff. (Cross-test EG-U12.)

## Invariants asserted

- D4-elastic (R11a, R8): `partition_index` indexes the range; self-partition is always slot 0.

## ARG/DISC/REF citation

None.

## Determinism notes

Synchronous; deterministic.

## Cross-test dependencies

EG-U12 (id-range non-collision after re-partition) is the dynamic counterpart. EG-P3 is the proptest version.
