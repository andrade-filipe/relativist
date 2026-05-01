//! Integration tests for TASK-0606 — SparseNet bench path for `dual_tree`
//! (D-011 Phase D-1, P1).
//!
//! These tests exercise the full bench-suite path with
//! `representation = NetRepresentation::Sparse` for `dual_tree`, validating:
//!
//! - **AC-1**: sparse path is gated to `BenchmarkId::DualTree` only.
//! - **AC-2**: sparse-construction → `to_dense` → reduction produces a
//!   graph-isomorphic result vs the dense path.
//! - **AC-3**: at `dual_tree(5000)` (Linux only), sparse construction-phase
//!   peak memory is `< 80%` of dense.
//! - **AC-4**: reduction-phase trace (interaction count, terminal agent
//!   count) is identical between sparse and dense paths.
//!
//! Spec authority:
//! - SPEC-22 R12 — sparse-net path.
//! - SPEC-09 R18a..R18g, R37c — Tier 3 metrics + construction isomorphism
//!   (commit `82b2d27`).
//! - SPEC-01 G1 — graph-isomorphism invariant.
//!
//! See `docs/tests/TASK-0606-tests.md` for the full test specification.

use relativist_core::bench::isomorphism::nets_isomorphic;
use relativist_core::bench::suite::{
    build_input_net_from_suite, get_benchmark, run_benchmark_suite,
};
use relativist_core::bench::{
    BenchmarkId, BenchmarkSuiteConfig, Mode, NetRepresentation,
    RecyclePolicy as BenchRecyclePolicy, DEFAULT_BENCH_MAX_PENDING_LIFETIME,
};
use relativist_core::reduction::reduce_all;

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

/// Build a minimal `BenchmarkSuiteConfig` for a single (benchmark, size,
/// representation) tuple. Sequential mode keeps the run fast and removes
/// grid-side variance from the equation — what we want here is bit-stable
/// sparse-vs-dense comparison through the construction path.
fn make_suite_config(
    bench_id: BenchmarkId,
    size: u32,
    representation: NetRepresentation,
) -> BenchmarkSuiteConfig {
    BenchmarkSuiteConfig {
        benchmarks: vec![bench_id],
        sizes: Some(vec![size]),
        workers: vec![],
        mode: Mode::Sequential,
        warmup_runs: 0,
        repetitions: 1,
        csv_detail_path: None,
        csv_rounds_path: None,
        csv_summary_path: None,
        max_rounds: None,
        strict_bsp: false,
        skip_g1: false,
        chunk_size: None,
        max_pending_lifetime: DEFAULT_BENCH_MAX_PENDING_LIFETIME,
        recycle_policy: BenchRecyclePolicy::DisableUnderDelta,
        representation,
        sparse_construction_memory_csv_path: None,
    }
}

/// Build the input net via the production bench-suite path AND fully reduce
/// it. Returns `(reduced_net, total_interactions)`.
fn build_and_reduce(
    bench_id: BenchmarkId,
    size: u32,
    representation: NetRepresentation,
) -> (relativist_core::net::Net, u64) {
    let bench = get_benchmark(bench_id);
    let cfg = make_suite_config(bench_id, size, representation);
    let mut net = build_input_net_from_suite(&cfg, bench.as_ref(), size, 1)
        .expect("TASK-0606: build_input_net_from_suite must succeed for the configured bench/size");
    let stats = reduce_all(&mut net);
    (net, stats.total_interactions)
}

// ---------------------------------------------------------------------------
// IT-0606-02 — small-size end-to-end isomorphism
// ---------------------------------------------------------------------------

