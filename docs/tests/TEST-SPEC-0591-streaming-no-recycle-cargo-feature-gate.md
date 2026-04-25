# TEST-SPEC-0591: `streaming-no-recycle` cargo feature gate (alternative one-liner closure of R37b)

**SPEC-21 §7 ID:** plumbing only (alternative-path closure of TEST-SPEC-0515 line 279 "BOTH gate states" coverage; cross-cuts TEST-SPEC-0589 / TEST-SPEC-0590 as the orthogonal compile-time safety net).
**Owning task:** TASK-0591.
**Parent spec:** SPEC-21 §3.7 R37b (G1 free-list interaction; alternative closure path; closes SC-007 alternate); §3.8 A6 (consumer of TASK-0515).
**Type:** unit (cargo-feature compile gate) + integration (cross-feature isomorphism).
**Theory anchor:** ARG-001 G1 (BSP determinism under streaming — preserved trivially under feature: no recycling = no slot-id reuse = no border violation possible); ARG-005 (preserved trivially).

---

## Inputs / Fixtures

- The post-TASK-0591 `Cargo.toml` declaring `[features] streaming-no-recycle = []`.
- The post-TASK-0591 `Net::create_agent` (or wherever TASK-0472 placed the pop site) with `#[cfg(feature = "streaming-no-recycle")]` short-circuit at the streaming-active branch.
- The post-TASK-0591 CI matrix entry: `cargo test --features streaming-no-recycle`.
- A test workload: 4 workers, 8 chunks, `ep_annihilation_pure(64)` with `chunk_size = 8` (canonical streaming pipeline).
- `GridConfig` with `recycle_under_delta = DisableUnderDelta` (Strategy A, default) AND `BorderClean` (Strategy B, opt-in) — both exercised under the feature.
- Test-only debug counter `Net.free_list_pops: AtomicU64` for both feature states.
- The `nets_isomorphic` helper for cross-feature comparison.

## Unit Tests (feature gate compile + linting)

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0591-01 | `cargo_build_succeeds_with_feature_on` | repo at `v2-development` post-TASK-0591 | `cargo build --features streaming-no-recycle -p relativist-core` | success; no compile errors. (TASK-0591 acceptance line 28; UT-0591-01 task-side.) |
| UT-0591-02 | `cargo_build_succeeds_with_feature_off` | same | `cargo build -p relativist-core` (default features) | success. (Sanity; the feature MUST NOT break the default build.) |
| UT-0591-03 | `feature_declaration_present_in_cargo_toml` | grep `relativist-core/Cargo.toml` | check for `streaming-no-recycle = []` | substring present. (TASK-0591 acceptance line 28.) |
| UT-0591-04 | `cfg_annotation_present_at_pop_site` | grep `relativist-core/src/net/free_list.rs` (or wherever TASK-0472 placed the pop site) | check for `#[cfg(feature = "streaming-no-recycle")]` annotation | present at the documented streaming-active branch. (TASK-0591 acceptance line 33; lint defense against feature-flag drift.) |
| UT-0591-05 | `cfg_annotation_at_documented_sites_only` | grep all source files | the cfg annotation appears at the EXPECTED sites (not silently scattered) | exhaustive list matches TASK-0591 acceptance line 33 documented set. (CI-lint regression catcher.) |

## Unit Tests (runtime behavior under feature ON)

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0591-06 | `feature_on_zero_pops_during_streaming` | `cargo test --features streaming-no-recycle`; 8-chunk streaming pipeline | run; check `Net.free_list_pops.load()` after run | `== 0`. (TASK-0591 acceptance line 30; UT-0591-03 task-side — verified via debug counter.) |
| UT-0591-07 | `feature_on_baseline_tests_pass` | `cargo test --features streaming-no-recycle` | run full test suite | 1181 default tests pass; pop-counter values MAY differ but functional outputs MUST match. (TASK-0591 acceptance line 30; UT-0591-02 task-side.) |
| UT-0591-08 | `feature_on_with_strategy_a_redundant_runtime_gate` | `cfg.recycle_under_delta = DisableUnderDelta`; feature ON | observe `Net::create_agent` call path | the cargo-feature short-circuit fires FIRST (before the runtime gate); the runtime gate is unreachable but NOT removed (TASK-0591 line 24 — gate MUST remain present and CORRECT). |
| UT-0591-09 | `feature_on_with_strategy_b_redundant_runtime_gate` | `cfg.recycle_under_delta = BorderClean`; feature ON | same | cargo-feature fires; Strategy B's per-id check is also unreachable but NOT removed. |

