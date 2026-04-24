# TEST-SPEC-0391: R42 behavioural regression — `delta_mode = false` preserves v1 smoke output exactly

**Task:** TASK-0391
**Spec:** SPEC-19 §3.6 — R42 (the load-bearing MUST of §3.6: "`delta_mode` MUST default to `false` to preserve backwards compatibility. No existing caller, test, or benchmark MUST change behavior when `delta_mode` is not explicitly set.")
**Amendment log ref:** `docs/spec-reviews/SPEC-19-section-3.5-3.6-2.26D-design-choices-2026-04-17.md` (AMB-D-2 — R42 is **behavioural** invariance, NOT source-diff zero; "Option A" ruling). This TEST-SPEC encodes the behavioural interpretation: identical **observable output** (decoded value + metrics) between the pre-bundle baseline and the post-bundle `delta_mode = false` default path.
**Generated:** 2026-04-17
**Baseline before this task:** post TEST-SPEC-0390 — cumulative 973 default lib / 1013 `--features zero-copy` (968 pre-bundle + 2 from TASK-0389 + 3 from TASK-0390).
**Cumulative target after this task:** **+2** new `#[test]` fns — 975 default lib / 1015 `--features zero-copy`. (The task spec allows 1 or 2; this TEST-SPEC elects the 2-test split — one per `delta_mode` polarity — because each polarity has a distinct operational meaning and a shared smoke workload.)

---

## Scope note

TASK-0391 operationalises R42 as a concrete regression smoke: a canonical `run_grid` invocation on a known-small workload produces the exact same decoded result and `total_interactions` count as the frozen v1 baseline, for BOTH polarities of the new `delta_mode` field:

- **`delta_mode = false`** (R42-strict path): MUST match the v1 baseline byte-for-byte at the observable level (decoded value + interaction count). This is the sub-bundle's load-bearing MUST.
- **`delta_mode = true`** (inert-for-now path): 2.26-D ships no consumer for this flag, so setting it to `true` is currently a no-op. This test pins the current observable behaviour (same output as `false`) so that when 2.26-C's `run_grid_delta` lands, the switch to a differentiated runtime is a *deliberate* source edit flagged by this test rather than a silent drift.

**AMB-D-2 framing (behavioural, not source-diff).** Per spec-critic's Option A ruling (2026-04-17), R42 is "no behavioural regression," NOT "zero source diff." TASK-0391 Notes §3 conditional branch about a "strict textual reading" is explicitly STRUCK by the spec-critic verdict; no `git diff --stat` assertion is required. This TEST-SPEC accordingly asserts ONLY on observable smoke output.

**Workload choice.** `church_add(2, 3) → 5` is the canonical SPEC-14 smoke: small, deterministic, fully specified in v1 frozen code (v0.10.0-bench), and the interaction count is a known constant from `results/locked/v1_local_baseline/`. The smoke runs in < 100 ms under `cargo test`.

**Out of scope for this TEST-SPEC:**
- Field presence + default polarity at the type layer → TEST-SPEC-0389.
- CLI flag threading → TEST-SPEC-0390.
- ROADMAP §3.5 narrative amendment (G1/D3/D6) → TEST-SPEC-0392.
- Docstring polish + doctest → TEST-SPEC-0393.
- Larger v1 benchmark replay — handled by the frozen benchmark campaign, not by unit tests.
- Formal proofs of R38/R39 — Section 8 / ARG-005 / DISC-011 (narrative-only here).

---

## Test target file paths

- A single existing smoke-test file hosts both tests. The task spec (line 98) lets the developer choose between `relativist-core/tests/` (integration) and `relativist-core/src/merge/grid.rs::tests` (inline unit). **RECOMMENDED location:** `relativist-core/src/merge/grid.rs::tests`, co-located with the pre-existing `test_run_grid_compute_add` (or equivalent) so the snapshot baseline is self-explanatory.
- **NO new test file is created** unless the existing grid.rs `#[cfg(test)]` module does not yet import `church_add` — in which case the developer may elect to add an integration test under `relativist-core/tests/r42_delta_mode_regression.rs`. Either placement is acceptable (TASK-0391 §Files to Create / Modify, line 98-102); this TEST-SPEC does NOT lock one.

