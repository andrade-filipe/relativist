# SPEC-07: Deployment and Execution

**Status:** Revised v3.1 — R11 amended per SPEC-21 §3.8 A3 (GridConfig gains `chunk_size`, `streaming_strategy`, `dispatch_mode`, `max_pending_lifetime`)
**Depends on:** SPEC-00 (Glossary), SPEC-02 (Net Representation), SPEC-03 (Reduction Engine), SPEC-04 (Partitioning), SPEC-05 (Merge and Grid Cycle), SPEC-06 (Wire Protocol)
**Extended by:** SPEC-10 (Security), SPEC-11 (Observability), SPEC-12 (User I/O), SPEC-13 (System Architecture)
**Amends:** SPEC-21 §3.8 A3 (R11 — CLI-to-GridConfig mapping gains streaming pipeline fields per SPEC-21 R24, R25, R34, R37g)
**Gray zones resolved:** ---
**References consumed:** REF-001 (Lafont 1990), REF-002 (Lafont 1997), REF-003 (HVM2), REF-017 (Foster, Kesselman, Tuecke 2001 -- Grid Anatomy)
**Discussions consumed:** DISC-007 v2 (fault tolerance -- out of scope justification), DISC-008 v2 (shared-to-distributed transition spectrum, operational dimensions)
**Arguments consumed:** ARG-004 (practical viability and limits, overhead decomposition, workload profiles A/B/C)
**Code analyses consumed:** AC-003 (Haskell Protocol/Network: NodeConfig, NodeRole, runCoordinator, runWorker, connectWithRetry), AC-004 (Haskell Grid/TreeMapReduce: runGridLocal, GridMetrics, mkTree, mkTreeBalanced, benchmark modes)

> **Supersession note (v3):** SPEC-07 is the foundational deployment and execution spec. Successor specs (SPEC-10, SPEC-11, SPEC-12, SPEC-13) extend and supersede specific requirements as noted inline. Where a supersession note appears, the successor spec is authoritative. SPEC-07 remains authoritative for all requirements not explicitly superseded.

---

## 1. Purpose

This spec defines how Relativist is configured, built, deployed, and executed -- from a single developer machine to a multi-node grid of up to 8 physical machines. It covers: the single-binary CLI with 7 subcommands (coordinator, worker, local, generate, reduce, inspect, compute), configuration via CLI arguments, the lifecycle of each execution mode, input/output formats for interaction nets and metrics, workload generators for benchmarks, Docker-based deployment for reproducibility, manual deployment for physical machines, and logging. This is the most operational spec in Relativist: it transforms the abstractions of SPEC-05 (grid loop) and SPEC-06 (wire protocol) into an executable program that researchers can use to reproduce the TCC's experimental results. Successor specs extend specific areas: SPEC-10 (security CLI flags), SPEC-11 (observability CLI flags), SPEC-12 (multi-format I/O and detailed subcommand arguments), SPEC-13 (FSM, architecture, additional subcommands).

## 2. Definitions

Terms defined in SPEC-00 (Glossary) are used without redefinition. Terms introduced or refined in this spec:

| Term | Definition |
|------|-----------|
| **Subcommand** | A variant of the CLI that determines Relativist's execution mode. SPEC-07 defines the original 4: `coordinator`, `worker`, `local`, `generate`. SPEC-13 R43 adds 3 more: `reduce`, `inspect`, `compute`. Total: 7 subcommands. Implemented via `clap` with `#[derive(Subcommand)]`. (Relativist) |
| **Local Mode** | Execution mode where the grid loop (SPEC-05, Section 4.4) runs entirely in a single process, without TCP. Workers are simulated by sequential (or parallel via `rayon`) iteration over partitions. Essential for testing, baseline benchmarks, and validation of the Fundamental Property. (Relativist) |
| **Distributed Mode** | Execution mode where the coordinator sends partitions to remote workers via TCP (SPEC-06). The coordinator executes the distributed grid loop (SPEC-06, Section 4.6); each worker executes the worker loop (SPEC-06, Section 4.7). (Relativist) |
| **Network Input** | An interaction net loaded from a file. For `coordinator`, `worker`, and `local` subcommands: bincode `.bin` only. For `reduce`, `inspect`, and `generate` subcommands: binary (`.bin`), text DSL (`.ic`), or JSON (`.json`) per SPEC-12 R1. (Relativist) |
| **Network Output** | The interaction net in Normal Form (or partially reduced if `max_rounds` was reached), serialized together with execution metrics. (Relativist) |
| **Workload** | A generator function that produces an IC net for testing and benchmarks. Each workload produces a `Net` parameterized by size. The pre-defined workloads in Relativist correspond to the benchmarks of SPEC-09. (Relativist) |
| **Deploy Target** | The environment where Relativist runs: `local` (single machine, no Docker), `docker-local` (single machine, Docker Compose), `docker-lan` (multiple machines, Docker per node), `bare-metal` (multiple physical machines, binary copied manually). (Relativist) |

---

## 3. Requirements

### 3.1 Single Binary with CLI Subcommands

**R1.** Relativist MUST be compiled as a single binary (`relativist`) with seven subcommands: `coordinator`, `worker`, `local`, `generate`, `reduce`, `inspect`, and `compute`. The original 4 subcommands are defined in this spec; `reduce`, `inspect`, and `compute` are defined in SPEC-13 R43 and detailed in SPEC-12 and SPEC-14. **(MUST)**

**R2.** CLI argument parsing MUST use the `clap` crate with derive macros (`#[derive(Parser)]`, `#[derive(Subcommand)]`), as per the confirmed technical decision. **(MUST)**

**R3.** The `coordinator` subcommand MUST accept the following arguments:
- `--workers <N>` (required): number of workers to wait for.
- `--bind <ADDR:PORT>` (optional, default: `127.0.0.1:9000`): socket address to bind to.
- `--input <PATH>` (required): path to the input network file (`.bin`).
- `--max-rounds <N>` (optional, default: unlimited): maximum grid rounds.
- `--output <PATH>` (optional): path to write the reduced network.
- `--metrics <PATH>` (optional): path to write execution metrics (`.json` or `.csv`).
- `--strategy <NAME>` (optional, default: `round-robin`): partitioning strategy (SPEC-04).
- `--log-format <FORMAT>` (optional, default: TTY-dependent): `text` or `json` (SPEC-11 R3).
- `--metrics-port <PORT>` (optional, default: `9090`, feature-gated on `metrics`): Prometheus HTTP port (SPEC-11 R20).

> **Supersession note (v3):** The `--bind` flag (combining host and port as a `SocketAddr`) supersedes the v2 `--host` and `--port` flags. Default changed from `0.0.0.0` to `127.0.0.1:9000` per SPEC-10 R5 and SPEC-13 R44. The `--input` flag supersedes the v2 `--net` flag for consistency with SPEC-13 R44 and SPEC-12 R24. Security flags (`--token`, `--token-file`, `--insecure`, `--tls-cert`, `--tls-key`) are defined in SPEC-10 Section 4.5 and are part of the coordinator's full flag set.
**(MUST)**

**R4.** The `worker` subcommand MUST accept the following arguments:
- `--coordinator <HOST:PORT>` (required): address of the coordinator.
- `--log-format <FORMAT>` (optional, default: TTY-dependent): `text` or `json` (SPEC-11 R3).

> **Supersession note (v3):** Security flags (`--token`, `--tls-ca`) are defined in SPEC-10 Section 4.5 and are part of the worker's full flag set.

