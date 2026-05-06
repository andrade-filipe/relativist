# TEST-SPEC EG-I5b: emergent borders across retained / evolved (R24d, SC-021)

**SPEC-20 §7.2 ID:** EG-I5b
**Owning task(s):** TASK-0440.
**Type:** integration.
**Test name:** `test_emergent_borders_across_retained_evolved`.

---

## Inputs / Fixtures

- v1 mode + hybrid; `elastic_departure = true`.
- A workload designed to produce **emergent border redexes** in BOTH the reclaimed partition (frozen at last_acked) AND the surviving partitions (which have evolved further).
- Test fixture provides a scenario where, after a worker departs, the reclaimed partition's border_ids would collide with newly-allocated border_ids if not rebased.

## Expected behaviour

R24d + SPEC-04 A3 (`allocate_border_ids`): the reclaim path REBASES the reclaimed border_ids to a fresh disjoint range. After re-split, the surviving partitions' (now further evolved) borders coexist with the rebased reclaimed borders without collision. The merge resolves all borders correctly; final result matches `reduce_all`.

## Assertions

| # | Assertion |
|---|-----------|
| A1 | `canonicalise(final) == canonicalise(reduce_all(net))`. |
| A2 | After reclaim, `{reclaimed_border_ids} ∩ {surviving_border_ids} == ∅` (cross-anchor EG-U7a). |
| A3 | Border-resolution invokes `BorderGraph::detect_border_redexes()` for the merged set; result matches a hypothetical exhaustive scan of `reduce_all`. |
| A4 | No agent is double-counted; total agent count preserved. |

## Edge / negative cases

- EC-1: the reclaimed partition's border_ids are originally `[0, 5)` and surviving allocations have advanced past 5 — without rebase, collision would occur; WITH rebase, the new range is `[k, k+5)` for k > current cursor.
- EC-2: the surviving partitions have produced emergent borders since the snapshot — those new borders are properly tracked in the BorderGraph during the reclaim cycle.

## Invariants asserted

- D3 (Border Completeness via D3-elastic R24c-d).
- D4 (ID Uniqueness for border_ids).

## ARG/DISC/REF citation

None.

## Determinism notes

`#[tokio::test(flavor = "current_thread", start_paused = true)]`.

## Cross-test dependencies

EG-U7a (border_id rebase unit test); TEST-SPEC-0411 (`allocate_border_ids` properties).
