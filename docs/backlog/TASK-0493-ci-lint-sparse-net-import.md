# TASK-0493: CI lint forbidding `SparseNet` imports in `src/reduction/**` (R23 — closes SC-008)

**Spec:** SPEC-22 §3.2 R23 (DESIGN CONSTRAINT — enforced by CI lint).
**Requirements:** R23 (`src/reduction/**/*.rs` MUST NOT contain `use crate::net::sparse::SparseNet;` or any other path resolving to `SparseNet`. The lint MUST be added to the existing `cargo clippy -- -D warnings` gate via a custom rule or an equivalent grep-based pre-commit check.).
**Priority:** P1 (CI-enforceable; non-blocking for runtime correctness but blocks merge of accidental SparseNet creep into hot path).
**Status:** TODO
**Depends on:** TASK-0486 (SparseNet exists; needed to test the lint actually catches the import).
**Blocked by:** none
**Estimated complexity:** S (~30 LoC CI script + ~10 LoC docs)
**Bundle:** SPEC-22 Arena Management — Phase D (SparseNet).

## Context

R23 demoted from MUST NOT runtime requirement to CI lint per Round 2 closure (SC-008). The reduction engine (SPEC-03) relies on O(1) guaranteed indexed access to `agents[id]` and `ports[id * 3 + port]`; HashMap lookup has O(1) amortized but with 5-10× worse constant factor (per AC-006 HVM2 flat-array rationale; AC-001 Haskell `Map AgentId Agent` baseline). The lint enforces the design constraint at the import-graph level.

R23 explicitly delegates the lint authoring to the **cicd agent** (SPEC-15) — SPEC-22 owns the rule statement, cicd owns the implementation. Coordinate accordingly.

## Acceptance Criteria

- [ ] Add a CI step (in `.github/workflows/` or equivalent) that runs a grep-based check: search `src/reduction/**/*.rs` for the patterns `use crate::net::sparse::SparseNet`, `use SparseNet`, `crate::net::sparse::SparseNet`, or `net::sparse::SparseNet`. If any match found, fail the build with a clear error message citing SPEC-22 R23.
- [ ] The check runs on every PR / push to `v2-development` and `main`.
- [ ] Document the check in `relativist-core/CONTRIBUTING.md` (or equivalent) with the rationale (AC-006 / AC-001 perf citation).
- [ ] Test: temporarily add `use crate::net::sparse::SparseNet;` to a file in `src/reduction/`; verify the CI fails. Remove the import; verify CI passes again. (This test is performed once; not a permanent test artifact.)
- [ ] Add a CI bypass for the `src/reduction/sparse_export.rs` file IF such a file exists for legitimate non-hot-path reasons (NONE expected currently — flag for ESPECIALISTA EM SPECS if needed).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `.github/workflows/ci.yml` *(or `lint.yml` / `clippy.yml`)* | modify | Add a `grep` step that fails on SparseNet imports in `src/reduction/`. |
| `relativist-core/CONTRIBUTING.md` *(or new doc)* | modify | Document the constraint and rationale. |

## Key Types / Signatures

(None — CI script only.)

## Test Expectations

TEST-SPEC-0493:
- Manual smoke test: add a forbidden import; CI fails. (Not a permanent test.)

## Invariants Touched

- None at runtime.
- Performance design constraint (AC-006 / AC-001 informed).

## Notes

- This task is the **cicd agent's** territory per the SPEC-22 R23 closure log. The task-splitter records the requirement; the cicd agent implements the lint at Stage 3 / Stage 5.
- A future "clippy lint plugin" (Rust-side custom lint) would be cleaner than grep-based; out of scope for this task.

## DAG Links

- **Predecessors:** TASK-0486.
- **Successors:** none.
