//! Command entry points for each CLI subcommand.
//!
//! Each function takes the parsed Args struct and returns
//! Result<(), RelativistError>. Wired from main.rs.

use crate::config::{
    build_grid_config, build_grid_config_from_local, build_node_config_coordinator,
    build_node_config_worker, parse_strategy, BenchArgs, CoordinatorArgs, GenerateArgs,
    InspectArgs, LocalArgs, ReduceArgs, WorkerArgs,
};
use crate::error::RelativistError;
use crate::io::{load_net_from_file, print_summary, save_net_to_file, write_metrics};
use crate::merge::run_grid;
use crate::reduction::reduce_all;

/// Execute local mode: grid loop in-process, no TCP (SPEC-07 R18).
pub fn run_local_command(args: LocalArgs) -> Result<(), RelativistError> {
    let grid_config = build_grid_config_from_local(&args);
    let strategy = parse_strategy(&args.strategy)?;

    let net = load_net_from_file(&args.input)?;
    let (reduced_net, metrics) = run_grid(net, &grid_config, &*strategy);

    print_summary(&reduced_net, &metrics);

    if let Some(ref path) = args.output {
        save_net_to_file(&reduced_net, path)?;
    }
    if let Some(ref path) = args.metrics {
        write_metrics(&metrics, path)?;
    }

    Ok(())
}

/// Execute reduce mode: purely local reduction, no partitioning (SPEC-13 R41).
pub fn run_reduce_command(args: ReduceArgs) -> Result<(), RelativistError> {
    let mut net = load_net_from_file(&args.input)?;
    let stats = reduce_all(&mut net);

    println!("=== Relativist Reduce Summary ===");
    println!("Interactions: {}", stats.total_interactions);
    println!("Final agents: {}", net.count_live_agents());

    if let Some(ref path) = args.output {
        save_net_to_file(&net, path)?;
    }

    Ok(())
}

/// Execute inspect mode: print summary statistics (SPEC-13 R47).
pub fn run_inspect_command(args: InspectArgs) -> Result<(), RelativistError> {
    let net = load_net_from_file(&args.input)?;

    println!("=== Relativist Inspect ===");
    println!("Agents:  {}", net.count_live_agents());
    println!(
        "  CON: {}",
        crate::io::count_agents_by_symbol(&net, crate::net::Symbol::Con)
    );
    println!(
        "  DUP: {}",
        crate::io::count_agents_by_symbol(&net, crate::net::Symbol::Dup)
    );
    println!(
        "  ERA: {}",
        crate::io::count_agents_by_symbol(&net, crate::net::Symbol::Era)
    );
    println!("Redexes: {}", net.redex_queue.len());
    println!(
        "Normal Form: {}",
        if net.redex_queue.is_empty() {
            "yes"
        } else {
            "no"
        }
    );

    Ok(())
}

/// Execute generate mode: create a workload network (SPEC-07 R8, SPEC-12 R35-R42a).
pub fn run_generate_command(args: GenerateArgs) -> Result<(), RelativistError> {
    use crate::io::generators;

    let net = generators::generate(args.example, args.size);

    println!("=== Relativist Generate ===");
    println!("Example: {:?}", args.example);
    println!("Size:    {}", args.size);
    println!("Agents:  {}", net.count_live_agents());
    println!("Redexes: {}", net.redex_queue.len());

    save_net_to_file(&net, &args.output)?;
    println!("Saved to: {}", args.output.display());

    Ok(())
}

/// Execute coordinator mode: distributed grid loop (SPEC-07 R13).
///
/// Full implementation depends on the async runtime wiring (TASK-0112).
/// This is a placeholder that validates config and loads the net.
pub fn run_coordinator_command(args: CoordinatorArgs) -> Result<(), RelativistError> {
    let _grid_config = build_grid_config(&args);
    let _node_config = build_node_config_coordinator(&args);
    let _strategy = parse_strategy(&args.strategy)?;
    let _net = load_net_from_file(&args.input)?;

    Err(RelativistError::Config(
        "coordinator: distributed mode not yet wired (needs async runtime)".into(),
    ))
}

/// Execute worker mode: connect to coordinator (SPEC-07 R16).
///
/// Full implementation depends on the async runtime wiring (TASK-0113).
/// This is a placeholder that validates config.
pub fn run_worker_command(args: WorkerArgs) -> Result<(), RelativistError> {
    let _node_config = build_node_config_worker(&args)?;

    Err(RelativistError::Config(
        "worker: distributed mode not yet wired (needs async runtime)".into(),
    ))
}

