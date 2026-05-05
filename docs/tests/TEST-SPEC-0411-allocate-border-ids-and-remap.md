# TEST-SPEC-0411: `PartitionPlan::allocate_border_ids` + `remap_partition_ids` (SPEC-04 A3, A4)

**SPEC-20 §7 ID:** none direct (transitively exercised by EG-U7a, EG-U12).
**Owning task:** TASK-0411.
**Parent spec:** SPEC-04 (amended via SPEC-20 §3.8 A3, A4); SPEC-20 R24d (border_id rebase), R30 (ID uniqueness preservation).
**Type:** unit.

---

## Inputs / Fixtures

- A `PartitionPlan` constructed with a small initial `next_border_id` (e.g. 0 or 100).
- A second `PartitionPlan` constructed with `next_border_id = u32::MAX - 5` (near-exhaustion fixture).
- A small `Partition` `p0` with 4 agents in `IdRange [10, 14)`, two internal wires, one `FreePort(bid=42)`.
- A `new_range = IdRange [200, 204)` for renumbering tests.
- An undersized `IdRange [200, 202)` for the error-path test.

## Unit Tests

### A3 — `allocate_border_ids`

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0411-01 | `allocate_border_ids_disjoint_successive_calls` | fresh plan, `next_border_id=0` | `let r1 = plan.allocate_border_ids(5)?; let r2 = plan.allocate_border_ids(7)?;` | `r1 == 0..5`, `r2 == 5..12`; ranges are disjoint; cursor advanced to 12. |
| UT-0411-02 | `allocate_border_ids_cursor_advances` | fresh plan | repeated calls of varying size | After N calls with sizes `c_i`, cursor equals `Σ c_i`. |
| UT-0411-03 | `allocate_border_ids_zero_count_is_legal_no_op` | fresh plan | `let r = plan.allocate_border_ids(0)?` | `r.is_empty()`; cursor unchanged; no error. |
| UT-0411-04 | `allocate_border_ids_exhaustion_error` | plan with `next_border_id = u32::MAX - 5` | `plan.allocate_border_ids(10)` | Returns `Err(PartitionError::BorderIdSpaceExhausted { requested: 10, available: 5 })`; cursor unchanged. |

### A4 — `remap_partition_ids`

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0411-05 | `remap_preserves_agent_count_and_symbols` | `p0` (4 agents in [10,14)), `new_range = [200,204)` | `let p = remap_partition_ids(p0.clone(), new_range)?` | `p.agent_count() == 4`; the symbol multiset of `p.agents()` equals the symbol multiset of `p0.agents()`; new agent ids are `{200, 201, 202, 203}`. |
| UT-0411-06 | `remap_rewrites_internal_edges` | same | same | Every internal `PortRef::AgentPort(old_id, port)` in `p0` is now `PortRef::AgentPort(new_id, port)` in `p`, where `new_id = 200 + (old_id - 10)`; no stale `AgentPort(<10..14>, _)` references remain in `p`. |
| UT-0411-07 | `remap_leaves_freeports_unchanged` | same | same | `p`'s `FreePort(bid)` entries are bit-identical to `p0`'s (e.g., `FreePort(42)` still equals `FreePort(42)`). border_ids are rebased separately via A3. |
| UT-0411-08 | `remap_too_small_range_error` | `p0`, undersized `new_range = [200, 202)` (size 2 < 4 agents) | `remap_partition_ids(p0.clone(), undersized)` | Returns `Err(PartitionError::NewRangeTooSmall { partition_size: 4, range_size: 2 })`; `p0` is consumed (Rust ownership) but error path returns immediately before mutation. |
| UT-0411-09 | `remap_idempotent_when_range_equals_existing_ids` | `p0`, `new_range = [10, 14)` (same as current) | `let p = remap_partition_ids(p0.clone(), [10, 14))?` | `p` structurally equals `p0` (canonicalised). |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | `allocate_border_ids` called with `count = u32::MAX - cursor` exactly | Returns the maximal range; subsequent call with `count >= 1` returns `BorderIdSpaceExhausted`. |
| EC-2 | `remap_partition_ids` on an empty partition | Returns an empty partition; `new_range` may also be empty; no error. |
| EC-3 | `remap_partition_ids` with `new_range` strictly larger than partition size | Allowed (caller guarantees disjointness from other partitions); only the first `partition.agent_count()` ids in `new_range` are used; the remainder is unused but reserved. |

## Invariants asserted

- D3 (Border Completeness) — A3's fresh disjoint ranges prevent collision.
- D4 (ID Uniqueness) — A4's renumber is structure-preserving and preserves the per-net uniqueness invariant.
- I1, I2 (per-agent invariants) — preserved by construction (edge rewrite is total).

## ARG/DISC/REF citation

None directly. Underpins ARG-006 P11 (retained-snapshot consistency) by giving the coordinator a sound mechanism to renumber reclaimed partitions before union.

## Determinism notes

Pure synchronous; no async; deterministic.

## Cross-test dependencies

UT-0411-09 (idempotence) anchors a property used by EG-U7a (border_id rebase test) — keep the implementation total and deterministic so EG-U7a can rely on it.
