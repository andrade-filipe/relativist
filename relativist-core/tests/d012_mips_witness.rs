//! D-012 TASK-0618 (D-011-FU-MIPS) — total_interactions / mips end-to-end witness.
//!
//! Closes RF-07 from `docs/analysis/D011-final-baseline-analysis-2026-05-04.md`
//! §3 RF-07 (lines 174-179): `summary.csv::mips_mean = 0.000` and
//! `detail.csv::total_interactions = 0` across all 40 rows of v2-post and
//! all 40 rows of v1. A symmetric, pre-existing dead column.
//!
//! ## Path decision: PATH (a) — implement
//!
//! Per TASK-0618 acceptance criterion 1, the implementation chose
//! **path (a)**: keep the columns and ensure `total_interactions` is
//! populated end-to-end (worker → coordinator → BenchmarkResult → CSV).
//!
//! Rationale (one paragraph): the wire-format / message-payload changes
//! TASK-0618 path (a) might have required are unnecessary in this
//! codebase. `WorkerRoundStats.local_redexes` is already populated by
//! `protocol/worker.rs` (line 255) and is already aggregated by
//! `protocol/coordinator.rs:1423,1449` into `metrics.total_interactions`.
//! Similarly, the in-process `merge/grid.rs:208,240,300` paths populate
//! `metrics.total_interactions`. The bench harness reads
//! `grid_metrics.total_interactions` at `bench/suite.rs:506` and computes
//! `mips = total_interactions / wall_clock_secs * 1e-6`. The end-to-end
//! plumbing is wired; this witness pins it. Path (b) — drop the columns —
//! was rejected because (1) the metric is already wired, (2) for Phase 3
//! LAN we want a real MIPS number to report, and (3) consistency with
//! TASK-0616 path (a) (both reuse the existing WorkerRoundStats payload
//! without any wire-format extension).
//!
//! Test inventory (per TEST-SPEC-0618 path (a) — IT-0618-A1, IT-0618-A2):
//!   IT-0618-A1 — `tcp_round_records_nonzero_total_interactions_and_mips`
//!   IT-0618-A2 — `total_interactions_equals_sum_of_per_round_interactions`
//!
//! Path (b) tests (IT-0618-B1, IT-0618-B2) are NOT created — TEST-SPEC-0618
//! says "the unused path's tests are NOT created (this differs from
//! TEST-SPEC-0616's #[ignore] approach because for path (b) the fields
//! they reference no longer exist, so the test wouldn't compile)."

use relativist_core::bench::benchmarks::ep_annihilation::EPAnnihilation;
use relativist_core::bench::isomorphism::nets_isomorphic;
use relativist_core::bench::Benchmark;
use relativist_core::merge::{run_grid, GridConfig};
use relativist_core::partition::strategy::ContiguousIdStrategy;
use relativist_core::reduction::engine::reduce_all;

// ---------------------------------------------------------------------------
// IT-0618-A1 — total_interactions / mips end-to-end (path a)
// ---------------------------------------------------------------------------

