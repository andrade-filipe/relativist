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
use crate::bench::memory::{get_peak_memory_bytes, get_peak_memory_during_construction};
use crate::bench::stats;
use crate::bench::{
    AggregatedStats, Benchmark, BenchmarkId, BenchmarkResult, BenchmarkSuiteConfig,
    InteractionsByRule, Mode, NetRepresentation, RecyclePolicy as BenchRecyclePolicy,
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

/// Map the bench-suite `RecyclePolicy` enum onto `net::core::RecyclePolicy`,
/// the wire-level enum understood by `GridConfig.recycle_under_delta` (SPEC-22
/// R10b). The two enums are intentionally distinct (the bench-suite enum
/// derives `clap::ValueEnum` for CLI parsing; the net-core enum is the
/// authoritative kind on the wire), so we do an explicit variant-by-variant
/// translation here. A buggy translation would silently route every workload
/// through the wrong recycle path — UT-0604-04 pins this 1:1 mapping down.
pub(crate) fn bench_recycle_to_net_core(
    bench: BenchRecyclePolicy,
) -> crate::net::core::RecyclePolicy {
    match bench {
        BenchRecyclePolicy::DisableUnderDelta => crate::net::core::RecyclePolicy::DisableUnderDelta,
        BenchRecyclePolicy::BorderClean => crate::net::core::RecyclePolicy::BorderClean,
    }
}

/// Build the `GridConfig` used by both the eager and streaming paths in the
/// bench harness (TASK-0604 §AC-2, AC-3).
///
/// Propagates the four Tier 3 fields landed in TASK-0602 (`max_pending_lifetime`,
/// `recycle_policy`) from `BenchmarkSuiteConfig` into the `GridConfig` literal,
/// in addition to the v1 fields (`num_workers`, `max_rounds`, `strict_bsp`).
///
/// **Critical regression sentinel.** Pre-TASK-0604, `measure_grid` and the
/// warmup loop used `..GridConfig::default()` with no propagation — so even
/// when an operator passed `--max-pending-lifetime 64`, the bench numbers were
/// produced under the default lifetime of 16. This helper centralises the
/// propagation so a single audit point covers every call site.
pub fn build_grid_config_from_suite(config: &BenchmarkSuiteConfig, workers: u32) -> GridConfig {
    GridConfig {
        num_workers: workers,
        max_rounds: config.max_rounds,
        strict_bsp: config.strict_bsp,
        max_pending_lifetime: config.max_pending_lifetime,
        recycle_under_delta: bench_recycle_to_net_core(config.recycle_policy),
        ..GridConfig::default()
    }
}

/// Build the input `Net` used by the bench harness for one (benchmark, size)
/// rep (TASK-0604 §AC-1, AC-4).
///
/// Branches on `config.chunk_size`:
/// - `None` → eager path: returns `bench.make_net(size)` (status quo).
/// - `Some(N)` → streaming path: invokes `bench.make_net_stream(size, N)` and
///   assembles the resulting `AgentBatch` stream into a `SparseNet`, then
///   converts to a dense `Net` via `SparseNet::to_dense`. This is the
///   construction-only streaming path: agents are emitted incrementally
///   (the v1 memory benefit) but the eventual handoff to `run_grid` operates
///   on the full Net, so the rest of the bench pipeline (sequential baseline,
///   `run_grid`, verification) is unchanged.
///
/// We deliberately do NOT route through
/// `generate_and_partition_chunked_with_chunk_size_and_lifetime` + `merge`
/// here, because `merge::core::merge` requires post-reduction partitions
/// (its debug assertions fire on partitions that still hold live redexes —
/// MF-006 / D3 invariant). Construction-only assembly via `SparseNet`
/// preserves R37g pending-lifetime semantics through the
/// `ConnectionDirective::Pending` resolution path (see below).
///
/// Per SPEC-21 R37c (commit `82b2d27` §4.9), the streaming Net MUST be agent-
/// isomorphic to the eager Net. The bench harness's regression suite
/// (IT-0604-07, PT-0604-10) pins this property down explicitly.
///
/// # Errors
///
/// Returns `Err(String)` if the assembly fails (e.g., a malformed stream where
/// a `Pending` directive's target agent is not introduced within
/// `max_pending_lifetime` chunks). Eager-path callers never hit this branch.
pub fn build_input_net_from_suite(
    config: &BenchmarkSuiteConfig,
    bench: &dyn Benchmark,
    size: u32,
    workers_for_streaming: u32,
) -> Result<Net, String> {
    use crate::net::sparse::SparseNet;
    use crate::net::PortRef;
    use crate::partition::streaming::ConnectionDirective;

    let _ = workers_for_streaming; // reserved for future strategies; harness assembly is partition-free.

    // QA-D011-001 (CRITICAL): the path-selection grid `(chunk_size, representation)`
    // has a forbidden quadrant. When `chunk_size = Some(N)` AND
    // `representation = Sparse`, the legacy `match` would (incorrectly) take the
    // streaming branch and silently bypass `make_sparse_net`'s "sparse only
    // supported for dual_tree" guard — producing a row whose
    // `representation=sparse` field misrepresents the data structure used
    // (the streaming branch always assembles via `SparseNet` regardless, but
    // operator intent is "sparse-eager", which is incompatible with chunked
    // streaming until D-012 ships a true sparse-streaming pipeline).
    //
    // Refuse the combination explicitly here. The error is structured so the
    // bench harness halts (R38 halt-on-failure regime) before any row is emitted.
    if let (Some(chunk), NetRepresentation::Sparse) = (config.chunk_size, config.representation) {
        return Err(format!(
            "QA-D011-001: representation=Sparse is not yet supported with chunk_size=Some({}); \
             use --representation dense for streaming benchmarks (bench={} size={}). \
             Tracking: D-012 sparse-streaming pipeline.",
            chunk,
            bench.id(),
            size,
        ));
    }

    match config.chunk_size {
        None => match config.representation {
            NetRepresentation::Dense => Ok(bench.make_net(size)),
            NetRepresentation::Sparse => {
                // D-011 Phase D-1 (TASK-0606): sparse-eager construction path.
                //
                // 1. Build directly into a `SparseNet` so the construction
                //    peak does not include dense-arena allocation.
                // 2. Convert to a dense `Net` via `SparseNet::to_dense(None)`.
                //    The handoff to `run_grid` / `reduce_all` is unchanged.
                //
                // Per SPEC-09 R37c the resulting Net is graph-isomorphic to
                // `bench.make_net(size)`; UT-0606-01 (io::generators::tests)
                // enforces this for `dual_tree`.
                //
                // Behavior A (per D-011 dispatch): benchmarks other than
                // `dual_tree` return an explicit error rather than silently
                // falling back to dense, so an operator typo does not produce
                // a row whose `representation=sparse` field misrepresents the
                // measurement.
                let sparse = bench.make_sparse_net(size).map_err(|e| {
                    format!(
                        "TASK-0606 sparse-eager assembly: bench={} size={}: {}",
                        bench.id(),
                        size,
                        e
                    )
                })?;
                sparse.to_dense(None).map_err(|e| {
                    format!(
                        "TASK-0606 sparse-eager assembly: SparseNet::to_dense \
                         failed for bench={} size={}: {:?}",
                        bench.id(),
                        size,
                        e
                    )
                })
            }
        },
        Some(chunk_size) => {
            // Streaming branch: use the benchmark's stream override
            // (`make_net_stream` is overridden by `EPAnnihilation` to
            // `ep_annihilation_stream`, closing C-4 — see TASK-0604).
            let stream = bench.make_net_stream(size, chunk_size as usize);

            // Assemble incrementally into a SparseNet. We track unresolved
            // `Pending` directives keyed by `target_agent_id`; when the target
            // agent is introduced in a later batch, the pending wire is
            // installed. SPEC-21 R37g `max_pending_lifetime` bounds how many
            // chunks a Pending may sit unresolved.
            let mut sparse = SparseNet::new();
            let mut pending: Vec<(PendingEntry, u32 /* recorded at chunk */)> = Vec::new();
            let mut chunk_idx: u32 = 0;
            let max_lifetime = config.max_pending_lifetime;

            for batch in stream {
                // Install agents.
                for (id, symbol) in &batch.agents {
                    sparse.create_agent_at(*id, *symbol);
                }
                // Install resolved + freeport directives; queue Pending.
                for directive in &batch.connections {
                    match directive {
                        ConnectionDirective::Resolved {
                            source: (src_id, src_port),
                            target: (tgt_id, tgt_port),
                        } => {
                            sparse.connect(
                                PortRef::AgentPort(*src_id, *src_port),
                                PortRef::AgentPort(*tgt_id, *tgt_port),
                            );
                        }
                        ConnectionDirective::FreePortInterface {
                            agent_port: (a_id, a_port),
                            free_port_id,
                        } => {
                            sparse.connect(
                                PortRef::AgentPort(*a_id, *a_port),
                                PortRef::FreePort(*free_port_id),
                            );
                        }
                        ConnectionDirective::Pending {
                            source: (src_id, src_port),
                            target_agent_id,
                            target_port,
                        } => {
                            pending.push((
                                PendingEntry {
                                    source_id: *src_id,
                                    source_port: *src_port,
                                    target_id: *target_agent_id,
                                    target_port: *target_port,
                                },
                                chunk_idx,
                            ));
                        }
                    }
                }

                // Resolve any pending whose target is now present.
                pending.retain(|(p, _recorded)| {
                    if sparse.agents.contains_key(&p.target_id) {
                        sparse.connect(
                            PortRef::AgentPort(p.source_id, p.source_port),
                            PortRef::AgentPort(p.target_id, p.target_port),
                        );
                        false // remove
                    } else {
                        true
                    }
                });

                // R37g lifetime check.
                if max_lifetime != u32::MAX {
                    if let Some((expired, recorded)) = pending
                        .iter()
                        .find(|(_, r)| chunk_idx >= *r && (chunk_idx - r) > max_lifetime)
                    {
                        return Err(format!(
                            "TASK-0604 streaming assembly: Pending directive for \
                             target_agent_id={} expired (recorded at chunk {}, current chunk {}, \
                             max_pending_lifetime={})",
                            expired.target_id, recorded, chunk_idx, max_lifetime
                        ));
                    }
                }

                chunk_idx = chunk_idx.saturating_add(1);
            }

            if !pending.is_empty() {
                return Err(format!(
                    "TASK-0604 streaming assembly: {} Pending directive(s) remained \
                     unresolved at end of stream (bench={} size={} chunk_size={})",
                    pending.len(),
                    bench.id(),
                    size,
                    chunk_size
                ));
            }

            // Convert to dense Net.
            sparse.to_dense(None).map_err(|e| {
                format!(
                    "TASK-0604 streaming assembly: SparseNet::to_dense failed for \
                     bench={} size={} chunk_size={}: {:?}",
                    bench.id(),
                    size,
                    chunk_size,
                    e
                )
            })
        }
    }
}

/// Internal entry for a deferred (cross-batch) wire pending its target agent.
struct PendingEntry {
    source_id: u32,
    source_port: u8,
    target_id: u32,
    target_port: u8,
}

/// Convert the raw VmHWM probe value to the row representation used by
/// `SparseConstructionRow.peak_memory_during_construction`.
///
/// The probe returns `0` on non-Linux targets where `/proc/self/status` is
/// unavailable. Per TEST-SPEC-0607, the CSV column should be **blank** in
/// that case (not literal `0`, which would be indistinguishable from
/// "sparse used zero memory" — a false success signal). Linux Hub captures
/// (`>0`) round-trip as `Some(_)`.
fn peak_for_sparse_row(raw: u64) -> Option<u64> {
    if raw == 0 {
        None
    } else {
        Some(raw)
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
///
/// SPEC-09 R18a (TASK-0605): `peak_memory_during_construction` is captured
/// once for the (benchmark, size) outside the rep loop and forwarded into
/// each row. The per-rep `peak_memory_bytes` (legacy R18) is sampled here
/// AFTER `reduce_all` returns.
fn measure_sequential(
    net: &Net,
    benchmark_id: BenchmarkId,
    size: u32,
    repetition: u32,
    peak_memory_during_construction: u64,
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
        peak_memory_during_construction,
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
    /// SPEC-21 R37g pending-store memory bound (TASK-0604 §AC-2). Propagated
    /// from `BenchmarkSuiteConfig.max_pending_lifetime` so the bench numbers
    /// reflect the operator's chosen lifetime, not the silent default.
    max_pending_lifetime: u32,
    /// SPEC-22 R10b free-list recycling policy (TASK-0604 §AC-3). Propagated
    /// from `BenchmarkSuiteConfig.recycle_policy` so the streaming + delta
    /// path's recycle behavior matches operator choice.
    recycle_policy: BenchRecyclePolicy,
    /// SPEC-09 R18a (TASK-0605): VmHWM watermark sampled at the
    /// construction-complete program point — captured once per (benchmark,
    /// size) before the rep loop and forwarded into every grid row.
    peak_memory_during_construction: u64,
}

fn measure_grid(params: &GridMeasureParams<'_>) -> BenchmarkResult {
    let net_clone = params.net.clone();
    let strategy = ContiguousIdStrategy;
    let config = GridConfig {
        num_workers: params.workers,
        max_rounds: params.max_rounds,
        strict_bsp: params.strict_bsp,
        max_pending_lifetime: params.max_pending_lifetime,
        recycle_under_delta: bench_recycle_to_net_core(params.recycle_policy),
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
        peak_memory_during_construction: params.peak_memory_during_construction,
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
    /// Sparse-construction-memory rows (D-011 Phase D-3, TASK-0607).
    ///
    /// Populated only when `BenchmarkSuiteConfig.sparse_construction_memory_csv_path`
    /// is `Some(_)`. One row per `(benchmark, size, representation)` triple:
    /// - When `representation == Sparse`, the harness ALSO does a paired
    ///   dense build for the same `(benchmark, size)` so the dense/sparse
    ///   ratio is computable.
    /// - `ratio_to_dense` is filled at end-of-suite via
    ///   `compute_ratios_for_sparse_rows`.
    ///
    /// Empty when the field is `None` — preserving zero-overhead status quo
    /// for existing rodadas (`v1_local_baseline` etc.).
    pub sparse_construction_rows: Vec<crate::bench::csv::SparseConstructionRow>,
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
    use crate::bench::csv::{compute_ratios_for_sparse_rows, SparseConstructionRow};

    let mut all_results: Vec<BenchmarkResult> = Vec::new();
    let mut all_summaries: Vec<AggregatedStats> = Vec::new();

    // TASK-0607: sparse-construction-memory rows. Populated only when
    // `sparse_construction_memory_csv_path` is set; otherwise this stays
    // empty and the existing CSV outputs are bit-identical to v1.
    let mut sparse_construction_rows: Vec<SparseConstructionRow> = Vec::new();
    let emit_sparse_rows = config.sparse_construction_memory_csv_path.is_some();

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

            // Measure sequential repetitions.
            //
            // TASK-0604 §AC-1, AC-4: route through `build_input_net_from_suite`
            // so `chunk_size: Some(N)` exercises the streaming construction path
            // (`bench.make_net_stream` → `generate_and_partition_chunked_*` →
            // `merge`). The streaming Net is agent-isomorphic to the eager
            // `make_net(size)` per SPEC-21 R37c. `workers_for_streaming` is the
            // first non-empty worker count when streaming, otherwise 1 (degenerate
            // partition). The eager branch is bit-stable and ignores this arg.
            let workers_for_streaming = config.workers.first().copied().unwrap_or(1).max(1);
            let input_net =
                build_input_net_from_suite(config, bench.as_ref(), size, workers_for_streaming)?;

            // SPEC-09 R18a (TASK-0605): sample VmHWM at the construction-complete
            // program point — AFTER `build_input_net_from_suite` returns AND
            // BEFORE the first `reduce_all` / `run_grid` invocation. The watermark
            // is captured ONCE per (benchmark, size) and forwarded into every
            // sequential and grid row, so all reps share the same R18a value
            // (the field is a property of the constructed input net, not of an
            // individual reduction trace).
            let peak_memory_during_construction = get_peak_memory_during_construction();

            // TASK-0607: sparse-construction-memory sub-CSV row collection.
            //
            // When the operator passed `--csv-sparse <path>`, emit one row
            // per (benchmark, size, representation) tuple. The row's
            // `peak_memory_during_construction` is `None` on non-Linux
            // platforms (where the VmHWM probe returns 0); on Linux it is
            // `Some(peak)`. Ratios are computed at end-of-suite via
            // `compute_ratios_for_sparse_rows` once all rows are present.
            if emit_sparse_rows {
                let peak_for_row = peak_for_sparse_row(peak_memory_during_construction);
                sparse_construction_rows.push(SparseConstructionRow {
                    benchmark: bench_id,
                    size,
                    representation: config.representation,
                    peak_memory_during_construction: peak_for_row,
                    ratio_to_dense: None, // filled after the loop
                });

                // When the main run is Sparse, AUTO-PAIR a Dense build for
                // the same (benchmark, size) so the ratio_to_dense column
                // is populated. The paired dense build runs AFTER the
                // sparse main run, so VmHWM is already at sparse-peak; the
                // larger dense allocation correctly raises VmHWM to its own
                // (higher) peak, which is what the second probe reads.
                if config.representation == NetRepresentation::Sparse {
                    let mut dense_cfg = config.clone();
                    dense_cfg.representation = NetRepresentation::Dense;
                    // Build dense (no reduction — we only need the construction peak).
                    let _dense_net = build_input_net_from_suite(
                        &dense_cfg,
                        bench.as_ref(),
                        size,
                        workers_for_streaming,
                    )?;
                    let dense_peak = get_peak_memory_during_construction();
                    sparse_construction_rows.push(SparseConstructionRow {
                        benchmark: bench_id,
                        size,
                        representation: NetRepresentation::Dense,
                        peak_memory_during_construction: peak_for_sparse_row(dense_peak),
                        ratio_to_dense: None, // filled after the loop
                    });
                    // _dense_net is dropped here; the kernel may or may not
                    // release pages — irrelevant for the captured probe.
                }
            }

            let mut seq_results: Vec<BenchmarkResult> = Vec::new();
            let mut seq_reference_net: Option<Net> = None;

            for rep in 0..config.repetitions {
                let (result, reduced_net) = measure_sequential(
                    &input_net,
                    bench_id,
                    size,
                    rep,
                    peak_memory_during_construction,
                );

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
                // Warmup. TASK-0604 §AC-2/AC-3: propagate Tier 3 fields
                // (`max_pending_lifetime`, `recycle_policy`) into the warmup
                // GridConfig too — otherwise the warmup pass is silently run
                // under defaults and may leave a divergent JIT/cache state for
                // the measurement passes.
                for _ in 0..config.warmup_runs {
                    let warmup_net = bench.make_net(size);
                    let strategy = ContiguousIdStrategy;
                    let grid_config = build_grid_config_from_suite(config, workers);
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
                        max_pending_lifetime: config.max_pending_lifetime,
                        recycle_policy: config.recycle_policy,
                        peak_memory_during_construction,
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

    // TASK-0607: compute ratio_to_dense at end-of-suite.
    if emit_sparse_rows {
        compute_ratios_for_sparse_rows(&mut sparse_construction_rows);
    }

    Ok(SuiteResult {
        results: all_results,
        summaries: all_summaries,
        all_correct,
        sparse_construction_rows,
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
        let (result, reduced) = measure_sequential(&net, BenchmarkId::EPAnnihilation, 10, 0, 0);
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
            max_pending_lifetime: 16,
            recycle_policy: BenchRecyclePolicy::DisableUnderDelta,
            peak_memory_during_construction: 0,
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
            let (r, _) = measure_sequential(&net, BenchmarkId::EPAnnihilation, 10, rep, 0);
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
            sparse_construction_memory_csv_path: None,
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
            sparse_construction_memory_csv_path: None,
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
                sparse_construction_memory_csv_path: None,
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
            sparse_construction_memory_csv_path: None,
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
            sparse_construction_memory_csv_path: None,
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
            max_pending_lifetime: 16,
            recycle_policy: BenchRecyclePolicy::DisableUnderDelta,
            peak_memory_during_construction: 0,
        });
        // Speedup = baseline / elapsed. Since EP is fast, speedup should be large
        assert!(result.speedup > 0.0);
        // Efficiency = speedup / workers
        assert!((result.efficiency - result.speedup / 2.0).abs() < 1e-10);
    }

    // ---------------------------------------------------------------------------
    // TASK-0604 — Bench harness path selection (Phase C-2 + C-4, P0)
    //
    // Per TEST-SPEC-0604:
    //   UT-0604-01 — path selection: chunk_size=Some(N) routes through streaming
    //   UT-0604-02 — path selection: chunk_size=None routes through eager
    //   UT-0604-03 — eager branch GridConfig propagates max_pending_lifetime
    //   UT-0604-04 — streaming branch GridConfig propagates recycle_policy
    //   UT-0604-05 — ep_annihilation streaming dispatch invokes stream impl
    // ---------------------------------------------------------------------------

    use crate::bench::{NetRepresentation, RecyclePolicy};

    /// Helper: make a `BenchmarkSuiteConfig` with default Tier 3 fields.
    fn suite_config_default_tier3() -> BenchmarkSuiteConfig {
        BenchmarkSuiteConfig {
            benchmarks: vec![BenchmarkId::EPAnnihilation],
            sizes: Some(vec![20]),
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
            recycle_policy: RecyclePolicy::DisableUnderDelta,
            representation: NetRepresentation::Dense,
            sparse_construction_memory_csv_path: None,
        }
    }

    /// UT-0604-01 — `Some(chunk_size)` routes through the streaming path.
    ///
    /// Behavioral check: for ep_annihilation the streaming branch produces a
    /// Net with the same live-agent count as the eager branch (R37c isomorphism),
    /// AND the streaming branch goes through `make_net_stream` rather than
    /// `make_net`. We can't introspect `which fn was called` directly, so we
    /// rely on the chain: streaming branch → `make_net_stream` → for
    /// ep_annihilation this is `ep_annihilation_stream` (per the override).
    /// The resulting Net round-trips through `merge::merge` exactly when the
    /// streaming branch fires. Both Nets must have the same live-agent count.
    #[test]
    fn ut_0604_01_path_selection_some_chunk_size_invokes_streaming_branch() {
        let bench = get_benchmark(BenchmarkId::EPAnnihilation);
        let mut config = suite_config_default_tier3();
        config.chunk_size = Some(50);
        config.max_pending_lifetime = 16;
        config.recycle_policy = RecyclePolicy::DisableUnderDelta;

        let net = build_input_net_from_suite(&config, bench.as_ref(), 200, 2)
            .expect("UT-0604-01: streaming branch must succeed for ep_annihilation");
        // ep_annihilation(200) = 400 live agents pre-reduction.
        assert_eq!(
            net.count_live_agents(),
            400,
            "UT-0604-01: streaming-built net must contain 2*size live agents"
        );
    }

    /// UT-0604-02 — `None` routes through the eager path.
    ///
    /// Symmetric pair with UT-0604-01. The eager branch's output is bit-stable
    /// across repeated invocations and identical to the unrouted
    /// `bench.make_net(size)`.
    #[test]
    fn ut_0604_02_path_selection_none_chunk_size_invokes_eager_branch() {
        let bench = get_benchmark(BenchmarkId::EPAnnihilation);
        let mut config = suite_config_default_tier3();
        config.chunk_size = None;

        let routed = build_input_net_from_suite(&config, bench.as_ref(), 200, 2)
            .expect("UT-0604-02: eager branch must always succeed");
        let direct = bench.make_net(200);
        assert_eq!(
            routed.count_live_agents(),
            direct.count_live_agents(),
            "UT-0604-02: eager branch must agree with bench.make_net (live-agent count)"
        );
        assert_eq!(
            routed.agents.len(),
            direct.agents.len(),
            "UT-0604-02: eager branch must agree with bench.make_net (arena size — bit-stability sentinel)"
        );
    }

    /// UT-0604-03 — eager branch `GridConfig.max_pending_lifetime` carries
    /// `BenchmarkSuiteConfig.max_pending_lifetime` (acceptance criterion #2).
    #[test]
    fn ut_0604_03_eager_branch_grid_config_carries_max_pending_lifetime() {
        let mut config = suite_config_default_tier3();
        config.chunk_size = None;
        config.max_pending_lifetime = 64;

        let grid_config = build_grid_config_from_suite(&config, 4);
        assert_eq!(
            grid_config.max_pending_lifetime, 64,
            "UT-0604-03: GridConfig.max_pending_lifetime MUST equal config.max_pending_lifetime (64), not GridConfig::default() (16)"
        );
        assert_ne!(
            grid_config.max_pending_lifetime,
            u32::MAX,
            "UT-0604-03: bench-path GridConfig MUST NOT use the legacy u32::MAX disabled sentinel"
        );
    }

    /// UT-0604-04 — streaming branch `GridConfig.recycle_under_delta` carries
    /// `BenchmarkSuiteConfig.recycle_policy` (acceptance criterion #3).
    #[test]
    fn ut_0604_04_streaming_branch_grid_config_carries_recycle_policy() {
        let mut config = suite_config_default_tier3();
        config.chunk_size = Some(100);
        config.recycle_policy = RecyclePolicy::BorderClean;

        let grid_config = build_grid_config_from_suite(&config, 4);
        assert_eq!(
            grid_config.recycle_under_delta,
            crate::net::core::RecyclePolicy::BorderClean,
            "UT-0604-04: GridConfig.recycle_under_delta MUST mirror config.recycle_policy"
        );

        // Symmetric: DisableUnderDelta also propagates.
        let mut config_disable = suite_config_default_tier3();
        config_disable.recycle_policy = RecyclePolicy::DisableUnderDelta;
        let gc_disable = build_grid_config_from_suite(&config_disable, 4);
        assert_eq!(
            gc_disable.recycle_under_delta,
            crate::net::core::RecyclePolicy::DisableUnderDelta,
            "UT-0604-04: DisableUnderDelta variant must also propagate (regression guard)"
        );
    }

    /// UT-0604-05 — when benchmark id = `EpAnnihilation` AND `chunk_size.is_some()`,
    /// the dispatch invokes `ep_annihilation_stream` (the stream override),
    /// NOT the default `default_chunked_iter` adapter.
    ///
    /// We verify this behaviorally: `ep_annihilation_stream(size, chunk_size)`
    /// produces multiple batches when `2*size > chunk_size`, while
    /// `default_chunked_iter(make_net(size))` always produces exactly 1 batch.
    /// If the dispatch wrongly fell through to the default adapter, the
    /// streaming-side "many small batches" property would be lost — but the
    /// final merged Net would still have 2*size agents, so we instead verify
    /// the override is wired by checking the stream itself (the dispatch
    /// invokes `bench.make_net_stream`, which is the override for EPAnnihilation).
    #[test]
    fn ut_0604_05_ep_annihilation_streaming_dispatch_invokes_stream_impl() {
        let bench = get_benchmark(BenchmarkId::EPAnnihilation);
        let chunk_size = 10usize;
        let size = 100u32; // 200 agents → 20 batches at chunk_size=10
        let stream = bench.make_net_stream(size, chunk_size);
        let batches: Vec<_> = stream.collect();
        // ep_annihilation_stream emits one pair (2 agents) per batch when
        // chunk_size=10 → pairs_per_batch = 5 → 100/5 = 20 batches.
        // default_chunked_iter would emit exactly 1 batch.
        assert!(
            batches.len() > 1,
            "UT-0604-05: EPAnnihilation.make_net_stream MUST be the native override \
             (multi-batch); got {} batch(es) — looks like the default impl was used",
            batches.len()
        );
        // Total agents must still equal 2*size.
        let total: usize = batches.iter().map(|b| b.agents.len()).sum();
        assert_eq!(
            total,
            (2 * size) as usize,
            "UT-0604-05: total agents from override must equal 2*size"
        );
    }

    /// UT-0604 — `bench_recycle_to_net_core` is a 1:1 variant translation.
    /// Internal helper test: catches a buggy mapping that would silently route
    /// every workload through the wrong recycle path.
    #[test]
    fn ut_0604_bench_recycle_to_net_core_is_one_to_one() {
        assert_eq!(
            bench_recycle_to_net_core(RecyclePolicy::DisableUnderDelta),
            crate::net::core::RecyclePolicy::DisableUnderDelta
        );
        assert_eq!(
            bench_recycle_to_net_core(RecyclePolicy::BorderClean),
            crate::net::core::RecyclePolicy::BorderClean
        );
    }

    // ---------------------------------------------------------------------------
    // QA-D011-001 (CRITICAL) — forbidden quadrant: chunk_size=Some + representation=Sparse
    //
    // Pre-fix: the outer `match config.chunk_size` would take the streaming
    // branch unconditionally, silently overriding the operator's
    // `representation=Sparse` choice and bypassing the `make_sparse_net`
    // "non-dual_tree" guard. Post-fix: the harness errors out explicitly.
    // ---------------------------------------------------------------------------

    /// QA-D011-001 — sparse + chunk_size combo errors loudly (regression sentinel).
    #[test]
    fn qa_d011_001_sparse_plus_chunk_size_errors_explicitly() {
        let bench = get_benchmark(BenchmarkId::EPAnnihilation);
        let mut config = suite_config_default_tier3();
        config.chunk_size = Some(50);
        config.representation = NetRepresentation::Sparse;

        let res = build_input_net_from_suite(&config, bench.as_ref(), 100, 2);
        assert!(
            res.is_err(),
            "QA-D011-001: sparse + chunk_size MUST error, got Ok(_)"
        );
        let err = res.unwrap_err();
        assert!(
            err.contains("QA-D011-001"),
            "QA-D011-001: error message MUST cite the finding ID; got: {err}"
        );
        assert!(
            err.contains("Sparse") || err.contains("sparse"),
            "QA-D011-001: error message MUST cite the offending representation; got: {err}"
        );
    }

    /// QA-D011-001 — even for the otherwise-supported `dual_tree` benchmark,
    /// sparse + chunk_size is rejected (the guard is purely on the path
    /// combination, not on the benchmark).
    #[test]
    fn qa_d011_001_sparse_plus_chunk_size_rejected_for_dual_tree_too() {
        let bench = get_benchmark(BenchmarkId::DualTree);
        let mut config = suite_config_default_tier3();
        config.chunk_size = Some(10);
        config.representation = NetRepresentation::Sparse;

        let res = build_input_net_from_suite(&config, bench.as_ref(), 5, 2);
        assert!(
            res.is_err(),
            "QA-D011-001: sparse + chunk_size MUST error even for dual_tree"
        );
    }

    /// QA-D011-001 (negative — the fix MUST NOT regress the three valid cells).
    /// Confirms that all three valid `(chunk_size, representation)` cells still
    /// build successfully:
    ///   - (None, Dense)  — eager-dense (v1 status quo)
    ///   - (Some, Dense)  — streaming
    ///   - (None, Sparse) — sparse-eager (dual_tree only)
    #[test]
    fn qa_d011_001_three_valid_quadrants_still_build() {
        let bench_ep = get_benchmark(BenchmarkId::EPAnnihilation);
        let bench_dt = get_benchmark(BenchmarkId::DualTree);

        // Cell 1: (None, Dense)
        let mut c1 = suite_config_default_tier3();
        c1.chunk_size = None;
        c1.representation = NetRepresentation::Dense;
        let _ = build_input_net_from_suite(&c1, bench_ep.as_ref(), 10, 1)
            .expect("(None, Dense) MUST still build");

        // Cell 2: (Some(N), Dense)
        let mut c2 = suite_config_default_tier3();
        c2.chunk_size = Some(20);
        c2.representation = NetRepresentation::Dense;
        let _ = build_input_net_from_suite(&c2, bench_ep.as_ref(), 50, 1)
            .expect("(Some, Dense) MUST still build");

        // Cell 3: (None, Sparse) on dual_tree
        let mut c3 = suite_config_default_tier3();
        c3.chunk_size = None;
        c3.representation = NetRepresentation::Sparse;
        let _ = build_input_net_from_suite(&c3, bench_dt.as_ref(), 4, 1)
            .expect("(None, Sparse) on dual_tree MUST still build");
    }
}
