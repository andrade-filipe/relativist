//! Configuration and CLI argument definitions (SPEC-07, SPEC-13).
//!
//! All CLI types live here so they can be reused by the config mapping
//! layer and tested independently from `main.rs`.

use std::net::SocketAddr;
use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

use crate::error::RelativistError;
use crate::merge::GridConfig;
use crate::partition::{ContiguousIdStrategy, PartitionStrategy};
use crate::protocol::NodeConfig;

// ---------------------------------------------------------------------------
// CLI root (SPEC-07 R1-R2, SPEC-13 R43)
// ---------------------------------------------------------------------------

/// Relativist: Distributed Interaction Combinator Reduction Engine.
#[derive(Parser, Debug)]
#[command(name = "relativist", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

/// Subcommands (SPEC-07 R1, SPEC-13 R43).
///
/// 7 subcommands total: the original 4 from SPEC-07 (`coordinator`,
/// `worker`, `local`, `generate`) plus 3 from SPEC-13 (`reduce`,
/// `inspect`, `compute`).
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Run as coordinator: load a network, partition, distribute to workers, merge results.
    Coordinator(CoordinatorArgs),

    /// Run as worker: connect to a coordinator and reduce assigned partitions.
    Worker(WorkerArgs),

    /// Run in-memory grid simulation (SPEC-07 R18, SPEC-05 run_grid).
    /// Executes the full BSP cycle in-process without TCP.
    Local(LocalArgs),

    /// Run purely local reduction (no distribution, no partitioning).
    /// Calls reduce_all directly on the parsed net.
    Reduce(ReduceArgs),

    /// Inspect an IC net file (print summary statistics).
    Inspect(InspectArgs),

    /// Generate a workload network and save to a file.
    Generate(GenerateArgs),

    /// Encode arithmetic, reduce, decode result (SPEC-14).
    Compute(ComputeArgs),

    /// Run the benchmark suite (SPEC-09).
    Bench(BenchArgs),

    /// Validate benchmark campaign CSV outputs (DATA-COLLECTION-PLAN Section 10).
    Validate(ValidateArgs),

    /// Check for and install the latest release (SPEC-15 R19).
    Update(UpdateArgs),

    /// Generate shell completion scripts (SPEC-15 R20).
    Completions(CompletionsArgs),
}

// ---------------------------------------------------------------------------
// Log format (shared across subcommands, SPEC-11 R3)
// ---------------------------------------------------------------------------

/// Log output format.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum LogFormat {
    /// Human-readable text output.
    Text,
    /// Structured JSON output.
    Json,
}

// ---------------------------------------------------------------------------
// Per-subcommand Args structs
// ---------------------------------------------------------------------------

/// Arguments for the `coordinator` subcommand (SPEC-07 R3, SPEC-13 R44).
#[derive(clap::Args, Debug)]
pub struct CoordinatorArgs {
    /// Number of workers to wait for before starting (must be >= 1).
    #[arg(short = 'w', long, value_parser = clap::value_parser!(u32).range(1..))]
    pub workers: u32,

    /// Socket address to bind to.
    /// Default: 127.0.0.1:9000 (SPEC-10 R5: localhost-only for safety).
    #[arg(short = 'b', long, default_value = "127.0.0.1:9000")]
    pub bind: SocketAddr,

    /// Path to the input network file (.bin, bincode-serialized Net).
    #[arg(short = 'i', long)]
    pub input: PathBuf,

    /// Maximum number of grid rounds. Unlimited if not specified.
    #[arg(long)]
    pub max_rounds: Option<u32>,

    /// Path to write the reduced network (.bin).
    #[arg(short = 'o', long)]
    pub output: Option<PathBuf>,

    /// Path to write execution metrics (.json or .csv).
    #[arg(short = 'm', long)]
    pub metrics: Option<PathBuf>,

    /// Partitioning strategy.
    #[arg(long, default_value = "round-robin")]
    pub strategy: String,

    /// Log output format (SPEC-11 R3). Default: auto-detect from TTY.
    #[arg(long)]
    pub log_format: Option<LogFormat>,

    /// Authentication token: "auto" to generate, or base64-encoded value (SPEC-10 R9).
    #[arg(long)]
    pub token: Option<String>,