/// IT-0606-02 — Small-size sparse-vs-dense end-to-end check.
///
/// Per TEST-SPEC-0606: configure two suite-driven runs (Dense and Sparse) at
/// `dual_tree(size=5)` (depth 5 ⇒ 62 live CON agents — small enough that the
/// O(N!) `nets_isomorphic` backtracking is tractable in CI). After reduction,
/// both nets MUST be graph-isomorphic and reach the same total-interaction
/// count.
///
/// Note on size choice: the test-spec wrote `sizes = [500]` but `dual_tree`
/// takes a **depth**, not an agent count, so `size=500` would mean depth=500,
/// which is intractable. The harness's `default_sizes` for `dual_tree` are
/// `[4, 6, 8, 10, 12, 14]`. Depth 5 is chosen here as the small-fast witness.
#[test]
fn isomorphism_at_dual_tree_small_size() {
    let depth = 5u32;
    let (dense_net, dense_interactions) =
        build_and_reduce(BenchmarkId::DualTree, depth, NetRepresentation::Dense);
    let (sparse_net, sparse_interactions) =
        build_and_reduce(BenchmarkId::DualTree, depth, NetRepresentation::Sparse);

    assert_eq!(
        dense_interactions, sparse_interactions,
        "IT-0606-02: dense vs sparse interaction counts must match at depth={}",
        depth
    );
    assert_eq!(
        dense_net.count_live_agents(),
        sparse_net.count_live_agents(),
        "IT-0606-02: final agent count must match (dense_live={}, sparse_live={})",
        dense_net.count_live_agents(),
        sparse_net.count_live_agents()
    );
    assert!(
        nets_isomorphic(&dense_net, &sparse_net),
        "IT-0606-02: post-reduction dense and sparse nets must be graph-isomorphic (G1 / SPEC-01)"
    );
}

// ---------------------------------------------------------------------------
// IT-0606-03 — production-target-size isomorphism
// ---------------------------------------------------------------------------

/// IT-0606-03 — Production-size sparse-vs-dense end-to-end check.
///
/// Per TEST-SPEC-0606 the target is `dual_tree(5000)`. As above, `size` is
/// the **depth**; depth 5000 is impossible. Per the bench `default_sizes`
/// ([4, 6, 8, 10, 12, 14]) we use depth 12 here as the at-target witness:
/// depth 12 gives ~8k CON agents per tree (~16k total — large enough to
/// scale-test id renumbering and allocator pressure during reduction)
/// while keeping the reduction tractable in CI (depth 14 took ~3 min in
/// debug). The Linux-only IT-0606-04 also runs at depth 12 to match.
///
/// Note on isomorphism: `dual_tree` reduces to the **empty** net (every CON
/// pair annihilates). At this depth, the post-reduction net has 0 live
/// agents (the harness's existing dual_tree benchmark verifies this).
/// `nets_isomorphic` on two empty nets is trivially true. The interaction
/// count comparison is the load-bearing assertion at this scale (catches
/// scaling-only bugs in id renumbering / allocator pressure).
#[test]
fn isomorphism_at_dual_tree_5000() {
    let depth = 12u32;
    let (dense_net, dense_interactions) =
        build_and_reduce(BenchmarkId::DualTree, depth, NetRepresentation::Dense);
    let (sparse_net, sparse_interactions) =
        build_and_reduce(BenchmarkId::DualTree, depth, NetRepresentation::Sparse);

    assert_eq!(
        dense_interactions, sparse_interactions,
        "IT-0606-03: dense vs sparse interaction counts must match at depth={}",
        depth
    );
    assert_eq!(
        dense_net.count_live_agents(),
        sparse_net.count_live_agents(),
        "IT-0606-03: final agent count must match (depth={})",
        depth
    );
    assert!(
        nets_isomorphic(&dense_net, &sparse_net),
        "IT-0606-03: post-reduction dense and sparse nets must be graph-isomorphic at depth={}",
        depth
    );
}

// ---------------------------------------------------------------------------
// IT-0606-04 — sparse < 80% dense memory at production size (Linux only)
// ---------------------------------------------------------------------------

