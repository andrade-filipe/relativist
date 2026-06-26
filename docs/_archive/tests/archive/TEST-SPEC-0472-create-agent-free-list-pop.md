# TEST-SPEC-0472: `create_agent` free-list pop branch

**SPEC-22 §7 ID:** spec-catalog T1, T2, T3, T4, T6 (all consume this primitive); plus this plumbing file.
**Owning task:** TASK-0472.
**Parent spec:** SPEC-22 §3.1 R3, R4, R5; §4.2 (create_agent body).
**Type:** unit.
**Theory anchor:** AC-006 (HVM2 LIFO recycle).

---

## Inputs / Fixtures

- Fresh `Net::new()`.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0472-01 | `create_agent_with_empty_free_list_falls_through_to_next_id` | `Net::new()` (free_list empty, next_id == 0) | `let id = net.create_agent(CON);` | `id == 0`; `net.next_id == 1`; `net.agents.len() == 1`; `net.free_list.is_empty()`. |
| UT-0472-02 | `create_agent_with_one_free_list_entry_recycles` | net with 1 prior create+remove cycle (free_list = [0], next_id = 1) | `let id = net.create_agent(DUP);` | `id == 0`; `net.next_id == 1` (unchanged); `net.free_list.is_empty()`; `net.agents[0].is_some_and(|a| a.symbol == DUP)`. |
| UT-0472-03 | `create_agent_recycle_does_not_grow_arena` | net with `agents.len() == 5`, free_list = `[2]`, next_id = 5 | `let id = net.create_agent(ERA);` | `id == 2`; `net.agents.len() == 5` (unchanged — R4(c)). |
| UT-0472-04 | `create_agent_recycle_re_initializes_port_slots` | net with recycled slot whose port slots were artificially mutated to non-DISCONNECTED before the recycle (test-only setup) | `create_agent(ERA)` | post-recycle, `net.ports[port_index(id, 0..3)]` are all DISCONNECTED. (R4(b) defensive re-init.) |
| UT-0472-05 | `create_agent_returned_id_is_consistent` | any setup | `let id = net.create_agent(CON);` | `net.agents[id as usize].as_ref().unwrap().id == id`. (Postcondition: returned ID equals the agent's stored ID.) |
| UT-0472-06 | `create_agent_postcondition_some_with_correct_symbol` | any setup | `let id = net.create_agent(symbol);` for `symbol in {CON, DUP, ERA}` | `net.agents[id].unwrap().symbol == symbol`. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Free-list with 1000 entries; 1000 successive creates | All 1000 IDs come from recycle; `next_id` unchanged. (Stress.) |
| EC-2 | `agents.len() == 0` (newly constructed) but `free_list.push(0)` injected (test-only synthetic state) | Recycle path takes ID 0; the postcondition `agents[0].is_some()` requires the arena to grow to length 1. The implementation MUST handle this: either grow the arena or panic on the assertion in §4.2 `create_agent` ("Free-list invariant violated: slot N is not None"). The synthetic-state test documents the boundary. |
| EC-3 | After `net.free_list.pop().unwrap_or(net.next_id)` is exercised in a tight loop | LIFO order preserved. |

## Invariants asserted

- R3 (free-list-pop OR next_id increment, not both).
- R4 (slot reuse semantics — Some(Agent), DISCONNECTED ports, no expansion on recycle).
- R5 (LIFO).

## ARG/DISC/REF citation

- AC-006 (HVM2 LIFO).

## Determinism notes

Pure synchronous. The pop is deterministic; the fall-through to `next_id` is deterministic.

## Cross-test dependencies

- T1 / T2 / T3 / T4 / T6 are the spec-catalog tests that exercise this primitive end-to-end.
- TEST-SPEC-0480 covers the `id_range` defensive check on the recycle path.
- TEST-SPEC-0482 covers the `RecyclePolicy` wiring on the recycle path.