    /// Path to write the generated token (SPEC-10 R12).
    #[arg(long, default_value = "./relativist-token")]
    pub token_file: std::path::PathBuf,

    /// TLS certificate file (PEM), requires --tls-key (SPEC-10 R25).
    #[cfg(feature = "tls")]
    #[arg(long)]
    pub tls_cert: Option<std::path::PathBuf>,

    /// TLS private key file (PEM), requires --tls-cert (SPEC-10 R25).
    #[cfg(feature = "tls")]
    #[arg(long)]
    pub tls_key: Option<std::path::PathBuf>,
    // --metrics-port (default 9090, feature-gated on `metrics`)
    // will be added in Phase 8 (SPEC-11 R20).
}

/// Arguments for the `worker` subcommand (SPEC-07 R4, SPEC-13 R45).
#[derive(clap::Args, Debug)]
pub struct WorkerArgs {
    /// Address of the coordinator (HOST:PORT).
    #[arg(short = 'c', long)]
    pub coordinator: String,

    /// Log output format (SPEC-11 R3). Default: auto-detect from TTY.
    #[arg(long)]
    pub log_format: Option<LogFormat>,

    /// Authentication token (base64-encoded) for coordinator auth (SPEC-10 R13).
    #[arg(long)]
    pub token: Option<String>,

    /// TLS CA certificate file (PEM) for verifying coordinator (SPEC-10 R26).
    #[cfg(feature = "tls")]
    #[arg(long)]
    pub tls_ca: Option<std::path::PathBuf>,
}

/// Arguments for the `local` subcommand (SPEC-07 R5, SPEC-13 R45a).
#[derive(clap::Args, Debug)]
pub struct LocalArgs {
    /// Number of simulated workers (must be >= 1).
    #[arg(short = 'w', long, value_parser = clap::value_parser!(u32).range(1..))]
    pub workers: u32,

    /// Path to the input network file (.bin).
    #[arg(short = 'i', long)]
    pub input: PathBuf,

    /// Maximum number of grid rounds. Unlimited if not specified.
    #[arg(long)]
    pub max_rounds: Option<u32>,

    /// Path to write the reduced network (.bin).
    #[arg(short = 'o', long)]
    pub output: Option<PathBuf>,

    /// Path to write execution metrics (.json or .csv).
    #[arg(short = 'm', long)]
    pub metrics: Option<PathBuf>,

    /// Partitioning strategy.
    #[arg(long, default_value = "round-robin")]
    pub strategy: String,

    /// Log output format (SPEC-11 R3). Default: auto-detect from TTY.
    #[arg(long)]
    pub log_format: Option<LogFormat>,
}

/// Arguments for the `reduce` subcommand (SPEC-13 R46).
///
/// Purely local reduction: calls reduce_all directly, no partitioning.
#[derive(clap::Args, Debug)]
pub struct ReduceArgs {
    /// Path to the input network file.
    #[arg(short = 'i', long)]
    pub input: PathBuf,

    /// Path to write the reduced network.
    #[arg(short = 'o', long)]
    pub output: Option<PathBuf>,
}

/// Arguments for the `inspect` subcommand (SPEC-13 R47).
#[derive(clap::Args, Debug)]
pub struct InspectArgs {
    /// Path to the IC net file to inspect.
    #[arg(short = 'i', long)]
    pub input: PathBuf,
}

/// Arguments for the `generate` subcommand (SPEC-07 R8, SPEC-12 R33).
///
/// CLI arguments for the `generate` subcommand (SPEC-07 R8, SPEC-12 R35-R42a).
#[derive(clap::Args, Debug)]
pub struct GenerateArgs {
    /// Example network to generate.
    #[arg(value_enum)]
    pub example: crate::io::generators::ExampleNet,

    /// Problem size (number of pairs, depth, or items depending on the benchmark).
    #[arg(short = 'n', long)]
    pub size: u32,

    /// Output path for the generated network (.bin or .ic).
    #[arg(short = 'o', long)]
    pub output: PathBuf,
}

/// Supported arithmetic operations for the compute subcommand (SPEC-14 R22).
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ArithmeticOp {
    /// Addition: a + b
    Add,
    /// Multiplication: a * b
    Mul,
    /// Exponentiation: a ^ b
    Exp,
}

