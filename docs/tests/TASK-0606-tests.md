# TEST-SPEC-0606 — Tests for TASK-0606 — SparseNet bench path for `dual_tree`

**Task:** TASK-0606 (Phase D-1 + D-2, P1)
**Spec:** SPEC-22 R12 (SparseNet); SPEC-09 R18a–R18g, R37c (Tier 3 metrics, committed `82b2d27`); SPEC-22 §3.7 (`to_dense` conversion); SPEC-01 G1 (graph-isomorphism invariant).
**Origin:** D-011 plan §D-1 + §D-2 (SparseNet micro-bench validation).
**Test floor delta:** **+5 default** (2 isomorphism + 2 memory-ratio at 2 sizes + 1 monotonicity).
**Prerequisites:** TASK-0602 (NetRepresentation enum), TASK-0604 (bench branch infrastructure), TASK-0605 (memory probe).

---

## Test inventory

| test_id | level | target file | prerequisite TASK | cfg gates |
|---|---|---|---|---|
| UT-0606-01 | unit | `relativist-core/src/io/generators/dual_tree.rs::tests::sparse_direct_construction_matches_dense_after_to_dense` | TASK-0602 | none |
| IT-0606-02 | integration | `relativist-core/tests/spec22_sparse_net_dual_tree_bench.rs::isomorphism_at_dual_tree_small_size` | TASK-0602, TASK-0604 | none |
| IT-0606-03 | integration | `relativist-core/tests/spec22_sparse_net_dual_tree_bench.rs::isomorphism_at_dual_tree_5000` | TASK-0602, TASK-0604 | none |
| IT-0606-04 | integration | `relativist-core/tests/spec22_sparse_net_dual_tree_bench.rs::sparse_construction_memory_below_80pct_of_dense_at_5000` | TASK-0605, TASK-0604 | `#[cfg(target_os = "linux")]` |
| IT-0606-05 | integration | `relativist-core/tests/spec22_sparse_net_dual_tree_bench.rs::reduction_phase_results_identical_sparse_vs_dense` | TASK-0604 | none |
| IT-0606-06 | integration | `relativist-core/tests/spec22_sparse_net_dual_tree_bench.rs::sparse_path_only_supports_dual_tree_other_benches_unsupported` | TASK-0602, TASK-0604 | none |

Total: **5 default tests + 1 Linux-gated** (effective +5 floor on Linux CI; +5 on non-Linux because IT-0606-04 still compiles, just skipped at runtime — cargo counts compiled tests).

Conservative floor delta: **+5 default**.

---

## Per-test specifications

### UT-0606-01 — `sparse_direct_construction_matches_dense_after_to_dense`

**Purpose.** Generator-level: the new "build sparse directly" helper produces a `SparseNet` that, after `to_dense(id_range)`, is graph-isomorphic to the dense form built by the original `dual_tree(N)` generator. This validates the **construction-isomorphism** contract per SPEC-09 R37c.
**Setup.**
- Pick `N = 64` (small for a unit test).
- Build `dense = dual_tree_dense(N)` (the existing dense generator).
- Build `sparse = dual_tree_sparse(N)` (the new sparse-direct generator).
**Action.** `let converted = sparse.to_dense(0..agent_count);`
**Assertions.**
- `nets_isomorphic(&dense, &converted) == true` (use the existing `nets_isomorphic` or `nets_equivalent` helper — agent-id renumbering allowed; structural equality required).
- `dense.agents.len() == converted.agents.len()`.
- `dense.wires.len() == converted.wires.len()`.
- For each `Symbol` (CON, DUP, ERA): the count in `dense` equals the count in `converted`.
**Boundary case coverage.** Catches a buggy sparse-direct generator that emits a different agent-set or wire-set than the dense baseline (which would invalidate the entire micro-bench).
**Why it must exist.** Acceptance criterion #2 (sparse-construction → `to_dense` → reduction pipeline produces a graph-isomorphic result). This is the unit-level construction-isomorphism witness per SPEC-09 R37c.

---

### IT-0606-02 — `isomorphism_at_dual_tree_small_size`

