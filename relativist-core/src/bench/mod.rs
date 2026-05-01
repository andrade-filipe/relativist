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
pub mod streaming;
pub mod suite;
pub mod validate;

use crate::net::Net;
use crate::partition::streaming::AgentBatch;

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
    /// SPEC-09 R17d: demonstrative sum-of-squares via composed mul/add.
    /// NOT part of frozen performance campaigns.
    ChurchSumOfSquares,
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
            Self::ChurchSumOfSquares => write!(f, "church_sum_of_squares"),
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
    ///
    /// This is the **source-of-truth** materialization path (SPEC-21 R11).
    /// It MUST remain a required method so that the `make_net_stream` default
    /// impl has a reliable fallback. Do NOT add a default impl here.
    fn make_net(&self, size: u32) -> Net;

    /// Generate the net as a streaming iterator of `AgentBatch` values.
    ///
    /// # Default implementation (SPEC-21 R10)
    ///
    /// The default wraps `make_net(size)` in a **single `AgentBatch`** containing
    /// all agents and their resolved connections. This is memory-equivalent to v1
    /// (the full net is materialized first) but preserves the streaming API so
    /// that all 13 existing implementations remain valid without changes (closes
    /// SC-008).
    ///
    /// Generators that benefit from native streaming (e.g., `ep_annihilation`,
    /// R12 MUST) SHOULD override this method to avoid the materialize-then-wrap
    /// memory cost.
    ///
    /// # Chunk-size argument in the default path
    ///
    /// For the default impl, `chunk_size` is **ignored** — the net is always
    /// emitted as a single batch. The argument exists only for API uniformity
    /// with native streaming overrides.
    ///
    /// # Isomorphism contract (R11 / T6)
    ///
    /// `make_net_stream(size, chunk_size).collect()` MUST be agent-isomorphic
    /// to `make_net(size)` for all `chunk_size` values (verified by T6 in
    /// TASK-0567).
    fn make_net_stream(
        &self,
        size: u32,
        _chunk_size: usize,
    ) -> Box<dyn Iterator<Item = AgentBatch>> {
        streaming::default_chunked_iter(self.make_net(size))
    }

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
    /// SPEC-09 R18a (D-011 Phase F-1, commit `82b2d27`): VmHWM sampled at the
    /// construction-complete program point — AFTER net materialization /
    /// chunked partition pipeline returns, BEFORE the first `reduce_all` /
    /// `run_grid` invocation. On non-Linux targets returns `0`.
    ///
    /// Per SPEC-09 §4.9 the legacy `peak_memory_bytes` field is preserved for
    /// backward compatibility with `v1_local_baseline`; the new field
    /// MUST always be populated (even for v1-equivalent rodadas — the column
    /// MUST NOT be omitted, per §4.9 line ~714).
    pub peak_memory_during_construction: u64,
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

/// Recycling policy for the partition free-list under the streaming +
/// delta path (SPEC-22 R10b). Selected at runtime by the bench harness
/// (TASK-0602 / TASK-0603 / TASK-0604).
///
/// - `DisableUnderDelta` — the safe default: free-list recycling is
///   suppressed for the duration of any delta-mode round under streaming
///   dispatch. Trades memory for simplicity / safety.
/// - `BorderClean` — recycle slots whose only post-deletion connections
///   were border ports already promoted into the BorderGraph. SPEC-22
///   R10b describes the correctness conditions.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    clap::ValueEnum,
)]
#[serde(rename_all = "kebab-case")]
#[clap(rename_all = "kebab-case")]
pub enum RecyclePolicy {
    /// SPEC-22 R10b: safe default — preserves the v1 semantics under
    /// streaming + delta. Matches the plan in
    /// `docs/plans/2026-04-30-d-011-tier3-hardening-plus-bench-enablement.md`.
    #[default]
    DisableUnderDelta,
    BorderClean,
}

