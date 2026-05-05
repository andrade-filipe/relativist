# TEST-SPEC-0496: SparseNet T1 / I1 / I2 debug assertions (R26)

**SPEC-22 §7 ID:** T12 (spec-catalog) joint coverage; plus this plumbing file.
**Owning task:** TASK-0496.
**Parent spec:** SPEC-22 §3.3 R26.
**Type:** unit (debug-only fences).

---

## Inputs / Fixtures

- A SparseNet with valid and synthetic-violation states.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0496-01 | `sparse_assert_invariants_passes_on_valid_net` | sparse with 3 chained agents (per T12 fixture) | `sparse.assert_invariants()` | does not panic. |
| UT-0496-02 | `sparse_assert_invariants_catches_one_way_port_violation` | sparse with `(0, 0) -> AgentPort(1, 0)` but missing reverse `(1, 0) -> AgentPort(0, 0)` (manually mutated) | `sparse.assert_invariants()` | panic; message cites T1/I1 violation and the offending entry. |
| UT-0496-03 | `sparse_assert_invariants_catches_dangling_agent_reference` | sparse with port `(0, 0) -> AgentPort(99, 0)` but no agent at ID 99 | `sparse.assert_invariants()` | panic; message cites I2 (reference validity). |
| UT-0496-04 | `sparse_assert_invariants_catches_oob_port_arity` | sparse with port `(0, 0) -> AgentPort(1, 5)` where agent 1 has arity 2 (so port 5 is OOB) | `sparse.assert_invariants()` | panic; message cites I2 port arity. |
| UT-0496-05 | `sparse_assert_invariants_freeport_skip_no_panic` | sparse with `(0, 0) -> FreePort(99)` (one-sided OK per SPEC-01 T1 root exception) | `sparse.assert_invariants()` | does not panic. |
| UT-0496-06 | `sparse_assert_invariants_root_exception_no_panic` | sparse with `root = Some(AgentPort(0, 0))` and that port has no reverse entry | `sparse.assert_invariants()` | does not panic (root-port exception per SPEC-01 T1). |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Empty SparseNet | trivially passes. |
| EC-2 | SparseNet under construction (mid-connect, partial state) | depending on call timing, may panic. (The helper is called at known-quiescent points.) |
| EC-3 | Self-loop `(0, 0) -> AgentPort(0, 1)` with reverse entry present | passes (T1 self-loop OK). |

## Invariants asserted

- R26 (T1, I1, I2 SparseNet equivalents).

## ARG/DISC/REF citation

- REF-002 (Lafont 1997 — wire bidirectionality).

## Determinism notes

The helper is `#[cfg(debug_assertions)]`-only. `HashMap` iteration is non-deterministic but the assertion checks per-entry; ordering doesn't affect outcome. Pure synchronous; no tokio.

## Cross-test dependencies

- T12 (spec-catalog) is the integration mirror.
- TEST-SPEC-0487 covers the SparseNet operations that produce the state.
