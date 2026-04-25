# TASK-0500: v1 backward-compat regression — all 1181/1224 tests pass with free-list always-on default

**Spec:** SPEC-22 §3.4 R28, R29; §6.1 step 7 ("All 690 v1 tests MUST pass without modification").
**Requirements:** R28 (arena recycling always-on by default — no feature gate; free-list adds negligible overhead), R29 (SparseNet always available — no feature gate).
**Priority:** P0 (bundle gate — all SPEC-22 tasks must complete before this regression assertion runs).
**Status:** TODO
**Depends on:** ALL SPEC-22 tasks (TASK-0460..TASK-0498).
**Blocked by:** none
**Estimated complexity:** S (~50 LoC test wiring + reuses existing 1181 + 1224 test suites)
**Bundle:** SPEC-22 Arena Management — Phase F (regression gate).

## Context

The free-list mechanism is backward-compatible: it simply starts empty and accumulates naturally during reduction. SparseNet is a new type but does not affect existing dense `Net` paths unless explicitly opted into (default `sparse_build = true` only takes effect under the 4× threshold). Per CLAUDE.md: "every change must pass all 690 v1 tests (floor) plus the current v2 baseline (1181 default / 1224 zero-copy) — zero regression."

This task is the **bundle gate**: with all SPEC-22 tasks landed, run the full test suite and assert no regression.

## Acceptance Criteria

- [ ] All SPEC-22 tasks (TASK-0460..TASK-0498) shipped (status DONE).
- [ ] `cargo test --workspace` ≥ 1181 default (current v2 baseline) — ZERO regression.
- [ ] `cargo test --workspace --features zero-copy` ≥ 1224 zero-copy — ZERO regression.
- [ ] v1 floor preserved: 690 tests on `v1-feature-complete` branch, untouched.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean (both feature configs).
- [ ] `cargo fmt --check` clean.
- [ ] New SPEC-22 tests added: ~30 (T1-T18 + T7a + T8a + T9a + T9b + T14a). New baseline: ~1211 default, ~1254 zero-copy (rough estimate; exact count is the result of TASK-0500's run).
- [ ] CI lint (TASK-0493 SparseNet import lint) passes.
- [ ] CI lint (TASK-0498 unsafe-free audit) passes.
- [ ] `Net::default()` (i.e., `Net::new()`) behaves identically to v2-baseline `Net::new()` modulo the empty `free_list` field — verified by an explicit byte-equality test on a non-distributed reduction trace.
- [ ] Document the new baseline in `docs/progress.md` (post-implementation).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/tests/spec22_regression.rs` | create | Run a representative v1 trace with free-list-aware net; assert byte-equal output (after `is_behaviorally_equal` normalization) to the v1-baseline frozen output. |
| `docs/progress.md` | modify (post-impl, by sdd-pipeline) | Document the new test baseline. |

## Test Expectations

TEST-SPEC-0500:
- `spec22_v1_baseline_no_regression` — full reduction of EP-Annihilation, DualTree, MixedNet bench fixtures with default `GridConfig`; assert metrics parity with v1 baseline.
- `spec22_serde_round_trip_continuity` — serialize a partial reduction state; deserialize; continue reduction; assert convergence to the same normal form as continuous reduction (T8 from SPEC-22 §7.1).
- `spec22_grid_g1_round_trip` — sparse build_subnet → reduce → merge → assert G1 isomorphism vs sequential reduction (T16).

## Invariants Touched

- All SPEC-22 invariants (T1-T7, D1-D6, I1-I7 incl. I3', G1, R-numbers R1-R32) — gated by this regression.

## Notes

- This task does NOT add new features; it asserts no regression after every SPEC-22 task lands.
- If the regression fails, the failing task in the SPEC-22 chain MUST be fixed before this gate passes. The pipeline does NOT advance past this gate with a failing test.
- The test count delta is approximate; the precise number depends on how many SPEC-22 tests are unit-level vs property-level vs integration-level.

## DAG Links

- **Predecessors:** ALL SPEC-22 tasks (TASK-0460..TASK-0498).
- **Successors:** none — this is the bundle gate.
