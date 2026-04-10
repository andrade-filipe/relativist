//! Benchmark suite for Relativist (SPEC-09).
//!
//! Defines the benchmark framework: parametric workloads, execution modes,
//! result collection, statistical aggregation, and CSV output. Used by both
//! the CLI binary (`src/bin/bench.rs`) and Criterion micro-benchmarks.

pub mod benchmarks;
pub mod csv;
pub mod isomorphism;
pub mod memory;
pub mod stats;
pub mod suite;
pub mod validate;

use crate::net::Net;

/// Benchmark identifier (SPEC-09 Section 4.1, R8).
///
/// Each variant maps to a parametric net generator and verification strategy.
/// Display uses snake_case for CSV compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum BenchmarkId {
    EPAnnihilation,
    EPAnnihilationCon,
    EPAnnihilationDup,
    ConDupExpansion,
    DualTree,
    TreeSum,
    TreeSumBalanced,
    MixedNet,
    ErasurePropagation,
    ChurchAdd,
    ChurchMul,
    CascadeCross,
}

impl std::fmt::Display for BenchmarkId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EPAnnihilation => write!(f, "ep_annihilation"),
            Self::EPAnnihilationCon => write!(f, "ep_annihilation_con"),
            Self::EPAnnihilationDup => write!(f, "ep_annihilation_dup"),
            Self::ConDupExpansion => write!(f, "condup_expansion"),
            Self::DualTree => write!(f, "dual_tree"),
            Self::TreeSum => write!(f, "tree_sum"),
            Self::TreeSumBalanced => write!(f, "tree_sum_balanced"),
            Self::MixedNet => write!(f, "mixed_net"),
            Self::ErasurePropagation => write!(f, "erasure_propagation"),
            Self::ChurchAdd => write!(f, "church_add"),
            Self::ChurchMul => write!(f, "church_mul"),
            Self::CascadeCross => write!(f, "cascade_cross"),
        }
    }
}

/// Execution mode (SPEC-09 Section 3.4.3, R6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Mode {
    /// Pure sequential: `reduce_all` without grid.
    Sequential,
    /// Simulated grid in a single process.
    Local,
    /// Grid with separate processes via TCP localhost.
    TcpLocalhost,
    /// Grid with physical machines via real-network TCP.
    TcpNetwork,
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sequential => write!(f, "sequential"),
            Self::Local => write!(f, "local"),
            Self::TcpLocalhost => write!(f, "tcp_localhost"),
            Self::TcpNetwork => write!(f, "tcp_network"),
        }
    }
}

/// Definition of a parametric benchmark (SPEC-09 R2).
///
/// Each benchmark generates input nets of varying sizes, executes
/// sequential and distributed reductions, and verifies correctness
/// of the distributed result against the sequential baseline.
pub trait Benchmark {
    /// Unique identifier of the benchmark.
    fn id(&self) -> BenchmarkId;

    /// Human-readable description for a given size.
    fn describe(&self, size: u32) -> String;

    /// Generate the input net for the specified size.
    fn make_net(&self, size: u32) -> Net;

    /// Default sizes for this benchmark (logarithmic variation, R24).
    fn default_sizes(&self) -> Vec<u32>;

    /// Verify correctness: compare distributed result with sequential result.
    ///
    /// Returns true if the distributed result is correct.
    /// This is the Fundamental Property (G1) verification hook (SPEC-09 R4).
    fn verify(&self, sequential_result: &Net, distributed_result: &Net) -> bool;
}

/// Complete result of a single benchmark execution (SPEC-09 R18).
/// One instance per datapoint: (benchmark, size, workers, mode, repetition).
#[derive(Debug, Clone, serde::Serialize)]
pub struct BenchmarkResult {
    // --- Identification ---
    pub benchmark: BenchmarkId,
    pub input_size: u32,
    pub mode: Mode,
    pub workers: u32,
    pub repetition: u32,

    // --- Correctness ---
    pub correct: bool,

    // --- Timing ---
    pub wall_clock_secs: f64,

    // --- Interactions ---
    pub total_interactions: u64,
    pub mips: f64,
    pub interactions_by_rule: InteractionsByRule,

    // --- Grid metrics ---
    pub rounds: u32,
    pub border_redexes_per_round: Vec<u32>,
    pub border_ratio_per_round: Vec<f64>,

    // --- Memory ---
    pub peak_memory_bytes: u64,
    pub agents_per_round: Vec<usize>,

    // --- Communication ---
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub bytes_sent_per_round: Vec<u64>,
    pub bytes_received_per_round: Vec<u64>,

