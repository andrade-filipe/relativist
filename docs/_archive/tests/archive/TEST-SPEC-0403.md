# TEST-SPEC-0403: LocalDeltaDispatch forwarding + `SKIP_ASYMMETRIC = false` canary flip — bundle acceptance gate

**See also:** [docs/backlog/TASK-0403.md](../backlog/TASK-0403.md); [TEST-SPEC-0400](./TEST-SPEC-0400.md); [TEST-SPEC-0401](./TEST-SPEC-0401.md); [TEST-SPEC-0402](./TEST-SPEC-0402.md); [TEST-SPEC-0385](./TEST-SPEC-0385.md) UT-0385-06/07/08.

**Task:** TASK-0403
**Parent spec:** SPEC-19 §3.3 R23/R23a/R24 (acceptance end-to-end), §3.4 R31/R32 (`Message::RoundStart` / `Message::RoundResult` payload parity), §3.5 R38 (G1 parity), §3.6 R48/R48a/R48b, §9 Change Log (close D-005 Option A, 2026-04-23).
**Bundle:** D-005 Option A — CLOSE SIGNAL.
**Date:** 2026-04-23
**Baseline before this task:** 1169 lib default / 1212 lib `--features zero-copy` (post-TASK-0402).
**Cumulative target after this task:** **≥ 1169 / ≥ 1212** (no new unit tests; TASK-0403 flips `SKIP_ASYMMETRIC` which activates pre-existing UT-0385-08 asymmetric branches within an unchanged test count).

**Invoker targets (per prompt):** `≥ 1151 / ≥ 1192`. Actual targets (1169 / 1212) comfortably exceed invoker targets, because TASK-0400/0401/0402 add net +23 tests vs. the +5 / +6 invoker minima.

---

## Scope

### Covers
1. `LocalDeltaDispatch` forwarding: verifies the in-crate test-only worker dispatch populates `target_symbols` + `local_wiring` identically to the wire path.
2. `SKIP_ASYMMETRIC = false` canary flip: exposes UT-0385-08's asymmetric branches (CON-DUP, CON-ERA, DUP-ERA).
3. Bundle acceptance: 6 fixtures × 2 strict modes = **12 parameterized cases** of UT-0385-08 — all 12 MUST pass with canonicalize + metrics parity.
4. Regression canary: UT-0385-06/07 (symmetric-rule fixtures) stay green post-flip.
5. Cross-path equivalence: the LocalDeltaDispatch test path uses the SAME transport as the TCP wire path — shared `commutation_batch_to_pending` helper (Option A) or inline copy with sync note (Option B).

### Does NOT cover
- Adding NEW `#[test]` fns. TASK-0403 changes test activation (via the const flip), not test enumeration.
- TCP live-socket testing.
- Performance metrics (break-even, throughput) — those are Passo 6 M1 exit measurement, out of bundle.

---

## Test target file paths

- `relativist-core/src/merge/grid_delta_integration_tests.rs` — modify `LocalDeltaDispatch::dispatch_round_start` (or analogue); flip `const SKIP_ASYMMETRIC: bool = false;`; replace 34-line TASK-0399 skip-comment with 3-line D-005 close citation.
- `relativist-core/src/merge/border_resolver.rs` — only if Option A chosen: promote `commutation_batch_to_pending` helper to `pub(crate)`.

No new test files. No new `#[test]` functions.

---

## Integration Tests

### IT-0403-01: `ut_0385_08_matrix_green_on_all_six_fixtures_both_strict_modes`

**Purpose:** Bundle acceptance gate. UT-0385-08 (`test_delta_vs_v1_parity`) is a parameterized test with 6 fixtures × 2 strict modes = 12 cases. After `SKIP_ASYMMETRIC = false`, all 12 MUST pass — the 3 asymmetric fixtures (CON-DUP, CON-ERA, DUP-ERA) are the new territory; the 3 symmetric fixtures (CON-CON, DUP-DUP, ERA-ERA) are regression canaries.

**Target:** `grid_delta_integration_tests.rs::test_delta_vs_v1_parity` — EXISTING test; TASK-0403 activates the skipped branches.

