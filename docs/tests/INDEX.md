# TEST-SPEC Index — Active

Active TEST-SPEC catalog for the v2 SDD pipeline. Archived specs from previous bundles live under `archive/`.

> Convention: tests reach the developer (Stage 3) via this directory. Specifications are markdown only — the developer turns each into Rust test code in `relativist-core/{src,tests}/`. **All test counts on disk MUST stay at 1181 default / 1224 `--features zero-copy` while these specs are pending implementation** — they are documentation, not code.

---

## SPEC-20 Elastic Grid (active bundle, Stage 2 deliverable)

Source: `specs/SPEC-20-elastic-grid.md` (Reviewed v2, Round 3 closed 2026-04-24).
Spec review: `docs/spec-reviews/SPEC-REVIEW-20-round-3-2026-04-24.md`.
Theory anchors: ARG-001 (CLOSED), ARG-002, ARG-004, ARG-005 (CLOSED at SPEC-19 boundary), ARG-006 (CLOSED for v1 + delta-conservative). See `docs/theory-bridge.md`.

### Plumbing / mechanical TEST-SPECs (numbered, one per task)

| File | Task | Subject |
|------|------|---------|
| `TEST-SPEC-0410-net-union.md` | TASK-0410 | `Net::union` structural concatenation (SPEC-02 A7) |
| `TEST-SPEC-0411-allocate-border-ids-and-remap.md` | TASK-0411 | `allocate_border_ids` + `remap_partition_ids` (SPEC-04 A3, A4) |
| `TEST-SPEC-0412-reconstruct-3arg.md` | TASK-0412 | `reconstruct` 3-arg amendment (SPEC-19 A8) |
| `TEST-SPEC-0414-fsm-enums-extension.md` | TASK-0414 | New `CoordinatorState/Event/Action` enum surface (SPEC-13 A2) |
| `TEST-SPEC-0415-gridconfig-elastic-fields.md` | TASK-0415 | 9 elastic GridConfig fields + defaults + validate (SPEC-05 A5) |
| `TEST-SPEC-0416-cli-elastic-flags.md` | TASK-0416 | CLI flags for elastic config (R34) |
| `TEST-SPEC-0418-message-enum-elastic-variants.md` | TASK-0418 | 5 new `Message` variants + supporting types (R35, R36, R21) |
| `TEST-SPEC-0419-handshake-register-vs-joinrequest.md` | TASK-0419 | Coordinator handshake branching (R37a, R0d, NF-009) |
| `TEST-SPEC-0426-timerkind-enum.md` | TASK-0426 | `TimerKind` enum `#[repr(u32)]` sentinel (NF-008) |
| `TEST-SPEC-0450-gridmetrics-elastic-fields.md` | TASK-0450 | `GridMetrics` 7 new fields + R45 disjointness (R38, R38a, R38b) |

### Unit tests (EG-U series, SPEC-20 §7.1)