/// Arguments for the `compute` subcommand (SPEC-14 R22-R25).
#[derive(clap::Args, Debug)]
pub struct ComputeArgs {
    /// Arithmetic operation to perform.
    #[arg(value_enum)]
    pub operation: ArithmeticOp,

    /// First operand (natural number).
    pub a: u64,

    /// Second operand (natural number).
    pub b: u64,

    /// Number of workers for distributed reduction (must be >= 1 if specified).
    /// If omitted, reduces locally via reduce_all.
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..))]
    pub workers: Option<u32>,

    /// Path to write the reduced net file.
    #[arg(short = 'o', long)]
    pub output: Option<PathBuf>,

    /// Path to write metrics JSON.
    #[arg(short = 'm', long)]
    pub metrics: Option<PathBuf>,
}

// ---------------------------------------------------------------------------
// Bench subcommand (SPEC-09 R1, R6)
// ---------------------------------------------------------------------------

/// CLI arguments for the `bench` subcommand (SPEC-09 R6).
#[derive(clap::Args, Debug)]
pub struct BenchArgs {
    /// Which benchmark to execute (all if omitted).
    #[arg(long, value_delimiter = ',')]
    pub benchmark: Option<Vec<String>>,

    /// Problem sizes (overrides per-benchmark defaults).
    #[arg(long, value_delimiter = ',')]
    pub sizes: Option<Vec<u32>>,

    /// Worker counts to test.
    #[arg(long, value_delimiter = ',', default_value = "1,2,4,8")]
    pub workers: Vec<u32>,

    /// Execution mode.
    #[arg(long, default_value = "local")]
    pub mode: String,

    /// Warmup runs (discarded).
    #[arg(long, default_value_t = 2)]
    pub warmup: u32,

    /// Timed repetitions.
    #[arg(long, default_value_t = 5)]
    pub repetitions: u32,

    /// Path for detail CSV output.
    #[arg(long)]
    pub csv_detail: Option<PathBuf>,

    /// Path for rounds CSV output.
    #[arg(long)]
    pub csv_rounds: Option<PathBuf>,

    /// Path for summary CSV output.
    #[arg(long)]
    pub csv_summary: Option<PathBuf>,

    /// Grid loop round limit.
    #[arg(long)]
    pub max_rounds: Option<u32>,
}

/// CLI arguments for the `validate` subcommand (DATA-COLLECTION-PLAN Section 10).
#[derive(clap::Args, Debug)]
pub struct ValidateArgs {
    /// Path to the detail CSV file.
    #[arg(long, default_value = "results/detail.csv")]
    pub detail: PathBuf,

    /// Path to the summary CSV file.
    #[arg(long, default_value = "results/summary.csv")]
    pub summary: PathBuf,

    /// Path to the rounds CSV file.
    #[arg(long, default_value = "results/rounds.csv")]
    pub rounds: PathBuf,
}

/// Arguments for the `update` subcommand (SPEC-15 R19).
#[derive(clap::Args, Debug)]
pub struct UpdateArgs {
    /// Only check for a new version without installing.
    #[arg(long)]
    pub check: bool,
}

/// Shell type for completion generation (SPEC-15 R20).
#[derive(Debug, Clone, ValueEnum)]
pub enum ShellType {
    Bash,
    Zsh,
    Fish,
    #[value(name = "powershell")]
    PowerShell,
}

/// Arguments for the `completions` subcommand (SPEC-15 R20).
#[derive(clap::Args, Debug)]
pub struct CompletionsArgs {
    /// Shell to generate completions for.
    pub shell: ShellType,
}

// ---------------------------------------------------------------------------
// CLI-to-config mapping (TASK-0102, SPEC-07 R10-R12)
// ---------------------------------------------------------------------------

/// Build GridConfig from coordinator CLI arguments.
pub fn build_grid_config(args: &CoordinatorArgs) -> GridConfig {
    GridConfig {
        num_workers: args.workers,
        max_rounds: args.max_rounds,
    }
}

/// Build GridConfig from local mode CLI arguments.
pub fn build_grid_config_from_local(args: &LocalArgs) -> GridConfig {
    GridConfig {
        num_workers: args.workers,
        max_rounds: args.max_rounds,
    }
}

/// Build NodeConfig for coordinator mode (SPEC-07 R11-R12).
pub fn build_node_config_coordinator(args: &CoordinatorArgs) -> NodeConfig {
    NodeConfig {
        bind: args.bind,
        num_workers: args.workers,
        ..NodeConfig::default()
    }
}

