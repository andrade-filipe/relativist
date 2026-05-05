# TEST-SPEC EG-U12a: partition_index vs WorkerId decoupling (R11a, SC-006)

**SPEC-20 §7.1 ID:** EG-U12a
**Owning task(s):** TASK-0420.
**Type:** unit.
**Test name:** `test_partition_index_vs_worker_id_decoupling`.

---

## Inputs / Fixtures

- Hybrid mode (so WorkerId 0 is the self).
- An active worker set with sparse WorkerIds: `{WorkerId(0), WorkerId(1), WorkerId(5), WorkerId(7)}`.
- K_eff = 4.

## Expected behaviour

`partition_index` is computed by sorting the active set by `WorkerId` ascending and assigning dense indices `[0, K_eff)`:
- WorkerId(0) → partition_index 0
- WorkerId(1) → partition_index 1
- WorkerId(5) → partition_index 2
- WorkerId(7) → partition_index 3

`compute_id_ranges(K_eff=4)` returns 4 ranges; range `[c*i, c*(i+1))` is assigned to `partition_index = i`, NOT to the value of WorkerId.

## Assertions

| # | Assertion |
|---|-----------|
| A1 | `partition_index_for(WorkerId(5)) == 2` (not 5). |
| A2 | `partition_index_for(WorkerId(7)) == 3` (not 7). |
| A3 | `id_range_for(WorkerId(7)) == ranges[3]` (the 4th range, not the 8th — there is no 8th, K_eff=4). |
| A4 | All 4 ranges are dense and non-overlapping. |

## Edge / negative cases

- EC-1: WorkerIds `{0, u32::MAX}` with K_eff=2 → `partition_index 0 → WorkerId(0)`, `partition_index 1 → WorkerId(u32::MAX)`. No allocation overflow.
- EC-2: a WorkerId is added (join) — sort order may shift the partition_index of an existing worker. Document this expected re-mapping.
- EC-3: a WorkerId is removed (departure) — surviving workers' partition_indices are recomputed densely.

## Invariants asserted

- D4-elastic (R11a).

## ARG/DISC/REF citation

None.

## Determinism notes

Synchronous; deterministic.

## Cross-test dependencies

EG-U12 (collision-free after re-partition). EG-P3 (proptest disjointness).
