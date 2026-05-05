# TEST-SPEC-T6: Commutation recycling (CON-DUP — 2 removes + 4 creates ⇒ 2 recycles + 2 fresh)

**SPEC-22 §7.1 ID:** T6.
**Owning task:** TASK-0472 (create_agent), TASK-0473 (remove_agent), exercised via SPEC-03 CON-DUP rule.
**Parent spec:** SPEC-22 §3.1 R3, R5; §3.8 A3 amended R10 (`f = k - r` accounting); §4.7.
**Type:** integration.
**Theory anchor:** REF-002 (Lafont 1997 — commutation rule γδ); AC-006 (HVM2 commutation arena allocation).

---

## Inputs / Fixtures

- Fresh `Net::new()`.
- Build a single CON-DUP active pair:
  - `let con = net.create_agent(Symbol::CON)` (ID 0).
  - `let dup = net.create_agent(Symbol::DUP)` (ID 1).
  - `net.connect(AgentPort(con, 0), AgentPort(dup, 0))` — principal-principal.
- Pre-reduction state: `agents.len() == 2`, `free_list == []`, `next_id == 2`, `redex_queue == [(0, 1)]`.
- Reduce one step: `net.reduce_step()` (or `reduce_all()` since there's only one redex initially).

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-T6-01 | `con_dup_fires_creates_4_agents` | pre-state | reduce | post-state has 4 live agents (2 new CONs + 2 new DUPs per the commutation rule). `count_live_agents() == 4`. |
| UT-T6-02 | `con_dup_recycles_2_of_4_slots` | pre-state | reduce | `net.next_id == 4` — exactly 2 fresh allocations (`f = 4 - 2 = 2`); the original IDs 0 and 1 (now `None` after the remove of CON and DUP) were recycled to 2 of the 4 newly-created agents. |
| UT-T6-03 | `con_dup_free_list_empty_post_fire` | pre-state | reduce | `net.free_list.is_empty()` — both freed slots were drained by the 4 successive `create_agent` calls. |
| UT-T6-04 | `arena_len_is_4_post_fire` | pre-state | reduce | `net.agents.len() == 4` (grew from 2 to 4 to accommodate the 2 fresh allocations). |
| UT-T6-05 | `con_dup_returned_ids_satisfy_i3_prime` | pre-state | reduce; collect the 4 returned IDs from the rule's create_agent calls (instrumented or inferred from post-state) | the 4 IDs are pairwise distinct AND `agents[id].is_some()` for all 4 (uniqueness, NOT monotonicity — see T7a for the explicit non-monotonicity check). |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | CON-DUP fires twice in sequence (build 2 CON-DUP pairs at IDs 0/1 and 2/3) | After 2 fires: 8 live agents, `next_id == 8`, free-list empty. Each fire individually has `f = 2` increments. |
| EC-2 | DUP-DUP active pair (annihilation, not commutation) | `next_id` unchanged (annihilation creates 0 agents); free_list grows by 2; count_live drops by 2. (This is T5's territory; here the test acts as a control for T6's commutation specificity.) |
| EC-3 | CON-DUP after a partial pre-population: free_list = `[100, 200]` injected, `next_id = 1000` | The 4 create_agent calls in the rule pop 100 then 200 (LIFO), then allocate 1000 and 1001 fresh. `next_id` becomes 1002. (Joint coverage with T7a.) |

## Invariants asserted

- R3 (free-list-pop OR next_id increment).
- §3.8 A3 (R10 amended `f = k - r` accounting verified by UT-T6-02).
- I3' (uniqueness of the 4 returned IDs — UT-T6-05).
- T1 (Port Linearity — preserved across the rule via `connect` calls on the 4 new agents).
- §4.7 intra-rule order non-determinism note: this test does NOT assert *which* of the 4 IDs are recycled (per SC-019 forbidden assertion); only the COUNT (2 recycled, 2 fresh).

## ARG/DISC/REF citation

- REF-002 (Lafont 1997 §3 — γδ commutation rule definition).
- AC-006 (HVM2 commutation rationale — same +2 agent pattern).

## Determinism notes

CON-DUP commutation is deterministic in COUNT but the order of `create_agent` calls within the rule body is implementation-defined (per SPEC-22 §4.7 / SC-019). The test asserts only the count and the post-state set — never which specific IDs end up at which roles in the resulting topology. Pure single-threaded reduction; no tokio.

## Cross-test dependencies

- T7a is the explicit non-monotonicity test (forbids `assert!(new_id > old_max_id)` in `src/reduction/`).
- T5 covers the annihilation-only counterpart.
- The 4-agent commutation topology is invariant content for T7 (Church arithmetic test).