## Unit Tests (runtime behavior under feature OFF)

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0591-10 | `feature_off_strategy_a_runtime_gate_load_bearing` | feature OFF; `cfg.recycle_under_delta = DisableUnderDelta` | streaming pipeline | TEST-SPEC-0589's runtime gate is the load-bearing path; `free_list_pops_during_streaming == 0`; same merged result as feature-ON run. |
| UT-0591-11 | `feature_off_strategy_b_runtime_gate_load_bearing` | feature OFF; `cfg.recycle_under_delta = BorderClean` | streaming pipeline | TEST-SPEC-0590's runtime gate is load-bearing; per-id protection holds; non-border IDs popped, border IDs protected. |

## Integration tests (cross-feature isomorphism)

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| IT-0591-01 | `cross_feature_isomorphism_strategy_a` | run same workload with `cargo test` (feature OFF) and `cargo test --features streaming-no-recycle` (feature ON); both with `cfg.recycle_under_delta = DisableUnderDelta` | merge both; compare | `nets_isomorphic(merged_off, merged_on) == true`. (TASK-0591 acceptance line 32; UT-0591-04 task-side.) |
| IT-0591-02 | `cross_feature_isomorphism_strategy_b` | same comparison with `cfg.recycle_under_delta = BorderClean` | merge both; compare | `nets_isomorphic == true`. |
| IT-0591-03 | `cross_feature_full_baseline` | run the v1 1181-test floor under both feature states | both pass | additive-compat regression confirmed. |

## Integration tests (CI matrix verification)

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| IT-0591-04 | `ci_matrix_includes_feature_column` | grep `.github/workflows/ci.yml` (or equivalent) | check for `--features streaming-no-recycle` in a matrix job | substring present. (TASK-0591 acceptance line 31.) |
| IT-0591-05 | `ci_matrix_default_does_not_include_feature` | same grep | the DEFAULT job MUST NOT enable the feature (it ships disabled by default per TASK-0591 NOTE line 73-74) | feature absent from default job's flags. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Feature ON; `streaming_active = false` (push mode or short-circuit path) | the cargo short-circuit gate is bypassed (it's wrapped in `if streaming_active` per TASK-0591 line 51); free-list pops continue normally per SPEC-22 R3. |
| EC-2 | Feature ON; `delta_mode = true` AND `streaming_active = false` (delta-only) | classic SPEC-22 R10b discipline applies; cargo gate is irrelevant. |
| EC-3 | Feature ON; cargo gate AND runtime gate both fire (Strategy A + delta + streaming) | cargo gate wins (fires FIRST in code flow); runtime gate is unreachable but defensive. |
| EC-4 | Future spec adds another feature gate that overlaps `streaming-no-recycle` | UT-0591-04 / UT-0591-05 lint gate catches accidental drift; document explicitly. |
| EC-5 | Feature ON; memory pressure exceeds available RAM (no recycling = full v1 growth) | runtime panic / OOM; documented trade-off per TASK-0591 line 21-22 (suitable only for small / short-lived runs). |

## Invariants asserted

- R37b alternative closure (build-time disable of free-list during streaming; closes SC-007 alternative path).
- §3.8 A6 (SPEC-22 R10b broadening — alternative-path documentation and CI matrix coverage).
- G1 (BSP determinism under streaming — preserved trivially under feature: no recycling = no slot-id reuse).
- ARG-005 INV-REC — preserved trivially.
- Additive-compat: the feature MUST NOT break the default build OR the 1181/1224 baseline (UT-0591-02 / UT-0591-07).

