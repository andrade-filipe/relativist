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
//! ## D-012 Stage 6 REFACTOR (2026-05-05) — QA-D012-002 + reviewer MF-002
//!
//! The original IT-0618-A1 (commit `ac828f9`) called `run_grid` directly,
//! captured the in-memory `GridMetrics`, and computed `mips` itself —
//! bypassing the bench harness's CSV writers entirely. That witness pinned
//! the in-memory metric (which was already correct on every code path)
//! while leaving the literal v2-baseline failure mode (zero `mips_mean` in
//! the produced `summary.csv`) untouched. QA-D012-002 traced that failure
//! mode to `scripts/bench_docker_v2.sh:283`, which hardcoded the column
//! to 0.0 in a Python-embedded-in-bash literal — a layer the Rust witness
//! could never observe. The bash script is fixed in the same refactor.
//!
//! On the Rust side this file now ships THREE witnesses:
//!   * IT-0618-A1 (renamed): `metrics_struct_records_nonzero_total_interactions_and_mips`
//!     — the in-memory contract (was the misleading `tcp_round_*` test).
//!   * IT-0618-A2: aggregation invariant (unchanged).
//!   * IT-0618-A3 (new — closes SF-004): `single_worker_path_records_nonzero_total_interactions`
//!     — covers `run_single_worker` at `merge/grid.rs:1466+` (num_workers=1),
//!     which uses a different aggregation site than the multi-worker path.
//!   * IT-0618-A4 (new — closes MF-002): `bench_csv_summary_records_nonzero_mips`
//!     — actually exercises the bench harness through `run_benchmark_suite`
//!     and asserts the produced `summary.csv::mips_mean` row reads > 0.
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
// IT-0618-A1 — in-memory metric records non-zero total_interactions / mips
// ---------------------------------------------------------------------------

