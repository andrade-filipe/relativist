# TEST-SPEC-0500: v1 backward-compat regression — all 1181/1224 tests pass with free-list always-on default

**SPEC-22 §7 ID:** none direct (bundle gate); plus this plumbing file.
**Owning task:** TASK-0500.
**Parent spec:** SPEC-22 §3.4 R28, R29; §6.1 step 7.
**Type:** integration + benchmark regression.

---

## Inputs / Fixtures

- The full v2-baseline test suite (1181 default / 1224 zero-copy).
- The 4490 frozen v1 baseline benchmarks.
- `cargo test`, `cargo clippy`, `cargo fmt --check` invocations.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0500-01 | `cargo_test_workspace_default_no_regression` | post all-SPEC-22 landing | `cargo test --workspace` | ≥ 1181 tests pass + new SPEC-22 tests (~30 new) — the new baseline is `>= 1181` (zero regression). |
| UT-0500-02 | `cargo_test_workspace_zero_copy_no_regression` | same | `cargo test --workspace --features zero-copy` | ≥ 1224 tests pass + new SPEC-22 tests with zero-copy parity — new baseline `>= 1224`. |
| UT-0500-03 | `clippy_clean_default_features` | same | `cargo clippy --workspace --all-targets -- -D warnings` | clean (no warnings). |
| UT-0500-04 | `clippy_clean_zero_copy_features` | same | `cargo clippy --workspace --all-targets --features zero-copy -- -D warnings` | clean. |
| UT-0500-05 | `cargo_fmt_check_clean` | same | `cargo fmt --check` | clean. |
| UT-0500-06 | `v1_floor_preserved` | the `v1-feature-complete` branch | `git checkout v1-feature-complete; cargo test` | exactly 690 tests pass (the v1 floor; never regressed). |
| UT-0500-07 | `spec22_v1_baseline_no_metric_regression` | `EP-Annihilation`, `DualTree`, `MixedNet` benchmarks | run with `default GridConfig` post-SPEC-22 | metrics (latency, throughput, peak memory) parity with v1 baseline (within ±5% noise). |
| UT-0500-08 | `spec22_serde_round_trip_continuity` | partial reduction state | serialize + deserialize + continue reduction | converges to same normal form as continuous reduction (`is_behaviorally_equal`). (Joint with T8.) |
| UT-0500-09 | `spec22_grid_g1_round_trip` | sparse build_subnet → reduce → merge | result is `is_behaviorally_equal` to sequential reduction | confirmed. (Joint with T16.) |
| UT-0500-10 | `net_default_byte_compatible_modulo_free_list` | `Net::default()` (== `Net::new()`) | byte-equality on a non-distributed reduction trace, after `is_behaviorally_equal` normalization | confirmed (R28 always-on default behaves identically to v2-baseline modulo the empty free_list field). |
| UT-0500-11 | `ci_lint_sparse_net_import_passes` | post all SPEC-22 landings | the TEST-SPEC-0493 lint runs | passes. |
| UT-0500-12 | `ci_unsafe_free_audit_passes` | same | the TEST-SPEC-0498 lint runs | passes. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | A test in the v2 baseline starts failing after a SPEC-22 task lands | The pipeline does NOT advance past this gate. The failing task is identified and fixed; UT-0500-01..02 must pass. |
| EC-2 | A new SPEC-22 test fails | Same — gate fails. Fix the test or the implementation. |
| EC-3 | The frozen v1 floor regresses (e.g., 689 of 690 tests pass on `v1-feature-complete`) | This indicates an environment problem, NOT a SPEC-22 issue (since `v1-feature-complete` is untouched). Investigate independently. |
| EC-4 | A benchmark regresses by > 5% | Investigate; may indicate the free-list / `validate_free_list` debug-mode cost is leaking into release. (R32 / TASK-0478 may need to be enabled.) |

## Invariants asserted

- All SPEC-22 R-numbers (R1..R32 inclusive of letter sub-clauses).
- All SPEC-22 §3.8 amendments (A1..A10).
- R28 (always-on default — no feature gate).
- R29 (SparseNet always available).
- v1 floor (690 tests) preserved.
- v2 baseline (1181 default / 1224 zero-copy) preserved.
- G1, D4, I3', T1, I1, I2, I6 — all preserved across SPEC-22.

## ARG/DISC/REF citation

- All SPEC-22 anchors: REF-002, REF-003, REF-014, AC-001, AC-006, AC-009, AC-011, AC-015, ARG-002, ARG-005.

## Determinism notes

`cargo test` is deterministic given a fixed seed for property tests (SPEC-08 contract). Benchmarks may have run-to-run noise; the ±5% margin absorbs it. Pure async/sync mix per the existing test suite; no new determinism considerations beyond what each individual SPEC-22 test introduces.

## Cross-test dependencies

- This is the **bundle gate**. ALL SPEC-22 TEST-SPEC files (T1..T18 + T7a/T8a/T9a/T9b/T14a + TEST-SPEC-0471..0498) must have implementations passing before TEST-SPEC-0500 runs.
- The test count delta is approximate; precise count depends on unit-vs-property-vs-integration mix.
- Documents the new baseline in `docs/progress.md` (post-impl, by sdd-pipeline).