All tests are synchronous `#[test]` units. No `tokio`, no `async`.

---

## Baseline constant (snapshot at bundle landing time)

Before implementing the tests, the developer:

1. Reads the v1 frozen benchmark record for `church_add(2, 3)` at `w = 1` (single worker) from `results/locked/v1_local_baseline/` OR recomputes it locally with `GridConfig::default()` on the current tree (both MUST agree — if they don't, that is a pre-existing regression, not a 2.26-D bug).
2. Hardcodes the `total_interactions` value as a `const CHURCH_ADD_2_3_W1_INTERACTIONS: u64 = <N>;` with a doc-comment naming the source.
3. Hardcodes the decoded result as `const CHURCH_ADD_2_3_EXPECTED: u64 = 5;`.

**NOTE on `w`:** TASK-0391 §Acceptance Criteria line 4 says "`church_add(2, 3)` w=1" in one line and "w=2" in the example; this TEST-SPEC elects **`w = 1`** (single-worker smoke) because (a) it is the simplest reproducible path; (b) v1 frozen baseline stores it; (c) the AMB-D-2 behavioural invariant is orthogonal to `num_workers` — either choice discharges R42. The developer may widen to `w = 2` (or both) without amending this TEST-SPEC.

---

## Unit Tests

### UT-0391-01: `r42_default_delta_mode_preserves_v1_smoke_output`

**Purpose:** R42 load-bearing MUST — `delta_mode = false` (default) on a canonical SPEC-14 smoke produces bit-identical observable output to the v1 frozen baseline.

**Target file:** `merge/grid.rs::tests` (recommended) OR `relativist-core/tests/r42_delta_mode_regression.rs` (alternative).

**Given:**
- An IC net built via the SPEC-14 helper `church_add(2, 3)` (returns a `Net` representing the unreduced expression).
- `GridConfig { delta_mode: false, num_workers: 1, ..GridConfig::default() }` — the R42 default-disabled configuration.

**When:** Call `run_grid(net, cfg)`.

**Then:**
```rust
#[test]
fn r42_default_delta_mode_preserves_v1_smoke_output() {
    // Baseline snapshot from the v1 frozen benchmark set
    // (results/locked/v1_local_baseline/). If this constant needs updating,
    // the change is a pre-existing regression, NOT a 2.26-D bug.
    const CHURCH_ADD_2_3_W1_INTERACTIONS: u64 = /* <N from frozen baseline> */;
    const CHURCH_ADD_2_3_EXPECTED: u64 = 5;

    let net = church_add(2, 3);
    let cfg = GridConfig {
        delta_mode: false,  // R42 explicit: default-disabled path
        num_workers: 1,
        ..GridConfig::default()
    };
    let (result_net, metrics) = run_grid(net, cfg)
        .expect("run_grid on church_add(2,3) w=1 with delta_mode=false MUST succeed");

    assert!(
        metrics.converged,
        "SPEC-19 R42: default-disabled run_grid MUST converge on the v1 baseline smoke"
    );
    let decoded = decode_church(&result_net)
        .expect("decoded result MUST be a Church numeral");
    assert_eq!(
        decoded, CHURCH_ADD_2_3_EXPECTED,
        "SPEC-19 R42: decoded result MUST equal v1 baseline (5 for 2+3)"
    );
    assert_eq!(
        metrics.total_interactions, CHURCH_ADD_2_3_W1_INTERACTIONS,
        "SPEC-19 R42: total_interactions MUST equal the v1 frozen baseline \
         for church_add(2,3) w=1. Any drift here is a behavioural regression."
    );
}
```

**Assertions:**
- `run_grid` succeeds (no error path triggered by the new struct field).
- `metrics.converged == true` (smoke reaches normal form).
- Decoded Church numeral equals 5.
- `metrics.total_interactions` matches the v1 frozen constant exactly.

**SPEC-19 R covered:** R42 (load-bearing MUST — behavioural invariance under the default-disabled path).

**Proof-pending vs operational:** operational — this is the spec's own R42 discharge.

---

### UT-0391-02: `r42_enabled_delta_mode_currently_matches_v1_until_2_26c`

**Purpose:** Pin the *current* observable behaviour of `delta_mode = true` on the same smoke workload. As of 2.26-D, `GridConfig.delta_mode` is inert (no consumer reads it; sub-bundle 2.26-C lands the `run_grid_delta` dispatch). This test documents that inertness as a concrete assertion, so that when 2.26-C flips the switch, the resulting behavioural divergence MUST be a deliberate source edit (updating this test's expected values) rather than a silent drift.

TASK-0391 §Acceptance Criteria lines 75-82 explicitly allow both (a) "same output as disabled" and (b) "explicit not-yet-implemented error" as acceptable current behaviours. This TEST-SPEC picks (a) — the working-tree reality — because it matches what `run_grid` does when the field is unread.

**Target file:** Same as UT-0391-01 (co-located).

**Given:**
- Same `church_add(2, 3)` net as UT-0391-01.
- `GridConfig { delta_mode: true, num_workers: 1, ..GridConfig::default() }`.

**When:** Call `run_grid(net, cfg)`.

**Then:**
```rust
#[test]
fn r42_enabled_delta_mode_currently_matches_v1_until_2_26c() {
    // TODO(2.26-C): when `run_grid_delta` lands, either (a) flip this test's
    // expected `total_interactions` to the delta-protocol value OR (b)
    // dispatch to `run_grid_delta` here and update the callee. Decision
    // lives in the 2.26-C PR.
    const CHURCH_ADD_2_3_W1_INTERACTIONS: u64 = /* same constant as UT-01 */;
    const CHURCH_ADD_2_3_EXPECTED: u64 = 5;

    let net = church_add(2, 3);
    let cfg = GridConfig {
        delta_mode: true,  // 2.26-D: inert; 2.26-C: activates run_grid_delta
        num_workers: 1,
        ..GridConfig::default()
    };
    let (result_net, metrics) = run_grid(net, cfg)
        .expect("run_grid with delta_mode=true MUST succeed until 2.26-C guards it");

    // Current (2.26-D) reality: field is unread; output matches disabled path.
    assert!(metrics.converged);
    let decoded = decode_church(&result_net).expect("Church decode");
    assert_eq!(decoded, CHURCH_ADD_2_3_EXPECTED);
    assert_eq!(
        metrics.total_interactions, CHURCH_ADD_2_3_W1_INTERACTIONS,
        "2.26-D: delta_mode = true is inert (field unread). When 2.26-C lands \
         the run_grid_delta dispatch, this assertion MUST be consciously updated."
    );
}
```

**Assertions:**
- Sets `delta_mode = true`; `run_grid` succeeds.
- Decoded result still equals 5 (inert flag).
- `total_interactions` still matches the v1 baseline (inert flag).
- A TODO comment marks the 2.26-C transition point for future maintainers.

**SPEC-19 R covered:** R41 (the enabled branch exists and parses), R42 (documents that the inert enabled-path does not break observable behaviour relative to the disabled-path baseline).

**Proof-pending vs operational:** operational — pins current working-tree behaviour; non-proof.

**NOTE on update discipline.** When sub-bundle 2.26-C flips `run_grid` to dispatch `run_grid_delta` under `delta_mode = true`, this test MUST be updated in the 2.26-C PR — either by:
- (a) replacing the `total_interactions` constant with the delta-protocol value (if the BSP loop produces a different — but correct — round/interaction count under the delta protocol; per SPEC-19 R40, `R_delta_strict ≤ N` suggests the interaction *count* stays equal and only the round count shifts), OR
- (b) updating the `run_grid` dispatch to invoke `run_grid_delta` under the flag and restating the assertion against the delta reference.

This TEST-SPEC deliberately does NOT pre-decide between (a) and (b); 2.26-C owns that decision.

---

## Coverage mapping

| Requirement | Covered by |
|---|---|
| R42 — `GridConfig::default().delta_mode == false` | Indirect here (TEST-SPEC-0389 UT-01 is the direct type-layer guard); UT-0391-01 uses `..GridConfig::default()` spread and relies on the default being `false` for the smoke to match baseline |
| R42 — default-disabled `run_grid` converges and produces v1 output | UT-0391-01 (full decoded-result + interaction-count check) |
| R42 — `total_interactions` snapshot parity against v1 frozen baseline | UT-0391-01 |
| R42 — enabled-path inertness (2.26-D): no silent behavioural divergence before 2.26-C lands | UT-0391-02 (documents current reality with a TODO for the 2.26-C flip) |
| R41 — field settable at construction time (`delta_mode: true` literal compiles + round-trips through `run_grid`) | UT-0391-02 |
| R38 — G1 reformulation (proof pending) | **Not covered by runtime test.** Narrative-only in TASK-0392's ROADMAP block (AMB-D-3 Group 1). No `#[ignore]` stub here because TASK-0391's scope is the behavioural regression, not the correctness invariant. |
| R39 — D3 reformulation (proof pending) | **Not covered by runtime test.** Same as R38 — narrative-only in TASK-0392. The operational equivalence between exhaustive `findBorderRedexes` (v1) and `BorderGraph.detect_border_redexes()` (delta) is exercised in-situ by sub-bundle 2.35's BorderGraph tests, NOT here. |
| R40 — D6 reformulation (operational, discharges in 2.26-C) | **Not covered by runtime test.** Narrative-only in TASK-0392 (AMB-D-3 Group 2). The termination progress guarantee is exercised by 2.26-C's BSP loop, NOT here. |

**Proof scaffolding note (AMB-D-3 Group 1 — R38/R39):** TEST-SPEC-0391 does NOT mark any test as `#[ignore]` for R38/R39 proof deferral. The spec-critic's AMB-D-3 ruling places the proof-pending obligation on narrative documentation (TASK-0392 ROADMAP block), not on test-level ignored stubs. A `#[ignore] #[should_panic(...)]` stub here would be theatrical — there is no ARG-005 proof artefact to "enable" in future; the proof lives in TCC §8 (DISC-011 → ARG-005), outside the Relativist test surface. Future maintainers looking for the R38/R39 regression hook should read ARG-005 (TCC work item), not grep for `#[ignore] = "TODO(ARG-005)"` in Relativist tests.

**R40 (AMB-D-3 Group 2 — operational).** R40's "Progress guarantee" is a self-contained spec argument (each round consumes ≥ 1 interaction from T7 ⇒ termination in ≤ N rounds strict / 1 round lenient). It discharges in the 2.26-C delta BSP loop. TEST-SPEC-0391 does NOT test R40 here because the sub-bundle 2.26-D has no runtime delta path; 2.26-C will ship a convergence test paired with Global Normal Form check (joint: `has_border_activity == false` per TASK-0348 + `BorderGraph.is_empty()` per 2.35 + zero local redexes) that discharges R40 operationally.

---

## Adversarial angles (QA candidates for Stage 5)

| # | Scenario | Why dangerous |
|---|----------|---------------|
| QA-0391-A | `GridConfig::default()` accidentally flips `delta_mode` to `true` in a future refactor | UT-0391-01 still runs with explicit `delta_mode: false`, so it does NOT catch the default flip directly; TEST-SPEC-0389 UT-0389-01 is the canary for the default-polarity bug |
| QA-0391-B | The v1 baseline constant `CHURCH_ADD_2_3_W1_INTERACTIONS` is snapshotted incorrectly at bundle-landing time | UT-0391-01 will fail on the first run; developer must cross-check against `results/locked/v1_local_baseline/`. Not a QA scenario — a Stage 3 DEV setup step. |
| QA-0391-C | `run_grid` silently changes its interaction counting semantics (pre-bundle regression) | UT-0391-01 fires; distinguishes pre-existing v1 regression from 2.26-D introduction by the constant-mismatch diagnostic |
| QA-0391-D | A future spread literal `..GridConfig::default()` elsewhere in the codebase accidentally consumes `delta_mode: true` from a different default source | UT-0391-01 would catch the resulting behavioural drift *iff* that drift flows into `run_grid`'s execution path for `church_add(2,3)`. Coverage is non-universal but high for the canonical smoke. |
| QA-0391-E | `church_add(2, 3)` helper in SPEC-14 changes semantics (e.g., different encoding) | UT-0391-01 + UT-0391-02 both fire with a decode mismatch; the failure localises to the helper, not to delta_mode. This TEST-SPEC is NOT a guard for SPEC-14 helper regressions. |
| QA-0391-F | 2.26-C lands and forgets to update UT-0391-02's `total_interactions` constant when the delta protocol produces a different round count | UT-0391-02 fires in the 2.26-C PR — this is the deliberate cross-bundle signal the TODO comment is there to catch. Stage 5 QA of 2.26-C should grep for `TODO(2.26-C)` in tests. |
| QA-0391-G | `metrics.total_interactions` field renamed to e.g. `total_interaction_count` | UT-0391-01/02 fail to compile; canary |
| QA-0391-H | `run_grid` returns `Result<_, E>` with a new `E` variant that silently wraps the v1 path — panic changes to `Ok` shape | `.expect("run_grid …MUST succeed")` stays green for the success path; compilation would fail or the `match` would need updating. Low risk under Rust's exhaustiveness rules. |

---

## Acceptance gate

1. `cargo test --workspace --lib` count: 973 → **975** (+2 new `#[test]` fns; 0 `#[ignore]` stubs).
2. `cargo test --workspace --lib --features zero-copy` count: 1013 → **1015** (+2; feature flag does not gate `merge/grid.rs::tests`).
3. If the developer elected the `relativist-core/tests/` integration-test placement, then `cargo test --workspace --test r42_delta_mode_regression` also passes, and the total (lib + integration) test count increases by +2 on the integration side instead of the lib side. Either placement satisfies the "test count never decreases" floor.
4. `cargo build --workspace` clean (default features).
5. `cargo build --workspace --features zero-copy` clean.
6. `cargo clippy --workspace --all-targets -- -D warnings` clean.
7. `cargo fmt --check` clean.
8. Bundle-level acceptance check (TASK-0391 §Acceptance Criteria line 86-94): a `grep -rn "\.delta_mode" relativist-core/src/` restricted to production code (no `#[cfg(test)]`, no `tests/`, no `benches/`) returns matches ONLY in: (a) the struct declaration in `merge/types.rs`, (b) the `Default` impl, (c) `config.rs::build_grid_config(_from_local)` bindings, (d) the `CoordinatorArgs`/`LocalArgs` structs. No runtime dispatch yet. The developer records this grep in the PR description. **NOT a runtime test** — a bundle-level PR-description checklist item.

---

## Out of scope (deferred to later TEST-SPECs in the bundle or future bundles)

- ROADMAP §3.5 G1/D3/D6 narrative amendment notes → TEST-SPEC-0392.
- Docstring polish on `delta_mode` + doctest on `GridConfig` → TEST-SPEC-0393.
- `run_grid_delta` dispatch + convergence test for the enabled path → sub-bundle 2.26-C.
- Formal proof of G1 recoverability (R38) → ARG-005 / DISC-011 / OQ-1 (TCC §8, not Relativist).
- Formal proof of D3 equivalence (R39) → ARG-005 (TCC §8).
- R40 operational termination check under delta protocol → sub-bundle 2.26-C convergence test (joint: per-worker `has_border_activity == false` + `BorderGraph.is_empty()` + zero local redexes).
- Larger v1 benchmark replay (multi-worker, multi-workload) → frozen benchmark campaign, not this unit test.
