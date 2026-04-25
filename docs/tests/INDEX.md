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

Total active TEST-SPEC files: **62**.

---

## Archive

`archive/` holds TEST-SPECs from previously shipped bundles (TEST-SPEC-0001..0030, 0383..0403). Do not edit.