| File | EG-ID | Owning task(s) |
|------|-------|---------------|
| `TEST-SPEC-EG-U1-hybrid-coordinator-single-machine.md` | EG-U1 | 0422, 0423, 0425, 0430 |
| `TEST-SPEC-EG-U1a-solo-join-during-solo-reduction.md` | EG-U1a | 0425, 0436 |
| `TEST-SPEC-EG-U1b-worker-id-zero-semantics-per-mode.md` | EG-U1b | 0420, 0436 |
| `TEST-SPEC-EG-U2-hybrid-partition-count.md` | EG-U2 | 0430 |
| `TEST-SPEC-EG-U3-hybrid-self-partition-id-range.md` | EG-U3 | 0421, 0430 |
| `TEST-SPEC-EG-U4-hybrid-merge-includes-self.md` | EG-U4 | 0423, 0430 |
| `TEST-SPEC-EG-U4-delta-hybrid-apply-deltas-includes-self.md` | EG-U4-delta | 0437 |
| `TEST-SPEC-EG-U4-delta-wire-symmetry-self-vs-remote.md` | EG-U4-delta-wire-symmetry | 0437 |
| `TEST-SPEC-EG-U5-dynamic-join-repartition-v1.md` | EG-U5 | 0421, 0432, 0433 |
| `TEST-SPEC-EG-U5-delta-dynamic-join-repartition-delta.md` | EG-U5-delta | 0446 |
| `TEST-SPEC-EG-U6-dynamic-join-mid-round-queued.md` | EG-U6 | 0432, 0434, 0436 |
| `TEST-SPEC-EG-U6a-join-window-boundary-race.md` | EG-U6a | 0434, 0435, 0436 |
| `TEST-SPEC-EG-U7-departure-reclaim-initial.md` | EG-U7 | 0438, 0440, 0436, 0451 |
| `TEST-SPEC-EG-U7a-departure-reclaim-border-id-rebase.md` | EG-U7a | 0440, 0452 |
| `TEST-SPEC-EG-U7b-departure-reclaim-last-acked-v1.md` | EG-U7b | 0439, 0440 |
| `TEST-SPEC-EG-U7c-departure-reclaim-last-acked-delta.md` | EG-U7c | 0439, 0443 |
| `TEST-SPEC-EG-U8-departure-multiple-workers-v1.md` | EG-U8 | 0440 |
| `TEST-SPEC-EG-U9-departure-all-workers-solo-fallback.md` | EG-U9 | 0442 |
| `TEST-SPEC-EG-U10-graceful-leave-after-round.md` | EG-U10 | 0441, 0436, 0451 |
| `TEST-SPEC-EG-U10a-graceful-leave-urgent-v1.md` | EG-U10a | 0440, 0441, 0436 |
| `TEST-SPEC-EG-U10b-graceful-leave-urgent-delta.md` | EG-U10b | 0441, 0443, 0436 |
| `TEST-SPEC-EG-U10c-graceful-leave-after-result-no-result-received.md` | EG-U10c | 0441, 0436 |
| `TEST-SPEC-EG-U11-join-and-departure-same-round.md` | EG-U11 | 0435, 0447, 0436, 0451 |
| `TEST-SPEC-EG-U12-id-ranges-no-collision-after-repartition.md` | EG-U12 | 0421, 0440 |
| `TEST-SPEC-EG-U12a-partition-index-vs-worker-id-decoupling.md` | EG-U12a | 0420 |
| `TEST-SPEC-EG-U13-retained-partition-atomic-release.md` | EG-U13 | 0439, 0452 |
| `TEST-SPEC-EG-U14-worker-id-exhaustion-join-nack.md` | EG-U14 | 0418, 0419, 0420, 0432 |
| `TEST-SPEC-EG-U15a-protocol-version-mismatch-register-path.md` | EG-U15a | 0417, 0418, 0419 |
| `TEST-SPEC-EG-U15b-protocol-version-mismatch-join-request-path.md` | EG-U15b | 0417, 0418, 0419, 0432 |
| `TEST-SPEC-EG-U16-self-partition-panic-to-error.md` | EG-U16 | 0422, 0423, 0436 |
| `TEST-SPEC-EG-U17-strict-bsp-self-partition-uniformity.md` | EG-U17 | 0424 |
| `TEST-SPEC-EG-U18-initial-wait-supersedes-worker-connect.md` | EG-U18 | 0413, 0425, 0436 |
| `TEST-SPEC-EG-U19-leave-ack-before-close.md` | EG-U19 | 0418, 0441 |

### Integration tests (EG-I series, SPEC-20 §7.2)

| File | EG-ID | Owning task(s) |
|------|-------|---------------|
| `TEST-SPEC-EG-I1-hybrid-grid-correctness-v1.md` | EG-I1 | 0430 |
| `TEST-SPEC-EG-I1-delta-hybrid-grid-correctness-delta.md` | EG-I1-delta | 0437 |
| `TEST-SPEC-EG-I2-elastic-join-correctness-v1.md` | EG-I2 | 0433 |
| `TEST-SPEC-EG-I2-delta-elastic-join-correctness-delta.md` | EG-I2-delta | 0446 |
| `TEST-SPEC-EG-I3-elastic-departure-correctness-v1.md` | EG-I3 | 0410, 0438, 0440 (cites ARG-006) |
| `TEST-SPEC-EG-I3-delta-elastic-departure-correctness-delta.md` | EG-I3-delta | 0410, 0412, 0438, 0443 |
| `TEST-SPEC-EG-I4-elastic-churn-correctness.md` | EG-I4 | 0447 |
| `TEST-SPEC-EG-I5-v1-compatibility-mode.md` | EG-I5 | 0416, 0455 |
| `TEST-SPEC-EG-I5a-condup-cascades-with-retained-redispatch.md` | EG-I5a | 0410, 0440 (cites ARG-006) |
| `TEST-SPEC-EG-I5b-emergent-borders-across-retained-evolved.md` | EG-I5b | 0440 |