/// Build NodeConfig for worker mode, parsing the coordinator address.
///
/// Accepts both IP:PORT (e.g. "192.168.1.100:9000") and HOST:PORT
/// (e.g. "coordinator:9000") formats per SPEC-07 R4.
pub fn build_node_config_worker(args: &WorkerArgs) -> Result<NodeConfig, RelativistError> {
    // Try direct SocketAddr parse first (IP:PORT), then fall back to DNS resolution.
    let addr: SocketAddr = args
        .coordinator
        .parse()
        .or_else(|_| {
            use std::net::ToSocketAddrs;
            args.coordinator
                .to_socket_addrs()
                .map_err(|e| e.to_string())
                .and_then(|mut addrs| addrs.next().ok_or_else(|| "no addresses found".into()))
        })
        .map_err(|e| {
            RelativistError::Config(format!(
                "invalid coordinator address '{}': {}",
                args.coordinator, e
            ))
        })?;
    Ok(NodeConfig {
        bind: addr,
        num_workers: 0,
        ..NodeConfig::default()
    })
}

/// Map strategy name to a PartitionStrategy implementation (SPEC-07 R12).
///
/// Only "round-robin" is supported in v1 (maps to ContiguousIdStrategy).
pub fn parse_strategy(name: &str) -> Result<Box<dyn PartitionStrategy>, RelativistError> {
    match name {
        "round-robin" => Ok(Box::new(ContiguousIdStrategy)),
        other => Err(RelativistError::Config(format!(
            "unknown partitioning strategy '{}' (supported: round-robin)",
            other
        ))),
    }
}

