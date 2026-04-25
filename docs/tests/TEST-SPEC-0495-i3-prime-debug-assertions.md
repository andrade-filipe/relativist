# TEST-SPEC-0495: I3' uniqueness debug assertions in `remove_agent` / `create_agent` (R24, R25, R27)

**SPEC-22 §7 ID:** T7 (spec-catalog) joint coverage; plus this plumbing file.
**Owning task:** TASK-0495.
**Parent spec:** SPEC-22 §3.3 R24, R25, R27.
**Type:** unit (mostly debug-mode fences) + integration (T7 trace).

---

## Inputs / Fixtures

- Net fixtures triggering each of the four R27 assertion families.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0495-01 | `r27_family_1_post_remove_agent_recycle` (debug-only) | net with agent `id` removed (R10b NOT triggered, regular recycle) | the assertion family at end of `remove_agent` recycle branch | passes: free-list contains `id`, `agents[id] == None`, port slots DISCONNECTED. |
| UT-0495-02 | `r27_family_1_catches_violation` (debug-only) | synthetic state: agent removed but agents[id] left as Some (manually mutated test-only) | call the assertion helper | panic; message cites R27 family 1. |
| UT-0495-03 | `r27_family_2_post_remove_agent_protected_tombstone` (debug-only) | net with `is_in_delta_round = true`, `border_entries_shadow = Some({id})`; remove agent `id` | assertion family at end of protected-tombstone branch | passes: agents[id] == None, ports DISCONNECTED, ID NOT in free_list, ID IS in protected_tombstones shadow. |
| UT-0495-04 | `r27_family_3_post_create_agent_recycle` (debug-only) | net with non-empty free_list; call create_agent | assertion family at end of recycle branch | passes: ID no longer in free_list, agents[id] == Some, free_list has no duplicates, returned ID is NOT in protected_tombstones shadow. |
| UT-0495-05 | `r27_family_3_catches_protected_tombstone_recycle` (debug-only) | synthetic: free_list contains an ID that is also in protected_tombstones (corrupted) | create_agent that pops that ID | panic; message cites R27 family 3. |
| UT-0495-06 | `r27_family_4_no_free_list_port_refs_passes` (debug-only) | post-reduce_all on a non-trivial net (e.g., Church(3)+Church(2) — joint with T7) | `net.assert_no_free_list_port_refs()` | does not panic. |
| UT-0495-07 | `r27_family_4_catches_synthetic_violation` (debug-only) | net with a synthetic port slot pointing to a free-list ID (test-only mutation) | `net.assert_no_free_list_port_refs()` | panic; message cites SPEC-22 R7 / R27. |
| UT-0495-08 | `debug_check_invariants_combines_all_four_families` | post-reduction net | `net.debug_check_invariants()` (helper composing all 4 families) | passes for valid net; panics for any of the 4 violation states. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Release build: all assertions are dead-code-eliminated | UT-0495-01..08 are `#[cfg(debug_assertions)]`-gated. Release-mode tests confirm zero overhead. |
| EC-2 | Empty free-list: family 4 trivially passes | confirmed. |
| EC-3 | Family 4 called per-step in a 1000-step reduction | Performance: O(ports.len()) per call ⇒ O(n²) over the reduction. Recommend calling once at end of `reduce_all`, NOT per-step (per TASK-0495 notes). |

## Invariants asserted

- R24, R25, R27 (all four families).
- I3' uniqueness, T1, I1, I2 (preserved on every state transition).

## ARG/DISC/REF citation

- None direct.

## Determinism notes

All assertions are deterministic given a fixed Net state. `HashSet::contains` is deterministic for any input. Pure synchronous; no tokio. All tests are `#[cfg(debug_assertions)]`-gated.

## Cross-test dependencies

- T7 (spec-catalog) is the integration-level mirror with Church arithmetic.
- TEST-SPEC-0497 covers the SPEC-03 in-rule audit (R27a) — separate but related.