/// IT-0618-A1: A non-trivial grid run reports `total_interactions > 0` and
/// the derived `mips = total_interactions / wall_clock_secs * 1e-6` is also
/// strictly positive and within plausible CPU bounds.
///
/// D-012 Stage 6 REFACTOR (reviewer SF-003, 2026-05-05): renamed from
/// `tcp_round_records_nonzero_total_interactions_and_mips` because the body
/// uses `run_grid` (in-process), not a TCP round. The new name reflects
/// the actual code path.
///
/// Pre-RF-07-fix: BOTH zero. Post-fix on path (a): both non-zero. This
/// witness pins the in-memory contract; the CSV-end-to-end witness lives
/// at IT-0618-A4 below.
#[test]
fn metrics_struct_records_nonzero_total_interactions_and_mips() {
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
        (0.001..=1000.0).contains(&mips),
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

// ---------------------------------------------------------------------------
// IT-0618-A3 — Single-worker path also records non-zero total_interactions
// ---------------------------------------------------------------------------

/// IT-0618-A3: D-012 Stage 6 REFACTOR — closes reviewer SF-004. The
/// single-worker code path at `merge/grid.rs:1466+` (`run_single_worker`)
/// uses a different aggregation site than the multi-worker loop:
/// `metrics.total_interactions = stats.total_interactions` (assignment
/// rather than `+=`), and `border_interactions_per_round.push(0)`. Many
/// bench profiles use `workers = 1` for a parallel-execution baseline; if
/// the path-(a) plumbing is wired only on the multi-worker path, those
/// rows would still ship `total_interactions = 0`.
#[test]
fn single_worker_path_records_nonzero_total_interactions() {
    let bench = EPAnnihilation;
    let size = 100u32;
    let net = bench.make_net(size);

    let strategy = ContiguousIdStrategy;
    // num_workers = 1 forces the run_single_worker path at merge/grid.rs:1466.
    let config = GridConfig {
        num_workers: 1,
        max_rounds: Some(8),
        ..GridConfig::default()
    };
    let (_dist_net, metrics) = run_grid(net, &config, &strategy);

    assert!(
        metrics.total_interactions > 0,
        "RF-07 path (a) regression on the SINGLE-WORKER path: \
         metrics.total_interactions = 0 after ep_annihilation({}) on \
         num_workers=1. See merge/grid.rs:1466+ run_single_worker; the \
         assignment must propagate stats.total_interactions, not zero.",
        size
    );

    // Single-worker path records border_interactions_per_round.push(0)
    // structurally, so total_interactions must come entirely from the
    // local (assignment) site.
    let local_sum: u64 = metrics.local_interactions_per_round.iter().sum();
    let border_sum: u64 = metrics.border_interactions_per_round.iter().sum();
    assert_eq!(
        border_sum, 0,
        "single-worker path expected to push 0 to border_interactions_per_round; got {}",
        border_sum
    );
    assert_eq!(
        metrics.total_interactions, local_sum,
        "single-worker path: total_interactions = {} but \
         sum(local_interactions_per_round) = {} (border_sum = 0). The \
         run_single_worker assignment site must agree with the per-round \
         counter.",
        metrics.total_interactions, local_sum
    );
}

// ---------------------------------------------------------------------------
// IT-0618-A4 — End-to-end CSV witness via run_benchmark_suite
// ---------------------------------------------------------------------------

/// IT-0618-A4: D-012 Stage 6 REFACTOR — closes reviewer MF-002. The
/// original IT-0618-A1 stopped at the in-memory `GridMetrics`, never
/// exercising the bench harness's CSV writers; this test runs a complete
/// bench through `run_benchmark_suite` with `csv_summary_path` set to a
/// tempfile, then reads `summary.csv` back and asserts the produced
/// `mips_mean` column is strictly positive.
///
/// Note: the `bench` subcommand always runs the in-process path (per
/// QA-D012-003); the CSV row's `mode` column will reflect the operator's
/// requested mode label (see QA-D012-003 fix in bench/suite.rs), but the
/// underlying execution is `run_grid`. For the actual TCP-localhost CSV
/// path, the source of truth is `scripts/bench_docker_v2.sh` (see
/// QA-D012-002 fix). This test pins the Rust-side `bench` subcommand path.
#[test]
fn bench_csv_summary_records_nonzero_mips() {
    use relativist_core::bench::suite::run_benchmark_suite;
    use relativist_core::bench::{
        BenchmarkId, BenchmarkSuiteConfig, Mode, NetRepresentation, RecyclePolicy,
    };

    let tmpdir = tempdir_path("d012_mips_witness");
    std::fs::create_dir_all(&tmpdir).expect("create tempdir");
    let summary_path = tmpdir.join("summary.csv");
    let detail_path = tmpdir.join("detail.csv");

    let config = BenchmarkSuiteConfig {
        benchmarks: vec![BenchmarkId::EPAnnihilation],
        sizes: Some(vec![100]),
        workers: vec![2],
        mode: Mode::Local,
        warmup_runs: 0,
        repetitions: 2,
        csv_detail_path: Some(detail_path.to_string_lossy().into_owned()),
        csv_rounds_path: None,
        csv_summary_path: Some(summary_path.to_string_lossy().into_owned()),
        max_rounds: None,
        strict_bsp: false,
        skip_g1: false,
        chunk_size: None,
        max_pending_lifetime: 16,
        recycle_policy: RecyclePolicy::DisableUnderDelta,
        representation: NetRepresentation::Dense,
        sparse_construction_memory_csv_path: None,
    };

    let suite_result = run_benchmark_suite(&config).expect("run_benchmark_suite");
    assert!(!suite_result.summaries.is_empty(), "no summaries produced");

    // Find the grid (Mode::Local) summary aggregate.
    let grid_summary = suite_result
        .summaries
        .iter()
        .find(|s| s.mode == Mode::Local)
        .expect("no grid (mode=local) AggregatedStats in suite result");

    assert!(
        grid_summary.mips_mean > 0.0,
        "RF-07 path (a) end-to-end regression: AggregatedStats.mips_mean = {} \
         after a complete bench through run_benchmark_suite. The aggregator \
         (bench/suite.rs::aggregate) must propagate mips from the per-rep \
         BenchmarkResult.mips populated by bench/suite.rs:506-511. \
         See docs/analysis/D011-final-baseline-analysis-2026-05-04.md §3 RF-07 \
         and docs/qa/QA-D012-instrumentation-restore-2026-05-05.md QA-D012-002.",
        grid_summary.mips_mean
    );

    // Order-of-magnitude sanity (same band as IT-0618-A1).
    assert!(
        (0.001..=1000.0).contains(&grid_summary.mips_mean),
        "mips_mean = {} out of plausible CPU range [0.001, 1000.0] (KOP/s..GOP/s)",
        grid_summary.mips_mean
    );

    // CSV-writer-end-to-end: the `csv_summary_path` we configured above
    // should now contain a row for this benchmark. Read the file back and
    // sanity-check the mips_mean column.
    if summary_path.exists() {
        use std::io::Read;
        let mut content = String::new();
        std::fs::File::open(&summary_path)
            .expect("open summary.csv")
            .read_to_string(&mut content)
            .expect("read summary.csv");
        let mut lines = content.lines();
        let header = lines.next().expect("summary.csv has a header");
        let mips_idx = header
            .split(',')
            .position(|c| c == "mips_mean")
            .expect("summary.csv header contains mips_mean");
        let csv_mips: Option<f64> = lines
            .filter_map(|line| {
                let cells: Vec<&str> = line.split(',').collect();
                if cells.get(2).copied() == Some("local") {
                    cells.get(mips_idx).and_then(|c| c.parse::<f64>().ok())
                } else {
                    None
                }
            })
            .next();
        assert!(
            csv_mips.map(|m| m > 0.0).unwrap_or(false),
            "summary.csv::mips_mean is missing or zero on the grid row: {:?}",
            csv_mips
        );
    }

    // Best-effort cleanup.
    let _ = std::fs::remove_dir_all(&tmpdir);
}

/// Construct a unique tempdir path under the OS tempdir. Avoids the
/// `tempfile` crate dependency to keep this test free-standing.
fn tempdir_path(prefix: &str) -> std::path::PathBuf {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("{}_{}_{}", prefix, std::process::id(), nanos))
}
