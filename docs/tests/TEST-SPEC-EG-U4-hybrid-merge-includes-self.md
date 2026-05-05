# TEST-SPEC EG-U4: hybrid merge includes self (R4-v1)

**SPEC-20 §7.1 ID:** EG-U4
**Owning task(s):** TASK-0423 (self-worker spawn), TASK-0430 (orchestrator).
**Type:** unit.
**Test name:** `test_hybrid_merge_includes_self`.

---

## Inputs / Fixtures

- Small terminating net `n0`.
- K_eff = 3 (1 self + 2 remote).
- Three pre-computed partitions `[p_self, p_a, p_b]` (split deterministically).
- Each partition reduces locally (mocked or real); the resulting `[p_self', p_a', p_b']` is fed to `merge`.

## Expected behaviour

`merge([p_self', p_a', p_b'])` produces `n_final` whose canonical form equals `canonicalise(reduce_all(n0))`. The self-partition is treated identically to remote in the merge — no special-casing.

## Assertions

| # | Assertion |
|---|-----------|
| A1 | `let n_final = merge(vec![p_self_prime, p_a_prime, p_b_prime]);`. |
| A2 | `canonicalise(n_final) == canonicalise(reduce_all(n0))`. |
| A3 | Removing `p_self_prime` from the merge input and re-running yields a DIFFERENT result (sanity: self-partition contributes; not a no-op). |
| A4 | The merge order does not matter: `merge([p_self, p_a, p_b])` == `merge([p_b, p_self, p_a])` up to canonicalisation. |

## Edge / negative cases

- EC-1: self-partition is empty (no agents allocated to slot 0) — merge succeeds; result equals merge of the remote partitions only.
- EC-2: net containing a single border redex straddling self and a remote — merge resolves it correctly.

## Invariants asserted

- D1 (Split/Merge Identity at K_eff=3).
- G1 partial (composed with EG-U1 / EG-I1 for the full grid path).

## ARG/DISC/REF citation

ARG-002 (split/merge), ARG-003 P3 (border completeness).

## Determinism notes

Synchronous; deterministic given pre-computed partition reductions.

## Cross-test dependencies

EG-U4-delta (delta version), EG-I1 (full hybrid grid).
