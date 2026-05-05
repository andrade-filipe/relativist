# TEST-SPEC EG-I5: v1 compatibility mode (R32, R39-G1-v1)

**SPEC-20 §7.2 ID:** EG-I5
**Owning task(s):** TASK-0416, TASK-0455.
**Type:** integration / regression gate.
**Test name:** `test_v1_compatibility_mode`.

---

## Inputs / Fixtures

- `GridConfig::default()` (post-normalize). All elastic flags false:
  - `hybrid_coordinator = false`
  - `elastic_departure = false`
  - `elastic_join = false`
  - `retain_partitions = false`
  - `checkpoint_partitions = false`
- A representative subset of v1 baseline tests / fixtures (e.g., the EP-Annihilation, DualTree, MixedNet benchmark scenarios used in `results/locked/v1_local_baseline/`).

## Expected behaviour

With elastic features disabled, the entire SPEC-20 code path collapses to v1 `run_grid` semantics:
- Coordinator behaves per SPEC-06 baseline.
- A `PhaseTimeout` is FATAL (no reclaim — R32 enforces).
- A worker disconnect is FATAL.
- No mid-session join — `Register` is the only handshake.
- Tests at v1 baseline (1181 default / 1224 zero-copy) pass unchanged.

## Assertions

| # | Assertion |
|---|-----------|
| A1 | For each v1 baseline fixture, `canonicalise(run_grid(net, GridConfig::default(), K)) == canonicalise(reduce_all(net))`. |
| A2 | `metrics.total_interactions` matches the frozen v1 metrics in `results/locked/v1_local_baseline/` byte-for-byte (modulo wall-clock fields). |
| A3 | Configuring `retain_partitions = false` AND `elastic_departure = true` is rejected by `validate()` (cross-anchor TEST-SPEC-0415 UT-0415-05). |
| A4 | Configuring `elastic_departure = false` and forcing a `PhaseTimeout` triggers `Err(GridError::PhaseTimeout)` — fatal, NO reclaim. |
| A5 | A v3 worker (older PROTOCOL_VERSION) is rejected per the unchanged SPEC-19 R37 path. |
| A6 | The 1181 default / 1224 zero-copy test count is preserved. |

## Edge / negative cases

- EC-1: `--elastic-join` true but other flags false → `elastic_join` is treated as a permitted opt-in but does not trigger any v2 state machine if no joiner ever connects; behavioural equivalence preserved when no actual join occurs.
- EC-2: Worker disconnect during a v1 run → coordinator returns `Err`; assertion: process exit non-zero in the integration harness.

## Invariants asserted

- All SPEC-01 invariants T1-T7, D1-D6, I1-I5, G1.
- R39-G1-v1 PRESERVED.

## ARG/DISC/REF citation

ARG-001 (G1).

## Determinism notes

`#[tokio::test(flavor = "current_thread", start_paused = true)]` for any tests that drive the runtime; otherwise pure synchronous wherever applicable.

## Cross-test dependencies

- TEST-SPEC-0415 (defaults).
- TEST-SPEC-0416 (CLI defaults).
- The frozen `v1_local_baseline` data is the regression anchor; do not regenerate.
