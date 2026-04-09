//! Relativist CLI — single binary with 10 subcommands (SPEC-07 R1, SPEC-13 R43, SPEC-15 R19-R20).
//!
//! The main function parses CLI arguments, initializes tracing,
//! and dispatches to the appropriate command entry point.
//! Exit codes follow SPEC-13 R17: 0=success, 1=config, 2=comms, 3=internal.

use clap::Parser;
use relativist::commands;
use relativist::config::{Cli, Command, LogFormat};
use relativist::observability::{
    init_tracing, LogFormat as ObsLogFormat, ObservabilityConfig, ProcessRole,
};

/// Extract the `--log-format` option from whichever subcommand was parsed.
/// Returns `None` if the subcommand does not carry a `log_format` field
/// (e.g., `reduce`, `inspect`, `generate`).
fn extract_log_format(cmd: &Command) -> Option<&LogFormat> {
    match cmd {
        Command::Coordinator(a) => a.log_format.as_ref(),
        Command::Worker(a) => a.log_format.as_ref(),
        Command::Local(a) => a.log_format.as_ref(),
        _ => None,
    }
}

/// Convert CLI `LogFormat` to observability `LogFormat`.
fn to_obs_log_format(fmt: &LogFormat) -> ObsLogFormat {
    match fmt {
        LogFormat::Text => ObsLogFormat::Text,
        LogFormat::Json => ObsLogFormat::Json,
    }
}

/// Determine the `ProcessRole` from the parsed subcommand.
fn extract_role(cmd: &Command) -> ProcessRole {
    match cmd {
        Command::Coordinator(_) => ProcessRole::Coordinator,
        Command::Worker(_) => ProcessRole::Worker,
        _ => ProcessRole::Local,
    }
}

fn main() {
    let cli = Cli::parse();

    // Build observability config from CLI flags (SPEC-07 R35, SPEC-11 R3).
    let obs_config = ObservabilityConfig {
        log_format: extract_log_format(&cli.command)
            .map(to_obs_log_format)
            .unwrap_or(ObsLogFormat::Text),
        role: extract_role(&cli.command),
    };
    init_tracing(&obs_config);

    let result = match cli.command {
        Command::Coordinator(args) => commands::run_coordinator_command(args),
        Command::Worker(args) => commands::run_worker_command(args),
        Command::Local(args) => commands::run_local_command(args),
        Command::Reduce(args) => commands::run_reduce_command(args),
        Command::Inspect(args) => commands::run_inspect_command(args),
        Command::Generate(args) => commands::run_generate_command(args),
        Command::Compute(args) => commands::run_compute_command(args),
        Command::Bench(args) => commands::run_bench_command(args),
        Command::Validate(args) => commands::run_validate_command(args),
        Command::Update(args) => commands::run_update_command(args),
        Command::Completions(args) => commands::run_completions_command(args),
    };

    if let Err(e) = result {
        eprintln!("error: {}", e);
        std::process::exit(e.exit_code());
    }
}
