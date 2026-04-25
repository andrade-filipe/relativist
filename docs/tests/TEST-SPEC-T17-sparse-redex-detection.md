# TEST-SPEC-T17: SparseNet redex detection on connect

**SPEC-22 §7.2 ID:** T17.
**Owning task:** TASK-0487 (SparseNet `connect` redex-enqueue logic).
**Parent spec:** SPEC-22 §3.2 R14 (`connect` — enqueue redex if both are principal ports).
**Type:** unit.
**Theory anchor:** REF-002 (Lafont 1997 — active pair = both endpoints are principal ports).

---

## Inputs / Fixtures

- Fresh `SparseNet::new()`.
- 4 agents created (IDs 0-3 — `[CON, DUP, CON, ERA]`).

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-T17-01 | `principal_principal_connect_enqueues_redex` | empty redex queue | `sparse.connect(AgentPort(0, 0), AgentPort(1, 0))` | `sparse.redex_queue.len() == 1`; the entry is `(0, 1)` or `(1, 0)` (ordering implementation-defined). |
| UT-T17-02 | `aux_aux_connect_does_not_enqueue` | continued | `sparse.connect(AgentPort(0, 1), AgentPort(2, 1))` | `sparse.redex_queue.len() == 1` (still only the first redex; aux-aux doesn't qualify). |
| UT-T17-03 | `aux_principal_connect_does_not_enqueue` | continued | `sparse.connect(AgentPort(0, 2), AgentPort(2, 0))` | `sparse.redex_queue.len() == 1`. (Mixed-port connect: NOT a redex per IC definition.) |
| UT-T17-04 | `principal_freeport_does_not_enqueue` | continued | `sparse.connect(AgentPort(3, 0), FreePort(99))` | `sparse.redex_queue.len() == 1`. (FreePort is not an agent port; cannot form an active pair.) |
| UT-T17-05 | `disconnect_does_not_dequeue` | continued | `sparse.disconnect(AgentPort(0, 0))` | `sparse.redex_queue.len() == 1` (the redex remains queued; disconnection does not retract a queued redex — that's the reduction engine's responsibility on dispatch). |
| UT-T17-06 | `multiple_principal_principal_connects_each_enqueue` | fresh sparse with 4 fresh agents | 2 connects: `(4,0)-(5,0)` and `(6,0)-(7,0)` (assumes more agents) | `sparse.redex_queue.len() == 2`. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Self-loop on same agent's principal port (e.g., `connect(AgentPort(0, 0), AgentPort(0, 0))`) | Implementation MAY reject as invalid OR enqueue self-redex; per SPEC-02 self-loops on different ports are allowed but principal-self is degenerate. Test asserts: no panic, queue state per the chosen branch. |
| EC-2 | ERA principal connected to ERA principal | Enqueues a redex (ERA-ERA is an active pair per the ε rules). Confirms the rule fires for ERA-ERA. |
| EC-3 | ERA principal connected to CON principal | Enqueues a redex (ERA-CON is a void rule active pair). |
| EC-4 | Connecting two FreePorts to each other | Implementation-defined; either rejected or stored without redex queue impact. (FreePorts are not agent ports.) |

## Invariants asserted

- R14 (`connect` semantics — redex queue only on principal-principal).
- T1 (Port Linearity preserved by connect — redex enqueueing is orthogonal).

## ARG/DISC/REF citation

- REF-002 (Lafont 1997 — active pair definition).

## Determinism notes

`VecDeque::push_back` is deterministic. The redex queue order within `redex_queue` after multiple connects is the connect-call order. `HashMap::insert` for the port entries does not affect the queue. Pure synchronous; no tokio.

## Cross-test dependencies

- T11 / T12 build the same fixture base.
- The redex-dispatch logic is in SPEC-03 (reduction engine), tested under T5/T6/T7; T17 is purely the connect-side queue-population test.
