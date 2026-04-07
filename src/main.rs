//! Relativist CLI — single binary with 7 subcommands (SPEC-07 R1, SPEC-13 R43).
//!
//! The main function parses CLI arguments, initializes tracing,
//! and dispatches to the appropriate command entry point.
//! Exit codes follow SPEC-13 R17: 0=success, 1=config, 2=comms, 3=internal.

use clap::Parser;
use relativist::commands;
use relativist::config::{Cli, Command};
use relativist::observability::init_tracing;

fn main() {
    init_tracing();
    let cli = Cli::parse();

    let result = match cli.command {
        Command::Coordinator(args) => commands::run_coordinator_command(args),
        Command::Worker(args) => commands::run_worker_command(args),
        Command::Local(args) => commands::run_local_command(args),
        Command::Reduce(args) => commands::run_reduce_command(args),
        Command::Inspect(args) => commands::run_inspect_command(args),
        Command::Generate(args) => commands::run_generate_command(args),
        Command::Compute(args) => commands::run_compute_command(args),
    };

    if let Err(e) = result {
        eprintln!("error: {}", e);
        std::process::exit(e.exit_code());
    }
}