- `--daemon` (optional, default: false): run in daemon mode, reconnecting to the coordinator after each job (SPEC-16 R1).
**(MUST)**

**R5.** The `local` subcommand MUST accept the following arguments:
- `--workers <N>` (required): number of simulated workers.
- `--input <PATH>` (required): path to the input network file (`.bin`).
- `--max-rounds <N>` (optional, default: unlimited): maximum rounds.
- `--output <PATH>` (optional): path to write the reduced network.
- `--metrics <PATH>` (optional): path to write execution metrics.
- `--strategy <NAME>` (optional, default: `round-robin`): partitioning strategy.
- `--log-format <FORMAT>` (optional, default: TTY-dependent): `text` or `json` (SPEC-11 R3).

> **Supersession note (v3):** The `--input` flag supersedes the v2 `--net` flag for consistency with `coordinator` and `reduce` subcommands. SPEC-13 R45a references `--net`; this is updated to `--input` for cross-subcommand consistency.
**(MUST)**

**R6.** If no subcommand is provided, Relativist MUST display the help message (`--help`) listing all 7 subcommands and exit with code 1. **(MUST)**

**R7.** All CLI arguments SHOULD be documented with textual descriptions in `--help` via `#[arg(help = "...")]` annotations from clap. **(SHOULD)**

### 3.2 Workload Generator Subcommand

**R8.** Relativist MUST provide a `generate` subcommand for creating network files from pre-defined examples.

> **Supersession note (v3):** The `generate` subcommand arguments are authoritatively defined in SPEC-12 R33. The v2 `--workload <NAME>` flag is superseded by a positional `example` argument with `value_enum` (SPEC-12 `ExampleNet` enum). The v2 short flag `-s` for `--size` is superseded by `-n`. See SPEC-12 R33 for the canonical `GenerateArgs` struct and the full list of 12 example generators.
**(MUST)**

**R9.** The `generate` subcommand MUST serialize the generated `Net` in the same bincode format used by the coordinator and by local mode (Section 4.6). **(MUST, conditional: if R8 is implemented)**

### 3.3 Configuration

**R10.** Relativist's configuration MUST be derived entirely from CLI arguments. There is no configuration file (TOML/YAML) in v1 -- CLI is the sole source of configuration. **(MUST)**

**R11.** CLI arguments MUST be mapped to the following internal configuration structures:
- `GridConfig` (SPEC-05, Section 4.1): `num_workers`, `max_rounds`, **`chunk_size`** (SPEC-21 R24, default `10_000`), **`streaming_strategy`** (SPEC-21 R25, default `RoundRobin`), **`dispatch_mode`** (SPEC-21 R34, default `Auto`), **`max_pending_lifetime`** (SPEC-21 R37g, optional, default `16`).
- `NodeConfig` (SPEC-06, Section 4.5): `role`, `bind` (as `SocketAddr`, parsed from `--bind`), `num_workers`, `max_payload_size`, timeouts.

> **Supersession note (v3):** The v2 separate `host` and `port` fields in `NodeConfig` are superseded by a single `bind: SocketAddr` field, consistent with the `--bind` CLI flag (SPEC-13 R44).

> **Amendment A3 (SPEC-21 §3.8 A3 / R24, R25, R34, R37g):** Closes SC-001 part 2. The four streaming-pipeline fields are additive — existing `GridConfig` fields and SPEC-05 R-N requirements are unchanged. The supporting enums `StreamingStrategyConfig` (R25 — variants `RoundRobin` (default), `Fennel` if implemented) and `DispatchMode` (R34 — variants `Push`, `Pull`, `Auto` (default)) are declared in SPEC-21 §4.x and re-exported from `src/config.rs`. The `chunk_size = 10_000` default is benchmark-TBD per SPEC-21 SC-024 ("MUST be re-evaluated and either confirmed or replaced before v2 release"); the same calibration disposition applies to `max_pending_lifetime = 16`. The canonical struct definition lives in SPEC-05 §4.1; this requirement (SPEC-07 R11) governs the CLI-to-config mapping surface.
**(MUST)**

**R12.** Sensible defaults MUST be provided for all optional parameters:
- `bind`: `127.0.0.1:9000` (coordinator) / not applicable (worker). See SPEC-10 R5.
- `max_rounds`: `None` (unlimited).
- `max_payload_size`: 256 MiB (SPEC-06, R9).
- `worker_connect_timeout`: 120 seconds (SPEC-06, R24).
- `distribute_timeout`: 60 seconds (SPEC-06, R30).
- `collect_timeout`: 600 seconds (SPEC-06, R30).
- `strategy`: `round-robin` (SPEC-04).
- `log_format`: TTY-dependent (SPEC-11 R3).
- `metrics_port`: `9090` (SPEC-11 R20, feature-gated on `metrics`).

> **Supersession note (v3):** Default bind address changed from `0.0.0.0` (v2) to `127.0.0.1:9000` (v3) per SPEC-10 R5 (security: prevent accidental network exposure).
**(MUST)**

### 3.4 Coordinator Lifecycle

**R13.** The coordinator MUST follow this lifecycle:
1. **Parse config:** Read CLI arguments, construct `GridConfig` and `NodeConfig`.
2. **Load network:** Read and deserialize the input network from the file specified by `--input`.
3. **Wait for workers:** Open TCP listener, accept connections until `num_workers` workers are connected (SPEC-06, R24).
4. **Distributed grid loop:** Execute the loop partition -> distribute -> collect -> merge -> resolve_borders (SPEC-06, Section 4.6) until Normal Form or `max_rounds`.
5. **Shutdown:** Send `Message::Shutdown` to all workers (SPEC-06, Section 4.12).
6. **Output:** Write reduced network (if `--output`), write metrics (if `--metrics`), print summary to `stdout`.
**(MUST)**

**R14.** If the `--input` file does not exist or cannot be deserialized, the coordinator MUST print a diagnostic error to `stderr` and exit with code 1. **(MUST)**

**R15.** The coordinator SHOULD print to `stdout` at the end of execution a human-readable summary containing at least: number of rounds, total interactions, total time, whether it converged, and the extracted result (number of agents per symbol). **(SHOULD)**

### 3.5 Worker Lifecycle

**R16.** The worker MUST follow this lifecycle:
1. **Parse config:** Read CLI arguments, construct `NodeConfig`.
2. **Connect to coordinator:** With retry and exponential backoff (SPEC-06, R23, Section 4.8).
3. **Worker loop:** Receive `AssignPartition`, execute `reduce_all`, reconstruct `free_port_index`, send `PartitionResult` (SPEC-06, Section 4.7). Repeat until `Shutdown` is received.
4. **Cleanup:** Close TCP connection and exit with code 0.
**(MUST)**

**R17.** If the worker fails to connect after 10 attempts (SPEC-06, R23), it MUST print an error to `stderr` and exit with code 1. **(MUST)**

### 3.6 Local Mode (No Network)

**R18.** The `local` subcommand MUST execute the grid loop entirely in-process, using the `run_grid` function from SPEC-05 (Section 4.4). It MUST NOT open TCP sockets. **(MUST)**

**R19.** Local mode MUST produce results identical to distributed mode for the same network and number of workers. Formally, the Fundamental Property (SPEC-01 G1) establishes the three-way equivalence: `reduce_all(net) ~ run_grid_local(net, n) ~ run_coordinator(net, n)` for every terminating net `net` and `n >= 1`, where `~` denotes isomorphism of graphs (structural equality modulo ID renaming). The `reduce` subcommand (SPEC-13 R41) provides the sequential baseline `reduce_all(net)` against which both `local` and `coordinator` results are verified. **(MUST)**

