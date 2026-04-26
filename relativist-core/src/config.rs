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

    /// List or inspect registered encoders (SPEC-27 R22).
    Encoders(EncodersArgs),
}

/// Arguments for the `encoders` subcommand (SPEC-27 R22).
#[derive(clap::Args, Debug)]
pub struct EncodersArgs {
    #[command(subcommand)]
    pub action: EncodersAction,
}

/// Actions under `encoders` (SPEC-27 R22).
#[derive(clap::Subcommand, Debug)]
pub enum EncodersAction {
    /// List all registered encoders with their descriptions.
    List,
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
#[derive(clap::Args, Debug, Default, Clone)]
pub struct CoordinatorArgs {
    /// Number of workers to wait for before starting (must be >= 1).
    #[arg(short = 'w', long, value_parser = clap::value_parser!(u32).range(1..))]
    pub workers: u32,

    /// Socket address to bind to.
    /// Accepts IP:PORT, HOST:PORT, or "tailscale[:PORT]" to auto-resolve
    /// the Tailscale IPv4 address. Default: 127.0.0.1:9000 (SPEC-10 R5).
    #[arg(short = 'b', long, default_value = "127.0.0.1:9000")]
    pub bind: String,

    /// Path to the input network file (.bin, bincode-serialized Net).
    #[arg(short = 'i', long)]
    pub input: PathBuf,

    /// Maximum number of grid rounds. Unlimited if not specified.
    #[arg(long)]
    pub max_rounds: Option<u32>,

    /// Run the grid loop in strict BSP mode (SPEC-05 R30a).
    ///
    /// When enabled, border cascades are not reduced at the coordinator;
    /// the grid loop iterates until Normal Form, forcing rounds > 1 for
    /// nets with cross-partition cascades. Default: false (lenient).
    #[arg(long, default_value_t = false)]
    pub strict_bsp: bool,

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

    /// Transport backend: "tcp" or "unix" (SPEC-17 R29).
    #[arg(long, default_value = "tcp")]
    pub transport: String,

    /// Unix domain socket path (only with --transport=unix) (SPEC-17 R30).
    #[arg(long)]
    pub socket_path: Option<PathBuf>,

    /// Disable TCP_NODELAY (enables Nagle's algorithm). TCP_NODELAY is on by default (SPEC-17 R30).
    #[arg(long)]
    pub no_tcp_nodelay: bool,

    /// TCP send buffer size in bytes (SO_SNDBUF) (SPEC-17 R30).
    #[arg(long, default_value_t = 4_194_304)]
    pub send_buffer: usize,

    /// TCP receive buffer size in bytes (SO_RCVBUF) (SPEC-17 R30).
    #[arg(long, default_value_t = 4_194_304)]
    pub recv_buffer: usize,

    /// TCP keepalive idle time in seconds; 0 to disable (SPEC-17 R30).
    #[arg(long, default_value_t = 30)]
    pub keepalive: u64,

    /// LZ4 frame compression threshold in bytes (SPEC-18 R37, item 2.23 §3.3).
    /// Frames whose uncompressed payload is shorter than this value are sent
    /// raw. Use `0` to compress every frame; the maximum `usize` value (passed
    /// as a very large literal) effectively disables compression.
    #[arg(long, default_value_t = 1024)]
    pub compression_threshold: usize,

    /// Enable rkyv zero-copy archive on hot-path messages
    /// (SPEC-18 §3.5 R36/R37, item 2.24). Defaults to `false`. Effective
    /// only when the binary is built with `--features zero-copy`; in
    /// default builds the flag is accepted (so configs are bit-identical
    /// across feature builds) but no FLAG_ARCHIVED is emitted.
    #[arg(long, default_value_t = false)]
    pub use_zero_copy: bool,

    /// Enable the delta-only BSP protocol (stateful workers).
    /// SPEC-19 §3.6 R41. Defaults to `false` (R42 — v1 backwards compatibility).
    /// When `true`, the grid loop dispatches `run_grid_delta` (bundle 2.26-C)
    /// instead of the v1 `run_grid` full-partition loop.
    #[arg(long, default_value_t = false)]
    pub delta_mode: bool,

    /// Enable hybrid coordinator mode (SPEC-20 R34).
    /// When enabled, the coordinator acts as a worker (WorkerId 0).
    #[arg(long, action = clap::ArgAction::SetTrue)]
    pub hybrid: bool,

    /// Enable dynamic worker departure recovery (SPEC-20 R34).
    #[arg(long, action = clap::ArgAction::SetTrue)]
    pub elastic_departure: bool,

    /// Explicitly enable retaining partitions on departure (SPEC-20 R34).
    /// Auto-enabled when --elastic-departure is set.
    #[arg(long, action = clap::ArgAction::SetTrue)]
    pub retain_partitions: bool,

    /// Enable dynamic worker joining (SPEC-20 R34).
    /// Auto-enabled when --hybrid or --elastic-departure is set.
    #[arg(long, action = clap::ArgAction::SetTrue)]
    pub elastic_join: bool,

    /// Enable partition checkpointing (SPEC-20 R34).
    #[arg(long, action = clap::ArgAction::SetTrue)]
    pub checkpoint_partitions: bool,

    /// Initial wait timeout for worker connections in seconds (SPEC-20 R34).
    #[arg(long, default_value_t = 30)]
    pub initial_wait_timeout: u64,

    /// Minimum join window duration in milliseconds (SPEC-20 R34).
    #[arg(long, default_value_t = 50)]
    pub join_window_min_ms: u64,

