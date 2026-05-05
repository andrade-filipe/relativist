# TEST-SPEC-T1: Basic free-list recycling

**SPEC-22 §7.1 ID:** T1.
**Owning task:** TASK-0472 (create_agent free-list pop). Joint coverage with TASK-0473 (remove_agent push).
**Parent spec:** SPEC-22 §3.1 R3, R4, R5; §4.2 (create_agent body).
**Type:** unit.
**Theory anchor:** REF-002 (Lafont 1997 — IC slot semantics); AC-006 (HVM2 flat-array rationale informs the LIFO/locality argument).

---

## Inputs / Fixtures

- A fresh `Net::new()` net.
- `Symbol::CON` and `Symbol::DUP` constructors (any concrete subset of SPEC-02's symbol set).
- Pre-state for the recycle moment:
  - `agents = [Some(CON_0), Some(CON_1), Some(CON_2)]` (post create x3).
  - `free_list = []`.
- Mid-state after removing the middle agent:
  - `agents = [Some(CON_0), None, Some(CON_2)]`.
  - `free_list = [1]`.
- Post-state after creating a new ERA agent:
  - `agents = [Some(CON_0), Some(ERA_1), Some(CON_2)]` — the ID `1` is reused, now bound to ERA.
  - `free_list = []`.
  - `next_id == 3` (unchanged from pre-state).

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-T1-01 | `recycle_reuses_freed_middle_id` | 3 CON agents created (IDs 0, 1, 2); middle (ID 1) removed | `let new_id = net.create_agent(Symbol::ERA)` | `new_id == 1`. |
| UT-T1-02 | `recycle_target_slot_is_some_with_new_symbol` | same | same | `net.agents[1].unwrap().symbol == Symbol::ERA`. |
| UT-T1-03 | `recycle_leaves_free_list_empty` | same | same | `net.free_list.is_empty() == true`. |
| UT-T1-04 | `recycle_does_not_increment_next_id` | same; `let nid_before = net.next_id` (== 3 after the 3 creates) | same | `net.next_id == nid_before` (i.e., 3). The ID `1` came from the free-list pop, not from `next_id`. |
| UT-T1-05 | `recycle_does_not_grow_arena` | same; `let arena_len_before = net.agents.len()` | same | `net.agents.len() == arena_len_before` (R4(c): the recycled slot already exists within bounds). |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | First and last agent (IDs 0 and 2) also removed before the new create | After 3 removes the free-list is `[1, 2, 0]` or any permutation respecting the remove order; the next `create_agent` returns the LIFO-top. (Strict LIFO assertion is T2's job; here we only assert reuse and slot/symbol correctness.) |
| EC-2 | Recycled ID's port slots have stale data from before the remove | Defensive `debug_assert` in `create_agent` recycle path catches this; in release builds the `remove_agent` disconnect loop guarantees DISCONNECTED. Test passes in both debug and release. |
| EC-3 | Repeated remove+create of the same logical slot in a tight loop (1000 iterations) | `net.agents.len()` stays bounded; `next_id` does not increase past 3; all 1000 reuses succeed. (Stress version of UT-T1-04/05.) |

## Invariants asserted

- I3' (Uniqueness of AgentIds) — only one live agent ever holds ID `1` across the recycle.
- T1 (Port Linearity) — port slots of recycled ID are DISCONNECTED before reuse.
- R3, R4, R5 (free-list pop semantics).
- R28 (always-on default — no feature gate).

## ARG/DISC/REF citation

- REF-002 (Lafont 1997) — the slot-reuse semantics are consistent with the syntactic identity of agents under interaction; no observable behavior depends on physical slot identity.
- AC-006 (HVM2 flat-array rationale) — informs the LIFO choice for cache locality.

## Determinism notes

Pure synchronous code, no tokio, no async. The recycle order (which freed ID gets reused) is deterministic given the call order of `remove_agent`/`create_agent` — Vec::pop returns the most-recently-pushed element. Tests are fully deterministic.

## Cross-test dependencies

- Shares the Net fixture pattern with T2, T3, T4. Use a helper `fn fresh_net_with_n_cons(n: usize) -> Net` to keep the pre-state setup compact across all four tests.
