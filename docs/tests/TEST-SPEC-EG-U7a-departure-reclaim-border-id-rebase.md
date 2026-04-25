# TEST-SPEC EG-U7a: departure reclaim border_id rebase (R24d, SC-021)

**SPEC-20 §7.1 ID:** EG-U7a
**Owning task(s):** TASK-0440, TASK-0452 (debug assertions).
**Type:** unit.
**Test name:** `test_departure_reclaim_border_id_rebase`.

---

## Inputs / Fixtures

- v1 mode + hybrid.
- A reclaimed partition with `border_ids in [100, 105]` (5 borders).
- Surviving partitions whose collective `border_ids in [200, 210]` (11 borders).

## Expected behaviour

R24d + SPEC-04 A3 (`allocate_border_ids`): when the reclaimed partition is re-introduced into the system, its border_ids are REBASED to a fresh disjoint range (e.g., `[211, 215]`) — NOT left at `[100, 105]` where they could collide with an emerging fresh border allocation that grows past 100.

## Assertions

| # | Assertion |
|---|-----------|
| A1 | After reclaim + rebase: every border_id in the reclaimed partition is `>= 211` (the surviving max + 1, give or take). |
| A2 | The set `{reclaimed_border_ids} ∩ {surviving_border_ids} == ∅`. |
| A3 | `PartitionPlan::next_border_id` was advanced by exactly 5 (one per rebased border). |
| A4 | The internal wire `FreePort(old_bid)` references inside the reclaimed partition's agents are renamed consistently to `FreePort(new_bid)`. |
| A5 | The mate side of each border (in the surviving partitions or the BorderGraph) is updated to reference the new bid. |

## Edge / negative cases

- EC-1: reclaimed has 0 borders → no rebase needed; allocate_border_ids(0) is a no-op.
- EC-2: surviving partitions have already allocated all border_ids up to `u32::MAX - 3`; reclaim of 5 borders → `BorderIdSpaceExhausted` error; coordinator transitions to `Error`.
- EC-3: the reclaimed partition's existing border_ids HAPPEN to not overlap with the surviving range — rebase still occurs (no special-case "skip rebase if no collision"); mathematically simpler.

## Invariants asserted

- D3 (Border Completeness, via D3-elastic R24c-d).
- D4 (ID Uniqueness — for border_ids, governed by R30 via SPEC-04 A3).

## ARG/DISC/REF citation

None.

## Determinism notes

Synchronous; deterministic.

## Cross-test dependencies

- TEST-SPEC-0411 covers `allocate_border_ids` itself.
- EG-I5b is the workload-level integration test for emergent borders across reclaim.