**R20.** Local mode SHOULD use `rayon::par_iter` or equivalent to execute partition reduction in parallel (across threads), enabling shared-memory parallelism benchmarks as a baseline for comparison with distributed mode (DISC-008 v2, Sections 2.1-2.2, shared vs. distributed comparison). **(SHOULD)**

**R21.** In local mode, the network metric fields (`network_send_time_per_round`, `network_recv_time_per_round`, `bytes_sent_per_round`, `bytes_received_per_round`) MUST be empty vectors, as per SPEC-05 R36. **(MUST)**

### 3.7 Input Format: IC Network

**R22.** The input format for IC networks in the `coordinator`, `worker`, and `local` subcommands MUST be a binary file containing the `Net` serialized with serde + bincode (default configuration: little-endian, fixed-int encoding). The conventional extension is `.bin`. **(MUST)**

> **Supersession note (v3):** SPEC-12 R1-R50 supersede R22-R25 for the `reduce`, `inspect`, and `generate` subcommands. Those subcommands accept three input formats: binary (`.bin`), text DSL (`.ic`), and JSON (`.json`), with auto-detection by file extension and a `--input-format` override flag (SPEC-12 R24). The bincode-only restriction in R22 applies exclusively to `coordinator`, `worker`, and `local`.

**R23.** The format MUST be self-contained: the file contains all information necessary to reconstruct the `Net` (agents, ports, redex_queue, next_id) without external data. **(MUST)**

**R24.** For the `coordinator` and `local` subcommands, the `.bin` input format MUST be the same format produced by `--output` of coordinator/local and by the `generate` subcommand (when outputting `.bin`). This allows chaining executions: the output of one run can be the input of the next. **(MUST)**

### 3.8 Output Format: Reduced Network

**R25.** If `--output <PATH>` is specified, Relativist MUST write the network in Normal Form (or partially reduced) in the same bincode format as the input (R22). **(MUST)**

**R26.** If `--output` is not specified, the reduced network is NOT written to a file. The summary printed to `stdout` (R15) serves as the primary output. **(MUST)**

### 3.9 Output Format: Metrics

**R27.** If `--metrics <PATH>` is specified, Relativist MUST write the execution metrics (`GridMetrics`, SPEC-05) to a file. **(MUST)**

**R28.** The metrics format SHOULD be JSON when the file has a `.json` extension, and CSV when the file has a `.csv` extension. If no recognized extension is provided, the default SHOULD be JSON. **(SHOULD)**

**R29.** The CSV format SHOULD contain a header line followed by one line per round, with columns covering at least:
```
round,agents,local_interactions,border_interactions,border_redexes,partition_time_ms,compute_time_ms,merge_time_ms,bytes_sent,bytes_received,network_send_time_ms,network_recv_time_ms
```
Plus a summary total at the end or a separate `*_summary.csv` file with totals.
**(SHOULD)**

**R30.** The JSON format SHOULD serialize the complete `GridMetrics` structure via serde, including all per-round fields. **(SHOULD)**

**R31.** The recorded metrics MUST be sufficient to reproduce all tables and charts of the experimental evaluation (Section 4 of the TCC, SPEC-09). **(MUST)**

### 3.10 Pre-defined Workload Generators

**R32.** Relativist MUST include generator functions for the workloads corresponding to SPEC-09 benchmarks. The minimum set includes: TreeSum, TreeSumBalanced, EpAnnihilation (ERA-ERA pairs), ConDupExpansion, and DualTree.

> **Supersession note (v3):** SPEC-12 R33 defines the authoritative list of 12 example generators via the `ExampleNet` enum, including the 5 listed above plus EpAnnihilationCon, EpAnnihilationDup, MixedRules, ErasurePropagation, ChurchNat, ChurchAdd, and ChurchMul. SPEC-12 R36 specifies that generators reside in `io/examples.rs`. The v2 name "EraChain" is renamed to "EpAnnihilation" for consistency with SPEC-09 and SPEC-12.
**(MUST)**

**R33.** Each workload generator function MUST accept a size parameter (`size: u32`) and produce a deterministic network: for the same `size`, the same network is always generated. **(MUST)**

**R34.** Each workload SHOULD have a verification function `verify(net: &Net, size: u32) -> bool` that confirms the reduction result is correct for that workload and size. For TreeSum, verification is `extract_result(net) == expected_sum`. **(SHOULD)**

### 3.11 Logging

**R35.** Relativist MUST use the `tracing` crate for structured logging, with subscriber configured via the `RUST_LOG` environment variable (via `tracing-subscriber` with `EnvFilter`). **(MUST)**

> **Supersession note (v3):** SPEC-11 supersedes and extends R35-R36 with detailed instrumentation requirements including: `--log-format` CLI flag for text/JSON output (R3), `#[instrument]` on key functions with structured fields (R6), FSM state transition logging at INFO (R7), per-component default log levels (R5), and ERROR-level events for invariant violations (R9a). SPEC-11 is authoritative for all observability concerns. R35-R36 here provide the baseline; SPEC-11 adds MUST-level requirements on top.

**R36.** Log levels SHOULD follow:
- `error`: irrecoverable failures (I/O, deserialization, timeout, worker crash).
- `warn`: anomalous but non-fatal situations (stale redexes discarded, max_rounds reached).
- `info`: high-level events (worker connected, round started/completed, Normal Form reached, summary metrics).
- `debug`: operational details (partition sizes, bytes sent/received, duration of each phase).
- `trace`: full data dumps for development (agent contents, serialized message contents).
**(SHOULD)**

### 3.12 Docker Deployment

**R37.** The Relativist repository SHOULD include a `Dockerfile` that builds the release binary in a multi-stage build:
- **Stage 1 (builder):** Based on `rust:latest` (or a specific stable Rust version). Runs `cargo build --release`. Copies only the binary.
- **Stage 2 (runtime):** Based on `debian:bookworm-slim` (or equivalent minimal image). Copies the binary from stage 1. Sets the binary as the `ENTRYPOINT`.
**(SHOULD)**

**R38.** The Relativist repository SHOULD include a `docker-compose.yml` that defines a local grid deployment with configurable number of workers:
- One `coordinator` service.
- N `worker` services (default N=3, configurable via environment variable or `--scale`).
- A shared Docker network connecting all services.
- Volume mounts for input networks and output files.
**(SHOULD)**

**R39.** The Docker-based deployment MUST produce results identical to bare-metal deployment for the same input network and number of workers. Docker is a deployment mechanism only; it MUST NOT affect the semantics of computation. **(MUST, conditional: if R37-R38 are implemented)**

**R40.** The `docker-compose.yml` SHOULD provide a simple invocation for running the standard benchmarks:
```bash
# Build the image
docker compose build

# Run with 4 workers, TreeSum workload of size 10000
docker compose up --scale worker=4

# Or generate and run a custom workload
docker compose run coordinator relativist generate ep-annihilation -n 10000 --output /data/net.bin
docker compose up --scale worker=4
```
**(SHOULD)**

### 3.13 Manual Deployment (Bare-Metal)

**R41.** Deployment on multiple physical machines MUST be supported via manual procedure: the user compiles the binary, copies it to each machine, starts the coordinator on one machine, and starts workers on the others pointing to the coordinator's address. **(MUST)**