/// Net representation selected by the bench harness during net
/// construction (SPEC-22 R12 sparse-net path; default Dense).
///
/// - `Dense` — the canonical contiguous arena (`Net`) used by every v1
///   benchmark. Status quo for all currently-frozen baselines.
/// - `Sparse` — the SPEC-22 sparse-net path; selected by the bench
///   harness when the workload's natural construction path is sparse
///   (e.g., `dual_tree`).
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    clap::ValueEnum,
)]
#[serde(rename_all = "kebab-case")]
#[clap(rename_all = "kebab-case")]
pub enum NetRepresentation {
    /// SPEC-22 R12: `Dense` is the v1 status quo; the sparse path is opt-in
    /// (TASK-0606). Default to preserve all currently-frozen baselines.
    #[default]
    Dense,
    Sparse,
}

/// Default value for `BenchmarkSuiteConfig::max_pending_lifetime`.
/// SPEC-21 R37g pending-store memory bound. Mirrors
/// `merge::types::default_max_pending_lifetime`.
pub const DEFAULT_BENCH_MAX_PENDING_LIFETIME: u32 = 16;

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
    /// Tier 3 (SPEC-09 R18a, SPEC-21 §3.8 A3). `None` selects the eager
    /// path (status quo); `Some(N)` selects streaming with chunk size N.
    pub chunk_size: Option<u32>,
    /// Tier 3 (SPEC-21 R37g). Pending-store memory bound for the
    /// streaming path. Default: 16, matching the coordinator CLI default
    /// in `merge::types::default_max_pending_lifetime`.
    pub max_pending_lifetime: u32,
    /// Tier 3 (SPEC-22 R10b). Free-list recycling policy under the
    /// streaming + delta path. Default: `DisableUnderDelta` (safe).
    pub recycle_policy: RecyclePolicy,
    /// Tier 3 (SPEC-22 R12). Net representation during construction.
    /// Default: `Dense` (v1 status quo).
    pub representation: NetRepresentation,
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

    // ---------------------------------------------------------------------------
    // TASK-0602 — Tier 3 fields on BenchmarkSuiteConfig (Phase C-1, P0)
    // ---------------------------------------------------------------------------
    //
    // Per TEST-SPEC-0602:
    //   UT-0602-01 — defaults match the eager-path status quo
    //   UT-0602-02 — RecyclePolicy enum traits + variants
    //   UT-0602-03 — NetRepresentation enum traits + variants
    //   UT-0602-04 — `Clone` + `Debug` round-trip on the struct preserves the
    //                4 new Tier 3 fields
    //
    // Plus dispatch additions (per TASK-0602 dispatch prompt):
    //   - serde round-trip for both enums (the enums DO derive Serialize +
    //     Deserialize even though `BenchmarkSuiteConfig` itself does not)
    //   - parse from CLI string forms used by clap value_enum
    //     ("disable-under-delta", "border-clean", "dense", "sparse")

    /// Build a `BenchmarkSuiteConfig` with all non-Tier-3 fields set to
    /// minimal/eager defaults. The Tier 3 fields are populated from the
    /// public spec-defaults so UT-0602-01 can read them back.
    fn make_default_suite_config() -> BenchmarkSuiteConfig {
        BenchmarkSuiteConfig {
            benchmarks: vec![BenchmarkId::EPAnnihilation],
            sizes: None,
            workers: vec![1],
            mode: Mode::Sequential,
            warmup_runs: 0,
            repetitions: 1,
            csv_detail_path: None,
            csv_rounds_path: None,
            csv_summary_path: None,
            max_rounds: None,
            strict_bsp: false,
            skip_g1: false,
            // Tier 3 — mirrors what TASK-0603 will populate from CLI defaults
            // and what the BenchmarkSuiteConfig spec mandates as the
            // eager-path status quo.
            chunk_size: None,
            max_pending_lifetime: DEFAULT_BENCH_MAX_PENDING_LIFETIME,
            recycle_policy: RecyclePolicy::default(),
            representation: NetRepresentation::default(),
        }
    }

    /// UT-0602-01 — Lock the exact default values of the 4 Tier 3 fields.
    /// Defaults MUST preserve the eager-path status quo (`chunk_size: None`,
    /// `representation: Dense`); a silent drift here would invalidate every
    /// frozen `v1_local_baseline` comparison.
    #[test]
    fn benchmark_suite_config_default_values() {
        let config = make_default_suite_config();

        assert_eq!(
            config.chunk_size, None,
            "UT-0602-01 R18a: default chunk_size must be None (eager path)"
        );
        assert_eq!(
            config.max_pending_lifetime, 16,
            "UT-0602-01 R37g: default max_pending_lifetime must be 16"
        );
        assert_eq!(
            config.recycle_policy,
            RecyclePolicy::DisableUnderDelta,
            "UT-0602-01 R10b: default recycle_policy must be DisableUnderDelta"
        );
        assert_eq!(
            config.representation,
            NetRepresentation::Dense,
            "UT-0602-01 R12: default representation must be Dense (v1 status quo)"
        );
    }

    /// UT-0602-02 — `RecyclePolicy` derives + variant identity.
    #[test]
    fn recycle_policy_enum_traits_and_variants() {
        // PartialEq distinguishes the two variants.
        assert_ne!(
            RecyclePolicy::DisableUnderDelta,
            RecyclePolicy::BorderClean,
            "UT-0602-02: PartialEq must distinguish variants"
        );
        // Eq: identity holds.
        assert_eq!(
            RecyclePolicy::DisableUnderDelta,
            RecyclePolicy::DisableUnderDelta
        );
        // Copy + Clone compile (the binding is reusable after copying).
        // We invoke `Clone::clone` explicitly here so a regression that
        // removes the derive (e.g., a manual `Clone` that drops the new
        // variants) would fail to compile. `#[allow(clippy::clone_on_copy)]`
        // is required because `RecyclePolicy: Copy` and clippy would
        // otherwise suggest removing the redundant `.clone()` — but the
        // explicit call IS the trait-presence check.
        let a = RecyclePolicy::BorderClean;
        let b = a;
        #[allow(clippy::clone_on_copy)]
        let _c = a.clone();
        assert_eq!(a, b);
        // Debug renders the variant name.
        assert_eq!(format!("{:?}", RecyclePolicy::BorderClean), "BorderClean");
        assert_eq!(
            format!("{:?}", RecyclePolicy::DisableUnderDelta),
            "DisableUnderDelta"
        );
        // Exhaustive match over RecyclePolicy compiles with exactly 2 arms.
        // Compile-time check; the runtime assertion just exercises both arms.
        let count = match RecyclePolicy::DisableUnderDelta {
            RecyclePolicy::DisableUnderDelta => 1u32,
            RecyclePolicy::BorderClean => 2u32,
        };
        assert_eq!(count, 1u32);
    }

    /// UT-0602-02 (extension per dispatch prompt): serde round-trip and
    /// kebab-case CLI parsing for `RecyclePolicy`.
    #[test]
    fn recycle_policy_serde_and_kebab_case_parsing() {
        use clap::ValueEnum;

        // serde Serialize/Deserialize round-trip via JSON.
        for variant in [RecyclePolicy::DisableUnderDelta, RecyclePolicy::BorderClean] {
            let json = serde_json::to_string(&variant)
                .expect("RecyclePolicy must Serialize via serde_json");
            let back: RecyclePolicy = serde_json::from_str(&json)
                .expect("RecyclePolicy must Deserialize round-trip via serde_json");
            assert_eq!(
                back, variant,
                "UT-0602-02: serde round-trip must preserve variant"
            );
        }

        // clap kebab-case parsing — these strings appear on the CLI surface
        // landed by TASK-0603. Spec-mandated forms.
        let parsed = RecyclePolicy::from_str("disable-under-delta", false)
            .expect("UT-0602-02: 'disable-under-delta' must parse via clap value_enum");
        assert_eq!(parsed, RecyclePolicy::DisableUnderDelta);

        let parsed = RecyclePolicy::from_str("border-clean", false)
            .expect("UT-0602-02: 'border-clean' must parse via clap value_enum");
        assert_eq!(parsed, RecyclePolicy::BorderClean);

        // Snake-case form MUST NOT parse (would violate the clap rename rule).
        assert!(
            RecyclePolicy::from_str("disable_under_delta", false).is_err(),
            "UT-0602-02: snake_case must NOT be accepted (kebab-case is mandated by spec)"
        );
    }

    /// UT-0602-03 — `NetRepresentation` derives + variant identity.
    #[test]
    fn net_representation_enum_traits_and_variants() {
        assert_ne!(
            NetRepresentation::Dense,
            NetRepresentation::Sparse,
            "UT-0602-03: PartialEq must distinguish variants"
        );
        let a = NetRepresentation::Dense;
        let b = a;
        #[allow(clippy::clone_on_copy)]
        let _c = a.clone();
        assert_eq!(a, b);
        assert_eq!(format!("{:?}", NetRepresentation::Sparse), "Sparse");
        assert_eq!(format!("{:?}", NetRepresentation::Dense), "Dense");
    }

    /// UT-0602-03 (extension per dispatch prompt): serde round-trip and
    /// kebab-case CLI parsing for `NetRepresentation`.
    #[test]
    fn net_representation_serde_and_kebab_case_parsing() {
        use clap::ValueEnum;

        for variant in [NetRepresentation::Dense, NetRepresentation::Sparse] {
            let json = serde_json::to_string(&variant)
                .expect("NetRepresentation must Serialize via serde_json");
            let back: NetRepresentation = serde_json::from_str(&json)
                .expect("NetRepresentation must Deserialize round-trip via serde_json");
            assert_eq!(
                back, variant,
                "UT-0602-03: serde round-trip must preserve variant"
            );
        }

        let parsed = NetRepresentation::from_str("dense", false)
            .expect("UT-0602-03: 'dense' must parse via clap value_enum");
        assert_eq!(parsed, NetRepresentation::Dense);

        let parsed = NetRepresentation::from_str("sparse", false)
            .expect("UT-0602-03: 'sparse' must parse via clap value_enum");
        assert_eq!(parsed, NetRepresentation::Sparse);
    }

    /// UT-0602-04 — `Clone` + `Debug` on `BenchmarkSuiteConfig` preserve the
    /// 4 Tier 3 fields. Catches a buggy manual `Clone` that orphans new fields.
    #[test]
    fn struct_clone_and_debug_round_trip_preserves_tier3_fields() {
        let mut config = make_default_suite_config();
        config.chunk_size = Some(123);
        config.max_pending_lifetime = 42;
        config.recycle_policy = RecyclePolicy::BorderClean;
        config.representation = NetRepresentation::Sparse;

        let cloned = config.clone();
        let debug_str = format!("{:?}", config);

        assert_eq!(
            cloned.chunk_size,
            Some(123),
            "UT-0602-04: Clone must preserve chunk_size"
        );
        assert_eq!(
            cloned.max_pending_lifetime, 42,
            "UT-0602-04: Clone must preserve max_pending_lifetime"
        );
        assert_eq!(
            cloned.recycle_policy,
            RecyclePolicy::BorderClean,
            "UT-0602-04: Clone must preserve recycle_policy"
        );
        assert_eq!(
            cloned.representation,
            NetRepresentation::Sparse,
            "UT-0602-04: Clone must preserve representation"
        );

        assert!(
            debug_str.contains("chunk_size: Some(123)"),
            "UT-0602-04: Debug must render chunk_size; got {debug_str}"
        );
        assert!(
            debug_str.contains("max_pending_lifetime: 42"),
            "UT-0602-04: Debug must render max_pending_lifetime; got {debug_str}"
        );
        assert!(
            debug_str.contains("BorderClean"),
            "UT-0602-04: Debug must render recycle_policy variant; got {debug_str}"
        );
        assert!(
            debug_str.contains("Sparse"),
            "UT-0602-04: Debug must render representation variant; got {debug_str}"
        );
    }

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
        assert_eq!(
            BenchmarkId::ChurchSumOfSquares.to_string(),
            "church_sum_of_squares"
        );
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

    // --- T5: BenchmarkId variant count (R8: at least 12) ---
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
            BenchmarkId::ChurchSumOfSquares,
        ];
        assert_eq!(all.len(), 13);
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
            peak_memory_during_construction: 0,
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
