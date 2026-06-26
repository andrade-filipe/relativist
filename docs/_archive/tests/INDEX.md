# TEST-SPEC Index — Active

Active TEST-SPEC catalog for the v2 SDD pipeline.

> Convention: tests reach the developer (Stage 3) via this directory. Specifications are markdown only — the developer turns each into Rust test code in `relativist-core/{src,tests}/`. When a bundle closes, its TEST-SPEC files move to `archive/`.

**Status (2026-05-06):** 19 active TEST-SPECs covering two parallel bundles:
- **D-014 Stress Curve Campaign:** 8 TEST-SPECs for TASKs 0700..0707 (TASK-0708 has no own TEST-SPEC — its acceptance criteria are met by the cumulative outputs of the prior 8 TEST-SPECs being green).
- **D-015 SPEC-27 Encoder/Decoder API + HornerCodec (Topic 2):** 11 TEST-SPECs for TASKs 0709..0719 (one TEST-SPEC per TASK, citing T1-T23 from SPEC-27 v3 §7).

The historical inventory of D-005..D-012 test specs (TEST-SPEC-0001..TEST-SPEC-0618 with the SPEC-20 EG-* + SPEC-21/22 T1..T18 series) is preserved at `archive/` (332 files).

---

## Active — D-014 (Stress Curve Campaign, Topic 1)

| ID | TASK | File | Tests added | Type | Cfg gating | Floor delta (default) |
|----|------|------|-------------|------|-----------|------------------------|
| TEST-SPEC-0700 | TASK-0700 | `TEST-SPEC-0700-memory-probe.md` | UT-0700-{01..04} | unit (in-module) | UT-02/03 cfg `not(macos)` | +4 |
| TEST-SPEC-0701 | TASK-0701 | `TEST-SPEC-0701-stop-rule.md` | UT-0701-{01..06} | unit (in-module) | none | +6 |
| TEST-SPEC-0702 | TASK-0702 | `TEST-SPEC-0702-campaign-descriptor.md` | IT-0702-01 | integration | none (macOS self-skip in body) | +1 |
| TEST-SPEC-0703 | TASK-0703 | `TEST-SPEC-0703-csv-schema.md` | IT-0703-{01, 02} | integration | none | +2 |
| TEST-SPEC-0704 | TASK-0704 | `TEST-SPEC-0704-bash-orchestrator.md` | IT-0704-01 | integration | `cfg(unix)` | +1 |
| TEST-SPEC-0705 | TASK-0705 | `TEST-SPEC-0705-plot-generator.md` | IT-0705-01 | integration | none (skips if python3 missing) | +1 |
| TEST-SPEC-0706 | TASK-0706 | `TEST-SPEC-0706-docs-page.md` | RC-0706-{01..12} (manual review checklist) | docs-only | n/a | **+0** |
| TEST-SPEC-0707 | TASK-0707 | `TEST-SPEC-0707-integration-tests.md` | IT-0707-{01..06} | integration (6 files) | mixed (`cfg(not(macos))` × 2, `cfg(unix)` × 2, none × 2) | +6 |

**Total tests specified for D-014:** 21 across 8 TEST-SPECs (10 unit + 11 integration; +0 from docs-only TEST-SPEC-0706).

---

## Active — D-015 (SPEC-27 Encoder/Decoder API + HornerCodec, Topic 2)

Each TEST-SPEC maps explicitly to T-IDs from SPEC-27 v3 §7 (T1-T23) and honors the Round 2 closure decisions SC-005..SC-013.

