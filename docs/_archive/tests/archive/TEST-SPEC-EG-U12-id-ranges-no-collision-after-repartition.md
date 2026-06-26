# TEST-SPEC EG-U12: id ranges no collision after re-partition (R13, R30, R11a)

**SPEC-20 §7.1 ID:** EG-U12
**Owning task(s):** TASK-0421, TASK-0440.
**Type:** unit.
**Test name:** `test_id_ranges_no_collision_after_repartition`.

---

## Inputs / Fixtures

A scripted sequence of K_eff transitions:
- Start: K_eff = 3.
- After 1 join: K_eff = 4.
- After 2 departures: K_eff = 2.
- After 1 join: K_eff = 3.
- After 1 join: K_eff = 4.

For each transition, capture all partitions' `IdRange` allocations.

## Expected behaviour

After every K_eff change, `compute_id_ranges(K_eff_new)` produces non-overlapping ranges. The pre-existing partitions' agent ids are renumbered (via `remap_partition_ids` for the reclaimed ones; survivor partitions either keep their ids if the new range subsumes them or are renumbered for safety — TASK-0421 defines).

## Assertions

| # | Assertion |
|---|-----------|
| A1 | At every K_eff transition, the union of `[IdRange.start, IdRange.end)` across all K_eff partitions is a contiguous, non-overlapping coverage of some prefix of `u32`. |
| A2 | No two partitions share any `AgentId` value (assert via set intersection across all pairs). |
| A3 | The total agent count across all partitions equals the input net's agent count at every transition. |
| A4 | At every transition, `partition_index` is dense `[0, K_eff)`; `WorkerId` may be sparse (cross-test EG-U12a). |

## Edge / negative cases

- EC-1: K_eff transitions back to a value previously seen (e.g., 3 → 4 → 3) → ranges are recomputed afresh; not memoised. Document.
- EC-2: K_eff=1 (everything in self) → single range covers full id space.
- EC-3: agent count near `u32::MAX / K_eff` → ranges remain disjoint up to the max representable.

## Invariants asserted

- D4-elastic (R11a, R13, R30).

## ARG/DISC/REF citation

None.

## Determinism notes

Synchronous; deterministic.

## Cross-test dependencies

EG-P3 is the proptest version. EG-U12a covers the WorkerId-sparse / partition_index-dense decoupling.