### Property tests (EG-P series, SPEC-20 §7.3)

| File | EG-ID | Owning task(s) |
|------|-------|---------------|
| `TEST-SPEC-EG-P1-prop-hybrid-normal-form-invariant.md` | EG-P1 | 0422 (Property coverage) |
| `TEST-SPEC-EG-P2-prop-departure-normal-form-invariant-v1.md` | EG-P2 | 0440 transitive (cites ARG-006) |
| `TEST-SPEC-EG-P3-prop-id-ranges-disjoint-after-repartition.md` | EG-P3 | 0452 |
| `TEST-SPEC-EG-P4-prop-full-matrix-correctness.md` | EG-P4 | 0455 transitive |
| `TEST-SPEC-EG-P5-prop-condup-heavy-churn.md` | EG-P5 | 0443 (cites ARG-006) |
| `TEST-SPEC-EG-P6-prop-delta-elastic-correctness.md` | EG-P6 | 0443 |

### Benchmark tests (EG-B series, SPEC-20 §7.4)

| File | EG-ID | Owning task(s) |
|------|-------|---------------|
| `TEST-SPEC-EG-B1-bench-hybrid-vs-nonhybrid.md` | EG-B1 | 0450 |
| `TEST-SPEC-EG-B2-bench-retention-memory-overhead.md` | EG-B2 | 0450 |
| `TEST-SPEC-EG-B3-bench-join-round-overhead-delta.md` | EG-B3 | 0446, 0450 |

### Coverage completeness

- Every TEST-SPEC-04XX forward-referenced from a TASK-04XX has a file (10/10).
- Every EG-U / EG-I / EG-P / EG-B id in SPEC-20 §7 has a file (33 + 10 + 6 + 3 = 52/52).
- ARG-006 empirical-signature tests EG-I3, EG-I5a, EG-P2, EG-P5 each cite ARG-006 in their description per `theory-bridge.md`.
- Determinism strategies are documented in every test that touches BSP scheduling, the join-window, departure timing, or `tokio::select!` arms.

Total active TEST-SPEC files (SPEC-20 sub-section): **62**.

---

## SPEC-22 Arena Management (active bundle, Stage 2 deliverable)

Source: `specs/SPEC-22-arena-management.md` (Reviewed v2, Round 2 closed 2026-04-25).
Spec reviews: `docs/spec-reviews/SPEC-REVIEW-22-round-2-2026-04-25.md`, `docs/spec-reviews/SPEC-REVIEW-22-round-1-2026-04-24.md`.
Theory anchors: REF-002 (Lafont 1997), REF-003 (HVM2), REF-014 (Kahl); AC-001, AC-006, AC-009, AC-011, AC-015; ARG-002, ARG-005. See `docs/theory-bridge.md`.

### Plumbing / mechanical TEST-SPECs (numbered, one per task)

