# D-005 Option A — Stages 4–6 Orchestration Plan (CON-DUP strict=false)

> **For agentic workers:** this is an *orchestration* plan (agent dispatches + diagnostic checkpoints), not a pure TDD implementation plan. The REFACTOR step (Stage 6) will follow `superpowers:test-driven-development` once the REVIEW + QA findings localize the root cause.

**Goal:** Close the 1/12 residual G1-parity failure (`UT-0385-08 CON-DUP strict=false`) by driving the SDD pipeline through Stages 4 (REVIEW, diagnostic-first), 5 (QA, adversarial), and 6 (REFACTOR), then flip D-003/D-004/D-005 to CLOSED in `DEFERRED-WORK.md`.

**Architecture:** Three sequential agent dispatches (reviewer → qa → developer), each with surgical scope. Reviewer's root-cause hypothesis feeds QA's adversarial probes, QA's findings feed the developer's refactor. Checkpoints enforce no stage skipping and protect the existing 11/12 green fixtures from regression.

**Tech stack:** Rust workspace (`relativist-core`), `cargo test` (default + `--features zero-copy`), clippy `-D warnings`, rustfmt, `canonicalize_net` comparator (SPEC-19 §3.7 G1).

---

## Baseline (snapshot at plan time — commit `7c07963`)

- Tests: **1168 / 1169** default, **1211 / 1212** `--features zero-copy`. Clippy + fmt clean.
- Failing fixture: `merge::grid_delta_integration_tests::run_grid_delta_result_matches_run_grid_under_both_strict_modes` at `CON-DUP strict=false`.
- Delta path produces **empty net**; v1 produces 4-agent residue (2 Con + 2 Dup cross-wired to FreePort 100/101/110/111).
- Upstream UTs (TASK-0402 `apply_pending_commutation`) are **green**, so minted agents land in the worker's partition post-wire. Regression isolates to the `FinalStateRequest → cleanup_t1_violations → merge` pipeline on this one fixture × strict-mode combination.

## Working hypotheses (to be confirmed/refuted by reviewer)

- **H1 — cleanup_t1_violations false-positive:** CON-DUP minted principals land with `PortRef::FreePort(BORDER_ID)`, not `DISCONNECTED`. But if a transient intermediate state (e.g., pre-wire cache snapshot) is what gets passed to cleanup, the minted agents may look DISCONNECTED and be dropped.
- **H2 — strict-mode asymmetry in resolver:** strict=false may take a different code path in `package_resolutions_with_pending` (lenient duplicate handling, R48 path) that fails to populate `local_wiring` for the CON-DUP case.
- **H3 — merge() drops cross-partition minted edges:** `reconstruct_partition_plan_from_collected` rebuilds borders from `BorderGraph` only; if CON-DUP's minted principals point across partitions and the BorderGraph entry was annihilated mid-round, the merge loses them.
- **H4 — slot-marker residue:** `SLOT_MARKER_BASE` placeholders persist into the worker's partition in the lenient-mode retry path, tripping a T1 guard downstream.

One, several, or none of these may hold — reviewer reports back with evidence, not speculation.

---

## Stage 4 — REVIEW (diagnostic-first)

**Agent:** `reviewer` (subagent_type: `reviewer`, model: **opus** — diagnostic complexity).
**Parallelizable:** No — single coherent diagnostic.
**Deliverable:** `docs/reviews/REVIEW-D-005-2026-04-24.md` with: root-cause identification, minimal repro steps, suggested fix location (file:line), blast radius on the other 11 green fixtures, and a binary decision (can Stage 5 QA proceed, or must a re-diagnosis loop happen first?).

**Scope handed to reviewer:**
- `relativist-core/src/merge/grid.rs::run_grid_delta_final_collect` (L806–850)
- `relativist-core/src/merge/grid.rs::reconstruct_partition_plan_from_collected` (L861–874)
- `relativist-core/src/merge/grid_delta_integration_tests.rs::dispatch_final_state_request` (L201–238) + `cleanup_t1_violations` (L252–271)
- `relativist-core/src/merge/grid_delta_integration_tests.rs::LocalDeltaDispatch::dispatch_round_start` (L86–199)
- `relativist-core/src/worker.rs::apply_pending_commutation` (L337–491) + `handle_final_state_request` (L720–741)
- `relativist-core/src/merge/border_resolver.rs::commutation_batch_to_pending`
- `relativist-core/src/merge/core.rs::merge` (reference only — pure, SPEC-05)

**Instrumented repro (reviewer adds then reverts):** insert `tracing::debug!` around (a) `apply_pending_commutation` exit with `minted_ids_per_pc` and their principal port resolution; (b) `cleanup_t1_violations` per-agent drop decision; (c) `reconstruct_partition_plan_from_collected` borders count + agents per partition. Run the single failing fixture; diff with a green fixture (e.g., CON-CON strict=false).

**Gate for Stage 5:** Review document exists, names one root-cause hypothesis with evidence (trace diff, code citation, or counter-example), and either (i) pinpoints the fix surface or (ii) hands QA an adversarial contract to probe.

---