/// Execute bench mode: run the benchmark suite (SPEC-09 R1, R6).
pub fn run_bench_command(args: BenchArgs) -> Result<(), RelativistError> {
    use crate::bench::csv::{write_csv_detail, write_csv_rounds, write_csv_summary};
    use crate::bench::suite::run_benchmark_suite;
    use crate::bench::{BenchmarkId, BenchmarkSuiteConfig, Mode};

    // Parse mode
    let mode = match args.mode.as_str() {
        "sequential" => Mode::Sequential,
        "local" => Mode::Local,
        "tcp-localhost" | "tcp_localhost" => Mode::TcpLocalhost,
        "tcp-network" | "tcp_network" => Mode::TcpNetwork,
        other => {
            return Err(RelativistError::Config(format!(
                "unknown mode '{}' (supported: sequential, local, tcp-localhost, tcp-network)",
                other
            )))
        }
    };

    // Parse benchmark IDs
    let benchmarks = if let Some(ref names) = args.benchmark {
        let mut ids = Vec::new();
        for name in names {
            let id = match name.as_str() {
                "ep_annihilation" => BenchmarkId::EPAnnihilation,
                "ep_annihilation_con" => BenchmarkId::EPAnnihilationCon,
                "ep_annihilation_dup" => BenchmarkId::EPAnnihilationDup,
                "condup_expansion" => BenchmarkId::ConDupExpansion,
                "dual_tree" => BenchmarkId::DualTree,
                "tree_sum" => BenchmarkId::TreeSum,
                "tree_sum_balanced" => BenchmarkId::TreeSumBalanced,
                "mixed_net" => BenchmarkId::MixedNet,
                "erasure_propagation" => BenchmarkId::ErasurePropagation,
                "church_add" => BenchmarkId::ChurchAdd,
                "church_mul" => BenchmarkId::ChurchMul,
                "all" => {
                    ids = vec![
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
                    ];
                    break;
                }
                other => {
                    return Err(RelativistError::Config(format!(
                        "unknown benchmark '{}'",
                        other
                    )))
                }
            };
            ids.push(id);
        }
        ids
    } else {
        // Default: all benchmarks
        vec![
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
        ]
    };

    let config = BenchmarkSuiteConfig {
        benchmarks,
        sizes: args.sizes,
        workers: args.workers,
        mode,
        warmup_runs: args.warmup,
        repetitions: args.repetitions,
        csv_detail_path: args.csv_detail.as_ref().map(|p| p.display().to_string()),
        csv_rounds_path: args.csv_rounds.as_ref().map(|p| p.display().to_string()),
        csv_summary_path: args.csv_summary.as_ref().map(|p| p.display().to_string()),
        max_rounds: args.max_rounds,
    };

    println!("=== Relativist Benchmark Suite ===");
    println!(
        "Benchmarks:  {}",
        config
            .benchmarks
            .iter()
            .map(|b| b.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!("Mode:        {}", config.mode);
    println!("Workers:     {:?}", config.workers);
    println!("Warmup:      {}", config.warmup_runs);
    println!("Repetitions: {}", config.repetitions);
    println!();

    let suite_result = run_benchmark_suite(&config).map_err(|e| {
        RelativistError::Config(format!("Benchmark suite failed: {}", e))
    })?;

    // Print summary table (R41)
    println!("=== Results ===");
    println!(
        "{:<25} {:>6} {:>7} {:>10} {:>8} {:>8} {:>10}",
        "Benchmark", "Size", "Workers", "Time(s)", "MIPS", "Speedup", "Efficiency"
    );
    println!("{}", "-".repeat(80));
    for s in &suite_result.summaries {
        println!(
            "{:<25} {:>6} {:>7} {:>10.6} {:>8.1} {:>8.4} {:>10.4}",
            s.benchmark,
            s.input_size,
            s.workers,
            s.wall_clock_median,
            s.mips_mean,
            s.speedup_mean,
            s.efficiency_mean,
        );
        if s.cv > 0.10 {
            println!(
                "  WARNING: high variance (CV={:.2}%) for {} size={} workers={}",
                s.cv * 100.0,
                s.benchmark,
                s.input_size,
                s.workers
            );
        }
    }
    println!();
    println!(
        "Total datapoints: {}  |  All correct: {}",
        suite_result.results.len(),
        suite_result.all_correct
    );

    // CSV output (R39-R40)
    if let Some(ref path) = args.csv_detail {
        let mut f = std::fs::File::create(path).map_err(|e| {
            RelativistError::Config(format!("cannot create {}: {}", path.display(), e))
        })?;
        write_csv_detail(&mut f, &suite_result.results).map_err(|e| {
            RelativistError::Config(format!("CSV detail write error: {}", e))
        })?;
        println!("Detail CSV written to: {}", path.display());
    }

    if let Some(ref path) = args.csv_rounds {
        let mut f = std::fs::File::create(path).map_err(|e| {
            RelativistError::Config(format!("cannot create {}: {}", path.display(), e))
        })?;
        write_csv_rounds(&mut f, &suite_result.results).map_err(|e| {
            RelativistError::Config(format!("CSV rounds write error: {}", e))
        })?;
        println!("Rounds CSV written to: {}", path.display());
    }

    if let Some(ref path) = args.csv_summary {
        let mut f = std::fs::File::create(path).map_err(|e| {
            RelativistError::Config(format!("cannot create {}: {}", path.display(), e))
        })?;
        write_csv_summary(&mut f, &suite_result.summaries).map_err(|e| {
            RelativistError::Config(format!("CSV summary write error: {}", e))
        })?;
        println!("Summary CSV written to: {}", path.display());
    }

    if !suite_result.all_correct {
        return Err(RelativistError::Config(
            "One or more benchmarks had correctness failures!".into(),
        ));
    }

    Ok(())
}

/// Execute compute mode: encode arithmetic, reduce, decode result (SPEC-14 R22-R25).
pub fn run_compute_command(args: crate::config::ComputeArgs) -> Result<(), RelativistError> {
    use crate::config::ArithmeticOp;
    use crate::encoding::{build_add, build_exp, build_mul, decode_nat, discover_root};

    let op_name = match args.operation {
        ArithmeticOp::Add => "add",
        ArithmeticOp::Mul => "mul",
        ArithmeticOp::Exp => "exp",
    };

    // Build the arithmetic net
    let mut net = match args.operation {
        ArithmeticOp::Add => build_add(args.a, args.b),
        ArithmeticOp::Mul => build_mul(args.a, args.b),
        ArithmeticOp::Exp => build_exp(args.a, args.b),
    };

    let initial_agents = net.count_live_agents();
    let initial_redexes = net.redex_queue.len();

    println!("=== Relativist Compute ===");
    println!("Expression:  {}({}, {})", op_name, args.a, args.b);
    println!(
        "Encoding:    {} agents, {} redexes",
        initial_agents, initial_redexes
    );

    // Reduce (local or distributed)
    if let Some(workers) = args.workers {
        // Distributed mode via run_grid
        let grid_config = crate::merge::GridConfig {
            num_workers: workers,
            max_rounds: None,
        };
        let strategy = crate::partition::ContiguousIdStrategy;
        let (reduced_net, metrics) = run_grid(net, &grid_config, &strategy);
        net = reduced_net;

        let mips = if metrics.total_time.as_secs_f64() > 0.0 {
            metrics.total_interactions as f64 / metrics.total_time.as_secs_f64() / 1_000_000.0
        } else {
            0.0
        };
        println!(
            "Reduction:   {} interactions in {:.2}s ({:.2} MIPS)",
            metrics.total_interactions,
            metrics.total_time.as_secs_f64(),
            mips
        );
        println!("Workers:     {}", workers);
        println!("Rounds:      {}", metrics.rounds);

        if let Some(ref path) = args.metrics {
            write_metrics(&metrics, path)?;
        }
    } else {
        // Local reduction
        let start = std::time::Instant::now();
        let stats = reduce_all(&mut net);
        let elapsed = start.elapsed();

        let mips = if elapsed.as_secs_f64() > 0.0 {
            stats.total_interactions as f64 / elapsed.as_secs_f64() / 1_000_000.0
        } else {
            0.0
        };
        println!(
            "Reduction:   {} interactions in {:.2}s ({:.2} MIPS)",
            stats.total_interactions,
            elapsed.as_secs_f64(),
            mips
        );
    }

    // Discover root of the resulting Church numeral
    discover_root(&mut net);

    // Decode result: try canonical decode, then shared-chain fallback for mul
    let result = decode_nat(&net)
        .or_else(|| crate::encoding::arithmetic::decode_shared_chain(&net));
    match result {
        Some(n) => println!("Result:      {}", n),
        None => {
            println!("Result:      (non-decodable normal form)");
            println!("  The net is in normal form but uses a non-canonical Church encoding");
            println!("  (e.g., cyclic DUP sharing from exponentiation).");
            println!("  Final agents: {}", net.count_live_agents());
        }
    }

    // Save output if requested
    if let Some(ref path) = args.output {
        save_net_to_file(&net, path)?;
    }

    Ok(())
}