**Purpose.** End-to-end at small size: bench-suite-driven sparse path → `to_dense` → reduction yields the same final reduced net as bench-suite-driven dense path → reduction.
**Setup.**
- Configure `BenchmarkSuiteConfig` with `benchmark = DualTree`, `sizes = [500]`, `representation = Sparse`.
- Configure a parallel run with `representation = Dense`.
**Action.** Invoke the bench suite for both runs; capture the post-reduction `Net` snapshots.
**Assertions.**
- `nets_isomorphic(&dense_post_reduce, &sparse_post_reduce) == true` (G1 invariant per SPEC-01).
- `dense_run.interaction_count == sparse_run.interaction_count` (identical reduction trace count).
- `dense_run.final_agent_count == sparse_run.final_agent_count`.
**Boundary case coverage.** Catches a divergence introduced by `to_dense` losing agents or wires during conversion.
**Why it must exist.** Acceptance criterion #2 + #4 (reduction-phase results identical between sparse and dense paths). This is the small-size integration witness.

---

### IT-0606-03 — `isomorphism_at_dual_tree_5000`

**Purpose.** Same as IT-0606-02 at the production target size `N = 5000` (the size at which the 80% memory gate is asserted in IT-0606-04). Splitting size 500 vs 5000 ensures both small-and-fast and at-target validations are present.
**Setup.** Same as IT-0606-02 but `sizes = [5000]`.
**Action.** Same as IT-0606-02.
**Assertions.**
- Same as IT-0606-02 (isomorphism + interaction count + agent count).
- Wall-clock under 30 s (informational; the test must complete in CI without timeout).
**Boundary case coverage.** Catches scaling bugs that only surface at the larger size (e.g. integer overflow in id renumbering, allocator pressure changing semantics).
**Why it must exist.** Acceptance criterion #2 + #4 at production size — pairs with IT-0606-04 to bind correctness and memory at the same N.

---

### IT-0606-04 — `sparse_construction_memory_below_80pct_of_dense_at_5000`

**Purpose.** The headline relaxed acceptance gate: at `dual_tree(5000)`, sparse construction-phase peak memory is `< 80%` of dense.
**Setup.**
- Configure two bench runs at `N = 5000`: one `Dense`, one `Sparse`.
- Capture `peak_memory_during_construction` (the canonical name per SPEC-09 R18a / line 482 of SPEC-09; see `82b2d27`) from each run via the C-5 probe.
**Action.** Compute `ratio = sparse_peak / dense_peak`.
**Assertions.**
- `dense_peak > 0` (probe returned a real value — Linux-gated; on non-Linux the probe returns 0 and this test must be skipped via `cfg`).
- `sparse_peak > 0`.
- `ratio < 0.80` (the relaxed acceptance gate).
- `ratio > 0.0` (sparse did not lie about being free).
- A doc-comment in the test text cites the relaxation rationale per the task notes ("80% gate is relaxed vs original SPEC-22 spec acceptance ('<30% dense in ep_construct(5M)'); intentional — `dual_tree(5000)` is a smaller and structurally different workload").
**Boundary case coverage.** Catches a sparse implementation that allocates as much as dense (no memory benefit) — would silently pass without the explicit ratio gate.
**cfg gate.** `#[cfg(target_os = "linux")]` (the C-5 probe returns 0 on non-Linux).
**Why it must exist.** Acceptance criterion #3 — THE headline metric for Phase D-1 + Phase F-2 narrative. Without this assertion, the entire micro-bench has no decision criterion.

**Implementation note.** The probe value MUST come from `peak_memory_during_construction` (canonical name per SPEC-09 R18a, line 482, commit `82b2d27`). Earlier drafts may have used `peak_construction_bytes` — these are the SAME metric; this test uses the canonical SPEC-09 R18a name.

---

### IT-0606-05 — `reduction_phase_results_identical_sparse_vs_dense`