**R42.** The repository SHOULD include a convenience shell script (`scripts/deploy.sh`) that automates the copy of the binary via `scp` and startup of workers via `ssh`. This script is a convenience, not part of the binary itself. **(SHOULD)**

### 3.14 Exit Codes

**R43.** The binary SHOULD return the following exit codes:
- `0`: execution completed successfully (Normal Form reached, or max_rounds/max_interactions reached without error).
- `1`: configuration error (invalid arguments, file not found, deserialization failed, encoding error).
- `2`: communication error (timeout, lost connection, checksum mismatch). Not applicable to `reduce`, `inspect`, `generate`, or `compute` subcommands.
- `3`: internal error (panic, reduction engine bug detected by assert).
**(SHOULD)**

### 3.15 Scope Exclusions

**R44.** Relativist v1 does NOT implement the following features, which are explicitly out of scope:
- Automatic discovery of workers. Workers specify the coordinator address manually.
- Fault tolerance beyond timeout (DISC-007 v2: Z5 out of scope).
- Configuration file (TOML/YAML). CLI is sufficient for v1.
- Hot-reload of configuration.
- Web dashboard or graphical interface.

> **Supersession note (v3):** The v2 exclusion "Authentication or encryption. The environment is considered trusted." has been removed. SPEC-10 defines a three-tier security model: Tier 1 (development, no auth) requires zero configuration; Tier 2 (private network, token auth) and Tier 3 (production, token + TLS) are optional. The TCC evaluation uses Tier 1 (trusted environment), but the security infrastructure is implemented per SPEC-10.
**(informative)**

---

## 4. Design

### 4.1 CLI Structure

```rust
use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Relativist: Distributed Interaction Combinator reducer for Grid Computing.
#[derive(Parser, Debug)]
#[command(name = "relativist", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Run as coordinator: load a network, partition, distribute to workers, merge results.
    Coordinator(CoordinatorArgs),

    /// Run as worker: connect to a coordinator and reduce assigned partitions.
    Worker(WorkerArgs),

    /// Run grid simulation locally (no TCP). For testing and baseline benchmarks.
    Local(LocalArgs),

    /// Generate an example network and save to a file.
    Generate(GenerateArgs),

    /// Reduce a network locally (sequential reduce_all, no partitioning). SPEC-13 R41, R46.
    Reduce(ReduceArgs),

    /// Inspect a network file and print statistics. SPEC-13 R47.
    Inspect(InspectArgs),

    /// Encode, reduce, and decode an arithmetic expression. SPEC-13 R48a, SPEC-14.
    Compute(ComputeArgs),
}

// Note: ReduceArgs, InspectArgs, and ComputeArgs are defined in SPEC-12 R24, R28, and SPEC-14 R22.
// They are listed here for completeness; their authoritative definitions are in the successor specs.

#[derive(clap::Args, Debug)]
pub struct CoordinatorArgs {
    /// Number of workers to wait for before starting.
    #[arg(short = 'w', long)]
    pub workers: u32,

    /// Socket address to bind to (HOST:PORT).
    #[arg(long, default_value = "127.0.0.1:9000")]
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

    /// Log output format: text or json. Default: text if TTY, json otherwise.
    #[arg(long)]
    pub log_format: Option<LogFormat>,

    // Security flags (--token, --token-file, --insecure, --tls-cert, --tls-key)
    // are defined in SPEC-10 Section 4.5 and added to this struct by the implementer.

    // Metrics port (--metrics-port, default 9090, feature-gated on `metrics`)
    // is defined in SPEC-11 R20 and added to this struct by the implementer.
}

#[derive(clap::Args, Debug)]
pub struct WorkerArgs {
    /// Address of the coordinator (HOST:PORT).
    #[arg(short = 'c', long)]
    pub coordinator: String,

    /// Log output format: text or json. Default: text if TTY, json otherwise.
    #[arg(long)]
    pub log_format: Option<LogFormat>,

    // Security flags (--token, --tls-ca)
    // are defined in SPEC-10 Section 4.5 and added to this struct by the implementer.
}

#[derive(clap::Args, Debug)]
pub struct LocalArgs {
    /// Number of simulated workers.
    #[arg(short = 'w', long)]
    pub workers: u32,

    /// Path to the input network file (.bin).
    #[arg(short = 'i', long)]
    pub input: PathBuf,

    /// Maximum number of grid rounds.
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

    /// Log output format: text or json. Default: text if TTY, json otherwise.
    #[arg(long)]
    pub log_format: Option<LogFormat>,
}

// GenerateArgs is authoritatively defined in SPEC-12 R33.
// The v2 definition (--workload, -s, -o) is superseded by SPEC-12's
// positional `example: ExampleNet` (value_enum), `--size -n`, `--output -o`.
// See SPEC-12 R33 for the canonical struct definition and ExampleNet enum.
```

### 4.2 CLI-to-Config Mapping

CLI arguments are translated into the internal configuration structures defined in SPEC-05 and SPEC-06.

```
fn build_grid_config(args: &CoordinatorArgs | &LocalArgs) -> GridConfig:
    GridConfig {
        num_workers: args.workers,
        max_rounds: args.max_rounds,
        // strategy: parse_strategy(args.strategy)
        // The ENGINEER decides how to map String -> Box<dyn PartitionStrategy>
    }

fn build_node_config_coordinator(args: &CoordinatorArgs) -> NodeConfig:
    NodeConfig {
        role: NodeRole::Coordinator,
        bind: args.bind,                                // SocketAddr from --bind
        num_workers: args.workers,
        max_payload_size: DEFAULT_MAX_PAYLOAD_SIZE,     // 256 MiB
        worker_connect_timeout: Duration::from_secs(120),
        distribute_timeout: Duration::from_secs(60),
        collect_timeout: Duration::from_secs(600),
    }

fn build_node_config_worker(args: &WorkerArgs) -> NodeConfig:
    // Parse "HOST:PORT" from args.coordinator
    let coordinator_addr: SocketAddr = args.coordinator.parse()?
    NodeConfig {
        role: NodeRole::Worker,
        bind: coordinator_addr,   // for worker, this is the coordinator address to connect to
        num_workers: 0,  // irrelevant for worker
        max_payload_size: DEFAULT_MAX_PAYLOAD_SIZE,
        worker_connect_timeout: Duration::ZERO,  // irrelevant for worker
        distribute_timeout: Duration::ZERO,
        collect_timeout: Duration::ZERO,
    }
```

### 4.3 Coordinator Lifecycle

```
async fn run_coordinator_command(args: CoordinatorArgs) -> Result<(), AppError>:
    // 1. Parse config
    let grid_config = build_grid_config(&args)
    let node_config = build_node_config_coordinator(&args)
    let strategy = parse_strategy(&args.strategy)?

    // 2. Load network
    tracing::info!("Loading network from {:?}", args.input)
    let net_bytes = tokio::fs::read(&args.input).await?
    let net: Net = bincode::deserialize(&net_bytes)
        .map_err(|e| AppError::Deserialize(args.input.clone(), e))?
    tracing::info!("Loaded network: {} agents, {} redexes",
        count_live_agents(&net), net.redex_queue.len())

    // 3-5. Execute distributed grid loop (SPEC-06, Section 4.6)
    let (reduced_net, metrics) = run_coordinator(
        net, &node_config, &grid_config, &*strategy
    ).await?

    // 6. Output
    print_summary(&reduced_net, &metrics)

    if let Some(output_path) = &args.output:
        let bytes = bincode::serialize(&reduced_net)?
        tokio::fs::write(output_path, &bytes).await?
        tracing::info!("Reduced network written to {:?}", output_path)

    if let Some(metrics_path) = &args.metrics:
        write_metrics(&metrics, metrics_path).await?
        tracing::info!("Metrics written to {:?}", metrics_path)

    Ok(())
```

