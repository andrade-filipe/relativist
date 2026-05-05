# TEST-SPEC-T7a: CON-DUP fire under a partial free-list (closes SC-010)

**SPEC-22 §7.1 ID:** T7a.
**Owning task:** TASK-0497 (SPEC-03 reduction-engine assertion audit).
**Parent spec:** SPEC-22 §3.3 R27a; §3.8 A6 (SPEC-03 §4.3 amendment); SC-010 closure.
**Type:** integration (exercises SPEC-03 CON-DUP rule body under a constructed pre-state).
**Theory anchor:** REF-002 (Lafont 1997 — γδ commutation); AC-006 (HVM2 +2 commutation rationale).

---

## Inputs / Fixtures

- Fresh `Net::new()`.
- **Pre-state construction (precise):**
  1. Create 4 placeholder CONs at IDs 0, 1, 2, 3.
  2. Remove CONs at IDs 0 and 2 ⇒ `free_list = [0, 2]` (in push order; LIFO top is 2).
  3. Build the active pair: create `con` at ID 4 (fresh, since free-list pop will take recycled IDs first — but we construct via `create_agent`, so the created CON actually goes to the LIFO top of the free-list, which is ID 2; this is a problem for setup).
  - **Workaround (canonical setup):** keep the 4 placeholder agents in place and connect the active pair using IDs 1 and 3 (still live). Then `remove_agent(1)` and `remove_agent(3)` are NOT done — instead, inject 2 IDs directly into `free_list` via a test-only helper or via a second remove pair that does not interfere with the active pair. The simplest deterministic setup is:
     - Create 4 CONs at IDs 0, 1, 2, 3 using fresh allocations.
     - Connect IDs 4 (CON) and 5 (DUP) — the active pair — created AFTER the 4 placeholders.
     - Remove CONs at IDs 0 and 2 ⇒ `free_list = [0, 2]`.
     - `connect(AgentPort(4, 0), AgentPort(5, 0))`.
  - Pre-fire state: `agents.len() == 6`; agents `[None, Some, None, Some, Some(CON), Some(DUP)]`; `free_list == [0, 2]`; `next_id == 6`.
- **Reduction step:** `net.reduce_step()` (fires the CON-DUP rule once).

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-T7a-01 | `condup_fire_returns_4_unique_ids` | pre-fire state | reduce_step; capture the 4 IDs assigned to the 4 new agents (e.g., via instrumentation in test build OR by inspecting the post-state and identifying the 4 newly-occupied slots) | the 4 IDs are pairwise distinct (multiset cardinality 4); `agents[id].is_some()` for all 4. |
| UT-T7a-02 | `condup_4_ids_NOT_monotonic_under_partial_free_list` | same | same | the 4 returned IDs MUST include both recycled IDs (0 and 2) AND fresh IDs (the next two `next_id` values, 6 and 7). The set is `{0, 2, 6, 7}`. The IDs are NOT in monotonic order across the rule body — explicitly: `{0, 2}` are smaller than the originals `{4, 5}` of the consumed pair, and `{6, 7}` are larger. SC-019 forbids asserting *which slot* gets which of the 4 logical roles; this test only asserts the SET equality. |
| UT-T7a-03 | `next_id_increment_is_2_post_fire` | same | same | `net.next_id == 8` (was 6; `f = k - r = 4 - 2 = 2`). §3.8 A3 verified at the rule level. |
| UT-T7a-04 | `free_list_empty_post_fire` | same | same | `net.free_list.is_empty()` — both recycled IDs were drained by the rule's create_agent calls. |
| UT-T7a-05 | `release_build_fires_no_assertion_panic` | same; **run with `cargo test --release`** | same | the reduction completes normally; no `assert!(new_id > old_max_id)` pattern fires (any leftover monotonicity assertion in `src/reduction/` would have been caught here). |
| UT-T7a-06 | `debug_build_fires_no_assertion_panic` | same; **run with `cargo test`** (debug) | same | the reduction completes normally; only allowed assertion patterns (R27a allowlist) execute and pass. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Free-list has exactly 4 entries (full coverage of the 4 creates) — all 4 IDs come from recycle | `next_id` unchanged across the fire (`f = k - r = 0`). The 4 returned IDs are the LIFO-popped 4 free-list entries. |
| EC-2 | Free-list has 0 entries — all 4 IDs are fresh | `next_id` increments by 4. The 4 returned IDs are `{6, 7, 8, 9}`. (Identical to T6 EC-2.) |
| EC-3 | Free-list has 1 entry — 1 recycle + 3 fresh | `next_id` increments by 3. (Documents the partial-coverage path.) |

## Invariants asserted

- I3' (uniqueness — UT-T7a-01).
- §3.8 A6 (SPEC-03 reduction assertion language is I3'-compatible — UT-T7a-05/06 verify no forbidden monotonicity assertion fires).
- §4.7 intra-rule order non-determinism note (SC-019) — UT-T7a-02 honors the forbidden-assertion boundary.
- R27a (SPEC-03 in-rule assertion audit — closes SC-010).

## ARG/DISC/REF citation

- REF-002 (γδ commutation — the load-bearing rule under recycling).
- AC-006 (HVM2 +2 commutation arena pattern — confirms HVM2 also recycles in this position).

## Determinism notes

**Critical:** UT-T7a-02 asserts the **set** of returned IDs, NOT the assignment of IDs to specific roles in the resulting topology. Per SPEC-22 §4.7 (SC-019), tests asserting *which* recycled ID gets *which* of the 4 logical roles within the rule are FORBIDDEN. The implementation may assign in any order; the rule body's `create_agent` call sequence is an internal detail. The test's set-cardinality + set-equality assertions are confluence-safe and order-independent.

UT-T7a-05 and UT-T7a-06 are the load-bearing assertions for SC-010 closure: they require the test to be run under BOTH `cargo test --release` AND `cargo test` (debug) and pass in both — proving that no `assert!(new_id > old_max)` pattern remains in `src/reduction/`. Document this as a CI requirement in TASK-0497's acceptance criteria; the test itself is single-run but the meta-assertion is dual-config.

## Cross-test dependencies

- Builds on T6's setup (CON-DUP active pair).
- Joint coverage with TASK-0497's optional meta-test (`audit_no_forbidden_assertion_patterns_in_reduction` — grep at test time).
- Uses T7's `assert_no_free_list_port_refs` helper as a smoke check post-fire.
