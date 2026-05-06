# TEST-SPEC-T9b: BorderGraph border-clean — Strategy B `BorderClean` (closes SC-005, opt-in policy)

**SPEC-22 §7.1 ID:** T9b.
**Owning task:** TASK-0482 (RecyclePolicy + GridConfig.recycle_under_delta + is_border_protected wiring).
**Parent spec:** SPEC-22 §3.1 R10b (Strategy B); §3.8 A10 (SPEC-19 §3.2 BorderGraph contract amendment); SC-005 closure.
**Type:** integration.
**Theory anchor:** ARG-005 INV-REC (delta border completeness — Strategy B satisfies the soundness condition by selectively excluding border-referenced IDs from recycle while permitting non-border recycle).

---

## Inputs / Fixtures

- A 2-partition delta-mode scenario with `GridConfig.recycle_under_delta == RecyclePolicy::BorderClean` (opt-in).
- **Canonical fixture (mirrors T9a + extension):** worker 0 owns IDs `[0, 100)`; coordinator's BorderGraph records a border at `AgentPort(47, 0)`.
- Pre-state of worker 0:
  - `agents[47] = Some(CON)`, `agents[50] = Some(DUP)`. Both have a principal-port partner consumed in round N.
  - `worker_0.net.recycle_policy = BorderClean`.
  - `worker_0.net.is_in_delta_round = true`.
  - **`worker_0.net.border_entries_shadow = Some({47})`** — only agent 47 is border-referenced; agent 50 is NOT.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-T9b-01 | `border_referenced_47_NOT_in_free_list` | pre-state | `worker_0.net.remove_agent(47)` | `!worker_0.net.free_list.contains(&47)`. (R10b Strategy B: border-referenced ID is NOT pushed.) |
| UT-T9b-02 | `non_border_50_IS_in_free_list` | continued | `worker_0.net.remove_agent(50)` | `worker_0.net.free_list.contains(&50)`. (R10b Strategy B: non-border IDs flow to free-list normally.) |
| UT-T9b-03 | `agents_47_slot_none_ports_disconnected` | post-removes | `worker_0.net.agents[47] == None` AND ports `[port_index(47, 0..3)]` all DISCONNECTED | confirmed (R10c: protected tombstone clearing identical to Strategy A; the difference is policy-side at the push). |
| UT-T9b-04 | `next_create_pops_50_not_47` | post-removes; pre-condition: free-list = `[50]` | `let id = worker_0.net.create_agent(ERA)` | `id == 50`. (R10b Strategy B: pop is allowed for non-border IDs.) |
| UT-T9b-05 | `bordergraph_invariance_post_round` | continue reduction; complete round N | for every `BorderState` in coordinator's `BorderGraph` referencing `AgentPort(id, _)`, `worker.agents[id].as_ref().map(|a| a.symbol)` is identical to round N's recorded symbol | confirmed. (G1 under delta mode preserved — the threat model in R10b is operationally closed by this test.) |
| UT-T9b-06 | `border_clean_with_pop_collision_re_pushes` | synthetic: pre-populate `border_entries_shadow = Some({50})` AND `free_list = [50]` (corrupted-state recovery); call `create_agent` | the create_agent's pop returns `Some(50)`, the `is_border_protected(50)` check fires, the implementation re-pushes 50 to free-list and falls through to `next_id` allocation | returned ID is the fresh `next_id`, NOT 50. The free-list still contains 50 post-fall-through. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | All freed IDs are border-referenced (worst case for Strategy B) | Free-list stays empty across the round; all freed IDs become protected tombstones. Behaviorally identical to Strategy A. |
| EC-2 | No freed IDs are border-referenced | Free-list grows freely; all recycles permitted. Behaviorally identical to non-delta mode. |
| EC-3 | `border_entries_shadow == None` under Strategy B (build_subnet did not populate) | `is_border_protected` returns `false` for every ID; behaves as Strategy A with `is_in_delta_round = false`. (Defensive default.) |
| EC-4 | After `reconstruct`, `border_entries_shadow` is cleared and protected_tombstones drained | Next round can recycle any previously-protected ID. Joint coverage with T9a UT-T9a-05. |

## Invariants asserted

- D2 (Border completeness).
- D3 (Cross-round border discovery).
- G1 (under delta mode — Strategy B variant).
- I3' stability subclause — border-referenced IDs preserved; non-border IDs free to recycle.
- ARG-005 INV-REC (delta border completeness — Strategy B variant).

## ARG/DISC/REF citation

- ARG-005 INV-REC.
- SPEC-19 §3.2 (BorderGraph contract — amended by §3.8 A10).

## Determinism notes

Same as T9a: in-process simulation with deterministic message ordering. The Strategy B selective check (consult `border_entries_shadow`) is a O(1) HashSet lookup; deterministic for any `border_entries_shadow` content.

UT-T9b-06's pop-then-re-push corner case is the load-bearing assertion for Strategy B's "or stored in a side-list for reuse after the next reconstruct" branch in §3.1 R10b. The implementer chooses between (a) immediate re-push to free-list (next pop tries again), (b) side-list stash drained at reconstruct. Both implementations are normative; the test asserts the OBSERVABLE behavior (the returned ID is fresh, not 50), not the implementation choice. Document the chosen path in TASK-0482's implementation comments.

## Cross-test dependencies

- T9a is the Strategy A counterpart; together they validate both code paths under `GridConfig.recycle_under_delta`.
- TEST-SPEC-0482 plumbing test covers the policy-enum primitives.
- The opt-in nature of Strategy B means UT-T9b-* are conditional on the test fixture using `RecyclePolicy::BorderClean` explicitly; do NOT inherit from the default `GridConfig`.
