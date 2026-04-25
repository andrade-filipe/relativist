# TEST-SPEC-0474: Free-list no-duplicates invariant (closes SC-018)

**SPEC-22 §7 ID:** T10 (spec-catalog) plus this plumbing file.
**Owning task:** TASK-0474.
**Parent spec:** SPEC-22 §3.1 R5, R6.
**Type:** unit (mix of debug-fence assertion test and LIFO smoke).

---

## Inputs / Fixtures

- Fresh `Net::new()` with test-only access to `Net.free_list` (or via `#[cfg(test)]` accessor).

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0474-01 | `pop_returns_most_recently_pushed` | direct `net.free_list.push(7); net.free_list.push(11);` | `net.free_list.pop()` | `Some(11)`. (R5 LIFO smoke.) |
| UT-0474-02 | `duplicate_push_via_remove_agent_triggers_debug_assert` (debug-only) | net with `agents[7] == None` AND `free_list = [7]` already (synthetic — R6 violation candidate); call `remove_agent` on a different agent that re-attempts `free_list.push(7)` via test-only injection | the `debug_assert!(!self.free_list.contains(&id))` in `remove_agent` panic | `panic::catch_unwind` captures the panic; the message contains "free-list" or the `debug_assert` text. (R6 closure of SC-018.) |
| UT-0474-03 | `release_build_compiles_assertion_out` (release-only) | same as UT-0474-02; build with `cargo test --release` | run | `debug_assert!` is dead-code-eliminated; the duplicate push silently succeeds (the duplicate is left in the Vec, which is acceptable in release per the SHOULD nature of release-mode no-dup enforcement). The free-list state post-test reflects the duplicate; the test asserts no panic. |
| UT-0474-04 | `shadow_consistency_after_push_pop_cycle` (CONDITIONAL — only if `free_list_shadow: HashSet<AgentId>` was adopted by DEVELOPER per TASK-0474 OPTIONAL acceptance criterion) | net with shadow active; push `[3, 7, 11, 13, 17]`, pop them all | shadow is empty after drain; len matches Vec at every intermediate state | confirmed. |
| UT-0474-05 | `shadow_kept_in_sync_under_remove_agent_recycle` (CONDITIONAL on shadow) | sequence of `create_agent` + `remove_agent` + `create_agent` (full cycle) | the shadow's `contains` and the Vec's `contains` agree at every step | confirmed. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | The `Vec::contains` cost in debug builds at large free-list (>10K entries) | If shadow is NOT adopted, debug-mode tests slow down with O(n²) cost across 10K push events. If shadow IS adopted, debug-mode tests stay O(n) total. The DEVELOPER's adoption decision is gated on this stress-test result per TASK-0474 notes. |
| EC-2 | `Net::clone` with shadow active | Shadow is also cloned (`Clone` derive flows through if the shadow is a regular field; OR the shadow is rebuilt from the Vec on clone — DEVELOPER decides). Test asserts: `clone.free_list.contains(id)` matches `clone.free_list_shadow.contains(id)` for every `id` in either. |

## Invariants asserted

- R5 (LIFO ordering — UT-0474-01).
- R6 (no duplicates — UT-0474-02 closes SC-018).

## ARG/DISC/REF citation

- None direct.

## Determinism notes

UT-0474-02 uses `panic::catch_unwind` and is debug-only (`#[cfg(debug_assertions)]`). UT-0474-03 is release-only (or `#[cfg(not(debug_assertions))]`). UT-0474-04 / UT-0474-05 are conditional on the shadow being adopted at Stage 3 — the DEVELOPER decides; the TEST-SPEC documents both branches.

Pure synchronous; no tokio.

## Cross-test dependencies

- T10 (spec-catalog) is the integration-level mirror; this plumbing test is the primitive.
- TEST-SPEC-0473 covers the push-site setup.
- TEST-SPEC-0495 R27 family (1) verifies the post-`remove_agent` recycle invariant including no-duplicate.