**Fixture enumeration (6, verify against actual test code):**
1. CON-CON border fixture — symmetric rule (regression canary).
2. CON-DUP border fixture — asymmetric (previously SKIPPED, now REQUIRED).
3. CON-ERA border fixture — asymmetric (previously SKIPPED, now REQUIRED).
4. DUP-DUP border fixture — symmetric (regression canary).
5. DUP-ERA border fixture — asymmetric (previously SKIPPED, now REQUIRED).
6. ERA-ERA border fixture — symmetric (regression canary).

**Given (per case):**
- An input `Net` from the fixture builder (e.g., `build_fixture_con_dup()`).
- `GridConfig { num_workers: 2, delta_mode: <case-dependent>, ... }`.
- Both strict modes iterate over (strict=true, strict=false) for each fixture.

**When:**
```
let (out_v1,    metrics_v1)    = run_grid(net.clone(), &cfg_v1);
let (out_delta, metrics_delta) = run_grid_delta(net.clone(), &cfg_delta, &ContiguousIdStrategy, &mut local_dispatch);
```

**Then (per case):**
- `metrics_v1.converged == true`.
- `metrics_delta.converged == true`.
- `canonicalize(out_v1) == canonicalize(out_delta)` — structural net equivalence under canonical node renaming.
- `metrics_v1.total_interactions == metrics_delta.total_interactions` — exact equality of total interaction counts.
- **Per-rule parity (if `metrics.interactions_by_rule` is available):** `metrics_v1.interactions_by_rule[r] == metrics_delta.interactions_by_rule[r]` for each `r ∈ {CON-CON, CON-DUP, CON-ERA, DUP-DUP, DUP-ERA, ERA-ERA}`.
- **Diagnostic on failure (per DC-0395-C):** the panic message includes `"fixture={name} strict={strict}"` so CI output pinpoints which of the 12 cases regressed.

**Edge cases:**
- EC-1: CON-DUP fixture with partition boundary on the CON aux (not principal) — should still trigger R24.1.6a/b/c via the principal-principal redex that emerges post-CON-DUP commute. G1 parity still holds.
- EC-2: ERA-ERA at the border — tests the symmetric-rule canary under asymmetric-branch-enabled compilation (SKIP_ASYMMETRIC = false with a symmetric fixture).

**SPEC covered:**
- §3.3 R23/R23a/R24 end-to-end.
- §3.5 R38 G1 parity reformulation (full 6-rule coverage).
- §3.6 R48a/R48b exercised by every asymmetric fixture.
- §3.4 R31/R32 wire-payload parity for `RoundStart` + `RoundResult`.

**Priority:** MANDATORY — this IS the bundle acceptance gate.

---

### IT-0403-02: `local_delta_dispatch_uses_shared_transport_with_wire_path`

**Purpose:** Verify the LocalDeltaDispatch path and the production wire path populate `PendingCommutation` identically. Prevents drift between test-only and production codepaths — which would mask bugs.

**Target:** Either (a) an explicit smoke test in `grid_delta_integration_tests.rs` (cheap, developer discretion), or (b) code review / compile-time inspection if Option A was selected (shared helper `commutation_batch_to_pending`).

**Given (if Option A):**
- `border_resolver.rs::commutation_batch_to_pending` is `pub(crate)`.
- `LocalDeltaDispatch::dispatch_round_start` (or analogue) calls the SAME helper.
- Wire path (the production `package_resolutions_with_pending` call site) ALSO calls the same helper.

**When:** Inspect compile-time — no runtime test required. Stage 4 REVIEW audits the call graph.

**Then:**
- Two call sites, one helper — drift-free.

**Given (if Option B inline copy):**
- LocalDeltaDispatch uses an inline `.iter().map().collect()` conversion.
- A TODO-sync comment pinning the two sites (LocalDeltaDispatch + `package_resolutions_with_pending`) exists.
- Stage 4 REVIEW audits parity.

**When:** Construct a hand-crafted `CommutationBatch`, produce a PC via both paths, `assert_eq!`.

