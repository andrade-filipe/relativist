//! Benchmark suite runner (SPEC-09 R1, R3, R4, R30, R36-R38).
//!
//! Orchestrates the full benchmark protocol:
//! 1. Sequential baseline for each (benchmark, size)
//! 2. Warmup runs (discarded)
//! 3. Timed measurement repetitions
//! 4. Correctness verification on every repetition (G1)
//! 5. Derived metrics (speedup, efficiency, overhead ratio)
//! 6. Statistical aggregation over repetitions
//! 7. CSV output

use std::time::Instant;

use crate::bench::benchmarks::{
    cascade_cross::CascadeCross,
    church_add::ChurchAdd,
    church_mul::ChurchMul,
    church_sum_of_squares::ChurchSumOfSquares,
    condup_expansion::ConDupExpansion,
    dual_tree::DualTree,
    ep_annihilation::{EPAnnihilation, EPAnnihilationCon, EPAnnihilationDup},
    erasure_propagation::ErasurePropagation,
    mixed_net::MixedNet,
    tree_sum::{TreeSum, TreeSumBalanced},
};
use crate::bench::isomorphism::nets_match_counts;
use crate::bench::memory::get_peak_memory_bytes;
use crate::bench::stats;
use crate::bench::{
    AggregatedStats, Benchmark, BenchmarkId, BenchmarkResult, BenchmarkSuiteConfig,
    InteractionsByRule, Mode,
};
use crate::merge::{run_grid, GridConfig};
use crate::net::Net;
use crate::partition::ContiguousIdStrategy;
use crate::reduction::reduce_all;

/// Get the concrete benchmark implementation for a given ID.
pub fn get_benchmark(id: BenchmarkId) -> Box<dyn Benchmark> {
    match id {
        BenchmarkId::EPAnnihilation => Box::new(EPAnnihilation),
        BenchmarkId::EPAnnihilationCon => Box::new(EPAnnihilationCon),
        BenchmarkId::EPAnnihilationDup => Box::new(EPAnnihilationDup),
        BenchmarkId::ConDupExpansion => Box::new(ConDupExpansion),
        BenchmarkId::DualTree => Box::new(DualTree),
        BenchmarkId::TreeSum => Box::new(TreeSum),
        BenchmarkId::TreeSumBalanced => Box::new(TreeSumBalanced),
        BenchmarkId::MixedNet => Box::new(MixedNet),
        BenchmarkId::ErasurePropagation => Box::new(ErasurePropagation),
        BenchmarkId::ChurchAdd => Box::new(ChurchAdd),
        BenchmarkId::ChurchMul => Box::new(ChurchMul),
        BenchmarkId::CascadeCross => Box::new(CascadeCross),
        BenchmarkId::ChurchSumOfSquares => Box::new(ChurchSumOfSquares),
    }
}

/// Convert the engine's [u64; 6] per-rule array to InteractionsByRule.
/// Index order: [CON-CON, CON-DUP, CON-ERA, DUP-DUP, DUP-ERA, ERA-ERA].
fn ibr_from_array(arr: [u64; 6]) -> InteractionsByRule {
    InteractionsByRule {
        con_con: arr[0],
        con_dup: arr[1],
        con_era: arr[2],
        dup_dup: arr[3],
        dup_era: arr[4],
        era_era: arr[5],
    }
}

/// Measure a single sequential execution (R3, R23).
fn measure_sequential(
    net: &Net,
    benchmark_id: BenchmarkId,
    size: u32,
    repetition: u32,
) -> (BenchmarkResult, Net) {
    let mut net_clone = net.clone();
    let start = Instant::now();
    let engine_stats = reduce_all(&mut net_clone);
    let elapsed = start.elapsed().as_secs_f64();

    let mips = if elapsed > 0.0 {
        engine_stats.total_interactions as f64 / elapsed / 1_000_000.0
    } else {
        0.0
    };

    let result = BenchmarkResult {
        benchmark: benchmark_id,
        input_size: size,
        mode: Mode::Sequential,
        workers: 0,
        repetition,
        correct: true, // sequential is always correct by definition (it IS the baseline)
        wall_clock_secs: elapsed,
        total_interactions: engine_stats.total_interactions,
        mips,
        interactions_by_rule: ibr_from_array(engine_stats.interactions_by_rule),
        rounds: 0,
        border_redexes_per_round: vec![],
        border_ratio_per_round: vec![],
        peak_memory_bytes: get_peak_memory_bytes(),
        agents_per_round: vec![],
        bytes_sent: 0,
        bytes_received: 0,
        bytes_sent_per_round: vec![],
        bytes_received_per_round: vec![],
        partition_time_per_round: vec![],
        compute_time_per_round: vec![],
        merge_time_per_round: vec![],
        network_time_per_round: vec![],
        worker_stats: vec![],
        speedup: 1.0,
        efficiency: 1.0,
        overhead_ratio: 0.0,
    };

    (result, net_clone)
}