    /// Maximum join window duration in milliseconds (SPEC-20 R34).
    #[arg(long, default_value_t = 500)]
    pub join_window_max_ms: u64,

    /// Maximum interactions per solo-reducing batch (SPEC-20 R34).
    #[arg(long, default_value_t = 10_000)]
    pub solo_budget: u32,

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

    /// Run in daemon mode: reconnect to coordinator after each job (SPEC-16 R1).
    #[arg(long, default_value_t = false)]
    pub daemon: bool,

    /// Transport backend: "tcp" or "unix" (SPEC-17 R29).
    #[arg(long, default_value = "tcp")]
    pub transport: String,

    /// Unix domain socket path (only with --transport=unix) (SPEC-17 R30).
    #[arg(long)]
    pub socket_path: Option<PathBuf>,

    /// Disable TCP_NODELAY (enables Nagle's algorithm). TCP_NODELAY is on by default (SPEC-17 R30).
    #[arg(long)]
    pub no_tcp_nodelay: bool,

    /// TCP send buffer size in bytes (SO_SNDBUF) (SPEC-17 R30).
    #[arg(long, default_value_t = 4_194_304)]
    pub send_buffer: usize,

    /// TCP receive buffer size in bytes (SO_RCVBUF) (SPEC-17 R30).
    #[arg(long, default_value_t = 4_194_304)]
    pub recv_buffer: usize,

    /// TCP keepalive idle time in seconds; 0 to disable (SPEC-17 R30).
    #[arg(long, default_value_t = 30)]
    pub keepalive: u64,

    /// LZ4 frame compression threshold in bytes (SPEC-18 R37, item 2.23 §3.3).
    /// Frames whose uncompressed payload is shorter than this value are sent
    /// raw. Use `0` to compress every frame.
    #[arg(long, default_value_t = 1024)]
    pub compression_threshold: usize,

    /// Enable rkyv zero-copy archive on hot-path messages
    /// (SPEC-18 §3.5 R36/R37, item 2.24). Defaults to `false`.
    #[arg(long, default_value_t = false)]
    pub use_zero_copy: bool,

    /// TLS CA certificate file (PEM) for verifying coordinator (SPEC-10 R26).
    #[cfg(feature = "tls")]
    #[arg(long)]
    pub tls_ca: Option<std::path::PathBuf>,
}

/// Arguments for the `local` subcommand (SPEC-07 R5, SPEC-13 R45a).
#[derive(clap::Args, Debug, Default, Clone)]
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

    /// Run the grid loop in strict BSP mode (SPEC-05 R30a).
    ///
    /// When enabled, border cascades are not reduced at the coordinator;
    /// the grid loop iterates until Normal Form, forcing rounds > 1 for
    /// nets with cross-partition cascades. Default: false (lenient).
    #[arg(long, default_value_t = false)]
    pub strict_bsp: bool,

    /// Enable the delta-only BSP protocol (stateful workers).
    /// SPEC-19 §3.6 R41. Defaults to `false` (R42). See `CoordinatorArgs::delta_mode`.
    #[arg(long, default_value_t = false)]
    pub delta_mode: bool,

    /// Enable hybrid coordinator mode (SPEC-20 R34).
    #[arg(long, action = clap::ArgAction::SetTrue)]
    pub hybrid: bool,

    /// Enable dynamic worker departure recovery (SPEC-20 R34).
    #[arg(long, action = clap::ArgAction::SetTrue)]
    pub elastic_departure: bool,

    /// Enable dynamic worker joining (SPEC-20 R34).
    #[arg(long, action = clap::ArgAction::SetTrue)]
    pub elastic_join: bool,

    /// Initial wait timeout for worker connections in seconds (SPEC-20 R34).
    #[arg(long, default_value_t = 30)]
    pub initial_wait_timeout: u64,

    /// Maximum interactions per solo-reducing batch (SPEC-20 R34).
    #[arg(long, default_value_t = 10_000)]
    pub solo_budget: u32,

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

/// Arguments for the `compute` subcommand (SPEC-14 R22-R25, SPEC-27 R21).
///
/// Two mutually-exclusive invocation modes:
/// - **Legacy:** positional `<op> <a> <b>` (Church arithmetic, SPEC-14).
/// - **Registry:** `--encoder <name> --input <json>` (SPEC-27 R21).
///
/// Exactly one mode must be used; this is enforced at runtime in
/// `run_compute_command`.
#[derive(clap::Args, Debug)]
pub struct ComputeArgs {
    /// Arithmetic operation (legacy SPEC-14 path). Required when --encoder is omitted.
    #[arg(value_enum)]
    pub operation: Option<ArithmeticOp>,

    /// First operand (legacy SPEC-14 path).
    pub a: Option<u64>,

    /// Second operand (legacy SPEC-14 path).
    pub b: Option<u64>,

    /// Encoder name from the registry (e.g., "lambda", "church_add"). SPEC-27 R21.
    #[arg(long)]
    pub encoder: Option<String>,

    /// Encoder input as a JSON string. Required when --encoder is set. SPEC-27 R21.
    #[arg(long, requires = "encoder")]
    pub input: Option<String>,

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

    /// Run the grid loop in strict BSP mode (SPEC-05 R30a).
    ///
    /// When enabled, border cascades are not reduced at the coordinator;
    /// the grid loop iterates until Normal Form, forcing rounds > 1 for
    /// nets with cross-partition cascades. Default: false (lenient).
    #[arg(long, default_value_t = false)]
    pub strict_bsp: bool,

