//! Command entry points for each CLI subcommand.
//!
//! Each function takes the parsed Args struct and returns
//! Result<(), RelativistError>. Wired from main.rs.

use crate::config::{
    build_grid_config, build_grid_config_from_local, build_node_config_coordinator,
    build_node_config_worker, parse_strategy, CoordinatorArgs, GenerateArgs, InspectArgs,
    LocalArgs, ReduceArgs, WorkerArgs,
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

/// Execute generate mode: create a workload network (SPEC-07 R8).
///
/// Generator implementations will be added in Phase 9 (TASK-0171+).
/// For now, this is a stub that returns an error.
pub fn run_generate_command(_args: GenerateArgs) -> Result<(), RelativistError> {
    Err(RelativistError::Config(
        "generate: no generators implemented yet (Phase 9)".into(),
    ))
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

    // Decode result
    let result = decode_nat(&net);
    match result {
        Some(n) => println!("Result:      {}", n),
        None => {
            println!("Warning: result is not a recognizable Church numeral. The net may not have reached Normal Form or the encoding may be incorrect.");
            println!("Final agents: {}", net.count_live_agents());
            println!("Redexes:     {}", net.redex_queue.len());
        }
    }

    // Save output if requested
    if let Some(ref path) = args.output {
        save_net_to_file(&net, path)?;
    }

    Ok(())
}
