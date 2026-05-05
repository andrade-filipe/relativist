# TEST-SPEC-T14a: Partition-scoped `to_dense(id_range)` (closes SC-006)

**SPEC-22 §7.2 ID:** T14a.
**Owning task:** TASK-0490 (`SparseNet::to_dense` with `id_range: Option<Range<AgentId>>`).
**Parent spec:** SPEC-22 §3.2 R20 (signature change closes SC-006); §4.6 backward-compat note + call-site audit list.
**Type:** unit.
**Theory anchor:** AC-011 (HVM4 static heap partitioning — partition-scoped allocation).

---

## Inputs / Fixtures

- Fresh `SparseNet::new()`.
- Insert agents at IDs **`{50, 51, 75, 99, 130, 175}`** (the canonical fixture from SPEC-22 §7.2 T14a). Use `sparse.agents.insert(id, Agent { symbol: CON, id })` directly (test-only construction; bypasses `create_agent`'s `next_id`).
- Set `sparse.next_id = 200` (so the conversion knows the ID space cap).

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-T14a-01 | `to_dense_some_50_to_100_yields_correct_free_list` | sparse with agents at `{50, 51, 75, 99, 130, 175}` | `let net = sparse.to_dense(Some(50..100));` | `net.free_list.iter().copied().collect::<HashSet<_>>() == {52, 53, 54, ..., 74, 76, 77, ..., 98}` (i.e., `[50..100)` minus `{50, 51, 75, 99}`). The set has cardinality `50 - 4 = 46`. |
| UT-T14a-02 | `to_dense_some_50_to_100_excludes_below_range` | same | same | `!net.free_list.iter().any(\|&id\| id < 50)`. **No IDs in `[0, 50)` appear.** |
| UT-T14a-03 | `to_dense_some_50_to_100_excludes_above_range` | same | same | `!net.free_list.iter().any(\|&id\| id >= 100)`. **No IDs in `[100, max_id]` appear.** |
| UT-T14a-04 | `to_dense_some_100_to_200_yields_correct_free_list` | same sparse fixture | `let net2 = sparse.to_dense(Some(100..200));` | `net2.free_list.iter().copied().collect::<HashSet<_>>() == {100, 101, ..., 129, 131, 132, ..., 174, 176, 177, ..., 199}` (i.e., `[100..200)` minus `{130, 175}`). The set has cardinality `100 - 2 = 98`. |
| UT-T14a-05 | `to_dense_some_id_range_set_on_returned_net` | same | post-conversion, check `net.id_range` | `net.id_range == Some(50..100)`. (TASK-0490 acceptance criterion: `id_range` is propagated to the returned `Net` for downstream R10 defensive check.) |
| UT-T14a-06 | `agents_present_in_range_preserved` | UT-T14a-01 | check `net.agents[50].is_some()` and `net.agents[51].is_some()` and `net.agents[75].is_some()` and `net.agents[99].is_some()` | all `true`. (Agents inside the range are placed in the dense arena.) |
| UT-T14a-07 | `agents_outside_range_also_placed_in_dense_arena` | UT-T14a-01 | check `net.agents[130].is_some()` and `net.agents[175].is_some()` | both `true`. (Agents are placed regardless of `id_range`; only the FREE-LIST is range-scoped per R20.) |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Empty range `Some(50..50)` | Free-list is empty. (Vacuous: no `None` slots in an empty range.) |
| EC-2 | Range that matches a single ID `Some(75..76)` | Since 75 IS occupied, free-list is empty. |
| EC-3 | Range entirely outside the agent set `Some(300..400)` | Free-list contains all of `[300..400)` IDs (100 entries). The dense arena length is `max(max_id, 399) + 1 = 400`. |
| EC-4 | Range that exceeds `max_id` (e.g., `Some(170..250)`) | `to_dense` clamps the iteration upper bound to `arena_len.min(range.end)`; free-list contains `[170..200)` minus `{175}` = 29 entries (no out-of-bounds panic). |
| EC-5 | `to_dense(None)` whole-net case for comparison | Free-list contains every `None` index in `[0, max_id+1)`. Joint coverage with T14. |

## Invariants asserted

- R20 (`to_dense` signature change with `id_range: Option<Range<AgentId>>` — closes SC-006).
- R10 / R10a (free-list confined to partition range — verified by UT-T14a-02/03).
- D4 (D4 violation prevention — partition-scoped free-list).
- §4.6 SC-006 fix.

## ARG/DISC/REF citation

- AC-011 (HVM4 static heap partitioning — same partition-scoped allocation rationale).

## Determinism notes

Pure synchronous; no tokio. The walk-and-push order in `to_dense`'s free-list construction is deterministic (ascending `i in lo..hi`). HashMap iteration over `sparse.agents` for placement is non-deterministic but the resulting dense arena is order-invariant (each agent placed at its own ID).

## Cross-test dependencies

- T14 covers the whole-net (`None`) variant; T14a covers the partition-scoped (`Some(range)`) variant. Together they exercise both branches of the §4.6 free-list construction.
- TEST-SPEC-0490 covers the conversion at plumbing level.
- T16 (sparse build_subnet) consumes `to_dense(Some(partition.id_range.clone()))` end-to-end.
