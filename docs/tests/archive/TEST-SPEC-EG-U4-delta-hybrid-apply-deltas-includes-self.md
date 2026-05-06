# TEST-SPEC EG-U4-delta: hybrid apply-deltas includes self (R4-delta)

**SPEC-20 §7.1 ID:** EG-U4-delta
**Owning task(s):** TASK-0437.
**Type:** unit.
**Test name:** `test_hybrid_apply_deltas_includes_self`.

---

## Inputs / Fixtures

- Small terminating net + initial `BorderGraph bg`.
- K_eff = 3 (self + 2 remote) in delta mode.
- Three `RoundResult` payloads with `border_deltas` from each partition's local reduction.

## Expected behaviour

The coordinator applies all 3 round results (including self) to `bg` via `BorderGraph::apply_deltas`. The converged border state matches what the equivalent v1-mode hybrid run would produce.

## Assertions

| # | Assertion |
|---|-----------|
| A1 | After `bg.apply_deltas(self_delta).apply_deltas(a_delta).apply_deltas(b_delta)`, `bg` is in the post-round-1 state. |
| A2 | `canonicalise(reconstruct(&bg, partitions, vec![]))` after all rounds matches `canonicalise(reduce_all(n0))`. |
| A3 | Apply-order independence: `bg.apply_deltas(a).apply_deltas(self).apply_deltas(b)` yields the same final `bg` (the deltas commute). Assert via canonicalisation. |
| A4 | The self-partition's `border_deltas` is structurally identical to a remote's (this is EG-U4-delta-wire-symmetry's territory; here we cross-link). |

## Edge / negative cases

- EC-1: self-partition emits an empty `border_deltas` (no border interaction) — `apply_deltas` is a no-op; post-state equals pre-state.
- EC-2: two partitions emit conflicting deltas on the same `border_id` — SPEC-19 R39 (D3a-d) defines resolution; assert deterministic outcome.

## Invariants asserted

- D3 (Border Completeness) via SPEC-19 R39 / ARG-005.
- G1 conditional on ARG-005.

## ARG/DISC/REF citation

ARG-005 (delta border completeness — gates this test for the delta-optimized path; conservative path is independent).

## Determinism notes

Synchronous; deterministic given pre-computed deltas. Apply-order test relies on commutativity proven by SPEC-19 R39.

## Cross-test dependencies

- EG-U4-delta-wire-symmetry asserts the wire shape of the self-partition's delta payload.
- EG-I1-delta is the integration counterpart.
