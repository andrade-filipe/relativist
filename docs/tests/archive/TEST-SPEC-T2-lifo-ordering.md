# TEST-SPEC-T2: Free-list LIFO ordering (pure-driver)

**SPEC-22 §7.1 ID:** T2.
**Owning task:** TASK-0472 (create_agent free-list pop). Joint coverage with TASK-0474 (R6/R5 closure).
**Parent spec:** SPEC-22 §3.1 R5; §4.7 intra-rule non-determinism note (SC-019/SC-020 annotation).
**Type:** unit.
**Theory anchor:** AC-006 (HVM2 flat-array rationale — LIFO maximizes hot-cache reuse).

---

## Inputs / Fixtures

- A fresh `Net::new()`.
- Pre-state (post 5 creates with `Symbol::CON`):
  - `agents = [Some, Some, Some, Some, Some]` (IDs 0-4).
  - `free_list = []`, `next_id = 5`.
- Mid-state after `remove_agent(1)`, `remove_agent(3)`, `remove_agent(2)` in that exact order:
  - `agents = [Some, None, None, None, Some]`.
  - `free_list = [1, 3, 2]` (push order; 2 is at top).
- Expected post-state after 3 successive `create_agent(Symbol::DUP)` calls:
  - The 3 returned IDs are `[2, 3, 1]` (LIFO pop order).
  - `agents = [Some(CON_0), Some(DUP_1), Some(DUP_2), Some(DUP_3), Some(CON_4)]`.
  - `free_list = []`, `next_id = 5` (unchanged).

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-T2-01 | `lifo_first_pop_returns_last_pushed` | net at mid-state | `let id1 = net.create_agent(Symbol::DUP)` | `id1 == 2`. |
| UT-T2-02 | `lifo_second_pop_returns_second_to_last` | continued | `let id2 = net.create_agent(Symbol::DUP)` | `id2 == 3`. |
| UT-T2-03 | `lifo_third_pop_returns_first_pushed` | continued | `let id3 = net.create_agent(Symbol::DUP)` | `id3 == 1`. |
| UT-T2-04 | `lifo_next_id_unchanged_when_recycling` | full sequence | same | `net.next_id == 5` (no fresh allocations occurred). |
| UT-T2-05 | `lifo_pop_returns_most_recently_pushed` (smoke) | direct `free_list.push(7); free_list.push(11)` | `let v = net.free_list.pop()` | `v == Some(11)`. (R5 unit-level confirmation; bypasses create_agent.) |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | All 5 IDs removed (free_list = [0, 1, 2, 3, 4]); 5 successive creates | Returned IDs in order: `[4, 3, 2, 1, 0]`. `next_id` unchanged at 5. |
| EC-2 | Interleaved remove/create — `remove(0); create; remove(4); create; create` | First create returns 0 (LIFO of `[0]`). Second create returns 4. Third create has empty free-list → `next_id` increments to 5; returned ID == 5. (This documents the LIFO contract under interleaving.) |
| EC-3 | Empty net: `let n = Net::new(); n.create_agent(CON)` | Falls through to fresh allocation; returned ID == 0; `next_id` becomes 1. (LIFO is a no-op when free-list is empty.) |

## Invariants asserted

- R5 (LIFO ordering — `Vec::pop` from end is O(1)).
- R6 (no duplicates — implicit; covered fully by T10/TASK-0474).
- I3' (uniqueness — every returned ID is `agents[id].is_some()` post-create).

## ARG/DISC/REF citation

- AC-006 (HVM2 flat-array rationale; SPEC-22 R5 rationale).

## Determinism notes

**SC-019 / SC-020 annotation (verbatim from SPEC-22 §7.1 T2):** T2 is a pure-driver test that verifies the LIFO contract from R5; the call order (`create`, `remove`, `create`) is fixed by the test. T2 is **NOT** a guarantee about ID ordering inside reduction rules — see SPEC-22 §4.7's intra-rule non-determinism note. Tests asserting specific recycled-ID assignments WITHIN a single rule fire are forbidden by R27a / SC-019 (tested in T7a).

Pure synchronous test; no tokio/async; fully deterministic.

## Cross-test dependencies

- Coupled to T1's fresh-net helper.
- T2 tests the **driver-level** LIFO; T7a tests that this LIFO contract does NOT leak into a rule-level assertion (the two together define the boundary).
