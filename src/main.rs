//! Relativist CLI — single binary with subcommands.
//!
//! Subcommands (SPEC-07 R1):
//! - `coordinator`: orchestrate distributed reduction
//! - `worker`: connect to coordinator and reduce partitions
//! - `local`: run grid simulation in-process (no TCP)
//! - `generate`: create benchmark networks

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "relativist")]
#[command(about = "Distributed reduction of Interaction Combinators on Grid Computing")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run as coordinator: partition, dispatch, collect, merge.
    Coordinator {
        /// Number of workers to wait for.
        #[arg(long)]
        workers: u32,

        /// TCP port for binding.
        #[arg(long)]
        port: u16,

        /// Bind address.
        #[arg(long, default_value = "0.0.0.0")]
        host: String,

        /// Path to input network file (.bin).
        #[arg(long)]
        net: String,

        /// Maximum grid rounds (unlimited if not set).
        #[arg(long)]
        max_rounds: Option<u32>,

        /// Path to write reduced network.
        #[arg(long)]
        output: Option<String>,

        /// Path to write execution metrics (.json or .csv).
        #[arg(long)]
        metrics: Option<String>,

        /// Partitioning strategy.
        #[arg(long, default_value = "round-robin")]
        strategy: String,
    },

    /// Run as worker: connect to coordinator and reduce assigned partitions.
    Worker {
        /// Address of the coordinator (host:port).
        #[arg(long)]
        coordinator: String,
    },

    /// Run grid simulation locally (no TCP). For testing and baseline benchmarks.
    Local {
        /// Number of simulated workers.
        #[arg(long)]
        workers: u32,

        /// Path to input network file (.bin).
        #[arg(long)]
        net: String,

        /// Maximum rounds (unlimited if not set).
        #[arg(long)]
        max_rounds: Option<u32>,

        /// Path to write reduced network.
        #[arg(long)]
        output: Option<String>,

        /// Path to write execution metrics.
        #[arg(long)]
        metrics: Option<String>,

        /// Partitioning strategy.
        #[arg(long, default_value = "round-robin")]
        strategy: String,
    },

    /// Generate benchmark networks.
    Generate {
        /// Benchmark name (e.g., ep-annihilation, dual-tree, con-dup-expansion).
        #[arg(long)]
        benchmark: String,

        /// Problem size.
        #[arg(long)]
        size: u32,

        /// Output path for the generated network (.bin).
        #[arg(long)]
        output: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Coordinator { .. } => {
            eprintln!("coordinator: not yet implemented");
            std::process::exit(1);
        }
        Commands::Worker { .. } => {
            eprintln!("worker: not yet implemented");
            std::process::exit(1);
        }
        Commands::Local { .. } => {
            eprintln!("local: not yet implemented");
            std::process::exit(1);
        }
        Commands::Generate { .. } => {
            eprintln!("generate: not yet implemented");
            std::process::exit(1);
        }
    }
}