/// IT-0606-04 — sparse construction-phase peak memory is `< 80%` of dense.
///
/// **80%-gate caveat (D-011 plan §D-2).** This relaxation vs the original
/// SPEC-22 acceptance text ("<30% dense in ep_construct(5M)") is intentional:
/// `dual_tree` at the depths the bench harness supports is a smaller and
/// structurally different workload than `ep_construct(5M)`. The headline
/// metric for the Phase D-1 + Phase F-2 narrative is whether sparse delivers
/// a measurable, reproducible memory benefit at all on this workload — not
/// the strict `<30%` ratio that holds for the canonical EP profile. If a
/// future spec amendment locks the strict gate, this test must be regenerated.
///
/// **Linux-gated.** `get_peak_memory_during_construction` reads `VmHWM` from
/// `/proc/self/status`, which only exists on Linux. On other platforms the
/// probe returns `0`, which would make the ratio meaningless — so the test
/// is `#[cfg(target_os = "linux")]`.
///
/// Implementation: rather than using `run_benchmark_suite` (which only
/// captures one watermark per (benchmark, size) and discards intermediate
/// states), we call `build_input_net_from_suite` directly for each
/// representation and probe VmHWM **between** the two builds. This gives us
/// the construction-phase peak for each path independently.
#[cfg(target_os = "linux")]
#[test]
fn sparse_construction_memory_below_80pct_of_dense_at_5000() {
    use relativist_core::bench::memory::get_peak_memory_during_construction;

    // depth 12 ≈ 8k CON agents per tree, ~16k total — large enough for the
    // arena to be the dominant allocation. See IT-0606-03 for the depth
    // choice rationale.
    let depth = 12u32;
    let bench = get_benchmark(BenchmarkId::DualTree);

    // VmHWM is a per-process high-water mark; once raised it cannot drop.
    // To compare sparse vs dense IN THE SAME PROCESS we must:
    //   1. Build the **smaller** path FIRST (so its peak is the unique
    //      VmHWM at that point);
    //   2. Read VmHWM ⇒ this is the sparse peak;
    //   3. Drop the sparse net to release pages (kernel may not return
    //      them, but the dense build that follows reuses the address
    //      space);
    //   4. Build the dense path;
    //   5. Read VmHWM ⇒ this is the dense peak (≥ sparse peak, monotonic).
    //
    // The assertion `sparse_peak < 0.80 * dense_peak` then holds iff dense
    // truly exceeds sparse by >25%. Note: if BOTH builds fit under the
    // process's pre-existing VmHWM (set by some earlier test in the same
    // binary), both probes return the same value and the assertion fails
    // — this is why IT-0606-04 is a `cargo test --test ...` integration
    // test (separate binary per file), not an in-suite unit test.

    // Step 1: sparse build first.
    let sparse_cfg = make_suite_config(BenchmarkId::DualTree, depth, NetRepresentation::Sparse);
    let sparse_net = build_input_net_from_suite(&sparse_cfg, bench.as_ref(), depth, 1)
        .expect("IT-0606-04: sparse build must succeed");
    let sparse_peak = get_peak_memory_during_construction();
    drop(sparse_net);

    // Step 2: dense build second.
    let dense_cfg = make_suite_config(BenchmarkId::DualTree, depth, NetRepresentation::Dense);
    let dense_net = build_input_net_from_suite(&dense_cfg, bench.as_ref(), depth, 1)
        .expect("IT-0606-04: dense build must succeed");
    let dense_peak = get_peak_memory_during_construction();
    drop(dense_net);

    assert!(
        dense_peak > 0,
        "IT-0606-04: dense_peak must be > 0 on Linux (VmHWM probe failed?)"
    );
    assert!(
        sparse_peak > 0,
        "IT-0606-04: sparse_peak must be > 0 on Linux (VmHWM probe failed?)"
    );

    let ratio = sparse_peak as f64 / dense_peak as f64;
    eprintln!(
        "IT-0606-04: sparse_peak={} bytes, dense_peak={} bytes, ratio={:.4}",
        sparse_peak, dense_peak, ratio
    );

    // Headline 80%-gate (relaxed per D-011 plan §D-2 — see doc-comment above).
    assert!(
        ratio < 0.80,
        "IT-0606-04: sparse_peak/dense_peak={:.4} must be < 0.80 (the relaxed acceptance gate). \
         If this fires, sparse construction is NOT delivering the headline memory benefit at \
         dual_tree(depth={}). Check whether some other test in this binary inflated VmHWM \
         before this test ran (the comparison requires sparse to be the initial high-water).",
        ratio,
        depth
    );
    assert!(
        ratio > 0.0,
        "IT-0606-04: ratio must be > 0.0 (sparse cannot literally use zero memory)"
    );
}

// ---------------------------------------------------------------------------
// IT-0606-05 — reduction-phase results identical
// ---------------------------------------------------------------------------