## Stage 5 — QA (adversarial)

**Agent:** `qa` (subagent_type: `qa`, model: **opus** if REVIEW left uncertainty; **sonnet** if REVIEW already pinpointed a one-line fix — adversarial scan is cheaper then).
**Parallelizable:** Up to two sonnet QA passes in parallel — one widening scope across the resolver lenient path, one across the worker-side pipeline — if reviewer hands back two disjoint hypotheses.
**Deliverable:** `docs/qa/QA-D-005-2026-04-24.md` bug report. Each finding: severity (CRITICAL/HIGH/MEDIUM/LOW), repro (preferably a new test case), expected vs. actual, suggested fix surface, blast-radius note.

**Scope:** Adversarial probing of reviewer's top-hypothesis code path. Mandatory probes:
- **P1:** CON-DUP strict=false and strict=true side-by-side (the one failing + the one passing) — diff the pipeline state.
- **P2:** R48 stray-slot guard under lenient mode with duplicate border resolutions (R23a clause 6 path).
- **P3:** Slot-marker residue check (SLOT_MARKER_BASE placeholders must never persist into `Partition.subnet`).
- **P4:** `cleanup_t1_violations` over-aggressive drops — synthetic partition with freshly minted CON-DUP cross-wire, assert all 4 survive cleanup.
- **P5:** `reconstruct_partition_plan_from_collected` edge reconstruction — synthetic 2-partition plan with cross-partition minted edges, assert borders map survives merge.

**Gate for Stage 6:** QA doc lists 0 CRITICAL residuals OR every CRITICAL has a reproducible test case the developer can drive.

---

## Stage 6 — REFACTOR

**Agent:** `developer` (subagent_type: `developer`, model: **opus** — code change must preserve 1168/1211 and fix the 1/12).
**Parallelizable:** No — single coherent change set.
**Deliverable:** Code fix on `v2-development` that:
1. Makes `UT-0385-08 CON-DUP strict=false` green (expected: 4-agent residue, `canonicalize(out_delta) == canonicalize(out_v1)`, `metrics.total_interactions` exact match).
2. Keeps the other 11 fixtures green.
3. Absorbs the 3 NR3 LOW items if still not absorbed:
   - NR3-001: prose `arity` → `pc.target_symbols.len()` in one R23a clause of the spec.
   - NR3-002: R37 wording sharpen (`DeserializationFailed` vs. `MalformedLocalWiring` dispatch).
   - NR3-003: optional 8th `MalformedLocalWiringReason::TargetSymbolsTooLong` (coordinator-side bound).
4. All QA CRITICAL/HIGH findings addressed, MEDIUM either addressed or explicitly deferred with justification.

**Discipline (from `superpowers:test-driven-development`):**
- Red → Green → Refactor per finding.
- No new `unwrap()`; `tracing` only; `thiserror` continues.
- `cargo test --workspace --lib` ≥ **1169 / 1212** (one more than baseline on each config for the now-green fixture, subject to QA probes that may add UTs).
- `cargo clippy -D warnings` and `cargo fmt --check` both configs.

**Gate for close:** All tests green both configs, clippy+fmt clean, reviewer endorses (either async re-review or the original reviewer signs off on the patched diff).

---

## Stage 6 tail — Housekeeping (fast, sonnet parallel)

After REFACTOR lands + tests green, fire two sonnet agents in parallel:

- **HK-A** (subagent_type: `general-purpose`, sonnet): Flip D-003, D-004, D-005 rows in `docs/DEFERRED-WORK.md` to CLOSED, cite the closing commit hash, and update `docs/progress.md` with the 12/12 gate result.
- **HK-B** (subagent_type: `sdd-pipeline`, inherit): Update `docs/pipeline-state.md` to mark D-005 bundle DONE, archive to "Prior Bundle", and queue the next bundle (M1 exit measurement OR Phase 3 LAN prep, per ROADMAP 2.40).

---

## Verification checklist (run before claiming close, per `superpowers:verification-before-completion`)

- [ ] `cargo test --workspace --lib` exits 0, count ≥ 1169 (default)
- [ ] `cargo test --workspace --lib --features zero-copy` exits 0, count ≥ 1212
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` exits 0
- [ ] `cargo fmt --check` exits 0
- [ ] `UT-0385-08` green on **6 fixtures × 2 strict modes** (12/12)
- [ ] `docs/DEFERRED-WORK.md` shows D-003/D-004/D-005 as CLOSED
- [ ] `docs/pipeline-state.md` shows D-005 bundle archived
- [ ] `docs/progress.md` updated with closing note + commit hash
- [ ] Commit on `v2-development` titled `feat: D-005 Option A CLOSED — G1 parity 12/12 green`

---

## Execution order

1. Dispatch **Stage 4 reviewer** (opus, foreground — blocking).
2. On reviewer sign-off with root-cause hypothesis, dispatch **Stage 5 qa** (opus or parallel-sonnet pair).
3. On QA sign-off, dispatch **Stage 6 developer** (opus, foreground).
4. On green tests, dispatch **HK-A + HK-B in parallel** (sonnet).
5. Final verification & single-commit close.
