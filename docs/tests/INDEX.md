# TEST-SPEC Index — Active

Active TEST-SPEC catalog for the v2 SDD pipeline.

> Convention: tests reach the developer (Stage 3) via this directory. Specifications are markdown only — the developer turns each into Rust test code in `relativist-core/{src,tests}/`. When a bundle closes, its TEST-SPEC files move to `archive/`.

**Status (2026-05-05):** ZERO active TEST-SPECs. The full inventory of D-005..D-012 test specs (TEST-SPEC-0001..TEST-SPEC-0618 with the SPEC-20 EG-* + SPEC-21/22 T1..T18 series) is preserved at `archive/` (332 files). The next bundle (D-013) will repopulate this file once `test-generator` runs against the freshly-split TASKs.

---

## Active

_(none)_

When a new bundle (D-013+) opens, `test-generator` populates this section after `task-splitter` has split the SPEC into TASKs. Spec, task, test-spec ordering is enforced by `WORKFLOWS.md`.

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

**Total test specs shipped:** 332 across all bundles. Per-spec files in `archive/`. ARG-006 / ARG-005 / ARG-001 empirical-signature anchors documented in `theory-bridge.md`.