**Then:**
- The two paths produce byte-equivalent `PendingCommutation` values.

**Priority:** OPTIONAL as a runtime test (Option B). MANDATORY as a code review check (both options). If Option A is chosen (preferred), this test is implicit via compile-time one-helper-two-callsites.

---

### IT-0403-03 (optional smoke): `run_grid_delta_con_dup_observable_round_count_and_canonical_parity`

**Purpose:** Adversarial smoke test complementing IT-0403-01. Not strictly required for acceptance, but provides a diagnostic anchor if UT-0385-08 fails on the CON-DUP fixture — this test isolates CON-DUP in a smaller scope for faster debugging.

**Target:** new optional `#[test]` fn in `grid_delta_integration_tests.rs`.

**Given:** `build_fixture_con_dup()` fixture.

**When:**
```
let (out_v1,    m_v1)    = run_grid(net.clone(), &cfg);
let (out_delta, m_delta) = run_grid_delta(net.clone(), &cfg, &ContiguousIdStrategy, &mut local_dispatch);
```

**Then:**
- `m_delta.converged == true && m_v1.converged == true`.
- `canonicalize(out_v1) == canonicalize(out_delta)`.
- `m_delta.total_interactions == m_v1.total_interactions`.
- `m_delta.rounds >= 3` (DC-B5 3-round cycle: emit → mint → finalize).

**Priority:** OPTIONAL. Adds 1 test to the baseline. Developer MAY include for diagnostic ergonomics.

---

## Unit Tests

None at this task level. TASK-0403's production footprint is "flip a const + migrate one dispatch path"; there is no new executable logic worth a unit-scale probe beyond what IT-0403-01 and IT-0403-02 cover.

## Property Tests

None at this task level. The existing 6-fixture matrix is the representative coverage; additional property-level coverage of G1 parity across arbitrary random nets is future work (see "Adversarial angles" below).

---

## Negative Tests