/// Measure a single grid (distributed) execution (R4, R22).
struct GridMeasureParams<'a> {
    net: &'a Net,
    benchmark: &'a dyn Benchmark,
    size: u32,
    workers: u32,
    repetition: u32,
    seq_result: &'a Net,
    seq_baseline_secs: f64,
    max_rounds: Option<u32>,
    /// Strict BSP mode (SPEC-05 R30a).
    strict_bsp: bool,
    /// When true, replace `benchmark.verify` with `nets_match_counts`
    /// (symbol-count fast check). L3 mitigation — see PHASE1-FINDINGS.md.
    skip_g1: bool,
}

fn measure_grid(params: &GridMeasureParams<'_>) -> BenchmarkResult {
    let net_clone = params.net.clone();
    let strategy = ContiguousIdStrategy;
    let config = GridConfig {
        num_workers: params.workers,
        max_rounds: params.max_rounds,
        strict_bsp: params.strict_bsp,
        ..GridConfig::default()
    };

    let start = Instant::now();
    let (result_net, grid_metrics) = run_grid(net_clone, &config, &strategy);
    let elapsed = start.elapsed().as_secs_f64();

    // Correctness verification (R36): verify on EVERY repetition.
    // When --skip-g1 is active, use the symbol-count fast check instead of
    // the full O(N!) isomorphism (L3 mitigation).
    let correct = if params.skip_g1 {
        nets_match_counts(params.seq_result, &result_net)
    } else {
        params.benchmark.verify(params.seq_result, &result_net)
    };

    let total_interactions = grid_metrics.total_interactions;
    let mips = if elapsed > 0.0 {
        total_interactions as f64 / elapsed / 1_000_000.0
    } else {
        0.0
    };

    // Derived metrics (R20)
    let speedup = if elapsed > 0.0 {
        params.seq_baseline_secs / elapsed
    } else {
        0.0
    };
    let efficiency = if params.workers > 0 {
        speedup / params.workers as f64
    } else {
        speedup
    };

    // Overhead ratio: fraction of time NOT spent on useful compute
    let compute_total: f64 = grid_metrics
        .compute_time_per_round
        .iter()
        .map(|d| d.as_secs_f64())
        .sum();
    let overhead_ratio = if elapsed > 0.0 {
        1.0 - (compute_total / elapsed)
    } else {
        0.0
    };

    // Border ratio per round
    let border_ratio_per_round: Vec<f64> = grid_metrics
        .border_redexes_per_round
        .iter()
        .zip(grid_metrics.local_interactions_per_round.iter())
        .map(|(&border, &local)| {
            let total = border as u64 + local;
            if total > 0 {
                border as f64 / total as f64
            } else {
                0.0
            }
        })
        .collect();

    BenchmarkResult {
        benchmark: params.benchmark.id(),
        input_size: params.size,
        mode: Mode::Local,
        workers: params.workers,
        repetition: params.repetition,
        correct,
        wall_clock_secs: elapsed,
        total_interactions,
        mips,
        interactions_by_rule: ibr_from_array(grid_metrics.total_interactions_by_rule),
        rounds: grid_metrics.rounds,
        border_redexes_per_round: grid_metrics.border_redexes_per_round.clone(),
        border_ratio_per_round,
        peak_memory_bytes: get_peak_memory_bytes(),
        agents_per_round: grid_metrics.agents_per_round.clone(),
        bytes_sent: grid_metrics.bytes_sent_per_round.iter().sum::<usize>() as u64,
        bytes_received: grid_metrics.bytes_received_per_round.iter().sum::<usize>() as u64,
        bytes_sent_per_round: grid_metrics
            .bytes_sent_per_round
            .iter()
            .map(|&x| x as u64)
            .collect(),
        bytes_received_per_round: grid_metrics
            .bytes_received_per_round
            .iter()
            .map(|&x| x as u64)
            .collect(),
        partition_time_per_round: grid_metrics
            .partition_time_per_round
            .iter()
            .map(|d| d.as_secs_f64())
            .collect(),
        compute_time_per_round: grid_metrics
            .compute_time_per_round
            .iter()
            .map(|d| d.as_secs_f64())
            .collect(),
        merge_time_per_round: grid_metrics
            .merge_time_per_round
            .iter()
            .map(|d| d.as_secs_f64())
            .collect(),
        network_time_per_round: grid_metrics
            .network_send_time_per_round
            .iter()
            .zip(grid_metrics.network_recv_time_per_round.iter())
            .map(|(s, r)| s.as_secs_f64() + r.as_secs_f64())
            .collect(),
        worker_stats: vec![], // Per-worker stats populated in distributed mode
        speedup,
        efficiency,
        overhead_ratio,
    }
}

