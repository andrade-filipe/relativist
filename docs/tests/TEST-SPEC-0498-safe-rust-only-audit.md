# TEST-SPEC-0498: Safe-Rust-only audit — no `unsafe` in SPEC-22 implementations (R31)

**SPEC-22 §7 ID:** none direct (CI hygiene); plus this plumbing file.
**Owning task:** TASK-0498.
**Parent spec:** SPEC-22 §3.5 R31 (closes SC-017).
**Type:** CI lint (manual smoke verification, not a permanent runtime test).

---

## Inputs / Fixtures

- The SPEC-22-modified file set: `src/net/core.rs`, `src/net/sparse.rs`, `src/net/free_list.rs`, `src/net/mod.rs`, `src/partition/helpers.rs`, `src/partition/types.rs`, `src/merge/engine.rs`, `src/merge/config.rs`, `src/error.rs`, `src/reduction/**/*.rs`.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0498-01 | `ci_grep_unsafe_step_present` | the workflow YAML | grep / inspect | a step exists running `grep -r "unsafe" src/net/ src/partition/ src/merge/ src/reduction/`. The step fails on match. |
| UT-0498-02 | `ci_smoke_unsafe_violation_fails_build` | scratch branch with `unsafe { }` injected into `src/net/core.rs` | run CI | the lint step fails. |
| UT-0498-03 | `ci_smoke_no_unsafe_passes_build` | scratch branch with the unsafe removed | run CI | the lint step passes. |
| UT-0498-04 | `r31_affirmation_comment_at_top_of_core_rs` | `src/net/core.rs` | grep | top-of-file comment present: "SPEC-22 R31: this module contains no `unsafe` blocks. Bit-packed migration is SPEC-23's responsibility." |
| UT-0498-05 | `audit_recursive_includes_all_target_files` | grep target list | inspect | every file in the SPEC-22-modified file set is covered by the grep step. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | The `bitvec` crate (TASK-0478 dep) uses `unsafe` internally | OK — audit scope is `src/`, not `deps/`. (TASK-0498 acceptance criterion.) |
| EC-2 | A `cfg(test)` block in `src/net/core.rs` includes `unsafe { }` for a manual fence | The audit treats this strictly — `unsafe` anywhere in the file fails. To exempt a test, factor it to a separate file outside the audit scope. |
| EC-3 | A doc-comment containing the literal word "unsafe" | The grep pattern targets `unsafe` as a token (e.g., `unsafe ` with a trailing space or curly brace); pure doc mentions might trigger false positives. Refine regex if needed. |

## Invariants asserted

- R31 (no `unsafe` in SPEC-22 implementations — closes SC-017).

## ARG/DISC/REF citation

- None direct.

## Determinism notes

Pure CI lint; deterministic grep over a fixed file set. The smoke verification is one-time manual.

## Cross-test dependencies

- TEST-SPEC-0493 (SparseNet import lint) shares the same CI lint infrastructure pattern.
- This task is OPTIONAL hygiene per priority P2; the production code modifications are the load-bearing items.