### 4.4 Worker Lifecycle

```
async fn run_worker_command(args: WorkerArgs) -> Result<(), AppError>:
    // 1. Parse config
    let node_config = build_node_config_worker(&args)?

    // 2-3. Connect and execute worker loop (SPEC-06, Section 4.7)
    tracing::info!("Connecting to coordinator at {}", node_config.bind)
    run_worker(&node_config).await?

    tracing::info!("Worker shutdown complete.")
    Ok(())
```

### 4.5 Local Mode

```
fn run_local_command(args: LocalArgs) -> Result<(), AppError>:
    // 1. Parse config
    let grid_config = build_grid_config_from_local(&args)
    let strategy = parse_strategy(&args.strategy)?

    // 2. Load network
    let net_bytes = std::fs::read(&args.input)?
    let net: Net = bincode::deserialize(&net_bytes)
        .map_err(|e| AppError::Deserialize(args.input.clone(), e))?
    tracing::info!("Loaded network: {} agents, {} redexes",
        count_live_agents(&net), net.redex_queue.len())

    // 3. Execute local grid loop (SPEC-05, Section 4.4)
    let (reduced_net, metrics) = run_grid(net, &grid_config, &*strategy)

    // 4. Output
    print_summary(&reduced_net, &metrics)

    if let Some(output_path) = &args.output:
        let bytes = bincode::serialize(&reduced_net)?
        std::fs::write(output_path, &bytes)?

    if let Some(metrics_path) = &args.metrics:
        write_metrics_sync(&metrics, metrics_path)?

    Ok(())
```

**Note on async:** Local mode does NOT require the tokio runtime. It uses synchronous I/O (`std::fs`) and CPU-bound computation. If `rayon` is used for parallelizing workers (R20), integration with tokio is not necessary. The `main` function can decide between starting the tokio runtime (for `coordinator` and `worker`) or executing synchronously (for `local` and `generate`).

### 4.6 Network Input/Output Format

The `.bin` file format is the direct serialization of `Net` (SPEC-02) via serde + bincode with default configuration:

```rust
use serde::{Serialize, Deserialize};

/// Serialize a Net to bytes (.bin format).
pub fn serialize_net(net: &Net) -> Result<Vec<u8>, bincode::Error> {
    bincode::serialize(net)
}

/// Deserialize a Net from bytes (.bin format).
pub fn deserialize_net(bytes: &[u8]) -> Result<Net, bincode::Error> {
    bincode::deserialize(bytes)
}
```

**Round-trip property:** `deserialize_net(serialize_net(net)?) == net` for every valid `Net`. This property MUST be verified in SPEC-08.

**Estimated `.bin` file sizes:**

| Workload | Size | Agents (approx) | Estimated .bin size |
|----------|------|-------------------|---------------------|
| TreeSum | 100 | ~300 | ~5 KB |
| TreeSum | 1,000 | ~3,000 | ~50 KB |
| TreeSum | 10,000 | ~30,000 | ~500 KB |
| TreeSum | 100,000 | ~300,000 | ~5 MB |
| DualTree(depth=10) | 1,024 | ~2,048 | ~35 KB |
| DualTree(depth=15) | 32,768 | ~65,536 | ~1.1 MB |

Sizes are estimates based on ~17 bytes per agent (Agent + 3 ports in the port array, bincode fixed-int). Actual networks may vary depending on the CON/DUP/ERA proportion and connection density.

### 4.7 Summary Output Function

```
fn print_summary(net: &Net, metrics: &GridMetrics):
    println!("=== Relativist Execution Summary ===")
    println!("Converged:          {}", metrics.converged)
    println!("Rounds:             {}", metrics.rounds)
    println!("Total interactions: {}", metrics.total_interactions)
    println!("Total time:         {:.3}s", metrics.total_time.as_secs_f64())
    println!("Final agents:       {}", count_live_agents(net))
    println!("  CON: {}", count_agents_by_symbol(net, Symbol::Con))
    println!("  DUP: {}", count_agents_by_symbol(net, Symbol::Dup))
    println!("  ERA: {}", count_agents_by_symbol(net, Symbol::Era))

    if metrics.rounds > 0:
        let avg_round_time = metrics.total_time.as_secs_f64() / metrics.rounds as f64
        println!("Avg round time:     {:.3}s", avg_round_time)

        let total_border: u64 = metrics.border_interactions_per_round.iter().sum()
        let total_local: u64 = metrics.local_interactions_per_round.iter().sum()
        println!("Local interactions:  {}", total_local)
        println!("Border interactions: {}", total_border)

    // Network metrics (distributed mode only)
    if !metrics.bytes_sent_per_round.is_empty():
        let total_sent: usize = metrics.bytes_sent_per_round.iter().sum()
        let total_recv: usize = metrics.bytes_received_per_round.iter().sum()
        println!("Bytes sent:         {}", total_sent)
        println!("Bytes received:     {}", total_recv)
        println!("Network overhead:   {:.1}%", metrics.network_overhead_fraction() * 100.0)
```

### 4.8 Metrics Output Function

```rust
/// Writes metrics in JSON or CSV format, determined by the file extension.
pub fn write_metrics(metrics: &GridMetrics, path: &Path) -> Result<(), AppError> {
    match path.extension().and_then(|e| e.to_str()) {
        Some("json") => write_metrics_json(metrics, path),
        Some("csv") => write_metrics_csv(metrics, path),
        _ => write_metrics_json(metrics, path), // default: JSON
    }
}
```

**JSON format:** Direct serialization of `GridMetrics` via `serde_json::to_string_pretty`. Requires that `GridMetrics` derives `Serialize`. Durations are serialized as floats in seconds.

**CSV format:** One header line + one line per round:

```csv
round,agents,local_interactions,border_interactions,border_redexes,partition_time_ms,compute_time_ms,merge_time_ms,bytes_sent,bytes_received,network_send_time_ms,network_recv_time_ms
0,10000,4523,12,8,1.2,45.6,3.4,125000,98000,0.5,46.1
1,5477,2100,5,3,0.8,22.3,1.8,62000,48000,0.3,22.6
...
```

Network fields (`bytes_sent`, `bytes_received`, `network_send_time_ms`, `network_recv_time_ms`) are `0` in local mode.

### 4.9 Workload Generator

> **Supersession note (v3):** The workload generator interface below is superseded by SPEC-12 R33-R42a, which defines the `ExampleNet` enum with 12 generators and pure function signatures `fn generate_<name>(size: u32) -> Net`. The code below is retained for historical context; the implementer MUST follow SPEC-12 for the authoritative generator definitions and module location (`io/examples.rs`).

```rust
/// Registers all available workloads (v2 interface, superseded by SPEC-12 ExampleNet).
/// Returns (generator function, verification function).
pub fn get_workload(name: &str) -> Option<(
    Box<dyn Fn(u32) -> Net>,        // generator
    Box<dyn Fn(&Net, u32) -> bool>,  // verifier
)> {
    match name {
        "tree-sum" => Some((
            Box::new(|size| mk_tree(&vec![1; size as usize])),
            Box::new(|net, size| extract_result(net) == size),
        )),
        "tree-sum-balanced" => Some((
            Box::new(|size| mk_tree_balanced(&vec![1; size as usize])),
            Box::new(|net, size| extract_result(net) == size),
        )),
        "ep-annihilation" => Some((
            Box::new(|size| mk_ep_annihilation(size)),
            Box::new(|net, _size| count_live_agents(net) == 0),
        )),
        "con-dup-expansion" => Some((
            Box::new(|size| mk_con_dup_expansion(size)),
            Box::new(|net, _size| net.redex_queue.is_empty()),
        )),
        "dual-tree" => Some((
            Box::new(|size| mk_dual_tree(size)),
            Box::new(|net, _size| net.redex_queue.is_empty()),
        )),
        _ => None,
    }
}
```

