# TASK-0455: v1-compatibility regression gate — all flags false = v1 baseline

**Spec:** SPEC-20 §3.4 R32 (disable retention → v1 fatal), §3.7 R39 (SPEC-01 invariants preserved), §6.2 feature-flags table.
**Requirements:** R32, R39-G1-v1 (PRESERVED), SC-010 (v1 baseline preserved).
**Priority:** P0 (zero-regression gate).
**Status:** TODO
**Depends on:** ALL SPEC-20 implementation tasks — this is the final validation.
**Blocked by:** TASK-0440, TASK-0446, TASK-0447 (all elastic paths must exist to be gated off).
**Estimated complexity:** S (~30-60 LoC production + integration harness; primarily a test task)
**Bundle:** SPEC-20 Elastic Grid — Phase 4 Regression Gate.

## Context

With all elastic flags `false` (hybrid_coordinator=false, elastic_departure=false, elastic_join=false, retain_partitions=false, checkpoint_partitions=false), behavior MUST be bit-identical to v1 `run_grid`. Zero regression on the 1181-default / 1224-zero-copy test baseline.

## Acceptance Criteria

- [ ] With all elastic flags disabled, the 1181-default / 1224-zero-copy v2 test baseline passes unchanged.
- [ ] R32 enforced: `retain_partitions = false` AND `elastic_departure = true` → validate error (TASK-0415).
- [ ] R32 runtime: with `elastic_departure = false`, `PhaseTimeout` → Error (v1 fatal) not reclaim.
- [ ] EG-I5 integration test gates the regression.
- [ ] CI ensures hard fail on any test count regression below 1181.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `.github/workflows/` *(CI)* | modify | Add explicit "v1 baseline" check using default flags. |
| `relativist-core/src/tests/v1_compat.rs` *(new or existing)* | modify | Assertions that defaults reproduce v1 behavior. |

## Test Expectations (forward-ref)

- EG-I5 `test_v1_compatibility_mode` (R32, R39-G1-v1).

## Invariants Touched

- All SPEC-01 invariants (T1-T7, D1-D6, I1-I7, G1) — preserved per §3.7.

## Notes

- **Zero-regression rule**: any PR that drops the test count below 1181 MUST be rejected in CI.

## DAG Links

- **Predecessors:** ALL SPEC-20 tasks (TASK-0410..0452).
- **Successors:** bundle close.