// ---------------------------------------------------------------------------
// Tests (TASK-0100, TASK-0102)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_coordinator_minimal() {
        let cli = Cli::try_parse_from([
            "relativist",
            "coordinator",
            "--workers",
            "4",
            "--input",
            "input.bin",
        ])
        .unwrap();
        match cli.command {
            Command::Coordinator(args) => {
                assert_eq!(args.workers, 4);
                assert_eq!(args.bind, "127.0.0.1:9000".parse::<SocketAddr>().unwrap());
                assert_eq!(args.input, PathBuf::from("input.bin"));
                assert_eq!(args.max_rounds, None);
                assert_eq!(args.strategy, "round-robin");
            }
            _ => panic!("expected Coordinator"),
        }
    }

    #[test]
    fn test_parse_coordinator_explicit_bind() {
        let cli = Cli::try_parse_from([
            "relativist",
            "coordinator",
            "-w",
            "4",
            "-b",
            "0.0.0.0:8080",
            "-i",
            "input.bin",
        ])
        .unwrap();
        match cli.command {
            Command::Coordinator(args) => {
                assert_eq!(args.bind, "0.0.0.0:8080".parse::<SocketAddr>().unwrap());
            }
            _ => panic!("expected Coordinator"),
        }
    }

    #[test]
    fn test_parse_worker() {
        let cli = Cli::try_parse_from(["relativist", "worker", "--coordinator", "127.0.0.1:9000"])
            .unwrap();
        match cli.command {
            Command::Worker(args) => {
                assert_eq!(args.coordinator, "127.0.0.1:9000");
            }
            _ => panic!("expected Worker"),
        }
    }

    #[test]
    fn test_parse_worker_short() {
        let cli = Cli::try_parse_from(["relativist", "worker", "-c", "192.168.1.1:9000"]).unwrap();
        match cli.command {
            Command::Worker(args) => {
                assert_eq!(args.coordinator, "192.168.1.1:9000");
            }
            _ => panic!("expected Worker"),
        }
    }

    #[test]
    fn test_parse_local_defaults() {
        let cli = Cli::try_parse_from([
            "relativist",
            "local",
            "--workers",
            "2",
            "--input",
            "test.bin",
        ])
        .unwrap();
        match cli.command {
            Command::Local(args) => {
                assert_eq!(args.workers, 2);
                assert_eq!(args.input, PathBuf::from("test.bin"));
                assert_eq!(args.strategy, "round-robin");
                assert!(args.max_rounds.is_none());
            }
            _ => panic!("expected Local"),
        }
    }

    #[test]
    fn test_parse_reduce() {
        let cli = Cli::try_parse_from(["relativist", "reduce", "-i", "net.bin"]).unwrap();
        match cli.command {
            Command::Reduce(args) => {
                assert_eq!(args.input, PathBuf::from("net.bin"));
                assert!(args.output.is_none());
            }
            _ => panic!("expected Reduce"),
        }
    }

    #[test]
    fn test_parse_inspect() {
        let cli = Cli::try_parse_from(["relativist", "inspect", "-i", "net.bin"]).unwrap();
        match cli.command {
            Command::Inspect(args) => {
                assert_eq!(args.input, PathBuf::from("net.bin"));
            }
            _ => panic!("expected Inspect"),
        }
    }

    #[test]
    fn test_parse_generate() {
        let cli = Cli::try_parse_from([
            "relativist",
            "generate",
            "dual-tree",
            "-n",
            "1000",
            "-o",
            "out.bin",
        ])
        .unwrap();
        match cli.command {
            Command::Generate(args) => {
                assert_eq!(args.example, crate::io::generators::ExampleNet::DualTree);
                assert_eq!(args.size, 1000);
                assert_eq!(args.output, PathBuf::from("out.bin"));
            }
            _ => panic!("expected Generate"),
        }
    }

    #[test]
    fn test_parse_compute() {
        let cli = Cli::try_parse_from(["relativist", "compute", "add", "3", "5"]).unwrap();
        match cli.command {
            Command::Compute(args) => {
                assert!(matches!(args.operation, ArithmeticOp::Add));
                assert_eq!(args.a, 3);
                assert_eq!(args.b, 5);
            }
            _ => panic!("expected Compute"),
        }
    }

    #[test]
    fn test_no_subcommand_fails() {
        let result = Cli::try_parse_from(["relativist"]);
        assert!(result.is_err());
    }

    // === Test helpers ===

    fn make_coordinator_args(workers: u32, max_rounds: Option<u32>) -> CoordinatorArgs {
        CoordinatorArgs {
            workers,
            bind: "127.0.0.1:9000".parse().unwrap(),
            input: PathBuf::from("test.bin"),
            max_rounds,
            output: None,
            metrics: None,
            strategy: "round-robin".to_string(),
            log_format: None,
            token: None,
            token_file: std::path::PathBuf::from("./relativist-token"),
            #[cfg(feature = "tls")]
            tls_cert: None,
            #[cfg(feature = "tls")]
            tls_key: None,
        }
    }

    fn make_worker_args(coordinator: &str) -> WorkerArgs {
        WorkerArgs {
            coordinator: coordinator.to_string(),
            log_format: None,
            token: None,
            #[cfg(feature = "tls")]
            tls_ca: None,
        }
    }

    // === TASK-0102: config mapping tests ===

    #[test]
    fn test_build_grid_config() {
        let args = make_coordinator_args(4, Some(10));
        let config = build_grid_config(&args);
        assert_eq!(config.num_workers, 4);
        assert_eq!(config.max_rounds, Some(10));
    }

    #[test]
    fn test_build_node_config_coordinator() {
        let args = make_coordinator_args(8, None);
        let config = build_node_config_coordinator(&args);
        assert_eq!(config.bind, "127.0.0.1:9000".parse::<SocketAddr>().unwrap());
        assert_eq!(config.num_workers, 8);
        assert_eq!(
            config.worker_connect_timeout,
            std::time::Duration::from_secs(120)
        );
        assert_eq!(config.collect_timeout, std::time::Duration::from_secs(600));
    }

    #[test]
    fn test_build_node_config_worker_valid() {
        let args = make_worker_args("127.0.0.1:9000");
        let config = build_node_config_worker(&args).unwrap();
        assert_eq!(config.bind, "127.0.0.1:9000".parse::<SocketAddr>().unwrap());
    }

    #[test]
    fn test_build_node_config_worker_invalid() {
        let args = make_worker_args("not-an-address");
        let result = build_node_config_worker(&args);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_strategy_round_robin() {
        let strategy = parse_strategy("round-robin");
        assert!(strategy.is_ok());
    }

    #[test]
    fn test_parse_strategy_unknown() {
        let strategy = parse_strategy("unknown-strategy");
        assert!(strategy.is_err());
    }
}
