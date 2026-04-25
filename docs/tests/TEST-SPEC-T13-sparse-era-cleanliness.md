# TEST-SPEC-T13: SparseNet ERA auxiliary slot cleanliness (I6 sparse equivalent)

**SPEC-22 §7.2 ID:** T13.
**Owning task:** TASK-0487 (SparseNet `connect` ERA aux suppression).
**Parent spec:** SPEC-22 §3.2 R17 (no port entries for ERA aux); SPEC-01 I6 (ERA Auxiliary Slot Cleanliness).
**Type:** unit.
**Theory anchor:** REF-002 (Lafont 1997 — ε agent has arity 0).

---

## Inputs / Fixtures

- Fresh `SparseNet::new()`.
- One ERA agent: `let era_id = sparse.create_agent(Symbol::ERA)` ⇒ `era_id == 0`.
- One CON partner: `let con_id = sparse.create_agent(Symbol::CON)` ⇒ `con_id == 1`.
- Connect ERA's principal to CON's aux: `sparse.connect(AgentPort(0, 0), AgentPort(1, 1))`.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-T13-01 | `era_aux_port_1_not_in_ports_hashmap` | pre-connect | `sparse.ports.get(&(0, 1))` | `None`. (R17: ERA arity 0 ⇒ no aux port entries.) |
| UT-T13-02 | `era_aux_port_2_not_in_ports_hashmap` | pre-connect | `sparse.ports.get(&(0, 2))` | `None`. |
| UT-T13-03 | `era_aux_still_absent_after_principal_connect` | post-connect (principal port wired) | `sparse.ports.get(&(0, 1))` and `sparse.ports.get(&(0, 2))` | both `None`. (Connecting principal port does NOT cause aux entries to appear.) |
| UT-T13-04 | `connect_attempt_to_era_aux_is_disallowed` | attempt `sparse.connect(AgentPort(0, 1), AgentPort(con_id, 2))` (ERA aux as endpoint) | depending on TASK-0487 implementation: either return `Err(NetError::InvalidPort)` OR `debug_assert!` panic | the connect is rejected (no aux entry materializes in the HashMap). The exact rejection mechanism is implementation-defined per TASK-0487; the test asserts the OBSERVABLE: `sparse.ports.get(&(0, 1))` remains `None` after the attempt. |
| UT-T13-05 | `era_principal_port_entry_correct` | post-connect | `sparse.ports.get(&(0, 0))` | `Some(AgentPort(1, 1))`. (Principal is wired; aux is NOT.) |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Multiple ERA agents (e.g., 5 ERAs at IDs 0-4) | None of the 5 has aux entries. `sparse.ports.iter().filter(|((id, p), _)| sparse.agents.get(id).map(|a| a.symbol == ERA).unwrap_or(false) && *p > 0).count() == 0`. |
| EC-2 | Convert dense Net (which has explicit DISCONNECTED in ERA aux slots) to SparseNet via `to_sparse` | The DISCONNECTED ERA aux slots are NOT copied to the sparse `ports` HashMap. (R17 verified across the conversion; joint coverage with TEST-SPEC-0489 `to_sparse_skips_era_auxiliary_ports`.) |
| EC-3 | ERA principal port disconnected; aux still absent | `sparse.disconnect(AgentPort(0, 0))` removes the principal entry; aux entries remain absent (never existed). |

## Invariants asserted

- R17 (no port entries for ERA aux — sparse equivalent of I6).
- I6 (ERA Auxiliary Slot Cleanliness — SPEC-01 T1 family).

## ARG/DISC/REF citation

- REF-002 (Lafont 1997 §2 — ε agent has arity 0).

## Determinism notes

Pure synchronous; no tokio. The HashMap absence-check is deterministic regardless of internal iteration order.

## Cross-test dependencies

- TEST-SPEC-0489 (`to_sparse_skips_era_auxiliary_ports`) covers the conversion side; T13 covers the construction side. Together they validate R17 end-to-end.
- T11 fixture extends naturally; T13 adds an ERA-specific construction.