    // --- Time breakdown per phase ---
    pub partition_time_per_round: Vec<f64>,
    pub compute_time_per_round: Vec<f64>,
    pub merge_time_per_round: Vec<f64>,
    pub network_time_per_round: Vec<f64>,

    // --- Per-worker metrics ---
    pub worker_stats: Vec<Vec<WorkerBenchStats>>,

    // --- Derived metrics ---
    pub speedup: f64,
    pub efficiency: f64,
    pub overhead_ratio: f64,
}

/// Interactions broken down by rule type (SPEC-09 R19).
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct InteractionsByRule {
    pub con_con: u64,
    pub dup_dup: u64,
    pub era_era: u64,
    pub con_dup: u64,
    pub con_era: u64,
    pub dup_era: u64,
}

impl InteractionsByRule {
    /// Sum of all interaction types.
    pub fn total(&self) -> u64 {
        self.con_con + self.dup_dup + self.era_era + self.con_dup + self.con_era + self.dup_era
    }
}

/// Per-worker statistics for a single round (SPEC-09 R19).
#[derive(Debug, Clone, serde::Serialize)]
pub struct WorkerBenchStats {
    pub worker_id: u32,
    pub agents_before: usize,
    pub agents_after: usize,
    pub local_interactions: u64,
    pub compute_time_secs: f64,
    pub idle_time_secs: f64,
}

/// Configuration for a benchmark suite run (SPEC-09 R6).
#[derive(Debug, Clone)]
pub struct BenchmarkSuiteConfig {
    pub benchmarks: Vec<BenchmarkId>,
    pub sizes: Option<Vec<u32>>,
    pub workers: Vec<u32>,
    pub mode: Mode,
    pub warmup_runs: u32,
    pub repetitions: u32,
    pub csv_detail_path: Option<String>,
    pub csv_rounds_path: Option<String>,
    pub csv_summary_path: Option<String>,
    pub max_rounds: Option<u32>,
    /// Strict BSP mode forwarded to GridConfig (SPEC-05 R30a).
    /// When true, the grid loop does not reduce border cascades at the
    /// coordinator, exposing multi-round BSP behavior required for Phase 3
    /// LAN measurements.
    pub strict_bsp: bool,
    /// When true, replace the full isomorphism check with a symbol-count
    /// fast check (L3 mitigation — see PHASE1-FINDINGS.md). A pass is
    /// recorded as "G1 weak" rather than "G1 strong".
    pub skip_g1: bool,
}

/// Aggregated statistics across repetitions (SPEC-09 R31-R34).
#[derive(Debug, Clone, serde::Serialize)]
pub struct AggregatedStats {
    pub benchmark: BenchmarkId,
    pub input_size: u32,
    pub mode: Mode,
    pub workers: u32,
    pub repetitions: u32,
    pub all_correct: bool,
    pub wall_clock_mean: f64,
    pub wall_clock_std: f64,
    pub wall_clock_median: f64,
    pub wall_clock_min: f64,
    pub wall_clock_max: f64,
    pub mips_mean: f64,
    pub speedup_mean: f64,
    pub efficiency_mean: f64,
    pub overhead_ratio_mean: f64,
    pub cv: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- T1: BenchmarkId Display (snake_case) ---
    #[test]
    fn test_benchmark_id_display() {
        assert_eq!(BenchmarkId::EPAnnihilation.to_string(), "ep_annihilation");
        assert_eq!(
            BenchmarkId::EPAnnihilationCon.to_string(),
            "ep_annihilation_con"
        );
        assert_eq!(
            BenchmarkId::EPAnnihilationDup.to_string(),
            "ep_annihilation_dup"
        );
        assert_eq!(BenchmarkId::ConDupExpansion.to_string(), "condup_expansion");
        assert_eq!(BenchmarkId::DualTree.to_string(), "dual_tree");
        assert_eq!(BenchmarkId::TreeSum.to_string(), "tree_sum");
        assert_eq!(
            BenchmarkId::TreeSumBalanced.to_string(),
            "tree_sum_balanced"
        );
        assert_eq!(BenchmarkId::MixedNet.to_string(), "mixed_net");
        assert_eq!(
            BenchmarkId::ErasurePropagation.to_string(),
            "erasure_propagation"
        );
        assert_eq!(BenchmarkId::ChurchAdd.to_string(), "church_add");
        assert_eq!(BenchmarkId::ChurchMul.to_string(), "church_mul");
        assert_eq!(BenchmarkId::CascadeCross.to_string(), "cascade_cross");
    }

