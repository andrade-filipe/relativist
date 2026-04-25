# TEST-SPEC-0481: `build_subnet` populates partition free-list with in-range `None` slots (R10a)

**SPEC-22 §7 ID:** T9 (joint coverage with TEST-SPEC-0480) plus this plumbing file.
**Owning task:** TASK-0481.
**Parent spec:** SPEC-22 §3.1 R10a; §3.8 A7.
**Type:** unit + integration.

---

## Inputs / Fixtures

- A net of 200 agents (IDs 0-199) with explicit pre-split removes:
  - Build with `(0..200).for_each(|_| net.create_agent(CON))`.
  - `net.remove_agent(50); net.remove_agent(75); net.remove_agent(90); net.remove_agent(150);`
- Partition into 2 workers via SPEC-04 contiguous-id strategy:
  - Partition 0: id_range `0..100`.
  - Partition 1: id_range `100..200`.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0481-01 | `build_subnet_populates_free_list_in_range` | partition 0 fixture | `let p0 = build_subnet(&net, &worker_agents[0], &sigma, &border_entries[0], 0..100, &config).unwrap();` | `p0.free_list.iter().copied().collect::<HashSet<_>>() == {50, 75, 90}`. |
| UT-0481-02 | `build_subnet_excludes_out_of_range_none_slots` | partition 0 fixture | same | `!p0.free_list.iter().any(\|&id\| id >= 100)`. The ID 150 (partition 1's range) is NOT in p0's free-list. |
| UT-0481-03 | `build_subnet_partition_1_only_contains_partition_1_freed` | partition 1 fixture | `let p1 = build_subnet(...);` | `p1.free_list.iter().copied().collect::<HashSet<_>>() == {150}`. |
| UT-0481-04 | `build_subnet_empty_id_range_yields_empty_free_list` | partition with `id_range = 0..0` (degenerate) | same | `p.free_list.is_empty()`. |
| UT-0481-05 | `build_subnet_id_range_clamped_to_arena_len` | partition with `id_range = 100..300` but `net.agents.len() == 200` | same | iterates only `[100..200)`; no out-of-bounds panic. |
| UT-0481-06 | `build_subnet_sets_id_range_on_returned_net` | any partition | post-call | `p.id_range == Some(partition_id_range.clone())`. |
| UT-0481-07 | `build_subnet_lifo_compatible_push_order` | partition 0 fixture | check `p0.free_list[..]` | the order is some LIFO-valid sequence of `{50, 75, 90}`; with ascending iteration, the implementation pushes in order `[50, 75, 90]` (LIFO top is 90). |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | All slots in partition's range are live (no `None`) | Free-list empty. |
| EC-2 | All slots in partition's range are `None` (no live agents) | Free-list contains every index in the range. |
| EC-3 | Partition's range overlaps with another's (CONFIGURATION ERROR — not allowed by SPEC-04) | Out of scope for TASK-0481; SPEC-04 rejects overlapping ranges. |
| EC-4 | `build_subnet` called on a net with `next_id == 50` and partition range `100..200` (the whole range is uninstantiated) | Free-list contains every index in `[100, 200)` IF the arena has been pre-allocated to length 200; OR free-list is empty if arena_len < 100 (guarded by the `arena_len.min(id_range.end)` clamp). |

## Invariants asserted

- R10a (build_subnet populates partition free-list within range).
- D4 (precondition supplier — partition free-list disjoint by construction).
- §3.8 A7.

## ARG/DISC/REF citation

- ARG-002 (Partitioning Preserves Structure — partition disjointness rationale).
- AC-011 (HVM4 static heap partitioning).

## Determinism notes

The walk-and-push in `build_subnet` is ascending (per TASK-0481 acceptance: "ascending iteration + push"); the resulting free-list is deterministic. Pure synchronous; no tokio.

## Cross-test dependencies

- T9 (spec-catalog) is the integration mirror.
- TEST-SPEC-0480 covers the defensive `id_range` check.
- TEST-SPEC-0492 covers the sparse-then-dense path (R22 threshold).