**Note:** The functions `mk_ep_annihilation`, `mk_con_dup_expansion`, and `mk_dual_tree` are defined in Relativist's workloads module (see SPEC-12 R36: `io/examples.rs`). Their detailed specifications are in SPEC-09 (benchmarks). This spec defines only the generation and verification interface.

### 4.10 Entrypoint (`main`)

```rust
#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    let result = match cli.command {
        Command::Coordinator(args) => run_coordinator_command(args).await,
        Command::Worker(args) => run_worker_command(args).await,
        Command::Local(args) => {
            // Local runs synchronously but needs tokio for uniform parse
            tokio::task::spawn_blocking(move || run_local_command(args))
                .await
                .unwrap()
        }
        Command::Generate(args) => run_generate_command(args),
        Command::Reduce(args) => run_reduce_command(args),    // SPEC-12 R23-R26
        Command::Inspect(args) => run_inspect_command(args),  // SPEC-12 R27-R31
        Command::Compute(args) => run_compute_command(args),  // SPEC-14 R22-R25
    };

    match result {
        Ok(()) => std::process::exit(0),
        Err(e) => {
            tracing::error!("{}", e);
            std::process::exit(e.exit_code());
        }
    }
}
```

**Note on tokio:** `#[tokio::main]` is required for `coordinator` and `worker`. For `local` and `generate`, the tokio runtime is not used for network I/O, but may be used for file I/O if needed. Alternatively, the ENGINEER may use `tokio::main` only conditionally, or use `spawn_blocking` for local mode. The implementation decision is left to the ENGINEER.

### 4.11 Docker Deployment

#### 4.11.1 Dockerfile

```dockerfile
# Stage 1: Builder
FROM rust:1.77-bookworm AS builder

WORKDIR /usr/src/relativist
COPY . .
RUN cargo build --release

# Stage 2: Runtime
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/relativist/target/release/relativist /usr/local/bin/relativist

ENTRYPOINT ["relativist"]
```

**Image size estimate:** The runtime image should be ~30-50 MB (debian-slim ~80 MB base, Rust binary ~5-15 MB, minus unused layers). This is acceptable for reproducibility; minimal Alpine-based images MAY be used as an optimization.

#### 4.11.2 Docker Compose

```yaml
# docker-compose.yml
version: "3.8"

services:
  coordinator:
    build: .
    command:
      - coordinator
      - --workers
      - "${NUM_WORKERS:-3}"
      - --bind
      - "0.0.0.0:9000"
      - --input
      - /data/input.bin
      - --output
      - /data/output.bin
      - --metrics
      - /data/metrics.json
    ports:
      - "9000:9000"
    volumes:
      - ./data:/data
    networks:
      - grid

  worker:
    build: .
    command:
      - worker
      - --coordinator
      - coordinator:9000
    depends_on:
      - coordinator
    networks:
      - grid
    deploy:
      replicas: ${NUM_WORKERS:-3}

networks:
  grid:
    driver: bridge
```

**Usage:**

```bash
# 1. Generate a workload (outside Docker or via docker run)
cargo run --release -- generate tree-sum -n 10000 --output data/input.bin

# 2. Run the grid with 4 workers
NUM_WORKERS=4 docker compose up --build

# 3. Results available in data/output.bin and data/metrics.json
```

**Note on Docker Compose scaling:** The `deploy.replicas` field works with `docker compose up`. For `docker compose up --scale worker=N`, the NUM_WORKERS env variable for the coordinator must match the number of scaled worker containers. The ENGINEER MAY add a wrapper script to ensure consistency.

#### 4.11.3 Deploy Targets Summary

| Target | When to use | Networking | Reproducibility |
|--------|------------|------------|-----------------|
| `local` subcommand | Unit tests, CI, quick experiments | None (in-process) | Perfect |
| `docker-local` | Benchmark reproducibility, integration tests | Docker bridge network | High (isolated) |
| `docker-lan` | Multi-machine benchmarks | Host networking or overlay | High (containerized) |
| `bare-metal` | Maximum performance, 8 physical machines | Direct TCP | Medium (OS-dependent) |

For the TCC's experimental evaluation, `docker-local` is the recommended default for reproducibility. `bare-metal` with 8 physical machines is the target for the primary benchmark results, using the manual deployment procedure (Section 4.12).

### 4.12 Manual Deployment (Bare-Metal)

For the TCC, deployment on 8 physical machines follows this manual procedure:

```bash
# On the coordinator machine (e.g., 192.168.1.100):
$ relativist coordinator --workers 7 --bind 0.0.0.0:9000 --input workload.bin \
    --output result.bin --metrics metrics.json

# On each worker machine (e.g., 192.168.1.101 through 192.168.1.107):
$ relativist worker --coordinator 192.168.1.100:9000
```

**Prerequisites:**
1. The `relativist` binary must be present on all machines (compile with `cargo build --release` and copy the binary).
2. The `workload.bin` file must be accessible on the coordinator machine.
3. All machines must be on the same network and port 9000 must be accessible (firewall open).
4. Workers must be started after the coordinator (the coordinator waits for connections).

**Convenience script (optional):**

```bash
#!/bin/bash
# scripts/deploy.sh -- Deploy Relativist to remote machines and run grid
#
# Usage: ./deploy.sh <coordinator_host> <worker_hosts...>
# Example: ./deploy.sh 192.168.1.100 192.168.1.101 192.168.1.102

set -euo pipefail

COORDINATOR_HOST="$1"
shift
WORKER_HOSTS=("$@")
NUM_WORKERS="${#WORKER_HOSTS[@]}"
BINARY="target/release/relativist"
REMOTE_DIR="/tmp/relativist"
PORT=9000

echo "Building release binary..."
cargo build --release

echo "Deploying to coordinator ($COORDINATOR_HOST)..."
ssh "$COORDINATOR_HOST" "mkdir -p $REMOTE_DIR"
scp "$BINARY" "$COORDINATOR_HOST:$REMOTE_DIR/relativist"
scp data/input.bin "$COORDINATOR_HOST:$REMOTE_DIR/input.bin"

for HOST in "${WORKER_HOSTS[@]}"; do
    echo "Deploying to worker ($HOST)..."
    ssh "$HOST" "mkdir -p $REMOTE_DIR"
    scp "$BINARY" "$HOST:$REMOTE_DIR/relativist"
done

echo "Starting coordinator on $COORDINATOR_HOST with $NUM_WORKERS workers..."
ssh "$COORDINATOR_HOST" "$REMOTE_DIR/relativist coordinator \
    --workers $NUM_WORKERS --bind 0.0.0.0:$PORT \
    --input $REMOTE_DIR/input.bin \
    --output $REMOTE_DIR/result.bin \
    --metrics $REMOTE_DIR/metrics.json" &

sleep 2  # Give the coordinator time to start listening

for HOST in "${WORKER_HOSTS[@]}"; do
    echo "Starting worker on $HOST..."
    ssh "$HOST" "$REMOTE_DIR/relativist worker \
        --coordinator $COORDINATOR_HOST:$PORT" &
done

echo "Grid running. Waiting for completion..."
wait
echo "Done. Fetching results..."
scp "$COORDINATOR_HOST:$REMOTE_DIR/result.bin" data/result.bin
scp "$COORDINATOR_HOST:$REMOTE_DIR/metrics.json" data/metrics.json
```