| File | Task | Subject |
|------|------|---------|
| `TEST-SPEC-0471-net-free-list-field.md` | TASK-0471 | `Net.free_list` field + constructor init (R1, R8) |
| `TEST-SPEC-0472-create-agent-free-list-pop.md` | TASK-0472 | `create_agent` recycle path (R3, R4, R5) |
| `TEST-SPEC-0473-remove-agent-free-list-push.md` | TASK-0473 | `remove_agent` push + freeport_redirects purge (R2, R7) |
| `TEST-SPEC-0474-free-list-no-duplicates.md` | TASK-0474 | R6 no-duplicates closure (closes SC-018) |
| `TEST-SPEC-0475-free-list-serde.md` | TASK-0475 | Free-list serde + bincode round-trip (R9) |
| `TEST-SPEC-0476-protocol-version-bump.md` | TASK-0476 | PROTOCOL_VERSION bump + v3-vs-v2 rejection (R9a; defensive landing-order-aware) |
| `TEST-SPEC-0477-count-live-agents-free-list-exclusion.md` | TASK-0477 | `count_live_agents` excludes free-list (R11) |
| `TEST-SPEC-0478-bitmap-free-list-fallback.md` | TASK-0478 | M5 bitmap fallback (R32) |
| `TEST-SPEC-0480-per-worker-id-range-recycle.md` | TASK-0480 | Per-worker `id_range` defensive check (R10) |
| `TEST-SPEC-0481-build-subnet-free-list-per-partition.md` | TASK-0481 | `build_subnet` partition free-list (R10a) |
| `TEST-SPEC-0482-recycle-policy-border-graph.md` | TASK-0482 | `RecyclePolicy` + Strategy A/B + protected tombstones (R10b/R10c) |
| `TEST-SPEC-0483-merge-free-list-reconciliation.md` | TASK-0483 | `merge` free-list reconciliation (R12 / A8) |
| `TEST-SPEC-0484-partition-error-dense-allocation-threshold.md` | TASK-0484 | `DenseAllocationExceedsThreshold` rejection (R30) |
| `TEST-SPEC-0486-sparse-net-struct.md` | TASK-0486 | `SparseNet` struct + constructors (R13, R18, R29) |
| `TEST-SPEC-0487-sparse-net-operations.md` | TASK-0487 | `SparseNet` operations (R14-R17) |
| `TEST-SPEC-0489-net-to-sparse.md` | TASK-0489 | `Net::to_sparse` (R19) |
| `TEST-SPEC-0490-sparse-to-dense-id-range.md` | TASK-0490 | `SparseNet::to_dense(id_range)` partition-scoped (R20; closes SC-006) |
| `TEST-SPEC-0491-is-behaviorally-equal-helper.md` | TASK-0491 | `Net::is_behaviorally_equal` (R21; closes SC-014) |
| `TEST-SPEC-0492-sparse-then-dense-build-subnet.md` | TASK-0492 | Sparse `build_subnet` integration at threshold (R22; closes SC-009) |
| `TEST-SPEC-0493-ci-lint-sparse-net-import.md` | TASK-0493 | CI lint forbidding SparseNet imports in `src/reduction/**` (R23) |
| `TEST-SPEC-0495-i3-prime-debug-assertions.md` | TASK-0495 | I3' debug assertion families (R24, R25, R27) |
| `TEST-SPEC-0496-sparse-net-debug-assertions.md` | TASK-0496 | SparseNet T1/I1/I2 assertions (R26) |
| `TEST-SPEC-0497-spec03-reduction-assertion-audit.md` | TASK-0497 | SPEC-03 in-rule assertion audit (R27a; closes SC-010) |
| `TEST-SPEC-0498-safe-rust-only-audit.md` | TASK-0498 | Safe-Rust-only audit (R31) |
| `TEST-SPEC-0500-v1-backward-compat-regression.md` | TASK-0500 | Bundle-gate regression (R28, R29) |

**Phase A predecessor-spec amendments** (TASK-0460..0469) and the **compile-time only** TASK-0488 do NOT have separate TEST-SPEC files — they are pure spec-text changes (A1..A10) or compile-time assertions, with their behavioral validation transitively covered by spec-catalog tests T1..T18 (mapping documented in each TASK-046X.md `## Test Expectations` section). TASK-0488's `static_assertions::assert_impl_all!` is the test (compile-error-on-failure); no markdown spec needed.

### Spec-catalog tests (SPEC-22 §7.1 — free-list, §7.2 — SparseNet)

