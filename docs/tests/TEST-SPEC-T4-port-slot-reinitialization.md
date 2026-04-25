# TEST-SPEC-T4: Port slot reinitialization on recycle

**SPEC-22 §7.1 ID:** T4.
**Owning task:** TASK-0472 (create_agent recycle path), TASK-0473 (remove_agent disconnect loop).
**Parent spec:** SPEC-22 §3.1 R4(b) (port slots DISCONNECTED on recycle); R7 (no `PortRef::AgentPort` references to free-list IDs).
**Type:** unit.
**Theory anchor:** REF-002 (Lafont 1997 — wire structure determines reduction; stale port refs would corrupt rule application).

---

## Inputs / Fixtures

- Fresh `Net::new()`.
- Step 1 (build a connected CON):
  - `let a = net.create_agent(Symbol::CON)` (ID 0).
  - `let b = net.create_agent(Symbol::CON)` (ID 1, partner).
  - `net.connect(AgentPort(a, 0), AgentPort(b, 0))`. Ports `[0*3+0]` and `[1*3+0]` now reference each other.
  - Optionally connect aux ports `(a, 1) <-> (b, 1)` and `(a, 2) <-> (b, 2)` to populate all 6 port slots.
- Step 2 (remove a):
  - `net.remove_agent(a)`. After this: `agents[0] == None`, `free_list = [0]`, port slots `[0..3]` all DISCONNECTED.
- Step 3 (recycle to ERA):
  - `let c = net.create_agent(Symbol::ERA)` — should return ID 0.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-T4-01 | `recycled_era_principal_port_disconnected` | post Step 3 | check `net.ports[port_index(0, 0)]` | equals `DISCONNECTED`. |
| UT-T4-02 | `recycled_era_aux_ports_disconnected` | post Step 3 | check `net.ports[port_index(0, 1)]` and `port_index(0, 2)` | both equal `DISCONNECTED`. (ERA has arity 0; aux slots exist in the dense array but MUST be DISCONNECTED — sparse equivalent I6 in T13.) |
| UT-T4-03 | `no_stale_back_references_from_b` | post Step 3 | scan `net.ports` for `PortRef::AgentPort(0, _)` references *outside* the (0, _) slots | no such references exist. The remove_agent disconnect loop already cleared b's side; recycle did not re-introduce them. |
| UT-T4-04 | `recycle_followed_by_connect_succeeds` | post Step 3 | `net.connect(AgentPort(c, 0), AgentPort(b, 0))` | succeeds; `net.ports[port_index(c, 0)] == AgentPort(b, 0)` and the reverse holds. |
| UT-T4-05 | `recycle_after_partial_disconnect_still_disconnected` | mid-test variant: only port 0 was connected in Step 1 (ports 1, 2 left DISCONNECTED) | Step 3 | all 3 port slots DISCONNECTED post-recycle. (Defensive `debug_assert` in §4.2 `create_agent` recycle path covers this.) |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Removed agent had a `FreePort` connection: `net.connect(AgentPort(a, 1), FreePort(99))` before remove | Post recycle, `ports[port_index(0, 1)] == DISCONNECTED`. The `FreePort(99)` side has its `freeport_redirects` entry purged by `remove_agent` (per §4.3 — closes SC-001 second surface; full coverage in TASK-0489 `to_sparse` and TEST-SPEC-0473 `freeport_redirects_purged_on_recycle`). |
| EC-2 | Symbol of new agent has different arity than old: CON (arity 2) → ERA (arity 0) | All 3 port slots are still cleared to DISCONNECTED defensively. The arity mismatch does not leak through the dense-array layout (`PORTS_PER_SLOT = 3`). |
| EC-3 | Recycled slot has been recycled multiple times (3 cycles of remove+create) | Each cycle leaves the port slots in DISCONNECTED state. Test runs 3 iterations and asserts the invariant after each. |

## Invariants asserted

- T1 (Port Linearity — bidirectionality preserved by re-init to DISCONNECTED before any new connect).
- R4(b) (port slots DISCONNECTED on recycle — verified directly).
- R7 (no AgentPort references to free-list IDs — verified by UT-T4-03).
- I6 (ERA Auxiliary Slot Cleanliness, SPEC-01 — preserved on recycle to ERA).

## ARG/DISC/REF citation

- REF-002 (Lafont 1997) — wire structure determines reduction; stale port refs would corrupt rule application.
- AC-001 (Haskell IC.Core baseline `Map AgentId Agent`) — informs the contract that port refs are always validated against live agent set.

## Determinism notes

Pure synchronous; no tokio/async. The remove_agent disconnect loop is single-threaded over a finite port count (`PORTS_PER_SLOT = 3`).

## Cross-test dependencies

- Joint coverage with TEST-SPEC-0473 (`freeport_redirects_purged_on_recycle` — the EC-1 scenario).
- T13 covers the sparse-side equivalent (no port entries for ERA aux).
