# TEST-SPEC-T9: Distributed per-worker ID range compliance

**SPEC-22 §7.1 ID:** T9.
**Owning task:** TASK-0480 (per-worker id_range constraint), TASK-0481 (build_subnet populates per-partition free-list).
**Parent spec:** SPEC-22 §3.1 R10, R10a; §3.3 R25 (D4 preservation under I3'); SPEC-04 §4.5 (build_subnet).
**Type:** integration.
**Theory anchor:** ARG-002 (border bijection — partition disjointness rationale); AC-011 (HVM4 static heap partitioning).

---

## Inputs / Fixtures

- A net of 200 agents (IDs 0-199) constructed via `(0..200).for_each(|_| net.create_agent(CON))` plus a deterministic wire pattern that creates redexes in both partitions.
- Pre-split removes: `remove_agent(50)`, `remove_agent(75)`, `remove_agent(150)`, `remove_agent(175)` ⇒ 4 freed slots, 2 in each partition.
- Split into 2 partitions:
  - Partition 0: `id_range = 0..100`, owns IDs `[0, 50, 75, 100)` minus the freed ones.
  - Partition 1: `id_range = 100..200`, owns IDs `[100, 150, 175, 200)` minus the freed ones.
- After `build_subnet`:
  - Partition 0's `Net.id_range == Some(0..100)`, `free_list ⊆ {50, 75}` (in some LIFO order).
  - Partition 1's `Net.id_range == Some(100..200)`, `free_list ⊆ {150, 175}`.
- Run reduction independently on each partition (single-worker per partition; no coordinator interaction).

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-T9-01 | `partition_0_recycles_only_in_range` | partition 0 post-build_subnet, with redexes that trigger ≥1 commutation | reduce_all on partition 0; collect every ID returned by `create_agent` (instrumented or via post-state diff) | every recycled ID is in `[0, 100)`. (Defensive `debug_assert` from TASK-0480 catches any violation; release builds rely on TASK-0481's correct construction.) |
| UT-T9-02 | `partition_1_recycles_only_in_range` | partition 1 post-build_subnet | same | every recycled ID is in `[100, 200)`. |
| UT-T9-03 | `partition_0_free_list_is_subset_of_range` | partition 0 post-build_subnet, pre-reduction | check every ID in `partition_0.free_list` | each is in `[0, 100)`. |
| UT-T9-04 | `partition_1_free_list_is_subset_of_range` | partition 1 post-build_subnet, pre-reduction | check every ID in `partition_1.free_list` | each is in `[100, 200)`. |
| UT-T9-05 | `partition_0_does_not_recycle_into_partition_1_range` | partition 0; pre-reduction inject test ID 150 into partition_0.free_list (synthetic, debug-only) | the next `create_agent` on partition 0 | the defensive `debug_assert` at TASK-0480 triggers a panic with message containing "SPEC-22 R10 violation". (Debug-only test.) |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Single-partition (whole-net) context with `id_range == None` | Defensive check is bypassed (R10 only applies under partition context). Test confirms non-distributed contexts are unaffected. |
| EC-2 | 4-partition split with id_ranges `[0,50), [50,100), [100,150), [150,200)` | Each partition's free-list is confined to its range. D4 disjointness preserved. |
| EC-3 | Pre-split removes happen INSIDE one partition's range only (e.g., remove only IDs 50, 75 — both in partition 0) | Partition 0's free-list = `{50, 75}`; partition 1's free-list is empty. (Confirms that removing in one partition does NOT leak into another.) |
| EC-4 | `id_range.end > arena_len` (last partition's range extends past arena) | `build_subnet` clamps to `arena_len.min(id_range.end)`; no out-of-bounds panic; free-list contains only existing-slot indices. (TASK-0481 acceptance criterion.) |

## Invariants asserted

- D4 (ID Uniqueness After Distributed Reduction — partition free-list disjoint per range).
- R10 (per-worker id range — defensive check verified in UT-T9-05).
- R10a (build_subnet populates partition free-list within range — verified in UT-T9-03/04).
- R25 (D4 preservation under I3' — UT-T9-01..04 confirm).

## ARG/DISC/REF citation

- ARG-002 P2 (split/reduce/remap/merge correctness — partition disjointness is a precondition for the merge identity).
- AC-011 (HVM4 static heap partitioning — same per-worker arena boundary).

## Determinism notes

`build_subnet` is pure synchronous; no tokio. The walk-and-push order in TASK-0481 (ascending iteration) is deterministic. Reduction within each partition is single-threaded. The test does NOT involve cross-partition message passing (that's T16's territory) — UT-T9-01..05 are local-only.

UT-T9-05 is debug-only: in release builds the defensive assertion is dead-code-eliminated. Document this in the test source with `#[cfg(debug_assertions)]`.

## Cross-test dependencies

- TEST-SPEC-0480 covers the defensive assertion primitive; TEST-SPEC-0481 covers the build_subnet free-list construction primitive. T9 is the integration-level joint test.
- T9a / T9b are the BorderGraph-aware extensions for delta mode; T9 is the v1-mode (non-delta) baseline.
- T16 covers the full split → reduce → merge G1 round-trip; T9 is a per-partition correctness slice.