## ARG/DISC/REF citation

- ARG-001 G1 (operational closure — alternative path).
- ARG-005 INV-REC (delta-recoverability — preserved trivially under feature).

## Determinism notes

**FEATURE GATE LIFECYCLE (CRITICAL):**

The `streaming-no-recycle` feature is a BUILD-TIME configuration. Tests MUST exercise BOTH gate states per TEST-SPEC-0515 line 279 ("streaming-no-recycle cargo gate: BOTH gate states"):

1. **Gate ON** (`cargo test --features streaming-no-recycle`): UT-0591-06..09 + IT-0591-01..03.
2. **Gate OFF** (`cargo test`, default): UT-0591-10..11 + IT-0591-01..03.

The CI matrix job is the harness for both states; IT-0591-04 / IT-0591-05 verify matrix integrity.

**RUNTIME GATE PRESERVATION (NON-NEGOTIABLE):** Per TASK-0591 line 24 + NOTE line 73-74, the runtime gates from TEST-SPEC-0589 / TEST-SPEC-0590 MUST REMAIN PRESENT AND CORRECT regardless of the feature flag. The cargo gate is an ADDITIONAL safety net, NOT a replacement. UT-0591-08 / UT-0591-09 enforce this contract by asserting the runtime-gate code paths still compile and are reachable in the feature-OFF build.

**LINT GATE (UT-0591-04 / UT-0591-05):** A CI lint must verify the `#[cfg(feature = "streaming-no-recycle")]` annotation is syntactically present at the documented pop site(s). This defends against accidental refactors that move the pop site without updating the cfg annotation. Anchored on the `Net::create_agent` function name + the `if streaming_active` block per TASK-0591 line 51.

**TOKIO ORDERING:** Feature-gate tests are cargo-build-level; tokio is not directly involved in the feature mechanics. Integration tests (IT-0591-01..03) MAY use `#[tokio::test(flavor = "current_thread")]` if the streaming pipeline is async-driven; otherwise plain `#[test]`.

**CROSS-FEATURE ISOMORPHISM (IT-0591-01..02):** Asserted via `nets_isomorphic`, NOT byte-equality. Under feature ON with Strategy B, the runtime per-id check is unreachable but the merged result is structurally equivalent. Under feature OFF with Strategy B, the runtime check IS load-bearing; result is structurally equivalent again. Cross-feature isomorphism is the operational closure for the alternative-path equivalence.

**TRADE-OFF DOCUMENTATION:** Per TASK-0591 line 21-22, the feature trades memory efficiency for safety simplicity. The CHANGELOG entry MUST cite this trade-off; UT-0591-03 (Cargo.toml grep) is the registration gate. A doc-comment on the feature declaration in `relativist-core/src/lib.rs` is the user-facing surface (TASK-0591 NOTE line 75).

## Cross-test dependencies

- **TEST-SPEC-0515 (SPEC-22 R10b broadening amendment-level coverage)** — predecessor; the "BOTH gate states" requirement comes from line 279 + UT-0515-05 / UT-0515-06.
- **TEST-SPEC-0589 (Strategy A streaming wiring)** — predecessor (SHOULD land first per TASK-0591 line 8); UT-0591-08 / UT-0591-10 cross-cut. Feature is an ADDITIONAL safety net; runtime gate MUST remain correct.
- **TEST-SPEC-0590 (Strategy B streaming wiring)** — predecessor (SHOULD land first); UT-0591-09 / UT-0591-11 cross-cut. Same dual-coverage discipline.
- **TEST-SPEC-0482 (SPEC-22 RecyclePolicy + `is_border_protected`)** — sibling; provides the underlying free-list pop site that the feature gates.
- **TEST-SPEC-0472 (or whichever TEST-SPEC owns the pop site)** — predecessor; TASK-0591 line 51 references the annotation site.
- **CI workflow (`.github/workflows/ci.yml`)** — IT-0591-04 / IT-0591-05 verify the matrix integration; coordinated with `cicd` agent.
