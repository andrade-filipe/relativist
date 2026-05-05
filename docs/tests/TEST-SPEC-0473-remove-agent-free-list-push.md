# TEST-SPEC-0473: `remove_agent` free-list push + `freeport_redirects` purge

**SPEC-22 §7 ID:** spec-catalog T1, T3, T4, T5, T9a (all consume this primitive); plus this plumbing file.
**Owning task:** TASK-0473.
**Parent spec:** SPEC-22 §3.1 R2, R7; §4.3 (remove_agent body); §4.1 freeport_redirects × recycle interaction (closes SC-001 second surface).
**Type:** unit.
**Theory anchor:** None direct.

---

## Inputs / Fixtures

- Fresh `Net::new()` + 3 created agents (IDs 0, 1, 2).

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0473-01 | `remove_agent_pushes_id_to_free_list` | net with `agents[1] == Some(CON)` | `net.remove_agent(1);` | `net.free_list.contains(&1) == true`; `net.free_list.last() == Some(&1)` (LIFO push at end). |
| UT-0473-02 | `remove_agent_marks_slot_none` | same | same | `net.agents[1].is_none()`. (Existing SPEC-02 R12 behavior; preserved.) |
| UT-0473-03 | `remove_agent_disconnects_all_ports` | net with agent 1's ports wired to agents 0 and 2 | `net.remove_agent(1);` | `net.ports[port_index(1, 0..3)]` all DISCONNECTED; the partners' ports are also disconnected (bidirectional). |
| UT-0473-04 | `freeport_redirects_purged_on_recycle` | net with `freeport_redirects.insert(1, AgentPort(0, 1));` | `net.remove_agent(1);` | `net.freeport_redirects.contains_key(&1) == false`. (Closes SC-001 second surface — closes the freeport_redirects × recycle interaction.) |
| UT-0473-05 | `freeport_redirects_only_keyed_by_removed_id_purged` | net with multiple freeport_redirects entries, only one keyed by 1 | `net.remove_agent(1);` | only the entry keyed by 1 is removed; other entries (e.g., keyed by 0 or 2) remain. |
| UT-0473-06 | `remove_agent_on_already_removed_id_is_idempotent` | `net.agents[1] == None` already | `net.remove_agent(1);` | no-op; free-list NOT pushed twice (the existing `if let Some(agent) = ...` guard short-circuits). |
| UT-0473-07 | `is_border_protected_stub_returns_false_in_pure_net` | `Net::new()` | `net.is_border_protected(any_id)` (test-only access if private) | `false`. (Documentation: stub method default behavior; TASK-0482 overrides for distributed contexts.) |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Remove an ID with no `freeport_redirects` entry | `freeport_redirects.remove(&id)` returns `None`; no error; free-list push proceeds normally. |
| EC-2 | Remove with `is_border_protected(id) == true` (synthetic test state via stub override) | Slot becomes `None`, ports DISCONNECTED, but ID NOT pushed to `free_list`. (R10c protected-tombstone path; full coverage in T9a/T9b.) |
| EC-3 | Multiple removes in sequence: `remove(0); remove(1); remove(2)` | Free-list = `[0, 1, 2]` in push order; LIFO top is 2. |

## Invariants asserted

- R2 (push to free-list on recycle path).
- R7 (no `PortRef::AgentPort` references to free-list IDs — verified by UT-0473-03 + the disconnect loop).
- §4.1 freeport_redirects × recycle interaction (UT-0473-04).
- R6 partial (no-duplicates assertion; full coverage in TASK-0474).

## ARG/DISC/REF citation

- None direct.

## Determinism notes

Pure synchronous; deterministic disconnect loop over the agent's `total_ports(symbol)` slots.

## Cross-test dependencies

- T1, T3, T4, T5, T9a all consume this primitive.
- TEST-SPEC-0474 strengthens R6 (no-duplicates).
- TEST-SPEC-0482 overrides `is_border_protected` for distributed contexts.
- TEST-SPEC-0489 (`to_sparse`) verifies `freeport_redirects` propagation through the conversion (separate from the purge).