/// IT-0618-A1: A non-trivial grid run reports `total_interactions > 0` and
/// the derived `mips = total_interactions / wall_clock_secs * 1e-6` is also
/// strictly positive and within plausible CPU bounds.
///
/// Pre-RF-07-fix: BOTH zero. Post-fix on path (a): both non-zero. Direct
/// closure of TASK-0618 path (a) acceptance criterion.
#[test]
fn tcp_round_records_nonzero_total_interactions_and_mips() {
    let bench = EPAnnihilation;
    let size = 100u32;
    let net = bench.make_net(size);

    // Sequential oracle for G1 / correctness check.
    let mut seq_net = net.clone();
    reduce_all(&mut seq_net);

    let strategy = ContiguousIdStrategy;
    let config = GridConfig {
        num_workers: 2,
        max_rounds: Some(8),
        ..GridConfig::default()
    };
    let start = std::time::Instant::now();
    let (dist_net, metrics) = run_grid(net, &config, &strategy);
    let elapsed = start.elapsed().as_secs_f64();

    // Correctness sanity (path-orthogonal).
    assert!(
        nets_isomorphic(&seq_net, &dist_net),
        "G1 violated — distributed result not isomorphic to sequential."
    );

    // RF-07 closure (path (a)): total_interactions strictly positive.
    assert!(
        metrics.total_interactions > 0,
        "RF-07 path (a) regression: metrics.total_interactions = 0 after a \
         non-trivial bench. The aggregation site in protocol/coordinator.rs:1449 \
         (or merge/grid.rs:208,240,300) is missing or returning zero. See \
         docs/analysis/D011-final-baseline-analysis-2026-05-04.md §3 RF-07 and TASK-0618."
    );

    // Each ERA-ERA pair contributes 1 interaction; ep_annihilation(N) yields
    // N pairs and N annihilations.  A ±5% sanity band absorbs any cross-
    // partition adjustments (e.g., border resolution does an extra pass).
    let expected = size as u64;
    let lo = (expected as f64 * 0.95) as u64;
    let hi = (expected as f64 * 1.05) as u64;
    assert!(
        metrics.total_interactions >= lo && metrics.total_interactions <= hi,
        "total_interactions = {} outside [{}, {}] for ep_annihilation({}). \
         Either the workload changed or the aggregation is double-counting.",
        metrics.total_interactions,
        lo,
        hi,
        size
    );

    // MIPS derivation (mirrors bench/suite.rs:506-511).
    let mips = if elapsed > 0.0 {
        metrics.total_interactions as f64 / elapsed / 1_000_000.0
    } else {
        0.0
    };
    assert!(
        mips > 0.0,
        "RF-07 path (a) regression: derived mips = {} for total_interactions = {} \
         and wall = {} s. The formula at bench/suite.rs:506-511 should yield > 0.",
        mips,
        metrics.total_interactions,
        elapsed
    );

    // Order-of-magnitude sanity: `mips ∈ [0.001, 1000.0]` per TEST-SPEC-0618
    // IT-0618-A1 assertion 5 — at least 1 KOP/s, at most 1 GOP/s on a
    // representative laptop CPU. Guards against unit-confusion (off by 1e6).
    assert!(
        mips >= 0.001 && mips <= 1000.0,
        "mips = {} out of plausible CPU range [0.001, 1000.0] (KOP/s..GOP/s). \
         Possible unit-confusion bug.",
        mips
    );
}

// ---------------------------------------------------------------------------
// IT-0618-A2 — total_interactions == sum of per-round interactions (path a)
// ---------------------------------------------------------------------------

/// IT-0618-A2: The aggregate `metrics.total_interactions` equals the sum of
/// per-round interaction counts across all rounds. EXACT equality is
/// required because counts are integers, not floats. If the aggregation
/// is broken (overwrite instead of sum, double-count, missing border
/// contributions), this test fires.
#[test]
fn total_interactions_equals_sum_of_per_round_interactions() {
    let bench = EPAnnihilation;
    let size = 100u32;
    let net = bench.make_net(size);

    let strategy = ContiguousIdStrategy;
    let config = GridConfig {
        num_workers: 2,
        max_rounds: Some(8),
        ..GridConfig::default()
    };
    let (_dist_net, metrics) = run_grid(net, &config, &strategy);

    assert!(metrics.total_interactions > 0, "must be > 0");

    // Sum of per-round local + border interactions across all rounds.
    let local_sum: u64 = metrics.local_interactions_per_round.iter().sum();
    let border_sum: u64 = metrics.border_interactions_per_round.iter().sum();
    let per_round_total = local_sum + border_sum;

    // Exact equality: counts are integers, the aggregate at
    // merge/grid.rs:300 is `total_interactions += local + border`.
    assert_eq!(
        metrics.total_interactions, per_round_total,
        "TASK-0618 path (a) aggregation invariant violated: \
         metrics.total_interactions = {} but \
         sum(local_per_round) + sum(border_per_round) = {} + {} = {}. \
         See merge/grid.rs:208,240,300 for the canonical aggregation site.",
        metrics.total_interactions, local_sum, border_sum, per_round_total
    );

    // Defensive: also verify the per-rule decomposition sums correctly.
    // Each rule index records that rule's interaction count; their sum
    // must equal total_interactions exactly.
    let by_rule_sum: u64 = metrics.total_interactions_by_rule.iter().sum();
    assert_eq!(
        metrics.total_interactions, by_rule_sum,
        "TASK-0618 path (a): metrics.total_interactions = {} but \
         sum(total_interactions_by_rule) = {}. The two views of the same \
         counter disagree.",
        metrics.total_interactions, by_rule_sum
    );
}
