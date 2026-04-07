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

/// Execute compute mode: encode, reduce, decode (SPEC-14).
///
/// Will be implemented in Phase 11 (Encoding).
pub fn run_compute_command(
    _args: crate::config::ComputeArgs,
) -> Result<(), RelativistError> {
    Err(RelativistError::Config(
        "compute: not yet implemented (Phase 11)".into(),
    ))
}