**Purpose.** Reduction-phase determinism: after the sparse path's `to_dense` conversion completes, the dense reduction proceeds identically — same interaction count, same final structure, same termination condition.
**Setup.** Same as IT-0606-02 but capture the full reduction trace (interaction count per step, terminal redex set if any).
**Action.** Invoke reduction on both nets; compare traces.
**Assertions.**
- Reduction terminates in both cases (no infinite loop introduced by sparse path).
- Total interaction count is exactly equal: `sparse.interactions == dense.interactions`.
- Final net is graph-isomorphic (delegates to UT-0606-01's helper).
- The terminal redex count is identical (both nets reach the same normal form).
**Boundary case coverage.** Catches a bug where sparse path produces a *structurally* equivalent net but with different reduction order (interaction count divergence) — this would break invariant comparisons in Phase F-2.
**Why it must exist.** Acceptance criterion #4 (reduction-phase results identical). The test is a stronger version of IT-0606-02/03 that locks the *count*, not just the structure.

---

### IT-0606-06 — `sparse_path_only_supports_dual_tree_other_benches_unsupported`

**Purpose.** Acceptance criterion #1: sparse path is gated to `BenchmarkId::DualTree` only. Other benchmarks under `representation = Sparse` either return an explicit `Err(BenchError::UnsupportedRepresentation)` OR fall back to dense (the task says either is acceptable subject to QA Stage 5).
**Setup.**
- Configure a bench run with `benchmark = EpAnnihilation`, `representation = Sparse`, `sizes = [100]`.
**Action.** Invoke the bench suite.
**Assertions.**
- The run produces one of the two contract behaviors:
  - **Behavior A (error):** the suite returns `Err(BenchError::UnsupportedRepresentation { benchmark: EpAnnihilation, representation: Sparse })`. Test asserts this exact variant.
  - **Behavior B (fallback):** the suite emits a row with `representation: Dense` (silently downgraded). Test asserts the row's `representation` column is `Dense` and a `tracing::warn!` event was emitted.
- The test must declare which behavior the production code implements (in a doc-comment) so the QA Stage 5 reviewer can sign off on the choice.
- No panic.
**Boundary case coverage.** Catches a buggy sparse path that *attempts* to handle `ep_annihilation` and produces incorrect results because `dual_tree_sparse` is the only sparse generator.
**Why it must exist.** Acceptance criterion #1 (sparse path is benchmark-gated).

---

## Coverage matrix

| test_id | AC-1 (gate dual_tree only) | AC-2 (isomorphism) | AC-3 (<80% memory) | AC-4 (reduction identical) | AC-5 (floor preserved) |
|---|---|---|---|---|---|
| UT-0606-01 | | ✅ | | ✅ | |
| IT-0606-02 | | ✅ | | ✅ | |
| IT-0606-03 | | ✅ | | ✅ | |
| IT-0606-04 | | | ✅ | | |
| IT-0606-05 | | ✅ | | ✅ | |
| IT-0606-06 | ✅ | | | | |

Every acceptance criterion 1-4 has ≥1 test. AC-5 is preserved by all tests passing without removing any existing test.

---

## Generator strategy decision (resolves task §Notes)

The task §Notes explicitly defers the "build sparse directly vs convert via `Net::to_sparse`" decision to the test-generator. **Decision:** build sparse directly via a new `dual_tree_sparse(N) -> SparseNet` generator. Rationale:
- The whole point of the micro-bench is to validate the **construction-phase** memory benefit. If we go via dense → `to_sparse`, we incur dense's allocation peak first, defeating the measurement.
- SPEC-09 R37c (committed `82b2d27`) requires construction-isomorphism — the sparse generator must produce a net isomorphic to the dense generator AT CONSTRUCTION TIME, not after a conversion. Building sparse directly is the only way to satisfy this with confidence.
- The conversion path (`Net::to_sparse`) remains useful for OTHER tests but is NOT used by this task.

UT-0606-01 enforces this decision.

---

## Out-of-scope tests (deferred to other tasks)

- The CSV writer that emits the memory ratio → TASK-0607.
- TCP-mode sparse runs → out of scope; D-1 is local-only per task §Dependencies.
- Sparse path during reduction → out of scope; per task §Notes, dominant memory is dense arena post-construction.
- Sparse path for benchmarks other than `dual_tree` → out of scope (IT-0606-06 documents this boundary).

---

## Known spec ambiguity (adversarial flag)

- SPEC-09 R18a (committed `82b2d27`) names the metric `peak_memory_during_construction`. SPEC-22 R12 acceptance text uses `<30% dense in ep_construct(5M)` for the original strict gate. The task relaxes this to `<80% in dual_tree(5000)`. **Flag:** the relaxation is documented in the task notes but is NOT in any spec file. If a future spec amendment locks the original strict gate, IT-0606-04 must be regenerated. Document the relaxation in the test's doc-comment with a citation to the D-011 plan §D-2.
- The "Behavior A vs Behavior B" choice in IT-0606-06 is a real ambiguity in the task itself. The QA Stage 5 reviewer must sign off on which the developer ships; the test is written to handle either but requires a doc-comment naming the chosen direction.
- SPEC-01 G1 (graph isomorphism invariant) is referenced as the correctness gate. The existing `nets_isomorphic` helper must compare under agent-id renumbering — if it does NOT (e.g. it requires identical id sets), IT-0606-02/03/05 will fail spuriously. Stage 3 developer must verify the helper or extend it.