None at this task level. The positive-acceptance signal is IT-0403-01 all-green. A failure in any of the 12 cases IS the negative signal — and the fix is NOT in TASK-0403 (it's in the predecessor that broke). See "If UT-0385-08 fails" in TASK-0403's notes section.

---

## Edge Cases Catalog

| # | Scenario | Expected Behavior | Test |
|---|----------|-------------------|------|
| EC-0403-01 | CON-DUP fixture, strict=true | G1 parity full | IT-0403-01 Case 2 |
| EC-0403-02 | CON-DUP fixture, strict=false | G1 parity full | IT-0403-01 Case 2 |
| EC-0403-03 | CON-ERA fixture, strict=true | G1 parity full | IT-0403-01 Case 3 |
| EC-0403-04 | CON-ERA fixture, strict=false | G1 parity full | IT-0403-01 Case 3 |
| EC-0403-05 | DUP-ERA fixture, strict=true | G1 parity full | IT-0403-01 Case 5 |
| EC-0403-06 | DUP-ERA fixture, strict=false | G1 parity full | IT-0403-01 Case 5 |
| EC-0403-07 | CON-CON fixture, strict=true (regression canary) | G1 parity preserved | IT-0403-01 Case 1 |
| EC-0403-08 | DUP-DUP fixture, strict=true (regression canary) | G1 parity preserved | IT-0403-01 Case 4 |
| EC-0403-09 | ERA-ERA fixture, strict=true (regression canary) | G1 parity preserved | IT-0403-01 Case 6 |
| EC-0403-10 | UT-0385-06 standalone | Still green | Existing test |
| EC-0403-11 | UT-0385-07 standalone | Still green | Existing test |

---

## Coverage mapping

| Requirement / contract | Covered by |
|------------------------|-----------|
| SPEC-19 R23 wire payload content end-to-end | IT-0403-01 (all 12 cases) |
| SPEC-19 R23a all 5 clauses + clause 6 | IT-0403-01 (exercised via worker R24.1.6a/b applied to resolver-produced PCs) |
| SPEC-19 R24 ordering invariant in full BSP loop | IT-0403-01 (implicit; if ordering were violated, metrics parity would drift) |
| SPEC-19 R31/R32 Message::RoundStart/RoundResult payload parity | IT-0403-01 via LocalDeltaDispatch (uses same types) |
| SPEC-19 R33/R33c worker-side dispatch | IT-0403-01 (positive path; R33c rejection paths covered at TEST-SPEC-0402) |
| SPEC-19 R37 PROTOCOL_VERSION | UT-0400-05 + handshake tests (unchanged by this task) |
| SPEC-19 R38 G1 parity full-6-rule reformulation | IT-0403-01 |
| SPEC-19 R48 correlation-key end-to-end | IT-0403-01 (via `MintedAgent` echo consumed by BorderGraph) |
| SPEC-19 R48a stray slot-marker (negative) | TEST-SPEC-0402 UT-0402-03 (unit-level); IT-0403-01 asserts absence of stray strays in correct fixtures |
| SPEC-19 R48b empty local_wiring (legal) | TEST-SPEC-0402 UT-0402-08 (unit-level); IT-0403-01 fixtures may or may not exercise (developer discretion) |
| DEFERRED-WORK D-003 asymmetric rules closure | IT-0403-01 |
| DEFERRED-WORK D-004 round-N+2 finalizer full-exercise | IT-0403-01 (register_minted_agents invoked end-to-end on asymmetric fixtures) |
| DEFERRED-WORK D-005 bundle closure | IT-0403-01 |

---

## Test count estimate

| Config | Baseline | Delta | Target | Invoker target |
|--------|----------|-------|--------|----------------|
| `cargo test --workspace --lib` | 1169 | 0 (test count unchanged; 12 parameterized cases re-enabled within existing fn) | **1169** | ≥ 1151 |
| `cargo test --workspace --lib --features zero-copy` | 1212 | 0 | **1212** | ≥ 1192 |

If IT-0403-03 (optional smoke) is included: +1 both configs → 1170 / 1213.

**Note on counting:** `cargo test` reports the number of `#[test]` functions, not parameterized cases. UT-0385-08 is ONE `#[test]` fn internally iterating over 12 cases (likely via a hand-rolled loop + diagnostic string). So the count delta is 0; the WORK is inside UT-0385-08's body.

---

## Acceptance gate (bundle close)

All of the following MUST hold for D-005 Option A to CLOSE:

1. `cargo test --workspace --lib` count ≥ 1169 (invoker minimum ≥ 1151; comfortable margin).
2. `cargo test --workspace --lib --features zero-copy` count ≥ 1212 (invoker minimum ≥ 1192).
3. `cargo test merge::grid_delta_integration_tests::test_delta_vs_v1_parity -- --nocapture` — all 12 parameterized cases pass (6 fixtures × 2 strict modes).
4. `cargo clippy --workspace --all-targets -- -D warnings` clean (default + zero-copy).
5. `cargo fmt --check` clean.
6. UT-0385-06/07 still green (symmetric-rule canary).
7. `const SKIP_ASYMMETRIC: bool = false;` at the top of `grid_delta_integration_tests.rs`.
8. The 34-line TASK-0399 skip-comment replaced with 3-line D-005 close citation.
9. `LocalDeltaDispatch` PC construction populates `target_symbols` + `local_wiring` (via Option A shared helper, preferred; or Option B inline copy with TODO-sync).
10. **DEFERRED-WORK.md:** D-003, D-004, D-005 all MOVE to "Resolved Deferrals" section with the TASK-0403 commit hash (handoff to sdd-pipeline).
11. **V2-FEATURE-MATRIX.md:** row 2.26 = `DONE`; M4 criterion de-PARTIAL-ized (handoff).

---

## Failure triage (if IT-0403-01 fails)

This is NOT a TASK-0403 bug. It means a gap exists in TASK-0400, TASK-0401, or TASK-0402 that slipped past its own Stage 2/3/4/5 gates. Escalation path per TASK-0403 §Notes:

1. Capture the failing canonicalize/metrics diff: `cargo test ... -- --nocapture 2>&1 | tee failure.log`.
2. Isolate which PC-application step broke using the diagnostic message `fixture={name} strict={strict}`:
   - If the failure is a panic inside `apply_pending_commutation` → TEST-SPEC-0402 gap (likely UT-0402-01..07 missed a case variant).
   - If the canonicalize diff shows an EXTRA agent in `out_delta` that's not in `out_v1` → the mint count is wrong (R24.1.6a bug).
   - If the canonicalize diff shows a DISCONNECTED port where `out_v1` has an edge → R24.1.6b wire step bug (possibly slot-marker decode: TEST-SPEC-0402 UT-0402-03 or UT-0402-05 or UT-0402-11).
   - If `metrics.total_interactions` diverges but structural canonicalize holds → R24 ordering invariant violation (TEST-SPEC-0402 UT-0402-09).
   - If `minted_agent_id` in response doesn't match coordinator expectation → R24.1.6c echo bug (TEST-SPEC-0402 UT-0402-10).
3. File a BLOCK back to the appropriate predecessor task.
4. DO NOT mask the failure by reverting `SKIP_ASYMMETRIC = true`. The flip IS the ship signal; reverting it means shipping broken code masked by a skipped test.

---

## Notes on interaction with other TEST-SPECs

- **TEST-SPEC-0400:** wire-layer foundation. If UT-0400-01..09 green, the PCs transported end-to-end are byte-correct.
- **TEST-SPEC-0401:** resolver populator. If UT-0401-01..06 green, the PCs produced are semantically correct.
- **TEST-SPEC-0402:** worker consumer. If UT-0402-01..11 green, the PCs decoded are applied correctly.
- **TEST-SPEC-0398:** D-004 plumbing. UT-0398-03/04/05 exercised end-to-end inside IT-0403-01 (the coordinator consumes the `MintedAgent` echoes workers produce under asymmetric fixtures).
- **TEST-SPEC-0399:** D-004 integration. UT-0385-08 was the closure mechanism introduced there; TASK-0403 flips the asymmetric gate.
- **TEST-SPEC-0385:** UT-0385-06/07/08 are the test ids activated by this task.

---

## Adversarial angles (for Stage 5 QA, NOT covered here)

| # | Scenario | Notes |
|---|----------|-------|
| QA-0403-A | Nested asymmetric redexes: a CON-DUP whose commuted siblings immediately form a CON-ERA on the next round | Tests round-N+2 + round-N+3 pending propagation. UT-0385-08 may exercise indirectly via fixture content. |
| QA-0403-B | 4-worker grid with asymmetric fixtures | D-005 Option A is validated at 2-worker scope; 4-worker is orthogonal but NOT a D-005 regression; flag for Stage 5. |
| QA-0403-C | `cascade_cross` fixture-family with deep asymmetric chains | Tests multi-round DC-B5 accumulation; UT-0385-08 fixtures may include; Stage 5 QA should verify enumeration. |
| QA-0403-D | Worker that echoes a valid `MintedAgent.minted_agent_id` but the id is outside its `id_range` | R48 worker-invariant check; Stage 5 adversarial harness. |
| QA-0403-E | `SKIP_ASYMMETRIC` accidentally re-flipped to `true` during REFACTOR | Stage 4 REVIEW should grep; CI sentinel via UT-0400-05 analog for this file OPTIONAL. |
| QA-0403-F | Property-level random-net G1 parity | Future work; not blocking. |

---

## Out of scope

- **New unit tests.** TASK-0403 is a const flip + transport migration; no new logic to unit-test.
- **Performance measurement.** Passo 6 M1 exit is a separate bundle.
- **TCP binding.** Production worker-over-real-network is a separate bundle (DC-C2 deferred).
- **rkyv serialization of `pending_new_borders` / `resolved_mints`** — coordinator-local; does not cross the wire.
- **DEFERRED-WORK.md edits** — handoff to sdd-pipeline, not TEST-SPEC-0403 territory.
- **V2-FEATURE-MATRIX.md row 2.26 update** — handoff to sdd-pipeline.
