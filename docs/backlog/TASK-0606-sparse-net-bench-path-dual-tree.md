# TASK-0606 — SparseNet bench path for `dual_tree` (Phase D-1 + D-2)

**Phase:** D-1 + D-2 (D-011 SparseNet micro-bench)
**Bundle:** D-011 — Tier 3 Hardening + Bench Enablement
**Status:** PENDING
**Priority:** P1 (validates ROADMAP §2.32 sparse-net memory acceptance gate)
**Spec:** SPEC-22 R12 (SparseNet); SPEC-09 R18a–R18g (Tier 3 metrics committed `82b2d27`); SPEC-22 §3.7 (to_dense conversion).
**Origin:** D-011 plan §D-1 + §D-2.
**Estimated complexity:** M (~100 LoC production + ~50 LoC tests)
**Estimated stages duration:** Stages 2→3→4→5→6 over ~0.75 day.

---

## Context

Per D-011 plan: validate the SparseNet construction-phase memory benefit on a single benchmark (`dual_tree`) at 2 sizes (small + large). The acceptance gate (relaxed from the original SPEC-22 spec wording) is "sparse < 80% of dense at dual_tree(5000)".

Implementation:
- When `representation: NetRepresentation::Sparse`, the benchmark builds a `SparseNet` instead of `Net` during `make_net()`.
- Peak memory is measured before any reduction (using the C-5 probe — `get_peak_memory_at_construction_complete`).
- After measurement, the SparseNet is converted to a dense `Net` via `SparseNet::to_dense(id_range)` (`relativist-core/src/net/sparse.rs:308`); reduction continues on the dense Net (no SparseNet-during-reduction in scope).

Limited to `dual_tree` and 2 sizes per the plan.

## Dependencies

- **TASK-0602 (C-1)** — REQUIRED for `NetRepresentation` enum and the `representation` field.
- **TASK-0604 (C-2/C-4)** — REQUIRED for the bench path selection infrastructure.
- **TASK-0605 (C-5)** — REQUIRED for the construction-phase memory probe.
- **TASK-0596 (B-1)** — RECOMMENDED if a `--mode tcp` sparse run is later attempted, but D-1 is local-only.

## Files in scope

| File | Change |
|------|--------|
| `relativist-core/src/bench/suite.rs` | Branch on `config.representation`: `Sparse` → build `SparseNet`, probe memory, `to_dense(id_range)`, then proceed; `Dense` → status quo. Limit the sparse path to `BenchmarkId::DualTree` (gate sizes via the bench config's `sizes` flag). |
| `relativist-core/src/io/generators/dual_tree.rs` (or wherever the dual_tree generator lives) | Add a sparse-construction variant — a function that produces a `SparseNet` directly. (Or, if simpler, build the dense form and convert via `Net::to_sparse` — but that defeats the memory-saving goal of the sparse path. Prefer building sparse directly.) |
| `relativist-core/tests/spec22_sparse_net_dual_tree_bench.rs` (new) | Integration: dual_tree at 2 sizes, sparse vs dense, assert isomorphism after `to_dense` and assert the construction-phase memory ratio. |

## Files explicitly OUT of scope

- The sparse sub-CSV writer — TASK-0607.
- Sparse path for benchmarks OTHER than dual_tree.
- SparseNet during reduction (dominant memory is dense arena post-construction).

## Acceptance criteria

1. `bench/suite.rs` honors `config.representation = Sparse` for `BenchmarkId::DualTree` only; other benchmarks under `Sparse` either return an explicit "unsupported" error or fall back to dense (per QA Stage 5 review preference).
2. The sparse-construction → `to_dense` → reduction pipeline produces a result graph-isomorphic to the dense-construction → reduction counterpart (T6 / G1).
3. At `dual_tree(5000)`, sparse construction-phase peak memory is < 80% of dense construction-phase peak memory (the relaxed acceptance gate).
4. The reduction-phase results (interaction count, final agent count) are identical between sparse and dense paths.
5. Test floor preserved (≥1683 default / ≥1726 zero-copy).

## Test floor delta expected

**+4 to +6 tests** added (isomorphism, memory-ratio gate at small + large sizes, monotonicity).

## Notes

- The 80% gate is relaxed vs the original SPEC-22 spec acceptance ("<30% dense in ep_construct(5M)"). This is intentional — `dual_tree(5000)` is a smaller and structurally different workload; the goal is informative micro-bench, not full spec acceptance. Document this caveat in the test's doc-comment.
- The "build sparse directly" path may require a new generator helper; alternatively, the conversion path (`Net::to_sparse`) can be used if its memory profile is documented as not invalidating the test. **Stage 2 (test-generator) MUST decide between these two and the choice MUST be tested in Stage 3 (developer).**