| File | T-ID | Owning task(s) |
|------|------|---------------|
| `TEST-SPEC-T1-basic-recycling.md` | T1 | 0472, 0473 |
| `TEST-SPEC-T2-lifo-ordering.md` | T2 | 0472, 0474 |
| `TEST-SPEC-T3-free-list-exhaustion.md` | T3 | 0472 |
| `TEST-SPEC-T4-port-slot-reinitialization.md` | T4 | 0472, 0473 |
| `TEST-SPEC-T5-reduction-with-recycling.md` | T5 | 0473 |
| `TEST-SPEC-T6-commutation-recycling.md` | T6 | 0472, 0473 |
| `TEST-SPEC-T7-invariant-t1-after-recycling.md` | T7 | 0495 |
| `TEST-SPEC-T7a-condup-partial-free-list.md` | T7a | 0497 (closes SC-010) |
| `TEST-SPEC-T8-serialization-round-trip.md` | T8 | 0475, 0491 |
| `TEST-SPEC-T8a-wire-version-rejection.md` | T8a | 0476 (closes SC-007) |
| `TEST-SPEC-T9-distributed-id-range-compliance.md` | T9 | 0480, 0481 |
| `TEST-SPEC-T9a-bordergraph-strategy-a-protected-tombstone.md` | T9a | 0482 (closes SC-005, Strategy A) |
| `TEST-SPEC-T9b-bordergraph-strategy-b-border-clean.md` | T9b | 0482 (closes SC-005, Strategy B) |
| `TEST-SPEC-T10-free-list-no-duplicates.md` | T10 | 0474 (closes SC-018) |
| `TEST-SPEC-T11-sparse-construction-and-count.md` | T11 | 0487 |
| `TEST-SPEC-T12-sparse-bidirectionality.md` | T12 | 0487, 0496 |
| `TEST-SPEC-T13-sparse-era-cleanliness.md` | T13 | 0487 |
| `TEST-SPEC-T14-conversion-round-trip-dense-sparse-dense.md` | T14 | 0489, 0490, 0491 (closes SC-014, SC-001 second surface) |
| `TEST-SPEC-T14a-partition-scoped-to-dense.md` | T14a | 0490 (closes SC-006) |
| `TEST-SPEC-T15-conversion-round-trip-sparse-dense-sparse.md` | T15 | 0489, 0490 |
| `TEST-SPEC-T16-sparse-build-subnet-g1.md` | T16 | 0492, 0483, 0489, 0490 (load-bearing G1 closure for SPEC-22) |
| `TEST-SPEC-T17-sparse-redex-detection.md` | T17 | 0487 |
| `TEST-SPEC-T18-sparse-serialization-round-trip.md` | T18 | 0486 |

### Coverage completeness — SPEC-22

- Every TEST-SPEC-04XX/05XX forward-referenced from a SPEC-22 TASK-046X..050X has a file (25/25; the gaps at 0470, 0479, 0485, 0494, 0499 are intentional; TASK-0488 is compile-time only; TASK-0460..0469 + TASK-0467 + TASK-0469 are amendment-only and route to spec-catalog T-tests).
- Every spec-catalog test ID in SPEC-22 §7.1 / §7.2 (T1..T18 + T7a + T8a + T9a + T9b + T14a) has a TEST-SPEC file (23/23).
- Closure-flag coverage: SC-001 second surface (T14, T14a, TEST-SPEC-0489, 0490, 0473), SC-005 (T9a, T9b), SC-006 (T14a, TEST-SPEC-0490), SC-007 (T8a, TEST-SPEC-0476), SC-008 (TEST-SPEC-0493), SC-009 (T16, TEST-SPEC-0492, 0484), SC-010 (T7a, TEST-SPEC-0497), SC-014 (T8, T14, TEST-SPEC-0491), SC-015 (TEST-SPEC-0478), SC-017 (TEST-SPEC-0498), SC-018 (T10, TEST-SPEC-0474).
- Theory citations: REF-002 / REF-003 / REF-014 / AC-001 / AC-006 / AC-009 / AC-011 / AC-015 / ARG-002 / ARG-005 — all present in `docs/theory-bridge.md`; cited only where applicable per the spec's Theory-bridge anchors.
- Determinism strategies documented in every test that touches BSP scheduling, free-list ordering, serde state, or `tokio::select!` arms (T8a wire-handshake, T9a/T9b BorderGraph simulation, T16 G1 reduction, TEST-SPEC-0476 PROTOCOL_VERSION landing-order-aware contract).

Total active TEST-SPEC files (SPEC-22 sub-section): **48** (25 plumbing + 23 spec-catalog).
**Combined active TEST-SPEC files (SPEC-20 + SPEC-22): 110.**

---

## Archive

`archive/` holds TEST-SPECs from previously shipped bundles (TEST-SPEC-0001..0030, 0383..0403). Do not edit.
