# TEST-SPEC Index — Active

Active TEST-SPEC catalog for the v2 SDD pipeline.

> Convention: tests reach the developer (Stage 3) via this directory. Specifications are markdown only — the developer turns each into Rust test code in `relativist-core/{src,tests}/`. When a bundle closes, its TEST-SPEC files move to `archive/`.

**Status (2026-05-06):** 8 active TEST-SPECs covering bundle D-014 (Stress Curve Campaign), TASKs 0700..0707. TASK-0708 is the overnight campaign run + lock + INDEX/ROADMAP/CHANGELOG updates — it has no own TEST-SPEC; its acceptance criteria are met by the cumulative outputs of the prior 8 TEST-SPECs being green.

The historical inventory of D-005..D-012 test specs (TEST-SPEC-0001..TEST-SPEC-0618 with the SPEC-20 EG-* + SPEC-21/22 T1..T18 series) is preserved at `archive/` (332 files).

---

## Active

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

### Cumulative test-floor projection after D-014 closes

| Profile | Pre-D-014 baseline | Δ from D-014 | Post-D-014 floor |
|---|---|---|---|
| `cargo test` (default) | 1798 | +21 | **≥ 1819** |
| `cargo test --features zero-copy` | 1842 | +21 | **≥ 1863** |
| `cargo test --features streaming-no-recycle` | 1789 | +21 | **≥ 1810** |
| `cargo test --release` | 1740 | +21 | **≥ 1761** |
| v1 floor | 690 | unchanged (inviolable) | **690** |

These projections match the cumulative ceilings declared in `docs/backlog/BACKLOG.md` for bundle D-014.

### Per-TASK test category breakdown

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
| **Total** | **10** | **11** | **12** | **21 cargo + 12 review** |

When the bundle closes (TASK-0708 lands), these 8 TEST-SPEC files move to `archive/` and this section clears per the existing housekeeping pattern.

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