/// Aggregate repetition results into summary statistics (R32-R34).
pub fn aggregate(results: &[BenchmarkResult]) -> AggregatedStats {
    assert!(!results.is_empty());
    let first = &results[0];

    let times: Vec<f64> = results.iter().map(|r| r.wall_clock_secs).collect();
    let mips_vals: Vec<f64> = results.iter().map(|r| r.mips).collect();
    let speedups: Vec<f64> = results.iter().map(|r| r.speedup).collect();
    let efficiencies: Vec<f64> = results.iter().map(|r| r.efficiency).collect();
    let overheads: Vec<f64> = results.iter().map(|r| r.overhead_ratio).collect();

    AggregatedStats {
        benchmark: first.benchmark,
        input_size: first.input_size,
        mode: first.mode,
        workers: first.workers,
        repetitions: results.len() as u32,
        all_correct: results.iter().all(|r| r.correct),
        wall_clock_mean: stats::mean(&times),
        wall_clock_std: stats::std_dev(&times),
        wall_clock_median: stats::median(&times),
        wall_clock_min: stats::min_f64(&times),
        wall_clock_max: stats::max_f64(&times),
        mips_mean: stats::mean(&mips_vals),
        speedup_mean: stats::mean(&speedups),
        efficiency_mean: stats::mean(&efficiencies),
        overhead_ratio_mean: stats::mean(&overheads),
        cv: stats::coeff_of_variation(&times),
    }
}

/// Result of running the full benchmark suite.
pub struct SuiteResult {
    /// All individual datapoints.
    pub results: Vec<BenchmarkResult>,
    /// Aggregated statistics per configuration.
    pub summaries: Vec<AggregatedStats>,
    /// Whether all datapoints passed correctness verification.
    pub all_correct: bool,
}

