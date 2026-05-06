# TEST-SPEC-T12: SparseNet bidirectionality (I1 / T1 sparse equivalent)

**SPEC-22 §7.2 ID:** T12.
**Owning task:** TASK-0487 (SparseNet `connect`/`disconnect`), TASK-0496 (SparseNet debug assertions).
**Parent spec:** SPEC-22 §3.2 R14, R26 (T1/I1 sparse-adapted); SPEC-01 T1, I1.
**Type:** unit.
**Theory anchor:** REF-002 (Lafont 1997 — wire-bidirectionality is fundamental to IC reduction).

---

## Inputs / Fixtures

- Fresh `SparseNet::new()`.
- 3 agents created (IDs 0, 1, 2 — `[CON, DUP, CON]`).
- Chain wiring:
  - `sparse.connect(AgentPort(0, 0), AgentPort(1, 0))` — principal-principal (creates a redex; verified in T17).
  - `sparse.connect(AgentPort(0, 1), AgentPort(2, 1))` — aux-aux.
  - `sparse.connect(AgentPort(1, 2), AgentPort(2, 0))` — aux-principal.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-T12-01 | `every_port_entry_has_reverse_entry` | post-wiring | iterate `sparse.ports`; for each `((a, p), q)` where `q == AgentPort(b, bp)`, assert `sparse.ports.get(&(b, bp)) == Some(&AgentPort(a, p))` | passes for every entry (root-port exception per SPEC-01 T1 not applicable here — no root). |
| UT-T12-02 | `disconnect_removes_both_directions` | post-wiring; `sparse.disconnect(AgentPort(0, 0))` | check `sparse.ports.get(&(0, 0))` and `sparse.ports.get(&(1, 0))` | both return `None`. (Bidirectional removal.) |
| UT-T12-03 | `assert_invariants_passes_on_valid_chain` | post-wiring (no disconnect) | `sparse.assert_invariants()` (helper from TASK-0496) | does not panic. |
| UT-T12-04 | `assert_invariants_catches_one_way_violation` | post-wiring; manually remove only `(0, 0)` from `sparse.ports` (test-only mutation) | `sparse.assert_invariants()` | panics with message citing T1/I1 violation and the offending entry. |
| UT-T12-05 | `freeport_endpoint_skips_reverse_check` | new wiring: `sparse.connect(AgentPort(0, 0), FreePort(99))` (replace earlier connect) | iterate `sparse.ports`; for the `((0, 0), FreePort(99))` entry, NO reverse entry is required | `assert_invariants` does not panic. (FreePort is a one-sided endpoint per SPEC-02; sparse representation honors this.) |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Empty SparseNet, no connects | `assert_invariants` does not panic (vacuously true). |
| EC-2 | A self-loop: `connect(AgentPort(0, 0), AgentPort(0, 1))` — same agent, different ports | Bidirectionality holds: `(0, 0) -> AgentPort(0, 1)` AND `(0, 1) -> AgentPort(0, 0)`. `assert_invariants` passes. (T1 self-loop check is per SPEC-01.) |
| EC-3 | Stress: 100 agents in a long chain | `assert_invariants` passes; iteration completes in finite time. |

## Invariants asserted

- T1 (Port Linearity — sparse-adapted).
- I1 (Bidirectional Consistency — sparse-adapted).
- R14 (`connect`/`disconnect` are bidirectional in the sparse representation).
- R26 (T1/I1 hold for SparseNet with adapted verification).

## ARG/DISC/REF citation

- REF-002 (Lafont 1997 — wire-bidirectionality requirement).

## Determinism notes

`HashMap` iteration is non-deterministic; UT-T12-01 must iterate then assert per-entry, NOT compare full HashMap structure to a literal. Pure synchronous; no tokio.

## Cross-test dependencies

- TEST-SPEC-0496 covers the `assert_invariants` helper at the plumbing level.
- T13 (ERA cleanliness) shares the SparseNet fixture but tests the I6 sparse equivalent.
- T17 (redex detection) reuses the same wiring.
