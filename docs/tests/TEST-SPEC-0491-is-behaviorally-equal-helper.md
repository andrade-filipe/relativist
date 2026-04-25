# TEST-SPEC-0491: `Net::is_behaviorally_equal` helper + R21 round-trip closure (closes SC-014)

**SPEC-22 §7 ID:** T8, T14 (spec-catalog) consume this primitive; plus this plumbing file.
**Owning task:** TASK-0491.
**Parent spec:** SPEC-22 §3.2 R21; SC-014 closure.
**Type:** unit.

---

## Inputs / Fixtures

- Pairs of `Net` values constructed to exercise the equality semantics across various differences.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0491-01 | `behaviorally_equal_returns_true_for_identical_nets` | two `Net::new()` instances | `n1.is_behaviorally_equal(&n2)` | `true`. |
| UT-0491-02 | `behaviorally_equal_returns_true_for_same_live_set_different_arena_len` | `n1.agents.len() == 5` (with trailing `None`); `n2.agents.len() == 3` (no trailing `None`); same live agents at IDs 0, 1 | helper | `true`. (R21 explicit: trailing slots ignored.) |
| UT-0491-03 | `behaviorally_equal_returns_false_for_different_live_set` | n1 has agent at ID 0; n2 has agent at ID 1 | helper | `false`. |
| UT-0491-04 | `behaviorally_equal_returns_false_for_different_freeport_redirects` | n1 has `freeport_redirects = {99 -> ...}`; n2 has `{}` | helper | `false`. (R21 full equality on freeport_redirects.) |
| UT-0491-05 | `behaviorally_equal_redex_queue_order_independent` | n1 has redex_queue `[(0, 1), (2, 3)]`; n2 has `[(2, 3), (0, 1)]` (same set, different order) | helper | `true`. (R21: queue compared up to ordering.) |
| UT-0491-06 | `behaviorally_equal_distinguishes_redex_queue_set` | n1 has `[(0, 1)]`; n2 has `[(0, 2)]` | helper | `false`. |
| UT-0491-07 | `behaviorally_equal_ignores_trailing_disconnected_ports` | n1.ports has 12 entries with last 3 DISCONNECTED; n2.ports has 9 entries; same live AgentPort entries | helper | `true`. |
| UT-0491-08 | `behaviorally_equal_distinguishes_live_port_targets` | n1.port[idx] == AgentPort(0, 1); n2.port[idx] == AgentPort(0, 2) | helper | `false`. |
| UT-0491-09 | `behaviorally_equal_distinguishes_root` | n1.root = Some(AgentPort(0, 0)); n2.root = None | helper | `false`. |
| UT-0491-10 | `behaviorally_equal_distinguishes_next_id` | n1.next_id = 5; n2.next_id = 7; same live set | helper | `false`. |
| UT-0491-11 | `behaviorally_equal_free_list_set_equality` | n1.free_list = `[1, 3]`; n2.free_list = `[3, 1]` (different order, same set) | helper | `true`. (R21: free-list compared as set.) |
| UT-0491-12 | `r21_round_trip_1_dense_sparse_dense_passes` | net with non-trivial state | `let net2 = net.to_sparse().to_dense(None); net.is_behaviorally_equal(&net2)` | `true`. |
| UT-0491-13 | `r21_round_trip_2_sparse_dense_sparse_full_eq` | sparse with non-trivial state | `let sparse2 = sparse.to_dense(None).to_sparse(); sparse == sparse2` | `true`. (Full structural `==` for SparseNet round-trip.) |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Both empty nets | helper returns `true`. |
| EC-2 | One net has port_index out of bounds (corrupted) | helper handles gracefully (e.g., short-circuit `false`); does not panic. (Defensive; corrupted state caught by `assert_invariants` elsewhere.) |
| EC-3 | Free-list contains an ID that is also `Some(_)` in agents (corrupted) | helper still returns either `true` or `false` based on the set comparison; the corruption is caught by `validate_free_list` separately. |

## Invariants asserted

- R21 (behavioral equality definition — closes SC-014).

## ARG/DISC/REF citation

- AC-001 (Haskell IC.Core baseline — equality semantics across representation switches).

## Determinism notes

The redex-queue comparison uses `HashSet<(AgentId, AgentId)>` projection (R21 verbatim "up to ordering"). Pure synchronous; no tokio. The helper is **the load-bearing closure for SC-014**.

## Cross-test dependencies

- T8, T14, T16 all use this helper.
- TEST-SPEC-0489 / TEST-SPEC-0490 cover the conversion primitives.