/// Run the full benchmark suite (SPEC-09 Section 4.3).
///
/// For each (benchmark, size):
///   1. Run sequential baseline (warmup + repetitions)
///   2. For each worker count: warmup + repetitions with grid
///   3. Verify correctness on every repetition
///   4. Aggregate statistics
///
/// Returns `Err` on correctness failure (R38: halt on first failure).
pub fn run_benchmark_suite(config: &BenchmarkSuiteConfig) -> Result<SuiteResult, String> {
    let mut all_results: Vec<BenchmarkResult> = Vec::new();
    let mut all_summaries: Vec<AggregatedStats> = Vec::new();

    for &bench_id in &config.benchmarks {
        let bench = get_benchmark(bench_id);
        let sizes = config
            .sizes
            .clone()
            .unwrap_or_else(|| bench.default_sizes());

        for &size in &sizes {
            // --- Sequential baseline (R3) ---
            // Warmup
            for _ in 0..config.warmup_runs {
                let mut warmup_net = bench.make_net(size);
                reduce_all(&mut warmup_net);
            }

            // Measure sequential repetitions
            let input_net = bench.make_net(size);
            let mut seq_results: Vec<BenchmarkResult> = Vec::new();
            let mut seq_reference_net: Option<Net> = None;

            for rep in 0..config.repetitions {
                let (result, reduced_net) = measure_sequential(&input_net, bench_id, size, rep);

                // R37b: verify each sequential repetition against the first.
                // Under --skip-g1, use the symbol-count fast check.
                if let Some(ref first_net) = seq_reference_net {
                    let matches = if config.skip_g1 {
                        nets_match_counts(first_net, &reduced_net)
                    } else {
                        bench.verify(first_net, &reduced_net)
                    };
                    if !matches {
                        return Err(format!(
                            "Sequential baseline mismatch! bench={}, size={}, rep={}. \
                             T6 violation: sequential reduction produced different normal forms.",
                            bench_id, size, rep
                        ));
                    }
                } else {
                    seq_reference_net = Some(reduced_net);
                }

                seq_results.push(result);
            }

            // Sequential baseline time (median of repetitions, R30)
            let seq_baseline_secs = stats::median(
                &seq_results
                    .iter()
                    .map(|r| r.wall_clock_secs)
                    .collect::<Vec<_>>(),
            );
            let seq_net = seq_reference_net.unwrap();

            println!("  {} — {}", bench_id, bench.describe(size));

            // Add sequential results to output
            all_results.extend(seq_results.iter().cloned());
            all_summaries.push(aggregate(&seq_results));

            // --- Distributed modes (R22) ---
            if config.mode == Mode::Sequential {
                continue; // Only sequential requested
            }

            for &workers in &config.workers {
                // Warmup
                for _ in 0..config.warmup_runs {
                    let warmup_net = bench.make_net(size);
                    let strategy = ContiguousIdStrategy;
                    let grid_config = GridConfig {
                        num_workers: workers,
                        max_rounds: config.max_rounds,
                        strict_bsp: config.strict_bsp,
                        ..GridConfig::default()
                    };
                    let _ = run_grid(warmup_net, &grid_config, &strategy);
                }

                // Measurement
                let mut rep_results: Vec<BenchmarkResult> = Vec::new();

                for rep in 0..config.repetitions {
                    let result = measure_grid(&GridMeasureParams {
                        net: &input_net,
                        benchmark: bench.as_ref(),
                        size,
                        workers,
                        repetition: rep,
                        seq_result: &seq_net,
                        seq_baseline_secs,
                        max_rounds: config.max_rounds,
                        strict_bsp: config.strict_bsp,
                        skip_g1: config.skip_g1,
                    });

                    // R38: halt on correctness failure
                    if !result.correct {
                        return Err(format!(
                            "Correctness failure! bench={}, size={}, workers={}, \
                             mode={}, rep={}",
                            bench_id, size, workers, config.mode, rep
                        ));
                    }

                    rep_results.push(result);
                }

                all_results.extend(rep_results.iter().cloned());
                all_summaries.push(aggregate(&rep_results));
            }
        }
    }

    let all_correct = all_results.iter().all(|r| r.correct);

    Ok(SuiteResult {
        results: all_results,
        summaries: all_summaries,
        all_correct,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_benchmark_all_variants() {
        let ids = [
            BenchmarkId::EPAnnihilation,
            BenchmarkId::EPAnnihilationCon,
            BenchmarkId::EPAnnihilationDup,
            BenchmarkId::ConDupExpansion,
            BenchmarkId::DualTree,
            BenchmarkId::TreeSum,
            BenchmarkId::TreeSumBalanced,
            BenchmarkId::MixedNet,
            BenchmarkId::ErasurePropagation,
            BenchmarkId::ChurchAdd,
            BenchmarkId::ChurchMul,
            BenchmarkId::CascadeCross,
            BenchmarkId::ChurchSumOfSquares,
        ];
        for id in ids {
            let bench = get_benchmark(id);
            assert_eq!(bench.id(), id);
        }
    }

    #[test]
    fn test_ibr_from_array() {
        let arr = [10, 20, 30, 40, 50, 60];
        let ibr = ibr_from_array(arr);
        assert_eq!(ibr.con_con, 10);
        assert_eq!(ibr.con_dup, 20);
        assert_eq!(ibr.con_era, 30);
        assert_eq!(ibr.dup_dup, 40);
        assert_eq!(ibr.dup_era, 50);
        assert_eq!(ibr.era_era, 60);
        assert_eq!(ibr.total(), 210);
    }

    #[test]
    fn test_measure_sequential_ep() {
        let bench = get_benchmark(BenchmarkId::EPAnnihilation);
        let net = bench.make_net(10);
        let (result, reduced) = measure_sequential(&net, BenchmarkId::EPAnnihilation, 10, 0);
        assert!(result.correct);
        assert_eq!(result.mode, Mode::Sequential);
        assert_eq!(result.workers, 0);
        assert_eq!(result.total_interactions, 10);
        assert!(result.wall_clock_secs > 0.0);
        assert_eq!(reduced.count_live_agents(), 0);
    }

    #[test]
    fn test_measure_grid_ep() {
        let bench = get_benchmark(BenchmarkId::EPAnnihilation);
        let net = bench.make_net(20);
        let mut seq_net = net.clone();
        reduce_all(&mut seq_net);
        let seq_baseline = 0.001; // arbitrary baseline for test

        let result = measure_grid(&GridMeasureParams {
            net: &net,
            benchmark: bench.as_ref(),
            size: 20,
            workers: 2,
            repetition: 0,
            seq_result: &seq_net,
            seq_baseline_secs: seq_baseline,
            max_rounds: None,
            strict_bsp: false,
            skip_g1: false,
        });
        assert!(result.correct);
        assert_eq!(result.mode, Mode::Local);
        assert_eq!(result.workers, 2);
        assert!(result.rounds > 0);
    }

    #[test]
    fn test_aggregate_basic() {
        let bench = get_benchmark(BenchmarkId::EPAnnihilation);
        let net = bench.make_net(10);
        let mut results = Vec::new();
        for rep in 0..3 {
            let (r, _) = measure_sequential(&net, BenchmarkId::EPAnnihilation, 10, rep);
            results.push(r);
        }
        let agg = aggregate(&results);
        assert_eq!(agg.benchmark, BenchmarkId::EPAnnihilation);
        assert_eq!(agg.input_size, 10);
        assert_eq!(agg.repetitions, 3);
        assert!(agg.all_correct);
        assert!(agg.wall_clock_mean > 0.0);
        assert_eq!(agg.speedup_mean, 1.0);
    }

    #[test]
    fn test_suite_sequential_only() {
        let config = BenchmarkSuiteConfig {
            benchmarks: vec![BenchmarkId::EPAnnihilation],
            sizes: Some(vec![10]),
            workers: vec![],
            mode: Mode::Sequential,
            warmup_runs: 0,
            repetitions: 3,
            csv_detail_path: None,
            csv_rounds_path: None,
            csv_summary_path: None,
            max_rounds: None,
            strict_bsp: false,
            skip_g1: false,
            chunk_size: None,
            max_pending_lifetime: 16,
            recycle_policy: crate::bench::RecyclePolicy::DisableUnderDelta,
            representation: crate::bench::NetRepresentation::Dense,
        };
        let result = run_benchmark_suite(&config).unwrap();
        assert!(result.all_correct);
        assert_eq!(result.results.len(), 3); // 3 sequential reps
        assert_eq!(result.summaries.len(), 1); // 1 config
    }

    #[test]
    fn test_suite_with_grid() {
        let config = BenchmarkSuiteConfig {
            benchmarks: vec![BenchmarkId::EPAnnihilation],
            sizes: Some(vec![20]),
            workers: vec![2],
            mode: Mode::Local,
            warmup_runs: 0,
            repetitions: 2,
            csv_detail_path: None,
            csv_rounds_path: None,
            csv_summary_path: None,
            max_rounds: None,
            strict_bsp: false,
            skip_g1: false,
            chunk_size: None,
            max_pending_lifetime: 16,
            recycle_policy: crate::bench::RecyclePolicy::DisableUnderDelta,
            representation: crate::bench::NetRepresentation::Dense,
        };
        let result = run_benchmark_suite(&config).unwrap();
        assert!(result.all_correct);
        // 2 sequential reps + 2 grid reps = 4
        assert_eq!(result.results.len(), 4);
        // 1 sequential config + 1 grid config = 2
        assert_eq!(result.summaries.len(), 2);
    }

    #[test]
    fn test_suite_correctness_holds_for_all_benchmarks() {
        // Run each benchmark with sequential + 2 workers to verify
        // correctness across all benchmark types
        let all_benchmarks = vec![
            BenchmarkId::EPAnnihilation,
            BenchmarkId::EPAnnihilationCon,
            BenchmarkId::EPAnnihilationDup,
            BenchmarkId::ConDupExpansion,
            BenchmarkId::DualTree,
            BenchmarkId::MixedNet,
            BenchmarkId::ErasurePropagation,
            BenchmarkId::TreeSum,
            BenchmarkId::ChurchAdd,
        ];
        for bench_id in all_benchmarks {
            let config = BenchmarkSuiteConfig {
                benchmarks: vec![bench_id],
                sizes: Some(vec![5]),
                workers: vec![2],
                mode: Mode::Local,
                warmup_runs: 0,
                repetitions: 1,
                csv_detail_path: None,
                csv_rounds_path: None,
                csv_summary_path: None,
                max_rounds: None,
                strict_bsp: false,
                skip_g1: false,
                chunk_size: None,
                max_pending_lifetime: 16,
                recycle_policy: crate::bench::RecyclePolicy::DisableUnderDelta,
                representation: crate::bench::NetRepresentation::Dense,
            };
            let result = run_benchmark_suite(&config).unwrap_or_else(|e| {
                panic!("Suite failed for {bench_id}: {e}");
            });
            assert!(result.all_correct, "Correctness failure for {bench_id}");
        }
    }

    #[test]
    fn test_suite_multiple_sizes() {
        let config = BenchmarkSuiteConfig {
            benchmarks: vec![BenchmarkId::EPAnnihilation],
            sizes: Some(vec![10, 50]),
            workers: vec![2],
            mode: Mode::Local,
            warmup_runs: 0,
            repetitions: 1,
            csv_detail_path: None,
            csv_rounds_path: None,
            csv_summary_path: None,
            max_rounds: None,
            strict_bsp: false,
            skip_g1: false,
            chunk_size: None,
            max_pending_lifetime: 16,
            recycle_policy: crate::bench::RecyclePolicy::DisableUnderDelta,
            representation: crate::bench::NetRepresentation::Dense,
        };
        let result = run_benchmark_suite(&config).unwrap();
        assert!(result.all_correct);
        // 2 sizes * (1 seq + 1 grid) = 4 results
        assert_eq!(result.results.len(), 4);
        // 2 sizes * 2 configs = 4 summaries
        assert_eq!(result.summaries.len(), 4);
    }

    #[test]
    fn test_suite_multiple_workers() {
        let config = BenchmarkSuiteConfig {
            benchmarks: vec![BenchmarkId::EPAnnihilation],
            sizes: Some(vec![20]),
            workers: vec![1, 2, 4],
            mode: Mode::Local,
            warmup_runs: 0,
            repetitions: 1,
            csv_detail_path: None,
            csv_rounds_path: None,
            csv_summary_path: None,
            max_rounds: None,
            strict_bsp: false,
            skip_g1: false,
            chunk_size: None,
            max_pending_lifetime: 16,
            recycle_policy: crate::bench::RecyclePolicy::DisableUnderDelta,
            representation: crate::bench::NetRepresentation::Dense,
        };
        let result = run_benchmark_suite(&config).unwrap();
        assert!(result.all_correct);
        // 1 seq + 3 grid configs = 4 results
        assert_eq!(result.results.len(), 4);
        // 1 seq summary + 3 grid summaries = 4
        assert_eq!(result.summaries.len(), 4);
    }

    #[test]
    fn test_speedup_computed_correctly() {
        let bench = get_benchmark(BenchmarkId::EPAnnihilation);
        let net = bench.make_net(100);
        let mut seq_net = net.clone();
        reduce_all(&mut seq_net);

        let result = measure_grid(&GridMeasureParams {
            net: &net,
            benchmark: bench.as_ref(),
            size: 100,
            workers: 2,
            repetition: 0,
            seq_result: &seq_net,
            seq_baseline_secs: 1.0, // 1 second baseline
            max_rounds: None,
            strict_bsp: false,
            skip_g1: false,
        });
        // Speedup = baseline / elapsed. Since EP is fast, speedup should be large
        assert!(result.speedup > 0.0);
        // Efficiency = speedup / workers
        assert!((result.efficiency - result.speedup / 2.0).abs() < 1e-10);
    }
}
