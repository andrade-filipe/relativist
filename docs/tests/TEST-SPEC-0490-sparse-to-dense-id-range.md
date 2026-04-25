# TEST-SPEC-0490: `SparseNet::to_dense(id_range)` with partition scoping (R20 — closes SC-006)

**SPEC-22 §7 ID:** T14, T14a, T15, T16 (spec-catalog) plus this plumbing file.
**Owning task:** TASK-0490.
**Parent spec:** SPEC-22 §3.2 R20, R21; §4.6.
**Type:** unit.

**Critical: `to_dense` signature change.** Old: `to_dense(&self) -> Net`. New: `to_dense(&self, id_range: Option<Range<AgentId>>) -> Net`. All call sites migrate to `to_dense(None)` for whole-net or `to_dense(Some(range))` for partition-scoped.

---

## Inputs / Fixtures

- A `SparseNet` with agents at IDs `{50, 51, 75, 99, 130, 175}` (the canonical fixture from SPEC-22 §7.2 T14a).
- `next_id = 200`.
- `freeport_redirects.insert(99, AgentPort(50, 0))` for SC-001 second-surface closure.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0490-01 | `to_dense_none_populates_full_free_list` | sparse fixture | `let net = sparse.to_dense(None);` | `net.free_list` contains every `None` index in `[0, max_id+1) = [0, 200)`. The set is `[0..200)` minus `{50, 51, 75, 99, 130, 175}`, cardinality `200 - 6 = 194`. |
| UT-0490-02 | `to_dense_some_partition_scoped_T14a` | same | `let net = sparse.to_dense(Some(50..100));` | `net.free_list.iter().copied().collect::<HashSet<_>>() == [50..100) - {50, 51, 75, 99}`, cardinality 46. (Joint with T14a UT-T14a-01.) |
| UT-0490-03 | `to_dense_some_excludes_below_range` | UT-0490-02 | `!net.free_list.iter().any(\|&id\| id < 50)`. | confirmed. |
| UT-0490-04 | `to_dense_some_excludes_above_range` | UT-0490-02 | `!net.free_list.iter().any(\|&id\| id >= 100)`. | confirmed. |
| UT-0490-05 | `to_dense_some_with_empty_range_yields_empty_free_list` | `to_dense(Some(50..50))` | `net.free_list.is_empty()`. |
| UT-0490-06 | `to_dense_id_range_propagated_to_returned_net` | UT-0490-02 | `net.id_range == Some(50..100)`. (TASK-0490 acceptance: `id_range` is set on the returned `Net` for downstream R10 defensive check.) |
| UT-0490-07 | `to_dense_preserves_freeport_redirects` | sparse with `freeport_redirects = {99 -> AgentPort(50, 0)}` | any `to_dense` call | `net.freeport_redirects == sparse.freeport_redirects`. (Closes SC-001 second surface.) |
| UT-0490-08 | `to_dense_preserves_redex_queue` | sparse with redex_queue `[(50, 51)]` | any `to_dense` call | `net.redex_queue == sparse.redex_queue`. |
| UT-0490-09 | `to_dense_preserves_next_id_and_root` | same | any `to_dense` call | preserved. |
| UT-0490-10 | `to_dense_arena_size_is_max_id_plus_one` | sparse with max ID 175 | `to_dense(None)` | `net.agents.len() == 176`. (R20: arena allocated at `max_id + 1`.) |
| UT-0490-11 | `to_dense_clamps_range_to_arena_len` | sparse with max ID 175; `to_dense(Some(170..250))` | post-call | free-list contains `[170..200)` minus `{175}` (clamped at `arena_len.min(range.end)`); no out-of-bounds panic. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Empty SparseNet | `to_dense(None)` produces a `Net` with `agents.len() == 1` (max_id == 0 → arena_len == 1, all `None`); free-list = `[0]`. |
| EC-2 | SparseNet with single agent at ID 0 | `to_dense(None)` produces arena_len 1, agents `[Some(...)]`, free-list empty. |
| EC-3 | `to_dense(Some(300..400))` on a sparse net with max_id 175 | Free-list contains `[300..400)` IF arena is grown to 400; OR empty if `to_dense` does NOT grow the arena beyond `max_id + 1`. The TASK-0490 acceptance prefers the larger of `(max_id + 1, range.end)`; the test asserts that contract. |
| EC-4 | rkyv `to_dense` round-trip under `--features zero-copy` | Same assertions hold (joint with TEST-SPEC-0489 / T18). |

## Invariants asserted

- R20 (`to_dense` signature change with `id_range: Option<Range<AgentId>>`).
- R10 / R10a / D4 (partition-scoped free-list — closes SC-006).
- D1c (FreePort bijectivity).

## ARG/DISC/REF citation

- AC-011 (HVM4 static heap partitioning).

## Determinism notes

`HashMap::keys().max()` is deterministic in the value (max is unique); the iteration order doesn't matter. The walk-and-push for free-list is ascending and deterministic. Pure synchronous; no tokio.

## Cross-test dependencies

- T14 / T14a / T15 / T16 spec-catalog mirrors.
- TEST-SPEC-0489 (`to_sparse`) is the inverse direction.
- TEST-SPEC-0491 (`is_behaviorally_equal`) covers round-trip.
- TEST-SPEC-0492 consumes `to_dense(Some(range))` for the sparse build_subnet path.