### 4.13 Sequence Diagram: Full Execution (Distributed Mode)

```
  User               Coordinator                    Worker 0              Worker 1
    |                       |                              |                     |
    |-- relativist          |                              |                     |
    |   coordinator ...     |                              |                     |
    |                       |                              |                     |
    |                  [load input.bin]                      |                     |
    |                  [bind TCP (--bind)]                  |                     |
    |                       |                              |                     |
    |                       |<--- connect ------------------|                     |
    |                       |<--- connect ----------------------------------------|
    |                       |                              |                     |
    |                  [all workers connected]              |                     |
    |                       |                              |                     |
    |                  [=== ROUND 0 ===]                   |                     |
    |                  [partition net]                      |                     |
    |                       |                              |                     |
    |                       |--- AssignPartition --------->|                     |
    |                       |--- AssignPartition -------------------------------->|
    |                       |                              |                     |
    |                       |                     [reduce_all]          [reduce_all]
    |                       |                     [rebuild_fpi]         [rebuild_fpi]
    |                       |                              |                     |
    |                       |<-- PartitionResult ----------|                     |
    |                       |<-- PartitionResult ---------------------------------|
    |                       |                              |                     |
    |                  [merge + resolve_borders]            |                     |
    |                  [check: more redexes?]               |                     |
    |                       |                              |                     |
    |                  [=== ROUND 1 === ... ]               |                     |
    |                       |                              |                     |
    |                  [no more redexes]                    |                     |
    |                       |--- Shutdown ---------------->|                     |
    |                       |--- Shutdown ---------------------------------------->|
    |                       |                              |                     |
    |                  [write result.bin]          [exit]               [exit]
    |                  [write metrics.json]                 |                     |
    |                  [print summary]                      |                     |
    |                       |                              |                     |
    |<---- exit 0 ----------|                              |                     |
```

### 4.14 Sequence Diagram: Full Execution (Local Mode)

```
  User               Relativist (single process)
    |                       |
    |-- relativist          |
    |   local ...           |
    |                       |
    |                  [load input.bin]
    |                       |
    |                  [=== ROUND 0 ===]
    |                  [partition net into N parts]
    |                  [for each part: reduce_all + rebuild_fpi]
    |                  [merge + resolve_borders]
    |                  [check: more redexes?]
    |                       |
    |                  [=== ROUND 1 === ... ]
    |                       |
    |                  [no more redexes]
    |                  [write result.bin (if --output)]
    |                  [write metrics.json (if --metrics)]
    |                  [print summary]
    |                       |
    |<---- exit 0 ----------|
```

### 4.15 Sequence Diagram: Docker Compose Deployment

```
  User                  Docker Compose               Coordinator          Workers (x3)
    |                       |                              |                     |
    |-- docker compose up   |                              |                     |
    |                       |                              |                     |
    |                  [build image]                        |                     |
    |                  [create network: grid]               |                     |
    |                       |                              |                     |
    |                  [start coordinator]                  |                     |
    |                       |----------------------------->|                     |
    |                       |                              |                     |
    |                  [start worker x3]                    |                     |
    |                       |---------------------------------------------------->|
    |                       |                              |                     |
    |                       |                     [workers connect to            |
    |                       |                      coordinator:9000]             |
    |                       |                              |                     |
    |                       |              [... grid loop as in 4.13 ...]        |
    |                       |                              |                     |
    |                       |                     [shutdown + exit]              |
    |                       |                              |                     |
    |                  [containers stop]                    |                     |
    |                  [results in ./data/]                 |                     |
    |                       |                              |                     |
    |<---- exit 0 ----------|                              |                     |
```

### 4.16 Application Error Types

```rust
/// High-level errors of the Relativist binary.
///
/// Encapsulates I/O, configuration, protocol, and logic errors.
/// Each variant maps to a specific exit code (R43).
#[derive(Debug)]
pub enum AppError {
    /// Configuration error (invalid arguments, unknown workload).
    Config(String),

    /// I/O error when reading/writing a file.
    Io(std::io::Error),

    /// Error when deserializing the input network.
    Deserialize(PathBuf, bincode::Error),

    /// Error when serializing the output network or metrics.
    Serialize(bincode::Error),

    /// Protocol/network error (propagated from ProtocolError, SPEC-06).
    Protocol(ProtocolError),
}

impl AppError {
    pub fn exit_code(&self) -> i32 {
        match self {
            AppError::Config(_) => 1,
            AppError::Io(_) => 1,
            AppError::Deserialize(_, _) => 1,
            AppError::Serialize(_) => 1,
            AppError::Protocol(_) => 2,
        }
    }
}
```

---

## 5. Rationale

### 5.1 Single Binary Instead of Separate Binaries

**Decision:** A single binary `relativist` with subcommands, instead of separate binaries (`relativist-coordinator`, `relativist-worker`, `relativist-local`).

**Rationale:** The single binary simplifies deployment: only one file needs to be copied to each machine. The size overhead is negligible (coordinator, worker, and local mode code share most dependencies). The Haskell prototype uses a similar scheme: a single `benchmark` executable with flags `--mode local|tcp` and `--role coordinator|worker` (AC-003, AC-004). Relativist refines this with `clap` subcommands, which are more ergonomic and self-documenting.

### 5.2 CLI Instead of Configuration File

**Decision:** Configuration exclusively via CLI (no TOML/YAML) in v1.

**Rationale:** For the TCC scope, the number of parameters is small (< 10 per subcommand) and all have sensible defaults. A configuration file would add parsing and validation complexity without clear benefit. The Haskell prototype also uses CLI only (via `optparse-applicative`, the Haskell equivalent of `clap`). If future versions of Relativist require more complex configuration (e.g., multi-cluster, composite partitioning policies), a TOML file can be added.

### 5.3 Local Mode as a Separate Subcommand

**Decision:** Local mode is a separate subcommand (`relativist local`), not a flag on the coordinator (`relativist coordinator --local`).

**Rationale:** Local mode is conceptually different from distributed mode: it does not open sockets, does not serialize messages, does not use tokio for network I/O. Having separate subcommands makes required arguments clear: `coordinator` requires `--port`, `local` does not. Additionally, local mode can execute without the tokio runtime, which simplifies tests and reduces overhead.

### 5.4 Bincode Instead of Textual Format for Networks

**Decision:** The input/output format for networks is binary bincode (`.bin`), not a textual format (JSON, DOT, or custom format).

**Rationale:** Bincode is Relativist's native serialization format (confirmed technical decision, SPEC-02 R22). Using the same format for persistence and communication eliminates additional conversions. **Update (v3):** SPEC-12 extends the format story: the `reduce`, `inspect`, and `generate` subcommands now support three formats (binary `.bin`, text DSL `.ic`, JSON `.json`), while `coordinator`, `worker`, and `local` retain bincode-only for performance. The text DSL enables human inspection and debugging; JSON enables programmatic integration. The original bincode-for-performance rationale still applies to the grid protocol path.

### 5.5 Docker for Reproducibility, Not Required

**Decision:** Include Dockerfile and docker-compose.yml as recommended (SHOULD) but not mandatory.