| ID | TASK | File | Tests added | Type | Cfg gating | SPEC-27 v3 §7 mapping | Floor delta (default) |
|----|------|------|-------------|------|-----------|------------------------|------------------------|
| TEST-SPEC-TASK-0709 | TASK-0709 | `TEST-SPEC-TASK-0709-r4-notnormalform-valid-pair-semantics.md` | UT-0709-{01..05} | unit (in-module) | none | T1, T2 (foundational) | +5 |
| TEST-SPEC-TASK-0710 | TASK-0710 | `TEST-SPEC-TASK-0710-churcharith-r8-operand-semantics-audit.md` | UT-0710-{01..05} | unit (in-module) | none | T3, T4 | +5 |
| TEST-SPEC-TASK-0711 | TASK-0711 | `TEST-SPEC-TASK-0711-r13a-wire-helpers-obligation-validation.md` | UT-0711-{01..04} + CT-0711-05 (compile-fail doc-test) | unit (in-module) + doc-test | none | foundational for T6, T7, T11, T13 | +4 (+2 doc-tests not counted) |
| TEST-SPEC-TASK-0712 | TASK-0712 | `TEST-SPEC-TASK-0712-biguint-readback-module.md` | UT-0712-{01..04} + PT-0712-05 + CT-0712-06 | unit + property + integration | none | T9, T9b, T11, T12 | +6 |
| TEST-SPEC-TASK-0713 | TASK-0713 | `TEST-SPEC-TASK-0713-horner-serial-oracle.md` | UT-0713-{01..08} + CT-0713-09 | unit + integration (source inspection) | none | T7, T9, T9b, T10, T11 (foundational) | +9 |
| TEST-SPEC-TASK-0714 | TASK-0714 | `TEST-SPEC-TASK-0714-hornercodec-encoder.md` | UT-0714-{01..10} + CT-0714-11 | unit + integration (source inspection) | none | T5, T6, T7, T8, T10 | +11 |
| TEST-SPEC-TASK-0715 | TASK-0715 | `TEST-SPEC-TASK-0715-hornercodec-decoder-and-codec-impl.md` | UT-0715-{01..05} + PT-0715-{06,07} + IT-0715-{08,09} | unit + property + integration (in-process MUST + Docker `#[ignore]`) | IT-0715-09 `#[ignore]` | T7, T8, T9, T9b, T10, T11, **T13 (G1 demo)** | +8 (Docker `#[ignore]` not counted) |
| TEST-SPEC-TASK-0716 | TASK-0716 | `TEST-SPEC-TASK-0716-default-registry-swap-lambda-for-horner.md` | UT-0716-{01..05} | unit (in-module) | none | T14, T15, T16 | +5 |
| TEST-SPEC-TASK-0717 | TASK-0717 | `TEST-SPEC-TASK-0717-cli-encoder-codec-conflicts-with.md` | UT-0717-{01..06} | unit (in-module) | none | T17, T18, T19, T20 | +6 |
| TEST-SPEC-TASK-0718 | TASK-0718 | `TEST-SPEC-TASK-0718-cli-encoders-list-subcommand.md` | UT-0718-{01..04} | unit (in-module or integration) | none | T21 | +4 |
| TEST-SPEC-TASK-0719 | TASK-0719 | `TEST-SPEC-TASK-0719-recipeencoder-r24-r28-audit.md` | UT-0719-{01..03} + IT-0719-04 | unit + integration | none | T22, T23 | +4 |

**Total tests specified for D-015:** 67 cargo-tests across 11 TEST-SPECs (54 unit + 4 property + 9 integration; 1 of 9 integration tests is `#[ignore]` Docker TCP, NOT counted). Plus 2 compile-fail doc-tests (in `cargo test --doc`).

### Per-TASK test category breakdown — D-015

| TASK | Unit | Property | Integration | Doc-test (compile-fail) | Total cargo-tests |
|---|---|---|---|---|---|
| TASK-0709 | 5 | 0 | 0 | 0 | 5 |
| TASK-0710 | 5 | 0 | 0 | 0 | 5 |
| TASK-0711 | 4 | 0 | 0 | 2 | 4 |
| TASK-0712 | 4 | 1 | 1 | 0 | 6 |
| TASK-0713 | 8 | 0 | 1 | 0 | 9 |
| TASK-0714 | 10 | 0 | 1 | 0 | 11 |
| TASK-0715 | 5 | 2 | 1 (in-process) + 1 `#[ignore]` Docker | 0 | 8 (default) |
| TASK-0716 | 5 | 0 | 0 | 0 | 5 |
| TASK-0717 | 6 | 0 | 0 | 0 | 6 |
| TASK-0718 | 4 | 0 | 0 | 0 | 4 |
| TASK-0719 | 3 | 0 | 1 | 0 | 4 |
| **Total** | **59** | **3** | **5 + 1 ignored** | **2** | **67 cargo-tests** |

### Round 2 closure decisions honored

The 11 D-015 TEST-SPECs explicitly carry these structural decisions from `docs/spec-reviews/SPEC-27-v2-round2-response.md`:

