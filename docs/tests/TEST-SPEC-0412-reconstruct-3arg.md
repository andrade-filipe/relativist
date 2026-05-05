# TEST-SPEC-0412: `reconstruct` 3-argument amendment (SPEC-19 A8)

**SPEC-20 §7 ID:** none direct (transitively exercised by EG-I3-delta, EG-U7c).
**Owning task:** TASK-0412.
**Parent spec:** SPEC-19 R38 (amended via SPEC-20 §3.8 A8).
**Type:** unit.

---

## Inputs / Fixtures

- An existing SPEC-19 test fixture (`bg`, `survivors`) that previously called `reconstruct(border_graph, surviving_partitions)`. Reuse the simplest / canonical SPEC-19 fixture from existing tests.
- A small reclaimed partition `r0` constructed via `remap_partition_ids` (TASK-0411) so its agent ids are disjoint from `survivors` ids.
- A second reclaimed partition `r1` likewise disjoint.
- A "collision" reclaimed partition that **violates** the disjointness precondition (used only for the debug-only panic test).

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0412-01 | `reconstruct_empty_reclaimed_matches_legacy` | existing SPEC-19 fixture (`bg`, `survivors`); empty `reclaimed = Vec::new()` | `let n_new = reconstruct(&bg, survivors.clone(), Vec::new());` and `let n_old = legacy_reconstruct(&bg, survivors.clone())` (or, if there is no legacy alias, the pre-amendment baseline captured as a structural canonicalised expectation) | `canonicalise(n_new) == canonicalise(n_old)` — bit-exact for all SPEC-19 fixtures. Test parameterises over at least 3 distinct SPEC-19 fixtures to anchor regression. |
| UT-0412-02 | `reconstruct_with_one_reclaimed_partition` | `bg`, `survivors`, `reclaimed = vec![r0]` | `let n = reconstruct(&bg, survivors, reclaimed)` | `n.agent_count() == survivors_total + r0.agent_count()`; every `AgentId` from both sets appears exactly once. |
| UT-0412-03 | `reconstruct_with_multiple_reclaimed_partitions` | `bg`, `survivors`, `reclaimed = vec![r0, r1]` (both disjoint) | same | `n.agent_count() == survivors_total + r0.count + r1.count`; multisets union; no duplication. |
| UT-0412-04 | `reconstruct_panics_on_overlapping_reclaimed_agent_ids_debug_build` | `bg`, `survivors`, `reclaimed = vec![colliding_partition]` whose ids overlap with `survivors`; under `#[cfg(debug_assertions)]` only | `panic::catch_unwind(\|\| reconstruct(&bg, survivors, reclaimed))` | Panic fires; payload mentions `SPEC-20 A7` (since the disjointness precondition is `Net::union`'s) OR `SPEC-20 A8` (since A8 is the caller surface). Either is acceptable — assert at least one of the two anchors appears. |
| UT-0412-05 | `reconstruct_with_reclaimed_preserves_border_completeness` | fixture with surviving partitions + 1 reclaimed; the reclaimed has its own `FreePort(bid)` entries | `let n = reconstruct(&bg, survivors, reclaimed)` | After reconstruction, all border references in `n` resolve into actual agents (no dangling FreePorts). D3 PRESERVED. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | `survivors.is_empty()` and `reclaimed = vec![r0]` | Equivalent to "all partitions are reclaimed"; result is reconstructible from `r0` alone (subject to BorderGraph consistency). |
| EC-2 | Both vectors empty | Equivalent to legacy `reconstruct(&bg, vec![])`; allowed if SPEC-19 R38 currently allows it. |
| EC-3 | Reclaimed partitions present but BorderGraph references only survivors | `reconstruct` succeeds; reclaimed partitions become structurally disconnected components in the result (this is the caller's concern; A8 does not enforce connectivity). |

## Invariants asserted

- D3 (Border Completeness) — preserved; reclaimed FreePorts are honored if BorderGraph references them.
- D4 (ID Uniqueness) — preserved by caller precondition.

## ARG/DISC/REF citation

None directly. Underpins ARG-006 P11 (retained-snapshot consistency).

## Determinism notes

Pure synchronous; deterministic; no async.

## Cross-test dependencies

- Depends on TASK-0410 (`Net::union`) and TASK-0411 (`remap_partition_ids`) being implemented first.
- UT-0412-01 anchors the "zero regression for legacy SPEC-19 callers" guarantee — must NOT regress any existing SPEC-19 test.