**Rationale:** Docker serves two purposes for the TCC: (1) isolating the build environment so reviewers can reproduce results without matching Rust toolchain versions, and (2) simplifying multi-worker deployment on a single machine for integration testing. Docker is NOT required for the primary benchmark results (8 physical machines), which use bare-metal deployment for minimal overhead. The Docker networking overhead (bridge network adds ~50-100us latency) is negligible compared to the millisecond-scale round times observed in the Haskell prototype (AC-005), but should be documented in benchmark results. This decision reverses the previous v1 position of excluding Docker entirely, based on the recognition that reproducibility is critical for a TCC: reviewers must be able to run the experiments with `docker compose up`.

### 5.6 No Automatic Discovery

**Decision:** Workers specify the coordinator address manually via `--coordinator HOST:PORT`.

**Rationale:** Automatic discovery (multicast, DNS-SD, Consul, etcd) is a significant infrastructure layer that does not contribute to validating the TCC hypothesis. The Haskell prototype uses the same manual approach (AC-003: `NodeConfig` with explicit `nodeHost` and `nodePort`). For the TCC scenario (8 machines on a local network), the coordinator address is known and static.

### 5.7 Deploy Target Spectrum (DISC-008 v2)

**Decision:** Support four deploy targets from simplest to most realistic: in-process local, Docker-local, Docker-LAN, bare-metal.

**Rationale:** DISC-008 v2 identifies the spectrum from shared memory to distributed memory as the central operational challenge of the TCC. The deploy targets correspond to positions on this spectrum:

- **Local mode** operates at the shared-memory end: no serialization overhead, no network latency, partitions are "distributed" to simulated workers in the same address space. This is the baseline for the Fundamental Property validation and for shared-memory parallelism benchmarks.
- **Docker-local** introduces real TCP (over loopback or Docker bridge) while keeping all processes on one machine. This isolates the effect of serialization + TCP overhead from network latency.
- **Docker-LAN / bare-metal** operates at the distributed end: real network latency, real bandwidth constraints. This is where the overhead decomposition from ARG-004 becomes empirically measurable.

The progression from local to bare-metal allows the TCC to separately measure the costs of serialization (local vs. Docker-local), network latency (Docker-local vs. LAN), and system heterogeneity (Docker vs. bare-metal).

---

## 6. Haskell Prototype Reference

### 6.1 IC.Network: `NodeConfig` and `NodeRole` (AC-003, lines 39-49)

The prototype defines `NodeRole = Coordinator | Worker` and `NodeConfig` with 5 fields (role, host, port, workerCount, logLevel). Relativist:
1. Preserves `NodeRole` and `NodeConfig` with additional fields (timeouts, max_payload_size).
2. Moves `logLevel` to `tracing` configuration via `RUST_LOG` (more idiomatic in Rust).
3. Adds configurable timeouts (absent in the prototype, which blocks indefinitely).

### 6.2 IC.Network: `runCoordinator` and `runWorker` (AC-003, lines 116-152, 310-328)

The Haskell prototype starts the coordinator and worker as functions called by the `benchmark` main:

```haskell
-- coordinator
runCoordinator :: NodeConfig -> Net -> IO (Net, GridMetrics)

-- worker
runWorker :: NodeConfig -> IO ()
```

Relativist replicates the same structure with `run_coordinator_command` and `run_worker_command`, adding:
1. CLI argument parsing (the Haskell prototype hard-codes configuration in `benchmark`).
2. File read/write for networks (the Haskell prototype generates networks in memory).
3. Persistent metrics output in file format (the prototype prints to terminal and writes CSV).

### 6.3 IC.Grid: `runGridLocal` (AC-004, lines ~95-100)

The prototype offers `runGridLocal :: LogLevel -> Net -> Int -> IO (Net, GridMetrics)` as a local simulation mode. Relativist replicates this in the `local` subcommand, which invokes `run_grid` (SPEC-05). The main differences are that Relativist:
1. Can parallelize local workers via `rayon` (the Haskell prototype is sequential: `map workerReduce`).
2. Collects metrics without overhead from `timePure`/`evaluate` (Rust is eager).
3. Reads and writes networks from/to files (the prototype generates in memory).

### 6.4 IC.Benchmark: Workloads (AC-004, `mkTree`, `mkTreeBalanced`)

The prototype defines `mkTree` and `mkTreeBalanced` in `IC.TreeMapReduce` and additional benchmarks (EPAnnihilation, CONDUPExpansion, DualTree) in `IC.Benchmark`. Relativist:
1. Consolidates all workloads in a generation module accessible via `relativist generate`.
2. Associates each workload with a verification function (`verify`) for automated testing.
3. Allows persisting generated networks to `.bin` files for benchmark reproducibility.

### 6.5 IC.Benchmark: CSV Format (AC-004, Section "CSV Format")

The prototype defines two CSV formats (local and distributed, AC-004). Relativist unifies them into a single CSV format with network fields zeroed in local mode (R29). JSON format is added as a more structured alternative.

### 6.6 Deployment Comparison

| Aspect | Haskell Prototype | Relativist |
|--------|-------------------|------------|
| Binary | Single `benchmark` executable | Single `relativist` binary with 7 subcommands |
| Config | Hard-coded in `main` + minimal CLI flags | Full CLI via `clap` with defaults |
| Input | Generated in-memory (`mkTree`) | `.bin` files (grid path); `.bin`/`.ic`/`.json` (utility path, SPEC-12) |
| Output | Terminal print + CSV | `.bin` network + `.json`/`.csv` metrics |
| Logging | `LogLevel` enum, manual `putStrLn` | `tracing` with `RUST_LOG` env var |
| Docker | None | Dockerfile + docker-compose.yml (SHOULD) |
| Deploy script | None | `scripts/deploy.sh` (SHOULD) |
| Timeout | None (blocks indefinitely) | Configurable per-phase timeouts |

---

## 7. Resolved Questions

*All questions resolved during Human Check review (2026-04-03).*

1. **Parallelization in local mode: rayon vs. sequential.** **RESOLVED: Use rayon.** Local mode MUST use `rayon::par_iter` to parallelize partition reduction (R20). Timing MUST measure wall-clock time of the slowest worker (maximum), not the sum, ensuring fair comparison with distributed mode. The `rayon` crate is a direct dependency. Fair comparison is the priority.

2. **Metrics format: summary row or separate file?** **RESOLVED: Separate file (Option B).** Total metrics (total_interactions, total_time, converged) MUST be written to a separate `*_summary.csv` file alongside the per-round `*.csv`. This keeps the per-round CSV clean (one row per round) and the summary self-contained.

3. **`inspect` subcommand for visualization.** **RESOLVED: Elevated to MUST by SPEC-13 R47 and SPEC-12 R27-R31.** The `inspect` subcommand is now a first-class MUST requirement with detailed argument definitions and statistics reporting. See SPEC-12 R27-R31 for the authoritative specification.

4. **`serde_json` dependency.** **RESOLVED: Direct dependency.** The `serde_json` crate is included as a direct dependency (no feature flag). JSON metrics output (R28, R30) is available by default.

5. **Docker Compose worker count synchronization.** **RESOLVED: Health check elevated to SHOULD.** The coordinator SHOULD implement a health-check mechanism that verifies the expected number of workers (`NUM_WORKERS`) have connected before starting the grid loop. Since the TCC assumes a stable, failure-free network, this health check ensures that assumption holds in practice. This is a correctness precondition, not just a convenience issue.
