# TEST-SPEC-0487: `SparseNet` operations — create/remove/connect/disconnect/get_target/get_agent/is_reduced/count_live (R14-R17)

**SPEC-22 §7 ID:** T11, T12, T13, T17 (spec-catalog) plus this plumbing file.
**Owning task:** TASK-0487.
**Parent spec:** SPEC-22 §3.2 R14, R15, R16, R17; §4.5.
**Type:** unit.

---

## Inputs / Fixtures

- Fresh `SparseNet::new()`.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0487-01 | `create_agent_inserts_into_hashmap` | empty sparse | `let id = sn.create_agent(CON);` | `sn.agents.contains_key(&id) == true`; `id == 0`; `sn.next_id == 1`. |
| UT-0487-02 | `remove_agent_removes_from_hashmap_and_ports` | sparse with agent 0 connected to agent 1 (principal-principal) | `sn.remove_agent(0)` | `!sn.agents.contains_key(&0)`; `sn.ports.get(&(0, 0)).is_none()`; `sn.ports.get(&(1, 0)).is_none()` (bidirectional removal — joint with T12). |
| UT-0487-03 | `connect_inserts_bidirectional_entries` | 2 agents | `sn.connect(AgentPort(0, 0), AgentPort(1, 1))` | `sn.ports.get(&(0, 0)) == Some(&AgentPort(1, 1))` AND `sn.ports.get(&(1, 1)) == Some(&AgentPort(0, 0))`. |
| UT-0487-04 | `connect_principal_principal_enqueues_redex` | 2 agents | `sn.connect(AgentPort(0, 0), AgentPort(1, 0))` | `sn.redex_queue.len() == 1`. (R14 redex detection; full coverage in T17.) |
| UT-0487-05 | `connect_aux_aux_no_redex` | 2 agents | `sn.connect(AgentPort(0, 1), AgentPort(1, 2))` | `sn.redex_queue.is_empty()`. |
| UT-0487-06 | `disconnect_removes_bidirectional_entries` | post-connect | `sn.disconnect(AgentPort(0, 0))` | both `(0, 0)` and the partner entry removed. |
| UT-0487-07 | `get_target_returns_some_for_connected_port` | post-connect | `sn.get_target(AgentPort(0, 0))` | `Some(AgentPort(1, 1))`. |
| UT-0487-08 | `get_target_returns_disconnected_for_freeport` | post-connect to FreePort | `sn.get_target(FreePort(99))` | `DISCONNECTED` (or implementation-equivalent). |
| UT-0487-09 | `get_agent_returns_some_for_live` | sparse with agent 0 | `sn.get_agent(0)` | `Some(&Agent { symbol: CON, id: 0 })`. |
| UT-0487-10 | `get_agent_returns_none_for_removed` | post `remove_agent(0)` | `sn.get_agent(0)` | `None`. |
| UT-0487-11 | `is_reduced_true_when_redex_queue_empty` | freshly constructed sparse, no connects | `sn.is_reduced()` | `true`. |
| UT-0487-12 | `count_live_agents_uses_hashmap_len` | sparse with N agents | `sn.count_live_agents()` | `== sn.agents.len()`. (R14 / R15 O(1).) |
| UT-0487-13 | `era_aux_ports_not_inserted_on_connect` | ERA agent created; attempt principal connect | `sn.connect(AgentPort(era_id, 0), AgentPort(other_id, 0))` then check `sn.ports.get(&(era_id, 1))` | `None`. (R17; full coverage in T13.) |
| UT-0487-14 | `live_agents_iterator_count` | sparse with 5 agents | `sn.live_agents().count()` | `== 5`. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Self-loop connect: `connect(AgentPort(0, 0), AgentPort(0, 1))` | Both entries inserted; bidirectionality preserved (each side points to the other slot of the same agent). |
| EC-2 | Connect attempt on non-existent agent (`AgentPort(999, 0)` when agent 999 doesn't exist) | Implementation-defined: either `Err(InvalidPort)` OR `debug_assert!` panic. The test asserts the OBSERVABLE: `sn.ports.contains_key(&(999, 0))` is FALSE post-attempt. |
| EC-3 | Remove agent that's not in HashMap | No-op; no panic; no state change. (Idempotent like Net's remove_agent.) |
| EC-4 | Connect with port arity exceeding agent's symbol arity (e.g., `AgentPort(era_id, 1)` for ERA arity 0) | Rejected per R17; UT-0487-13 covers. |

## Invariants asserted

- R14 (operation parity with Net).
- R15 (O(1) amortized — empirically observable; not benchmark-hard).
- R16 (no tombstones — UT-0487-02).
- R17 (no ERA aux entries — UT-0487-13).

## ARG/DISC/REF citation

- AC-001 (Haskell IC.Core baseline).

## Determinism notes

`HashMap` iteration is non-deterministic for `live_agents()` (UT-0487-14 only asserts count). Pure synchronous; no tokio.

## Cross-test dependencies

- T11 / T12 / T13 / T17 are spec-catalog mirrors.
- TEST-SPEC-0489 / TEST-SPEC-0490 (conversions) consume these primitives.
- TEST-SPEC-0496 covers the SparseNet debug-assertion helper.
