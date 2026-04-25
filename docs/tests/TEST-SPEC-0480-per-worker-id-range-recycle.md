# TEST-SPEC-0480: Per-worker `id_range` defensive check on recycle (R10)

**SPEC-22 §7 ID:** T9 (spec-catalog) plus this plumbing file.
**Owning task:** TASK-0480.
**Parent spec:** SPEC-22 §3.1 R10; §3.3 R25.
**Type:** unit (debug-only assertion fence).

---

## Inputs / Fixtures

- A `Net` with `id_range = Some(0..100)` and `free_list` populated by `build_subnet`.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0480-01 | `id_range_field_is_pub_with_serde_skip` | compile-time | check the `Net.id_range` field has `#[serde(skip)]` and rkyv skip | confirmed via the derive surface. (Field is partition-context state, not on the wire.) |
| UT-0480-02 | `id_range_none_skips_assertion_in_release` | `Net::new()` (no id_range) | `create_agent(CON)` | succeeds; no debug assertion fires. (Non-distributed contexts unaffected.) |
| UT-0480-03 | `id_range_some_in_range_id_pop_succeeds` | net with `id_range = Some(0..100)`, `free_list = [50]` | `create_agent(CON)` | returns ID 50; no assertion fires. |
| UT-0480-04 | `id_range_some_traps_out_of_range_pop` (debug-only) | net with `id_range = Some(0..100)`, `free_list = [150]` (synthetic invalid state) | `create_agent(CON)` | `panic::catch_unwind` captures a panic with message "SPEC-22 R10 violation: popped id 150 not in partition range 0..100". |
| UT-0480-05 | `release_build_does_not_panic_on_out_of_range` (release-only) | UT-0480-04's setup; `cargo test --release` | `create_agent` | `debug_assert!` is dead-code-eliminated; the recycle proceeds (the violation is a bug elsewhere, not caught in release). The OBSERVABLE: `id == 150`. |
| UT-0480-06 | `id_range_set_by_build_subnet` | `build_subnet(...)` per TASK-0481 | post-call | the returned `Net` has `id_range == Some(partition.id_range.clone())`. (Joint with TEST-SPEC-0481.) |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | `id_range = Some(0..0)` (empty range) | Free-list cannot legitimately contain any ID (vacuous); any `create_agent` falls through to fresh allocation. |
| EC-2 | `id_range = Some(u32::MAX-1..u32::MAX)` (boundary) | Range contains exactly one ID; recycling that ID is in-range. |
| EC-3 | `id_range = Some(0..100)` + `next_id = 200` (`next_id` outside the range; pre-existing global state) | Defensive check only fires on POP, not on FRESH; `next_id` increment to 200 is allowed but the resulting ID 200 is outside the partition range — the implementation MUST guard this elsewhere (e.g., in the worker dispatch code) but TASK-0480's defensive assert does not catch it. (Documents the boundary of TASK-0480's responsibility.) |

## Invariants asserted

- R10 (per-worker ID range — defensive check).
- R25 (D4 preservation under I3').

## ARG/DISC/REF citation

- AC-011 (HVM4 static heap partitioning).

## Determinism notes

UT-0480-04 is `#[cfg(debug_assertions)]`-gated. UT-0480-05 is `#[cfg(not(debug_assertions))]`-gated. Both must be present and correctly cfg-gated.

Pure synchronous; no tokio.

## Cross-test dependencies

- T9 (spec-catalog) is the integration mirror.
- TEST-SPEC-0481 covers `build_subnet` populating `id_range` correctly.
- TEST-SPEC-0482 reuses the `id_range` field for RecyclePolicy::BorderClean.