    /// Skip full graph isomorphism (G1) in favor of a fast symbol-count check.
    ///
    /// When the distributed result has > ~5000 non-empty agents, the O(N!)
    /// backtracking in `nets_isomorphic` becomes intractable (see
    /// `PHASE1-FINDINGS.md` L3). This flag switches every benchmark's
    /// `verify()` to `nets_match_counts`, a necessary-but-not-sufficient
    /// check. Results are marked "G1 weak" in the CSV.
    #[arg(long, default_value_t = false)]
    pub skip_g1: bool,
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
///
/// Applies SPEC-19 R43 normalization via [`GridConfig::normalize`]: if
/// `--delta-mode` is set, `coordinator_free_rounds` is auto-enabled
/// (TASK-0397, DC-0397-B). Programmatic constructors that do not route
/// through this function are responsible for calling `.normalize()` if
/// they want R43 enforcement.
pub fn build_grid_config(args: &CoordinatorArgs) -> GridConfig {
    GridConfig {
        num_workers: args.workers,
        max_rounds: args.max_rounds,
        strict_bsp: args.strict_bsp,
        delta_mode: args.delta_mode,
        hybrid_coordinator: args.hybrid,
        elastic_departure: args.elastic_departure,
        retain_partitions: args.retain_partitions,
        elastic_join: args.elastic_join,
        checkpoint_partitions: args.checkpoint_partitions,
        coordinator_free_rounds: false, // Normalizer will handle auto-enabling if delta_mode is true
        initial_wait_timeout: std::time::Duration::from_secs(args.initial_wait_timeout),
        join_window_min: std::time::Duration::from_millis(args.join_window_min_ms),
        join_window_max: std::time::Duration::from_millis(args.join_window_max_ms),
        solo_budget: args.solo_budget,
    }
    .normalize()
}

/// Build GridConfig from local mode CLI arguments.
///
/// See [`build_grid_config`] for SPEC-19 R43 normalization semantics.
pub fn build_grid_config_from_local(args: &LocalArgs) -> GridConfig {
    GridConfig {
        num_workers: args.workers,
        max_rounds: args.max_rounds,
        strict_bsp: args.strict_bsp,
        delta_mode: args.delta_mode,
        hybrid_coordinator: args.hybrid,
        elastic_departure: args.elastic_departure,
        elastic_join: args.elastic_join,
        initial_wait_timeout: std::time::Duration::from_secs(args.initial_wait_timeout),
        solo_budget: args.solo_budget,
        ..GridConfig::default()
    }
    .normalize()
}

/// Build TransportConfig from CLI transport flags (SPEC-17 R29-R32 + SPEC-18 R37 + SPEC-18 §3.5 R36/R37).
#[allow(clippy::too_many_arguments)]
fn build_transport_config(
    backend_str: &str,
    socket_path: Option<PathBuf>,
    no_tcp_nodelay: bool,
    send_buffer: usize,
    recv_buffer: usize,
    keepalive: u64,
    compression_threshold: usize,
    use_zero_copy: bool,
) -> Result<crate::protocol::config::TransportConfig, RelativistError> {
    use crate::protocol::config::{TransportBackend, TransportConfig};

    let backend = match backend_str {
        "tcp" => TransportBackend::Tcp,
        "unix" => {
            #[cfg(not(unix))]
            return Err(RelativistError::Config(
                "--transport=unix is not supported on this platform (SPEC-17 R31)".to_string(),
            ));
            #[cfg(unix)]
            TransportBackend::Unix
        }
        other => {
            return Err(RelativistError::Config(format!(
                "unknown transport backend '{}' (supported: tcp, unix)",
                other
            )));
        }
    };

    // R32: --socket-path without --transport=unix → warning
    if socket_path.is_some() && backend != TransportBackend::Unix {
        tracing::warn!("--socket-path is ignored when --transport is not 'unix' (SPEC-17 R32)");
    }

    let keepalive_idle = if keepalive == 0 {
        None
    } else {
        Some(std::time::Duration::from_secs(keepalive))
    };

    Ok(TransportConfig {
        backend,
        tcp_nodelay: !no_tcp_nodelay,
        send_buffer_bytes: Some(send_buffer),
        recv_buffer_bytes: Some(recv_buffer),
        keepalive_idle,
        unix_socket_path: socket_path,
        compression_threshold,
        use_zero_copy,
        ..TransportConfig::default()
    })
}

/// Build NodeConfig for coordinator mode (SPEC-07 R11-R12).
///
/// Resolves the bind address, supporting "tailscale[:PORT]" shorthand.
pub fn build_node_config_coordinator(
    args: &CoordinatorArgs,
) -> Result<NodeConfig, RelativistError> {
    let bind = resolve_bind_address(&args.bind)?;
    let transport = build_transport_config(
        &args.transport,
        args.socket_path.clone(),
        args.no_tcp_nodelay,
        args.send_buffer,
        args.recv_buffer,
        args.keepalive,
        args.compression_threshold,
        args.use_zero_copy,
    )?;
    Ok(NodeConfig {
        bind,
        num_workers: args.workers,
        transport,
        ..NodeConfig::default()
    })
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
    let transport = build_transport_config(
        &args.transport,
        args.socket_path.clone(),
        args.no_tcp_nodelay,
        args.send_buffer,
        args.recv_buffer,
        args.keepalive,
        args.compression_threshold,
        args.use_zero_copy,
    )?;
    Ok(NodeConfig {
        bind: addr,
        num_workers: 0,
        transport,
        ..NodeConfig::default()
    })
}

/// Resolve the bind address, supporting "tailscale[:PORT]" shorthand.
///
/// When the host part is "tailscale", queries the Tailscale daemon for
/// the machine's IPv4 address via `tailscale ip -4`.
pub fn resolve_bind_address(bind: &str) -> Result<SocketAddr, RelativistError> {
    // Check for "tailscale:PORT" prefix
    if let Some(port_str) = bind.strip_prefix("tailscale:") {
        let port: u16 = port_str
            .parse()
            .map_err(|e| RelativistError::Config(format!("invalid port in '{}': {}", bind, e)))?;
        let ip = query_tailscale_ip()?;
        return Ok(SocketAddr::new(ip, port));
    }
    // Check for bare "tailscale" (default port 9000)
    if bind == "tailscale" {
        let ip = query_tailscale_ip()?;
        return Ok(SocketAddr::new(ip, 9000));
    }

    // Standard: try SocketAddr parse, then DNS resolution
    bind.parse()
        .or_else(|_| {
            use std::net::ToSocketAddrs;
            bind.to_socket_addrs()
                .map_err(|e| e.to_string())
                .and_then(|mut a| a.next().ok_or_else(|| "no addresses found".into()))
        })
        .map_err(|e| RelativistError::Config(format!("invalid bind address '{}': {}", bind, e)))
}

/// Query the Tailscale daemon for this machine's IPv4 address.
fn query_tailscale_ip() -> Result<std::net::IpAddr, RelativistError> {
    let output = std::process::Command::new("tailscale")
        .args(["ip", "-4"])
        .output()
        .map_err(|e| {
            RelativistError::Config(format!(
                "failed to run 'tailscale ip -4': {}. Is Tailscale installed?",
                e
            ))
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(RelativistError::Config(format!(
            "'tailscale ip -4' failed: {}. Is Tailscale running?",
            stderr.trim()
        )));
    }
    let ip_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    ip_str.parse().map_err(|e| {
        RelativistError::Config(format!("invalid IP from Tailscale '{}': {}", ip_str, e))
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
                assert_eq!(args.bind, "127.0.0.1:9000");
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
                assert_eq!(args.bind, "0.0.0.0:8080");
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
    fn test_parse_worker_daemon() {
        let cli = Cli::try_parse_from(["relativist", "worker", "-c", "127.0.0.1:9000", "--daemon"])
            .unwrap();
        match cli.command {
            Command::Worker(args) => {
                assert!(args.daemon);
            }
            _ => panic!("expected Worker"),
        }
    }

    #[test]
    fn test_parse_worker_no_daemon() {
        let cli = Cli::try_parse_from(["relativist", "worker", "-c", "127.0.0.1:9000"]).unwrap();
        match cli.command {
            Command::Worker(args) => {
                assert!(!args.daemon);
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
                assert!(matches!(args.operation, Some(ArithmeticOp::Add)));
                assert_eq!(args.a, Some(3));
                assert_eq!(args.b, Some(5));
                assert!(args.encoder.is_none());
                assert!(args.input.is_none());
            }
            _ => panic!("expected Compute"),
        }
    }

    // SPEC-27 R21: --encoder + --input parses without positional args.
    #[test]
    fn test_parse_compute_encoder_flag() {
        let cli = Cli::try_parse_from([
            "relativist",
            "compute",
            "--encoder",
            "lambda",
            "--input",
            r#"{"term":"λx. x"}"#,
        ])
        .unwrap();
        match cli.command {
            Command::Compute(args) => {
                assert_eq!(args.encoder.as_deref(), Some("lambda"));
                assert!(args.input.is_some());
                assert!(args.operation.is_none());
                assert!(args.a.is_none());
                assert!(args.b.is_none());
            }
            _ => panic!("expected Compute"),
        }
    }

    // SPEC-27 R21: --input without --encoder rejected by clap (requires).
    #[test]
    fn test_parse_compute_input_without_encoder_rejected() {
        let res = Cli::try_parse_from(["relativist", "compute", "--input", "{}"]);
        assert!(res.is_err());
    }

    // SPEC-27 R22: encoders list parses.
    #[test]
    fn test_parse_encoders_list() {
        let cli = Cli::try_parse_from(["relativist", "encoders", "list"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Encoders(EncodersArgs {
                action: EncodersAction::List
            })
        ));
    }

    // SPEC-27 R22: encoders without action fails (clap requires subcommand).
    #[test]
    fn test_parse_encoders_no_action_fails() {
        let res = Cli::try_parse_from(["relativist", "encoders"]);
        assert!(res.is_err());
    }

    // QA: --encoder without --input parses (clap doesn't enforce the reverse
    // direction); runtime check in run_compute_command must catch this.
    #[test]
    fn test_parse_compute_encoder_without_input_parses() {
        let cli = Cli::try_parse_from(["relativist", "compute", "--encoder", "lambda"]).unwrap();
        match cli.command {
            Command::Compute(args) => {
                assert_eq!(args.encoder.as_deref(), Some("lambda"));
                assert!(args.input.is_none());
            }
            _ => panic!("expected Compute"),
        }
    }

    // QA: empty positional/flags is rejected at parse time (no defaults).
    #[test]
    fn test_parse_compute_no_args_parses_but_runtime_rejects() {
        // Note: clap accepts zero args because all are Optional; runtime
        // dispatch (run_compute_command) is responsible for rejecting.
        let cli = Cli::try_parse_from(["relativist", "compute"]).unwrap();
        match cli.command {
            Command::Compute(args) => {
                assert!(args.operation.is_none());
                assert!(args.encoder.is_none());
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
            bind: "127.0.0.1:9000".to_string(),
            input: PathBuf::from("test.bin"),
            max_rounds,
            strict_bsp: false,
            output: None,
            metrics: None,
            strategy: "round-robin".to_string(),
            log_format: None,
            token: None,
            token_file: std::path::PathBuf::from("./relativist-token"),
            transport: "tcp".to_string(),
            socket_path: None,
            no_tcp_nodelay: false,
            send_buffer: 4_194_304,
            recv_buffer: 4_194_304,
            keepalive: 30,
            compression_threshold: 1024,
            use_zero_copy: false,
            delta_mode: false,
            #[cfg(feature = "tls")]
            tls_cert: None,
            #[cfg(feature = "tls")]
            tls_key: None,
            ..Default::default()
        }
    }

    fn make_worker_args(coordinator: &str) -> WorkerArgs {
        WorkerArgs {
            coordinator: coordinator.to_string(),
            log_format: None,
            token: None,
            daemon: false,
            transport: "tcp".to_string(),
            socket_path: None,
            no_tcp_nodelay: false,
            send_buffer: 4_194_304,
            recv_buffer: 4_194_304,
            keepalive: 30,
            compression_threshold: 1024,
            use_zero_copy: false,
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

    // TASK-0390 UT-01: SPEC-19 R42 — absence of `--delta-mode` MUST leave
    // `delta_mode = false` on both the `CoordinatorArgs`/`LocalArgs` struct
    // and the derived `GridConfig`. A silent flip here would route every
    // default run through the delta path.
    #[test]
    fn cli_delta_mode_default_is_false_on_coordinator_and_local() {
        let coord_args = make_coordinator_args(4, None);
        assert!(
            !coord_args.delta_mode,
            "make_coordinator_args must default delta_mode = false"
        );
        let coord_cfg = build_grid_config(&coord_args);
        assert!(
            !coord_cfg.delta_mode,
            "build_grid_config must thread delta_mode = false (R42)"
        );

        let local_cli = Cli::try_parse_from([
            "relativist",
            "local",
            "--workers",
            "2",
            "--input",
            "test.bin",
        ])
        .unwrap();
        match local_cli.command {
            Command::Local(args) => {
                assert!(!args.delta_mode, "local --delta-mode default must be false");
                let cfg = build_grid_config_from_local(&args);
                assert!(
                    !cfg.delta_mode,
                    "build_grid_config_from_local must thread delta_mode = false (R42)"
                );
            }
            _ => panic!("expected Local"),
        }
    }

    // SPEC-20 UT: CLI defaults match R33a (TASK-0416).
    #[test]
    fn cli_defaults_match_r33a() {
        let cli = Cli::try_parse_from([
            "relativist",
            "coordinator",
            "--workers",
            "1",
            "--input",
            "in.bin",
        ])
        .unwrap();
        if let Command::Coordinator(args) = cli.command {
            let cfg = build_grid_config(&args);
            assert!(!cfg.hybrid_coordinator);
            assert!(!cfg.elastic_departure);
            assert_eq!(cfg.solo_budget, 10_000);
            assert_eq!(cfg.initial_wait_timeout.as_secs(), 30);
        } else {
            panic!("expected Coordinator");
        }
    }

    // SPEC-20 UT: CLI hybrid flag sets field (TASK-0416).
    #[test]
    fn cli_hybrid_flag_sets_field() {
        let cli = Cli::try_parse_from([
            "relativist",
            "coordinator",
            "--workers",
            "1",
            "--input",
            "in.bin",
            "--hybrid",
        ])
        .unwrap();
        if let Command::Coordinator(args) = cli.command {
            let cfg = build_grid_config(&args);
            assert!(cfg.hybrid_coordinator);
            assert!(cfg.elastic_join, "normalize must auto-enable elastic_join");
        }
    }

    // SPEC-20 UT: CLI join window flags (TASK-0416).
    #[test]
    fn cli_join_window_flags() {
        let cli = Cli::try_parse_from([
            "relativist",
            "coordinator",
            "--workers",
            "1",
            "--input",
            "in.bin",
            "--join-window-min-ms",
            "100",
            "--join-window-max-ms",
            "1000",
        ])
        .unwrap();
        if let Command::Coordinator(args) = cli.command {
            let cfg = build_grid_config(&args);
            assert_eq!(cfg.join_window_min.as_millis(), 100);
            assert_eq!(cfg.join_window_max.as_millis(), 1000);
        }
    }

    // TASK-0390 UT-02: SPEC-19 R41 — `--delta-mode` on the coordinator
    // subcommand parses and reaches `GridConfig.delta_mode = true`.
    #[test]
    fn cli_delta_mode_flag_threads_through_coordinator() {
        let cli = Cli::try_parse_from([
            "relativist",
            "coordinator",
            "--workers",
            "4",
            "--input",
            "input.bin",
            "--delta-mode",
        ])
        .unwrap();
        match cli.command {
            Command::Coordinator(args) => {
                assert!(args.delta_mode, "--delta-mode must set args.delta_mode");
                let cfg = build_grid_config(&args);
                assert!(
                    cfg.delta_mode,
                    "build_grid_config must thread delta_mode = true"
                );
            }
            _ => panic!("expected Coordinator"),
        }
    }

    // TASK-0390 UT-03: SPEC-19 R41 — `--delta-mode` on the local subcommand
    // parses and reaches `GridConfig.delta_mode = true`.
    #[test]
    fn cli_delta_mode_flag_threads_through_local() {
        let cli = Cli::try_parse_from([
            "relativist",
            "local",
            "--workers",
            "2",
            "--input",
            "test.bin",
            "--delta-mode",
        ])
        .unwrap();
        match cli.command {
            Command::Local(args) => {
                assert!(args.delta_mode, "--delta-mode must set args.delta_mode");
                let cfg = build_grid_config_from_local(&args);
                assert!(
                    cfg.delta_mode,
                    "build_grid_config_from_local must thread delta_mode = true"
                );
            }
            _ => panic!("expected Local"),
        }
    }

    // TASK-0397 UT-0397-04 (coordinator): SPEC-19 R43 — `--delta-mode`
    // on the coordinator subcommand MUST normalize through
    // `build_grid_config`, setting `coordinator_free_rounds = true` by
    // default even though the CLI does not expose a flag for that field.
    // This is the CLI-integration regression canary for DC-0397-B option
    // (a)+(b): normalization fires automatically on CLI construction.
    #[test]
    fn ut_0397_04_build_grid_config_with_delta_mode_flag_normalizes_coordinator_free_rounds() {
        let cli = Cli::try_parse_from([
            "relativist",
            "coordinator",
            "--workers",
            "4",
            "--input",
            "input.bin",
            "--delta-mode",
        ])
        .unwrap();
        match cli.command {
            Command::Coordinator(args) => {
                assert!(args.delta_mode);
                let cfg = build_grid_config(&args);
                assert!(cfg.delta_mode, "build_grid_config threads delta_mode");
                assert!(
                    cfg.coordinator_free_rounds,
                    "SPEC-19 R43: build_grid_config must normalize \
                     coordinator_free_rounds=true under delta_mode=true"
                );
                assert_eq!(cfg.num_workers, 4);
            }
            _ => panic!("expected Coordinator"),
        }
    }

    // TASK-0397 UT-0397-04 (local mirror): same as above for the local
    // subcommand, exercising `build_grid_config_from_local`.
    #[test]
    fn ut_0397_04_build_grid_config_from_local_with_delta_mode_flag_normalizes_coordinator_free_rounds(
    ) {
        let cli = Cli::try_parse_from([
            "relativist",
            "local",
            "--workers",
            "2",
            "--input",
            "test.bin",
            "--delta-mode",
        ])
        .unwrap();
        match cli.command {
            Command::Local(args) => {
                assert!(args.delta_mode);
                let cfg = build_grid_config_from_local(&args);
                assert!(cfg.delta_mode);
                assert!(
                    cfg.coordinator_free_rounds,
                    "SPEC-19 R43: build_grid_config_from_local must normalize \
                     coordinator_free_rounds=true under delta_mode=true"
                );
                assert_eq!(cfg.num_workers, 2);
            }
            _ => panic!("expected Local"),
        }
    }

    // TASK-0397 — negative-path sanity: absence of --delta-mode leaves
    // coordinator_free_rounds=false (R42 baseline preserved through
    // normalize's no-op branch).
    #[test]
    fn ut_0397_build_grid_config_without_delta_mode_preserves_r42_baseline() {
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
                assert!(!args.delta_mode);
                let cfg = build_grid_config(&args);
                assert!(!cfg.delta_mode);
                assert!(
                    !cfg.coordinator_free_rounds,
                    "R42 baseline: without --delta-mode, coordinator_free_rounds stays false"
                );
            }
            _ => panic!("expected Coordinator"),
        }
    }

    #[test]
    fn test_build_node_config_coordinator() {
        let args = make_coordinator_args(8, None);
        let config = build_node_config_coordinator(&args).unwrap();
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

    // === resolve_bind_address tests ===

    #[test]
    fn test_resolve_bind_standard_ip() {
        let addr = resolve_bind_address("0.0.0.0:9000").unwrap();
        assert_eq!(addr, "0.0.0.0:9000".parse::<SocketAddr>().unwrap());
    }

    #[test]
    fn test_resolve_bind_localhost_default() {
        let addr = resolve_bind_address("127.0.0.1:9000").unwrap();
        assert_eq!(addr, "127.0.0.1:9000".parse::<SocketAddr>().unwrap());
    }

    #[test]
    fn test_resolve_bind_tailscale_no_daemon() {
        // If Tailscale is not installed, should return a clear error.
        // This test passes in CI where Tailscale is not available.
        let result = resolve_bind_address("tailscale:9000");
        // We can't assert success (depends on environment), but we CAN assert
        // that it doesn't panic and returns a meaningful error if Tailscale is absent.
        if let Err(e) = result {
            let err = e.to_string();
            assert!(err.contains("tailscale") || err.contains("Tailscale"));
        }
    }

    #[test]
    fn test_resolve_bind_tailscale_default_port() {
        let result = resolve_bind_address("tailscale");
        if let Ok(addr) = result {
            assert_eq!(addr.port(), 9000);
        }
        // If Tailscale not installed, error is acceptable
    }

    #[test]
    fn test_resolve_bind_invalid() {
        let result = resolve_bind_address("not-a-valid-address");
        assert!(result.is_err());
    }

    // === TASK-0310: CLI transport flag tests ===

    // CL1: coordinator transport flags parse correctly
    #[test]
    fn test_parse_coordinator_transport_flags() {
        let cli = Cli::try_parse_from([
            "relativist",
            "coordinator",
            "-w",
            "2",
            "-i",
            "input.bin",
            "--transport",
            "tcp",
            "--send-buffer",
            "2097152",
            "--recv-buffer",
            "2097152",
            "--keepalive",
            "60",
        ])
        .unwrap();
        match cli.command {
            Command::Coordinator(args) => {
                assert_eq!(args.transport, "tcp");
                assert_eq!(args.send_buffer, 2_097_152);
                assert_eq!(args.recv_buffer, 2_097_152);
                assert_eq!(args.keepalive, 60);
                assert!(!args.no_tcp_nodelay); // default: TCP_NODELAY on
                assert!(args.socket_path.is_none());
            }
            _ => panic!("expected Coordinator"),
        }
    }

    // CL5: worker transport flags parse correctly
    #[test]
    fn test_parse_worker_transport_flags() {
        let cli = Cli::try_parse_from([
            "relativist",
            "worker",
            "-c",
            "127.0.0.1:9000",
            "--transport",
            "tcp",
            "--keepalive",
            "0",
        ])
        .unwrap();
        match cli.command {
            Command::Worker(args) => {
                assert_eq!(args.transport, "tcp");
                assert_eq!(args.keepalive, 0);
            }
            _ => panic!("expected Worker"),
        }
    }

    // CL3: --transport=unix on Windows produces config error (R31)
    #[cfg(not(unix))]
    #[test]
    fn test_transport_unix_on_windows_error() {
        let mut args = make_coordinator_args(4, None);
        args.transport = "unix".to_string();
        let result = build_node_config_coordinator(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not supported"));
    }

    // Transport config populated correctly from CLI
    #[test]
    fn test_build_node_config_coordinator_transport() {
        let mut args = make_coordinator_args(4, None);
        args.keepalive = 60;
        args.send_buffer = 2_097_152;
        let config = build_node_config_coordinator(&args).unwrap();
        assert_eq!(
            config.transport.keepalive_idle,
            Some(std::time::Duration::from_secs(60))
        );
        assert_eq!(config.transport.send_buffer_bytes, Some(2_097_152));
        assert!(config.transport.tcp_nodelay);
    }

    // keepalive=0 disables keepalive
    #[test]
    fn test_build_node_config_keepalive_disabled() {
        let mut args = make_coordinator_args(4, None);
        args.keepalive = 0;
        let config = build_node_config_coordinator(&args).unwrap();
        assert!(config.transport.keepalive_idle.is_none());
    }

    // --no-tcp-nodelay disables TCP_NODELAY
    #[test]
    fn test_build_node_config_no_tcp_nodelay() {
        let mut args = make_coordinator_args(4, None);
        args.no_tcp_nodelay = true;
        let config = build_node_config_coordinator(&args).unwrap();
        assert!(!config.transport.tcp_nodelay);
    }

    // Worker transport config populated correctly
    #[test]
    fn test_build_node_config_worker_transport() {
        let mut args = make_worker_args("127.0.0.1:9000");
        args.send_buffer = 1_048_576;
        args.recv_buffer = 1_048_576;
        let config = build_node_config_worker(&args).unwrap();
        assert_eq!(config.transport.send_buffer_bytes, Some(1_048_576));
        assert_eq!(config.transport.recv_buffer_bytes, Some(1_048_576));
    }

    // Unknown transport backend produces error
    #[test]
    fn test_build_node_config_unknown_transport() {
        let mut args = make_coordinator_args(4, None);
        args.transport = "quic".to_string();
        let result = build_node_config_coordinator(&args);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("unknown transport"));
    }

    /// TASK-0346 R8: `--compression-threshold` flag threads through CLI →
    /// CoordinatorArgs → TransportConfig (SPEC-18 R37).
    #[test]
    fn cli_compression_threshold_flag_threads_through_coordinator() {
        let cli = Cli::try_parse_from([
            "relativist",
            "coordinator",
            "--workers",
            "4",
            "--input",
            "input.bin",
            "--compression-threshold",
            "2048",
        ])
        .unwrap();
        match cli.command {
            Command::Coordinator(args) => {
                assert_eq!(args.compression_threshold, 2048);
                let cfg = build_node_config_coordinator(&args).unwrap();
                assert_eq!(cfg.transport.compression_threshold, 2048);
            }
            _ => panic!("expected Coordinator"),
        }
    }

    /// TASK-0346 R8 (worker side): same flag works on the worker subcommand.
    #[test]
    fn cli_compression_threshold_flag_threads_through_worker() {
        let cli = Cli::try_parse_from([
            "relativist",
            "worker",
            "-c",
            "127.0.0.1:9000",
            "--compression-threshold",
            "8192",
        ])
        .unwrap();
        match cli.command {
            Command::Worker(args) => {
                assert_eq!(args.compression_threshold, 8192);
                let cfg = build_node_config_worker(&args).unwrap();
                assert_eq!(cfg.transport.compression_threshold, 8192);
            }
            _ => panic!("expected Worker"),
        }
    }

    /// TASK-0346 R9 (CLI default): omitting the flag yields the 1024 default.
    #[test]
    fn cli_compression_threshold_default_is_1024() {
        let args = make_coordinator_args(4, None);
        assert_eq!(args.compression_threshold, 1024);
        let cfg = build_node_config_coordinator(&args).unwrap();
        assert_eq!(cfg.transport.compression_threshold, 1024);
    }

    // === SPEC-18 §3.3 — QA stage adversarial probes ===

    /// QA probe #7 (CLI side): `--compression-threshold 0` parses without
    /// error and propagates 0 all the way through to `TransportConfig`.
    /// Pairs with `frame::tests::qa_probe_7_threshold_zero_compresses_minimal_message`,
    /// which verifies the wire-level effect of that 0.
    #[test]
    fn qa_probe_7_cli_threshold_zero_threads_through_coordinator_and_worker() {
        let coord_cli = Cli::try_parse_from([
            "relativist",
            "coordinator",
            "-w",
            "4",
            "--input",
            "input.bin",
            "--compression-threshold",
            "0",
        ])
        .unwrap();
        match coord_cli.command {
            Command::Coordinator(args) => {
                assert_eq!(args.compression_threshold, 0);
                let cfg = build_node_config_coordinator(&args).unwrap();
                assert_eq!(cfg.transport.compression_threshold, 0);
            }
            _ => panic!("expected Coordinator"),
        }

        let worker_cli = Cli::try_parse_from([
            "relativist",
            "worker",
            "-c",
            "127.0.0.1:9000",
            "--compression-threshold",
            "0",
        ])
        .unwrap();
        match worker_cli.command {
            Command::Worker(args) => {
                assert_eq!(args.compression_threshold, 0);
                let cfg = build_node_config_worker(&args).unwrap();
                assert_eq!(cfg.transport.compression_threshold, 0);
            }
            _ => panic!("expected Worker"),
        }
    }

    /// QA probe #8 (CLI side): `--compression-threshold` accepts the
    /// `usize::MAX` literal (`18446744073709551615` on 64-bit hosts) and
    /// threads it through. Pairs with
    /// `frame::tests::qa_probe_8_threshold_usize_max_skips_compression_on_large_message`.
    #[test]
    fn qa_probe_8_cli_threshold_usize_max_threads_through_coordinator_and_worker() {
        // The literal must match `usize::MAX` on the target. On 32-bit
        // hosts (which Relativist does not officially support) this would
        // need a different literal; the assertion below makes the
        // assumption explicit.
        let max_literal = format!("{}", usize::MAX);

        let coord_cli = Cli::try_parse_from([
            "relativist",
            "coordinator",
            "-w",
            "4",
            "--input",
            "input.bin",
            "--compression-threshold",
            &max_literal,
        ])
        .unwrap();
        match coord_cli.command {
            Command::Coordinator(args) => {
                assert_eq!(args.compression_threshold, usize::MAX);
                let cfg = build_node_config_coordinator(&args).unwrap();
                assert_eq!(cfg.transport.compression_threshold, usize::MAX);
            }
            _ => panic!("expected Coordinator"),
        }

        let worker_cli = Cli::try_parse_from([
            "relativist",
            "worker",
            "-c",
            "127.0.0.1:9000",
            "--compression-threshold",
            &max_literal,
        ])
        .unwrap();
        match worker_cli.command {
            Command::Worker(args) => {
                assert_eq!(args.compression_threshold, usize::MAX);
                let cfg = build_node_config_worker(&args).unwrap();
                assert_eq!(cfg.transport.compression_threshold, usize::MAX);
            }
            _ => panic!("expected Worker"),
        }
    }

    // === TASK-0358: --use-zero-copy CLI flag (SPEC-18 §3.5 R36/R37) ===

    /// UT-0358-CLI-01: omitting `--use-zero-copy` keeps the default `false`
    /// on both subcommands (R20 / R37 — opt-in by design).
    #[test]
    fn cli_use_zero_copy_default_is_false_on_both_subcommands() {
        let coord_args = make_coordinator_args(4, None);
        assert!(!coord_args.use_zero_copy);
        let coord_cfg = build_node_config_coordinator(&coord_args).unwrap();
        assert!(!coord_cfg.transport.use_zero_copy);

        let worker_args = make_worker_args("127.0.0.1:9000");
        assert!(!worker_args.use_zero_copy);
        let worker_cfg = build_node_config_worker(&worker_args).unwrap();
        assert!(!worker_cfg.transport.use_zero_copy);
    }

    /// UT-0358-CLI-02: `--use-zero-copy` parses on the coordinator
    /// subcommand and threads through to `TransportConfig.use_zero_copy`.
    #[test]
    fn cli_use_zero_copy_flag_threads_through_coordinator() {
        let cli = Cli::try_parse_from([
            "relativist",
            "coordinator",
            "--workers",
            "4",
            "--input",
            "input.bin",
            "--use-zero-copy",
        ])
        .unwrap();
        match cli.command {
            Command::Coordinator(args) => {
                assert!(args.use_zero_copy);
                let cfg = build_node_config_coordinator(&args).unwrap();
                assert!(cfg.transport.use_zero_copy);
            }
            _ => panic!("expected Coordinator"),
        }
    }

    /// UT-0358-CLI-03: `--use-zero-copy` parses on the worker subcommand
    /// and threads through.
    #[test]
    fn cli_use_zero_copy_flag_threads_through_worker() {
        let cli = Cli::try_parse_from([
            "relativist",
            "worker",
            "-c",
            "127.0.0.1:9000",
            "--use-zero-copy",
        ])
        .unwrap();
        match cli.command {
            Command::Worker(args) => {
                assert!(args.use_zero_copy);
                let cfg = build_node_config_worker(&args).unwrap();
                assert!(cfg.transport.use_zero_copy);
            }
            _ => panic!("expected Worker"),
        }
    }
}