/// IT-0606-05 — Reduction-phase results identical sparse vs dense.
///
/// Stronger version of IT-0606-02/03: locks the **interaction count**, not
/// just the structural isomorphism of the final net. This catches a sparse
/// path that produces a structurally equivalent net with a different
/// reduction order (e.g., due to redex-queue ordering being affected by
/// `to_dense`'s id renumbering). Such a divergence would invalidate
/// invariant comparisons in Phase F-2.
#[test]
fn reduction_phase_results_identical_sparse_vs_dense() {
    let depth = 8u32; // mid-range — exercises non-trivial trace without the depth-14 cost
    let (dense_net, dense_interactions) =
        build_and_reduce(BenchmarkId::DualTree, depth, NetRepresentation::Dense);
    let (sparse_net, sparse_interactions) =
        build_and_reduce(BenchmarkId::DualTree, depth, NetRepresentation::Sparse);

    // Reduction terminates in both cases (no infinite loop).
    // (Implicitly true: `reduce_all` is total. The assertion here is that
    // we got matching counts.)

    // Total interaction count must match exactly.
    assert_eq!(
        dense_interactions, sparse_interactions,
        "IT-0606-05: total_interactions must match exactly (depth={})",
        depth
    );

    // Final net identical structurally.
    assert_eq!(
        dense_net.count_live_agents(),
        sparse_net.count_live_agents(),
        "IT-0606-05: final live-agent count must match (depth={})",
        depth
    );

    assert!(
        nets_isomorphic(&dense_net, &sparse_net),
        "IT-0606-05: final reduced nets must be graph-isomorphic (depth={})",
        depth
    );
}

// ---------------------------------------------------------------------------
// IT-0606-06 — sparse path gated to dual_tree only (Behavior A: hard error)
// ---------------------------------------------------------------------------

/// IT-0606-06 — Sparse path is gated to `BenchmarkId::DualTree` only.
///
/// **Behavior A (chosen by the developer per dispatch).** The bench harness
/// returns an explicit error rather than silently falling back to dense for
/// non-`dual_tree` benchmarks under `representation = Sparse`. Rationale:
/// silent fallback would produce a row whose `representation=sparse`
/// column misrepresents the actual measurement, contaminating downstream
/// CSV analysis. An explicit error forces the operator to fix the typo or
/// add explicit support for the new benchmark.
///
/// The error is currently surfaced via `Result<_, String>` from
/// `build_input_net_from_suite`. The test asserts the call returns `Err`
/// and that the message mentions both the benchmark id and the
/// `representation=sparse` directive (so the operator can diagnose).
#[test]
fn sparse_path_only_supports_dual_tree_other_benches_unsupported() {
    let bench = get_benchmark(BenchmarkId::EPAnnihilation);
    let cfg = make_suite_config(BenchmarkId::EPAnnihilation, 100, NetRepresentation::Sparse);
    let result = build_input_net_from_suite(&cfg, bench.as_ref(), 100, 1);

    assert!(
        result.is_err(),
        "IT-0606-06 (Behavior A): EPAnnihilation under representation=Sparse must return Err"
    );
    let err = result.unwrap_err();
    assert!(
        err.contains("ep_annihilation"),
        "IT-0606-06: error must mention the benchmark id ('ep_annihilation'); got: {err}"
    );
    assert!(
        err.contains("sparse"),
        "IT-0606-06: error must mention 'sparse' to help the operator diagnose; got: {err}"
    );

    // Sanity: the same benchmark under representation=Dense must succeed.
    let cfg_dense = make_suite_config(BenchmarkId::EPAnnihilation, 100, NetRepresentation::Dense);
    let result_dense = build_input_net_from_suite(&cfg_dense, bench.as_ref(), 100, 1);
    assert!(
        result_dense.is_ok(),
        "IT-0606-06: EPAnnihilation under representation=Dense must still succeed"
    );

    // Suite-level: the full `run_benchmark_suite` must propagate the error
    // (it should NOT silently downgrade to dense and emit a contaminated row).
    let suite_result = run_benchmark_suite(&cfg);
    assert!(
        suite_result.is_err(),
        "IT-0606-06: run_benchmark_suite must propagate the unsupported-representation error"
    );
}
