# TEST-SPEC-0489: `Net::to_sparse` conversion (R19)

**SPEC-22 §7 ID:** T14 / T15 (joint integration); plus this plumbing file.
**Owning task:** TASK-0489.
**Parent spec:** SPEC-22 §3.2 R19; §4.6.
**Type:** unit.

---

## Inputs / Fixtures

- A dense `Net` with:
  - 10 agents (mix of CON, DUP, ERA).
  - 2 removed (free-list = `[2, 5]`, slots `None`).
  - Wires: principal-principal, aux-aux, ERA aux explicitly DISCONNECTED.
  - `freeport_redirects.insert(99, AgentPort(7, 0))`.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0489-01 | `to_sparse_skips_none_slots` | dense with `agents[2] == None` and `agents[5] == None` | `let sn = net.to_sparse();` | `!sn.agents.contains_key(&2)` AND `!sn.agents.contains_key(&5)`. |
| UT-0489-02 | `to_sparse_includes_all_live_agents` | same | same | `sn.agents.len() == 8` (10 - 2 removed). For each live ID `i`, `sn.agents.contains_key(&i) == true`. |
| UT-0489-03 | `to_sparse_skips_disconnected_ports` | dense with `ports[port_index(0, 1)] == DISCONNECTED` | same | `!sn.ports.contains_key(&(0, 1))`. |
| UT-0489-04 | `to_sparse_skips_era_auxiliary_ports` | ERA agent at ID 3 with DISCONNECTED aux slots | same | `!sn.ports.contains_key(&(3, 1))` AND `!sn.ports.contains_key(&(3, 2))`. (Closes the I6 sparse-equivalent end-to-end.) |
| UT-0489-05 | `to_sparse_preserves_freeport_redirects` | net with `freeport_redirects = {99 -> AgentPort(7, 0)}` | same | `sn.freeport_redirects == net.freeport_redirects`. (Closes SC-001 second surface.) |
| UT-0489-06 | `to_sparse_preserves_redex_queue_clone` | net with `redex_queue = [(0, 1), (3, 4)]` | same | `sn.redex_queue == net.redex_queue` (clone is byte-equal). |
| UT-0489-07 | `to_sparse_preserves_next_id` | net with `next_id == 10` | same | `sn.next_id == 10`. |
| UT-0489-08 | `to_sparse_preserves_root` | net with `root = Some(AgentPort(0, 0))` | same | `sn.root == Some(AgentPort(0, 0))`. |
| UT-0489-09 | `to_sparse_does_not_carry_free_list` | net with non-empty `free_list = [2, 5]` | same | `sn` has no free-list field per R13. (The free-list is dense-specific; conversion intentionally drops it. Recovered on `to_dense(Some(range))` per R20.) |
| UT-0489-10 | `to_sparse_complexity_is_o_arena_len` | net with `arena_len == 1000`, 100 live | conversion runs in time roughly proportional to arena_len, not just live count | confirmed via timing smoke (no benchmark assertion; just wall-clock < 1ms in release for arena_len 1000). |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Empty net | `to_sparse` returns empty SparseNet. |
| EC-2 | Net with all `None` slots (full removal) | SparseNet has empty `agents` and `ports`. |
| EC-3 | Net with port pointing to a removed agent (stale ref — would be a bug elsewhere) | `to_sparse` faithfully copies the stale ref. (Bug propagates; caught by R26 `assert_invariants`.) |

## Invariants asserted

- R19 (`to_sparse` semantics).
- D1c (FreePort bijectivity preserved).
- I6 sparse equivalent (R17 — UT-0489-04).

## ARG/DISC/REF citation

- AC-006 (HVM2 flat-array rationale — contrasts with sparse representation; informs the conversion direction).

## Determinism notes

The conversion iterates `self.agents.iter().flatten()` (deterministic ascending order over the Vec). Per-agent port iteration is deterministic. `HashMap::insert` is order-independent. Pure synchronous; no tokio.

## Cross-test dependencies

- T14 / T15 / T16 / T18 consume this primitive.
- TEST-SPEC-0490 is the inverse conversion (`to_dense`).
- TEST-SPEC-0491 covers `is_behaviorally_equal` for round-trip closure.
