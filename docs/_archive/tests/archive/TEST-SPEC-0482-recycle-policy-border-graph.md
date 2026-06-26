# TEST-SPEC-0482: `RecyclePolicy` enum + `GridConfig.recycle_under_delta` + `is_border_protected` wiring (R10b/R10c)

**SPEC-22 §7 ID:** T9a, T9b (spec-catalog) plus this plumbing file.
**Owning task:** TASK-0482.
**Parent spec:** SPEC-22 §3.1 R10b, R10c; §3.8 A10; SC-005 closure.
**Type:** unit + integration.

---

## Inputs / Fixtures

- A `GridConfig` with `recycle_under_delta: RecyclePolicy::DisableUnderDelta` (default).
- A `Net` with the new fields:
  - `border_entries_shadow: Option<HashSet<AgentId>>`
  - `protected_tombstones: Option<HashSet<AgentId>>` (debug-only)
  - `recycle_policy: RecyclePolicy`
  - `is_in_delta_round: bool`

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0482-01 | `default_policy_is_disable_under_delta` | `let cfg: GridConfig = Default::default();` | `cfg.recycle_under_delta` | `== RecyclePolicy::DisableUnderDelta`. |
| UT-0482-02 | `recycle_policy_enum_derives_serde` | `RecyclePolicy::BorderClean` | bincode round-trip | preserved. |
| UT-0482-03 | `is_border_protected_returns_false_in_pure_net_context` | `Net::new()` (no `border_entries_shadow`) | `net.is_border_protected(any_id)` | `false`. (Default behavior; matches TASK-0473 stub.) |
| UT-0482-04 | `is_border_protected_returns_true_for_border_id` | net with `border_entries_shadow = Some({47})` | `net.is_border_protected(47)` | `true`. |
| UT-0482-05 | `is_border_protected_returns_false_for_non_border_id` | same | `net.is_border_protected(50)` | `false`. |
| UT-0482-06 | `strategy_a_skips_pop_during_delta_round` | `recycle_policy = DisableUnderDelta`, `is_in_delta_round = true`, `free_list = [50]` | `create_agent(CON)` | falls through to `next_id` allocation; returned ID is `next_id` (NOT 50). Free-list still `[50]` post-call. |
| UT-0482-07 | `strategy_a_pops_when_not_in_delta_round` | same fixture, `is_in_delta_round = false` | `create_agent(CON)` | returns 50 (free-list pop succeeds). |
| UT-0482-08 | `strategy_b_pops_non_border_id` | `recycle_policy = BorderClean`, `is_in_delta_round = true`, `free_list = [50]`, `border_entries_shadow = Some({47})` | `create_agent(CON)` | returns 50 (50 is NOT in border_entries_shadow). |
| UT-0482-09 | `strategy_b_re_pushes_border_id_on_pop_collision` | `recycle_policy = BorderClean`, `free_list = [47]`, `border_entries_shadow = Some({47})` | `create_agent(CON)` | falls through (re-push 47 OR stash; the OBSERVABLE: returned ID is fresh, NOT 47). |
| UT-0482-10 | `r10c_protected_tombstone_on_remove_agent` | net with `is_in_delta_round = true`, `border_entries_shadow = Some({47})`; `agents[47] == Some(CON)` | `net.remove_agent(47)` | `agents[47] == None`; ports DISCONNECTED; free-list does NOT contain 47; protected_tombstones (debug) contains 47. |
| UT-0482-11 | `protected_tombstone_drained_at_reconstruct` | continued from UT-0482-10; trigger `reconstruct` | post-reconstruct | `protected_tombstones` is empty (or `None`); `free_list.contains(&47) == true` (drained back). |
| UT-0482-12 | `non_distributed_context_unaffected_by_recycle_policy` | `Net::new()` (border_entries_shadow == None) | full create-remove-create cycle | behaves as v1: free-list pop works; no protection applies. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Strategy A with empty free-list + delta round | `create_agent` falls through to fresh; same as v1 fresh-only. |
| EC-2 | Strategy B with empty `border_entries_shadow` | Behaves identically to non-delta mode (no protection). |
| EC-3 | Multiple border IDs in free-list under Strategy B | Each pop iterates: pop, check, re-push if border, repeat until non-border found OR free-list empty. The test asserts the iteration terminates and returns either a non-border ID or fresh. |
| EC-4 | Recycle policy changed mid-execution (Strategy A → Strategy B) | Implementation behavior depends on whether the worker dispatch loop re-reads the policy per call OR caches it. Document the chosen path; the test asserts the OBSERVABLE matches. |

## Invariants asserted

- R10b (Strategy A and Strategy B both honored).
- R10c (protected tombstone semantics).
- D2, D3, G1 under delta mode (preserved).
- ARG-005 INV-REC.
- §3.8 A10 (SPEC-19 BorderGraph contract amendment).

## ARG/DISC/REF citation

- ARG-005 (Delta Border Completeness — INV-REC).
- SPEC-19 §3.2 (amended by §3.8 A10).

## Determinism notes

In-process simulation; no live tokio sockets needed. The `is_in_delta_round` toggle and `border_entries_shadow` population are synchronous test-side setup. The `reconstruct` step is invoked directly. If any portion of the production code is async, use `#[tokio::test(flavor = "current_thread")]` with deterministic single-runtime.

## Cross-test dependencies

- T9a / T9b are the spec-catalog mirrors.
- TEST-SPEC-0473 covers the `is_border_protected` stub default.
- TEST-SPEC-0480 covers the `id_range` field reused here.
- Coordinate with TEST-SPEC-0415 (SPEC-20 GridConfig fields) on the field-list ordering and serde-default attributes.