- **SC-005** — TASK-0709's `count_valid_active_pairs(net)` helper distinguishes "valid active pairs after stale pruning per SPEC-01 I4" from raw `redex_queue.len()`. Used by TASK-0712 (`decode_biguint`) and TASK-0715 (HornerCodec decoder).
- **SC-006** — TASK-0713 / TASK-0715 T9 uses `coeffs.len() == 25` (result `≈ 1.11×10²⁴`, strictly exceeds u64::MAX); T9b boundary `[10000;5] @ 10000` is a separate test.
- **SC-007** — TASK-0713's `horner_serial` returns `Result<BigUint, OracleError>`; T11 negative cross-check (≥30 cases) in TASK-0715 PT-0715-07.
- **SC-008** — TASK-0717 tests clap `ErrorKind::ArgumentConflict` programmatically; UT-0717-06 verifies both `--encoder` and `--codec` appear separately in `--help`.
- **SC-009** — TASK-0715 IT-0715-08 (T13) cites **G1 (Fundamental Property)** with **P1 as engine + P3+P4 as distribution-side preconditions**, NOT P3 alone.
- **SC-010** — TASK-0715 IT-0715-08 in-process MUST (round-robin partition per SPEC-07 R3); IT-0715-09 Docker TCP `#[ignore]` (CI integration suite via cicd follow-up); decoder stage explicit (NG5 — coordinator merged net).
- **SC-013** — TASK-0711 is **promotion-and-validation** (helpers exist at `arithmetic.rs:92,224` per Q5); TASK-0711 tests R13a' obligations directly without new construction logic.

### Cumulative test-floor projection after both bundles close

The two bundles ship in parallel; the test floors compound additively.

| Profile | Pre-D-014 baseline | Δ from D-014 | Δ from D-015 | Combined post-D-014 + D-015 floor |
|---|---|---|---|---|
| `cargo test` (default) | 1798 | +21 | +67 | **≥ 1886** |
| `cargo test --features zero-copy` | 1842 | +21 | +67 | **≥ 1930** |
| `cargo test --features streaming-no-recycle` | 1789 | +21 | +67 | **≥ 1877** |
| `cargo test --release` | 1740 | +21 | +67 | **≥ 1828** |
| v1 floor | 690 | unchanged (inviolable) | unchanged | **690** |

(Per-bundle floors:
- After D-014 only: default ≥ 1819 / zero-copy ≥ 1863 / streaming-no-recycle ≥ 1810 / release ≥ 1761 (the cumulative ceilings already declared in `docs/backlog/BACKLOG.md` for D-014).
- After D-015 only (assuming D-014 already shipped): default ≥ 1886 / zero-copy ≥ 1930 / streaming-no-recycle ≥ 1877 / release ≥ 1828.
)

### Per-TASK test category breakdown — D-014

| TASK | Unit | Integration | Manual review | Total |
|---|---|---|---|---|
| TASK-0700 | 4 | 0 | 0 | 4 |
| TASK-0701 | 6 | 0 | 0 | 6 |
| TASK-0702 | 0 | 1 | 0 | 1 |
| TASK-0703 | 0 | 2 | 0 | 2 |
| TASK-0704 | 0 | 1 | 0 | 1 |
| TASK-0705 | 0 | 1 | 0 | 1 |
| TASK-0706 | 0 | 0 | 12 (RC-checklist) | **0 cargo-tests** |
| TASK-0707 | 0 | 6 | 0 | 6 |
| **Total D-014** | **10** | **11** | **12** | **21 cargo + 12 review** |

When each bundle closes (TASK-0708 lands for D-014; TASK-0719 lands for D-015), the corresponding TEST-SPEC files move to `archive/` and this section clears per the existing housekeeping pattern.

---

## Cumulative test specs delivered (per `progress.md`)

| Bundle | Test specs | Archive | Closure narrative |
|--------|-----------|---------|--------------------|
| Phase 1..11 (v1) | TEST-SPEC-0001..0030, 0184..0399 | `archive/` | `progress.md` "Local Benchmark Phase" |
| D-005 | TEST-SPEC-0410..0419 (partial), no spec for 0400..0403 (no test phase) | `archive/` | `progress.md` D-005 |
| D-006 (SPEC-20) | TEST-SPEC-04xx + EG-U1..U19, EG-I1..I5b, EG-P1..P6, EG-B1..B3 | `archive/` | `progress.md` D-006 |
| D-009 (SPEC-22) | TEST-SPEC-04xx + T1..T14a (free-list + SparseNet) | `archive/` | `progress.md` D-009 |
| D-010 (SPEC-21) | TEST-SPEC-051x..059x + T1..T14 (streaming) | `archive/` | `progress.md` D-010 |
| D-011 (BLOCKER) | TASK-0612-tests.md, TASK-0613-tests.md | `archive/` | `progress.md` D-011 |
| D-012 (instrumentation) | TASK-0615..0618-tests.md | `archive/` | `progress.md` D-012 |
| **D-014 (active)** | **TEST-SPEC-0700..0707 (8 files; 21 cargo-tests + 12 review checks)** | **active in this directory** | pending — `progress.md` updated after TASK-0708 |

**Total test specs shipped (closed bundles):** 332 across all bundles. Per-spec files in `archive/`. ARG-006 / ARG-005 / ARG-001 empirical-signature anchors documented in `theory-bridge.md`.