    // --- T2: Mode Display ---
    #[test]
    fn test_mode_display() {
        assert_eq!(Mode::Sequential.to_string(), "sequential");
        assert_eq!(Mode::Local.to_string(), "local");
        assert_eq!(Mode::TcpLocalhost.to_string(), "tcp_localhost");
        assert_eq!(Mode::TcpNetwork.to_string(), "tcp_network");
    }

    // --- T3: BenchmarkId is Copy ---
    #[test]
    fn test_benchmark_id_is_copy() {
        let a = BenchmarkId::DualTree;
        let b = a; // Copy, not move
        assert_eq!(a, b);
    }

    // --- T4: BenchmarkId serde round-trip ---
    #[test]
    fn test_benchmark_id_serde_roundtrip() {
        let original = BenchmarkId::TreeSum;
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: BenchmarkId = serde_json::from_str(&json).unwrap();
        assert_eq!(original, deserialized);
    }

    // --- T5: BenchmarkId variant count (R8: at least 10) ---
    #[test]
    fn test_benchmark_id_variant_count() {
        let all = [
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
        ];
        assert_eq!(all.len(), 12);
    }

    // --- Edge case: Display != Debug ---
    #[test]
    fn test_display_is_not_debug() {
        let id = BenchmarkId::EPAnnihilation;
        let display = format!("{}", id);
        let debug = format!("{:?}", id);
        assert_ne!(display, debug);
        assert_eq!(display, "ep_annihilation");
        assert_eq!(debug, "EPAnnihilation");
    }

    // --- Edge case: Hash consistency ---
    #[test]
    fn test_benchmark_id_hash_consistency() {
        use std::collections::HashMap;
        let mut map = HashMap::new();
        map.insert(BenchmarkId::DualTree, 42);
        assert_eq!(map[&BenchmarkId::DualTree], 42);
    }

    // --- TASK-0183: BenchmarkResult and metric structs ---

    fn make_sample_result() -> BenchmarkResult {
        BenchmarkResult {
            benchmark: BenchmarkId::EPAnnihilation,
            input_size: 100,
            mode: Mode::Sequential,
            workers: 1,
            repetition: 0,
            correct: true,
            wall_clock_secs: 0.001,
            total_interactions: 100,
            mips: 100.0,
            interactions_by_rule: InteractionsByRule::default(),
            rounds: 0,
            border_redexes_per_round: vec![],
            border_ratio_per_round: vec![],
            peak_memory_bytes: 0,
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
        }
    }

    // T1: Construct BenchmarkResult
    #[test]
    fn test_benchmark_result_construction() {
        let r = make_sample_result();
        assert_eq!(r.benchmark, BenchmarkId::EPAnnihilation);
        assert_eq!(r.input_size, 100);
        assert!(r.correct);
    }

    // T2: InteractionsByRule default is all zeros
    #[test]
    fn test_interactions_by_rule_default() {
        let ibr = InteractionsByRule::default();
        assert_eq!(ibr.con_con, 0);
        assert_eq!(ibr.dup_dup, 0);
        assert_eq!(ibr.era_era, 0);
        assert_eq!(ibr.con_dup, 0);
        assert_eq!(ibr.con_era, 0);
        assert_eq!(ibr.dup_era, 0);
    }

    // T3: BenchmarkResult serializes to JSON
    #[test]
    fn test_benchmark_result_serialize() {
        let r = make_sample_result();
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"benchmark\""));
        assert!(json.contains("\"wall_clock_secs\""));
        assert!(json.contains("\"interactions_by_rule\""));
    }

    // T4: WorkerBenchStats construction
    #[test]
    fn test_worker_bench_stats() {
        let ws = WorkerBenchStats {
            worker_id: 0,
            agents_before: 100,
            agents_after: 50,
            local_interactions: 50,
            compute_time_secs: 0.005,
            idle_time_secs: 0.001,
        };
        assert_eq!(ws.worker_id, 0);
        assert_eq!(ws.agents_before, 100);
    }

    // T5: InteractionsByRule total
    #[test]
    fn test_interactions_by_rule_total() {
        let ibr = InteractionsByRule {
            con_con: 10,
            dup_dup: 5,
            era_era: 3,
            con_dup: 7,
            con_era: 2,
            dup_era: 1,
        };
        assert_eq!(ibr.total(), 28);
    }

    // Edge case: empty per-round vectors serialize correctly
    #[test]
    fn test_empty_rounds_serialize() {
        let r = make_sample_result();
        assert_eq!(r.rounds, 0);
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"border_redexes_per_round\":[]"));
    }
}
