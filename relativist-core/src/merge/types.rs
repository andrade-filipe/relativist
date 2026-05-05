//! Types for merge and grid cycle (SPEC-05, Section 4.1).
//!
//! GridMetrics, WorkerRoundStats, and GridConfig are pure data structures
//! with no logic. They are used by run_grid and the merge function.

use std::time::Duration;

use crate::error::GridError;
use crate::merge::border_graph::{BorderDelta, MintedAgent};
use crate::merge::border_resolver::RoundStartDispatch;
use crate::partition::{Partition, PartitionPlan, WorkerId};

/// Metrics collected during grid loop execution (SPEC-05, R34-R35, R35a).
///
/// Accumulates per-round data for experimental analysis (SPEC-09).
/// Inspired by GridMetrics from the Haskell prototype (AC-004).
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct GridMetrics {
    /// Total number of rounds executed.
    pub rounds: u32,

    /// Sum of all interactions (local + border) across all rounds.
    pub total_interactions: u64,

    /// Per-rule interaction totals across all rounds and all sources
    /// (workers + border resolution):
    /// [CON-CON, CON-DUP, CON-ERA, DUP-DUP, DUP-ERA, ERA-ERA].
    /// Required by SPEC-11 R12 for Prometheus `interactions_by_rule_total`.
    pub total_interactions_by_rule: [u64; 6],

    /// Local worker interactions per round.
    pub local_interactions_per_round: Vec<u64>,

    /// Border interactions (coordinator, after merge) per round.
    pub border_interactions_per_round: Vec<u64>,

    /// Border redexes detected by merge per round.
    pub border_redexes_per_round: Vec<u32>,

    /// Number of live agents at the start of each round.
    pub agents_per_round: Vec<usize>,

    /// Partitioning time per round.
    pub partition_time_per_round: Vec<Duration>,

    /// Local reduction time (all workers) per round.
    /// In local simulation, includes rebuild_free_port_index unless
    /// index_rebuild_time_per_round is separately tracked.
    pub compute_time_per_round: Vec<Duration>,

    /// Structural merge time per round (excludes border resolution).
    pub merge_time_per_round: Vec<Duration>,

    /// Time for reduce_all after merge per round (border resolution).
    pub border_reduce_time_per_round: Vec<Duration>,

    /// Time for rebuild_free_port_index per round (SHOULD, R35a).
    /// Enables accurate overhead decomposition for SPEC-09 benchmarks.
    pub index_rebuild_time_per_round: Vec<Duration>,

    // --- Network metrics (SPEC-06 R33-R34, populated in distributed mode) ---
    /// Total bytes sent by the coordinator per round (headers + payloads).
    pub bytes_sent_per_round: Vec<usize>,

    /// Total bytes received by the coordinator per round (headers + payloads).
    pub bytes_received_per_round: Vec<usize>,

    /// Wall-clock time to send all partitions per round.
    pub network_send_time_per_round: Vec<Duration>,

    /// Wall-clock time to collect all results per round.
    pub network_recv_time_per_round: Vec<Duration>,

    /// Wall-clock total execution time.
    pub total_time: Duration,

    /// Did the grid converge to Normal Form?
    /// false if max_rounds was reached before convergence.
    pub converged: bool,

    /// Per-worker statistics, per round (populated in distributed context).
    pub worker_stats_per_round: Vec<Vec<WorkerRoundStats>>,

    /// SPEC-19 §3.1 R4 (TASK-0350): number of rounds in which the
    /// merge phase was skipped because every worker reported
    /// `has_border_activity == false`.
    ///
    /// Only the v2 coordinator-free path (TASK-0351, gated by
    /// `GridConfig::coordinator_free_rounds`) increments this counter.
    /// In v1 / default mode this remains `0` for the entire run.
    pub coordinator_free_rounds: u32,

    /// SPEC-19 R20 (TASK-0384): `true` iff this run used the delta BSP
    /// loop (`run_grid_delta`). `false` for v1 `run_grid`. Populated at
    /// the entry point; immutable after that. Larger delta-specific
    /// metric extensions (per-round delta byte volumes, border-resolution
    /// timings) belong to sub-bundle 2.26-D — this minimal marker is
    /// the only field TASK-0384 adds.
    pub delta_mode: bool,

    /// SPEC-19 R30 (TASK-0388): populated only in delta mode; records
    /// whether `run_grid_delta` hit the `max_rounds` cap without
    /// reaching Global Normal Form. `None` in v1 mode or when
    /// convergence was reached naturally.
    pub delta_max_rounds_hit: Option<bool>,

    // --- Elastic grid metrics (SPEC-20 R38, R38a; TASK-0450) ---
    //
    // R38a Audit (SC-019/NF-004): Non-collision with SPEC-19 R45 fields.
    // SPEC-19 Prefixes: bytes_, network_, compute_, agents_, partition_, ...
    // SPEC-20 Prefixes: workers_, effective_, partitions_, retained_, join_round_
    //
    // All SPEC-20 telemetry fields are strictly prefix-disjoint from SPEC-19
    // ensuring clean additive serialization for CSV/JSON consumers.
    /// Count of workers that successfully joined in each round's join window.
    pub workers_joined_per_round: Vec<u32>,

    /// Count of workers that departed (detected or graceful) in each round.
    pub workers_departed_per_round: Vec<u32>,

    /// Effective slot count (K_eff) at the start of each round's partitioning.
    pub effective_slots_per_round: Vec<u32>,

    /// Number of partitions that had to be redispatched due to mid-round departure.
    pub partitions_redispatched_per_round: Vec<u32>,

    /// Count of partitions materialized from retained_initial (conservative path).
    pub retained_initial_reclaims_per_round: Vec<u32>,

    /// Count of partitions materialized from retained_last_acked (optimized path).
    pub retained_last_acked_reclaims_per_round: Vec<u32>,

    /// Cumulative time spent in rejoin round-trip handshakes (milliseconds).
    pub join_round_overhead_ms_per_round: Vec<u64>,

    /// Wall-clock duration of the elastic join window per round (TASK-0450 / QA-001).
    ///
    /// Distinct from `merge_time_per_round` (structural merge timing). Only
    /// populated when `GridConfig::elastic_join` is true; remains empty in
    /// non-elastic mode. Spec intent (SPEC-20 R38 spirit): the join window
    /// is its own observable, not a contamination of the merge timer.
    pub join_window_time_per_round: Vec<Duration>,
}

impl GridMetrics {
    /// Returns the total bytes transferred across all rounds (sent + received).
    /// SPEC-06, Section 4.10.
    pub fn total_network_bytes(&self) -> usize {
        self.bytes_sent_per_round.iter().sum::<usize>()
            + self.bytes_received_per_round.iter().sum::<usize>()
    }

    /// Returns the communication overhead as a fraction of total time.
    /// Formula: (sum(send_time) + sum(recv_time)) / total_time
    /// Cf. DISC-006 v2, Section 1.1; SPEC-06 R35.
    pub fn network_overhead_fraction(&self) -> f64 {
        let send_total: Duration = self.network_send_time_per_round.iter().sum();
        let recv_total: Duration = self.network_recv_time_per_round.iter().sum();
        let network_total = send_total + recv_total;
        if self.total_time.is_zero() {
            0.0
        } else {
            network_total.as_secs_f64() / self.total_time.as_secs_f64()
        }
    }
}

/// Statistics of a single worker in a specific round.
/// Canonical definition: SPEC-05 R37. Resolves SPEC-11 OQ-1.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(
    feature = "zero-copy",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
pub struct WorkerRoundStats {
    /// Identifier of the worker.
    pub worker_id: WorkerId,
    /// Number of live agents in the partition before local reduction.
    pub agents_before: usize,
    /// Number of live agents in the partition after local reduction.
    pub agents_after: usize,
    /// Number of local redexes reduced by this worker.
    pub local_redexes: usize,
    /// Wall-clock duration of reduce_all for this worker (seconds).
    pub reduce_duration_secs: f64,
    /// Per-rule interaction counts:
    /// [CON-CON, CON-DUP, CON-ERA, DUP-DUP, DUP-ERA, ERA-ERA].
    pub interactions_by_rule: [u64; 6],
    /// SPEC-19 §3.1 R1, R2: `true` iff at least one entry of
    /// `partition.free_port_index` points at a principal port
    /// (`AgentPort(_, 0)`) after local reduction. Computed by
    /// `merge::helpers::compute_border_activity`.
    ///
    /// IC concept: a "principal port" is the active interaction port.
    /// When a local border endpoint is principal, a future merge could
    /// produce a border redex if the remote side is also principal.
    /// When **no** worker has a principal-port border endpoint, no
    /// border redex can fire this round — the coordinator can safely
    /// skip the merge-redistribute cycle (R3, R5 strong confluence).
    ///
    /// The field is part of the bincode v2 wire payload of
    /// `Message::PartitionResult` (R7 — additive, no new variant).
    pub has_border_activity: bool,

    /// SPEC-20 R7, R38b: True if this worker is the coordinator itself
    /// (hybrid mode).
    #[serde(default)]
    pub is_coordinator_self: bool,
}

/// SPEC-19 R26 (TASK-0384): pure-core mirror of `Message::RoundResult`'s
/// payload plus the `worker_id` that sent it. Used by `WorkerDispatch`
/// implementations to return collected worker reports from a single
/// round without dragging the wire-level `Message` enum into pure-core.
///
/// Kept separate from `Message::RoundResult` (in `protocol/`) so that
/// `merge/` — per the SPEC-13 dependency direction `net → reduction →
/// partition → merge → protocol → coordinator/worker` — does NOT back-
/// reference protocol types. The `From` / `TryFrom` bridging between
/// `Message::RoundResult` and `RoundResultPayload` lives in `protocol/`
/// (OUT of this bundle).
#[derive(Debug, Clone)]
#[allow(dead_code)] // TASK-0385+ wires callers; kept scaffolded to anchor the trait signature.
pub(crate) struct RoundResultPayload {
    pub(crate) worker_id: WorkerId,
    pub(crate) round: u32,
    pub(crate) border_deltas: Vec<BorderDelta>,
    pub(crate) stats: WorkerRoundStats,
    pub(crate) has_border_activity: bool,
    /// SPEC-19 §3.3 R26 / DC-B5 (TASK-0398 — D-004 closure): the
    /// worker-side mint echo. Each entry pairs the coordinator-issued
    /// `request_id` (encoded via
    /// [`crate::merge::border_resolver::encode_request_id`]) with the
    /// AgentId the worker allocated from its `id_range`. The
    /// coordinator's round-N+2 finalizer
    /// [`crate::merge::BorderGraph::register_minted_agents`] consumes
    /// this field to resolve `PendingPortRef::Pending` tokens in
    /// `pending_new_borders` and promote fully-resolved entries to
    /// `AddBorderEntry`s via `add_border_states`.
    ///
    /// Closes the MF-003 gap flagged by TASK-0395's sub-agent during
    /// DEV of 2026-04-23: wire-level `Message::RoundResult.minted_agents`
    /// (shipped 2.26-A) used to be silently dropped by the pure-core
    /// bridge; this field reinstates the echo.
    pub(crate) minted_agents: Vec<MintedAgent>,
}

/// SPEC-19 R20, R21 (TASK-0384, DC-C2 option (c) ratified 2026-04-17):
/// abstraction over the actual I/O path for delta-mode coordination.
///
/// Two concrete implementations are anticipated:
/// - An async tokio/TCP wrapper used by the real distributed coordinator
///   (lives in `protocol/coordinator.rs` — OUT of this bundle; binds the
///   synchronous trait to an async transport via `block_on`).
/// - An in-process `LocalDeltaDispatch` used by integration tests and
///   benchmarks (lives behind tests until needed).
///
/// **Pure-core discipline:** this trait MUST NOT require `Send + Sync`
/// or any async-related supertraits — that would force `futures` /
/// `tokio` imports into the pure-core layer, breaking SPEC-13 R6-R8.
/// Concrete async implementations wrap the runtime behind a synchronous
/// `block_on` facade; see `PartitionStrategy` precedent.
#[allow(dead_code)] // TASK-0385+ wires coordinator callers; tests already cover object-safety.
pub(crate) trait WorkerDispatch {
    /// SPEC-19 R21 phase 1 (Round 0): send `Message::InitialPartition`
    /// to every worker carrying its slice of the `PartitionPlan`. The
    /// coordinator does NOT wait for acks (DC-C1 option (b) locked-in
    /// 2026-04-17 — arrival is implicit from the first `RoundResult`).
    fn dispatch_initial(&mut self, plan: &PartitionPlan) -> Result<(), GridError>;

    /// SPEC-19 R21 phase 2 (Rounds 1+): send `Message::RoundStart` to
    /// every worker carrying its per-worker payload and block until all
    /// `Message::RoundResult`s arrive. Output length equals the number
    /// of workers; ordering is by `WorkerId` ascending.
    fn dispatch_round_start(
        &mut self,
        dispatch: &[(WorkerId, RoundStartDispatch)],
    ) -> Result<Vec<RoundResultPayload>, GridError>;

    /// SPEC-19 R21 phase 3 (Final State Collection): send
    /// `Message::FinalStateRequest` to every worker and collect their
    /// `Message::FinalStateResult` payloads. Output length equals the
    /// number of workers; ordering is by `WorkerId` ascending.
    fn dispatch_final_state_request(&mut self, round: u32) -> Result<Vec<Partition>, GridError>;
}

/// Configuration for the grid loop (SPEC-05, R25, R29, R30a; SPEC-19 §3.6).
///
/// The partition strategy is NOT stored here because trait objects
/// are not Clone. It is passed as a separate parameter to `run_grid`.
///
/// # Examples
///
/// Opt into the delta protocol from a builder pattern (SPEC-19 §3.6 R41):
///
/// ```
/// use relativist_core::merge::GridConfig;
///
/// let cfg = GridConfig {
///     num_workers: 4,
///     delta_mode: true,
///     ..GridConfig::default()
/// };
/// assert!(cfg.delta_mode);
/// assert_eq!(cfg.num_workers, 4);
/// // All other fields retain defaults:
/// assert!(!cfg.strict_bsp);
/// assert!(!cfg.coordinator_free_rounds);
/// ```
///
/// The default is the v1 path (SPEC-19 §3.6 R42 — backwards
/// compatibility; no caller is silently routed through the delta loop):
///
/// ```
/// use relativist_core::merge::GridConfig;
///
/// let cfg = GridConfig::default();
/// assert!(!cfg.delta_mode);
/// assert!(!cfg.strict_bsp);
/// assert!(!cfg.coordinator_free_rounds);
/// ```
/// SPEC-20 §3.0 M0: The 4-mode execution matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum ExecutionMode {
    V1Lenient,
    V1Strict,
    DeltaLenient,
    DeltaStrict,
}

// Helper functions for serde defaults (SPEC-20 R33a)
fn default_initial_wait_timeout() -> Duration {
    Duration::from_secs(30)
}
fn default_join_window_min() -> Duration {
    Duration::from_millis(50)
}
fn default_join_window_max() -> Duration {
    Duration::from_millis(500)
}
fn default_solo_budget() -> u32 {
    10_000
}

// ---------------------------------------------------------------------------
// SPEC-21 §3.8 A3 — streaming strategy selector + dispatch mode
// ---------------------------------------------------------------------------

/// Streaming partition strategy selector (SPEC-21 §3.8 A3 / R25).
///
/// `GridConfig.streaming_strategy` selects which `StreamingPartitionStrategy`
/// implementation to use.  `RoundRobin` is the default and requires no
/// extra parameters.  `Fennel` activates the Fennel balanced-partition
/// heuristic with the given balance factor `alpha`.
///
/// This is a *config* type — it is serializable and clonable. The concrete
/// `Box<dyn StreamingPartitionStrategy>` is produced by [`StreamingStrategyConfig::build`].
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub enum StreamingStrategyConfig {
    /// Cyclic (round-robin) assignment: batch agent k → worker k % num_workers.
    #[default]
    RoundRobin,
    /// Fennel balanced partition heuristic (SPEC-21 R5).
    /// `alpha` controls the balance penalty: larger `alpha` → stronger balance preference.
    Fennel {
        /// Fennel balance factor; must be > 0.0. Default: 1.0.
        alpha: f32,
    },
}

impl PartialEq for StreamingStrategyConfig {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (StreamingStrategyConfig::RoundRobin, StreamingStrategyConfig::RoundRobin) => true,
            (
                StreamingStrategyConfig::Fennel { alpha: a },
                StreamingStrategyConfig::Fennel { alpha: b },
            ) => a.to_bits() == b.to_bits(),
            _ => false,
        }
    }
}

// Eq is sound because PartialEq is based on bit equality (not NaN-aware f32 compare).
impl Eq for StreamingStrategyConfig {}

impl StreamingStrategyConfig {
    /// Build a boxed `StreamingPartitionStrategy` for the configured variant.
    ///
    /// `num_workers` is required because the strategies are initialized with
    /// knowledge of the worker count.
    ///
    /// # Panics
    ///
    /// Panics if `num_workers == 0` or (for `Fennel`) `alpha` is not finite.
    /// Use [`try_build`] to surface these as `PartitionError`.
    ///
    /// [`try_build`]: Self::try_build
    pub fn build(&self, num_workers: u32) -> Box<dyn crate::partition::StreamingPartitionStrategy> {
        self.try_build(num_workers)
            .expect("StreamingStrategyConfig::build: invalid num_workers or alpha")
    }

    /// QA-D010-005 + QA-D010-007: fallible variant returning a
    /// `PartitionError` rather than panicking on `num_workers == 0` or
    /// non-finite `alpha`.
    pub fn try_build(
        &self,
        num_workers: u32,
    ) -> Result<Box<dyn crate::partition::StreamingPartitionStrategy>, crate::error::PartitionError>
    {
        match self {
            StreamingStrategyConfig::RoundRobin => Ok(Box::new(
                crate::partition::RoundRobinStreamingStrategy::try_new(num_workers)?,
            )),
            StreamingStrategyConfig::Fennel { alpha } => Ok(Box::new(
                crate::partition::FennelStreamingStrategy::try_new(num_workers, *alpha as f64)?,
            )),
        }
    }
}

fn default_streaming_strategy() -> StreamingStrategyConfig {
    StreamingStrategyConfig::RoundRobin
}

fn default_chunk_size() -> u32 {
    // SPEC-21 §3.3 R24 — placeholder default pending SC-024 benchmark calibration.
    // Q2 / SC-024: this value will be tuned after Phase 3 LAN benchmarks.
    10_000
}

fn default_max_pending_lifetime() -> u32 {
    16
}

fn default_dispatch_mode() -> DispatchMode {
    DispatchMode::Auto
}

/// Pull-dispatch mode selector (SPEC-21 §3.6 R34).
///
/// Controls whether the coordinator uses push dispatch (v1 / batch) or
/// pull dispatch (SPEC-21 R30-R32). `Auto` resolves at runtime: when
/// `chunk_size == u32::MAX` (R26 short-circuit) it behaves as `Push`;
/// when `chunk_size < u32::MAX && estimated stream length > num_workers`
/// it behaves as `Pull`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DispatchMode {
    /// Automatic selection: uses Push when chunk_size == u32::MAX, Pull otherwise.
    #[default]
    Auto,
    /// Force push mode: coordinator sends all partitions eagerly (v1 / R26 compatible).
    Push,
    /// Force pull mode: coordinator waits for RequestWork before sending next chunk.
    Pull,
}

/// Configuration for the grid loop (SPEC-05, R25, R29, R30a; SPEC-19 §3.6; SPEC-20 §3.4).
///
/// The partition strategy is NOT stored here because trait objects
/// are not Clone. It is passed as a separate parameter to `run_grid`.
///
/// # Examples
///
/// Opt into the delta protocol from a builder pattern (SPEC-19 §3.6 R41):
///
/// ```
/// use relativist_core::merge::GridConfig;
///
/// let cfg = GridConfig {
///     num_workers: 4,
///     delta_mode: true,
///     ..GridConfig::default()
/// };
/// assert!(cfg.delta_mode);
/// assert_eq!(cfg.num_workers, 4);
/// // All other fields retain defaults:
/// assert!(!cfg.strict_bsp);
/// assert!(!cfg.coordinator_free_rounds);
/// ```
///
/// The default is the v1 path (SPEC-19 §3.6 R42 — backwards
/// compatibility; no caller is silently routed through the delta loop):
///
/// ```
/// use relativist_core::merge::GridConfig;
///
/// let cfg = GridConfig::default();
/// assert!(!cfg.delta_mode);
/// assert!(!cfg.strict_bsp);
/// assert!(!cfg.coordinator_free_rounds);
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[cfg_attr(
    feature = "zero-copy",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
pub struct GridConfig {
    /// Number of workers for parallel reduction.
    /// Must be >= 1.
    pub num_workers: u32,

    /// Maximum number of rounds before forced termination.
    /// None = no limit (loop until Normal Form).
    /// Some(limit) = terminate after `limit` rounds even if not converged (R29).
    pub max_rounds: Option<u32>,

    /// Strict BSP mode (SPEC-05 R30a).
    ///
    /// When false (lenient, default), `run_grid` performs a full `reduce_all`
    /// on the merged net after each round, concentrating all cascade work at
    /// the coordinator and terminating in exactly 1 round for most inputs.
    ///
    /// When true (strict), border resolution is deferred: the merged net is
    /// left with its border redexes in the queue, and the grid loop iterates
    /// — redistributing those redexes to workers in the next round. Cascades
    /// that cross partition boundaries force additional rounds until Normal
    /// Form is reached. The Fundamental Property G1 (SPEC-01) holds in both
    /// modes; only the round distribution changes.
    pub strict_bsp: bool,

    /// SPEC-19 §3.6 R41 (TASK-0389): opt-in for the delta-only BSP
    /// protocol (stateful workers).
    ///
    /// When `false` (default — R42 backwards compatibility), `run_grid`
    /// uses the v1 full-partition protocol (SPEC-05 R24-R30a): every
    /// round re-partitions the net and redistributes it to workers.
    ///
    /// When `true`, the grid loop dispatches the delta BSP loop
    /// (`run_grid_delta`, bundle 2.26-C), which keeps workers stateful
    /// across rounds and sends only border deltas between them. The
    /// delta loop is functionally equivalent to the v1 loop up to
    /// isomorphism (G1 amendment, R38), with the same Normal Form and
    /// the same total interaction count (T7).
    ///
    /// IC concept: the delta protocol exploits strong confluence (T4) —
    /// the *order* of independent reductions does not affect the Normal
    /// Form, so keeping local partitions stable and exchanging only
    /// border changes is safe. Partial state distributed across workers
    /// is a valid representation of the sequential intermediate state
    /// (recoverable via Final State Collection, R27-R29).
    ///
    /// Independent of `coordinator_free_rounds` (SPEC-19 §3.6 R44): both
    /// flags are opt-ins that can be combined or enabled separately. The
    /// combination has no cross-field interaction beyond the individual
    /// R4 (coordinator-free skip) and R41 (delta dispatch) semantics.
    ///
    /// Inert until bundle 2.26-C's dispatch site reads the flag. Setting
    /// it to `true` in the current codebase is a no-op beyond the field
    /// round-trip (see TASK-0391 for the R42 regression proof).
    pub delta_mode: bool,

    /// SPEC-19 §3.1 R3, R4 (TASK-0350): opt-in for the coordinator-free
    /// round (merge avoidance) optimization.
    ///
    /// When `false` (default — v1 behaviour), every BSP round runs the
    /// full split → reduce → merge → resolve cycle.
    ///
    /// When `true`, after each local-reduction phase the coordinator
    /// inspects the per-worker `has_border_activity` flags. If **every**
    /// worker reports `false`, the merge phase is skipped for that round
    /// and `GridMetrics::coordinator_free_rounds` is incremented (R4).
    /// Termination still happens via Global Normal Form: when no worker
    /// has local redexes AND no worker has border activity, the loop
    /// exits with `converged = true`. The wire-format (R7) and FSM
    /// remain untouched — only the coordinator schedules differently.
    ///
    /// IC concept: under T4 (strong confluence) the order of independent
    /// reductions is irrelevant. If no border endpoint is principal, no
    /// merge can produce a new redex this round, so the merge phase is
    /// pure overhead and may safely be skipped.
    ///
    /// SPEC-19 §3.6 R43 (TASK-0397): when `delta_mode` is `true`, this
    /// field is AUTO-ENABLED via [`GridConfig::normalize`] — the delta
    /// protocol's design assumes the coordinator-free-round optimization
    /// is in effect (merge rounds become rare because workers hold
    /// persistent state). `Default::default()` alone leaves this field
    /// `false` to preserve R42 v1 baseline; CLI construction paths
    /// (`build_grid_config*`) always call `.normalize()` so users passing
    /// `--delta-mode` on the CLI get R43 behaviour automatically.
    ///
    /// Per DC-0397-A (option c), the normalization is UNCONDITIONAL: even
    /// a caller who sets `delta_mode=true, coordinator_free_rounds=false`
    /// explicitly sees `coordinator_free_rounds` forced to `true` after
    /// `.normalize()`. A `tracing::debug!` records the coercion. If user-
    /// choice preservation is required in the future, DC-0397-A is
    /// revised and a `coordinator_free_rounds_user_set` tracking bit is
    /// introduced.
    pub coordinator_free_rounds: bool,

    /// SPEC-20 R33a: Hybrid coordinator mode. When true, the coordinator
    /// also acts as a worker (WorkerId 0).
    #[serde(default)]
    pub hybrid_coordinator: bool,

    /// SPEC-20 R33a: Dynamic departure. Enables worker departure recovery.
    #[serde(default)]
    pub elastic_departure: bool,

    /// SPEC-20 R33a: Retain partitions on departure (derived).
    /// Default false, auto-true when elastic_departure is true.
    #[serde(default)]
    pub retain_partitions: bool,

    /// SPEC-20 R33a: Dynamic joining (derived).
    /// Auto-true when hybrid_coordinator or elastic_departure is true.
    #[serde(default)]
    pub elastic_join: bool,

    /// SPEC-20 R33a: Checkpoint partitions.
    #[serde(default)]
    pub checkpoint_partitions: bool,

    /// SPEC-20 R33a: Initial wait timeout.
    #[serde(default = "default_initial_wait_timeout")]
    pub initial_wait_timeout: Duration,

    /// SPEC-20 R33a: Join window minimum.
    #[serde(default = "default_join_window_min")]
    pub join_window_min: Duration,

    /// SPEC-20 R33a: Join window maximum.
    #[serde(default = "default_join_window_max")]
    pub join_window_max: Duration,

    /// SPEC-20 R33a: Solo budget (max interactions in SoloReducing state).
    #[serde(default = "default_solo_budget")]
    pub solo_budget: u32,

    /// SPEC-22 R10b / §3.8 A10 / SPEC-19 R12a: recycle policy for delta-mode rounds.
    ///
    /// Controls whether workers may pop from the free-list during a delta-mode round,
    /// preventing G1 violations when `BorderGraph` slot-id stability is required.
    ///
    /// - `DisableUnderDelta` (default, Strategy A): workers MUST NOT pop from the
    ///   free-list during a delta-mode round; `create_agent` falls through to fresh
    ///   `next_id` allocation. The free-list is drained at the next clean partition
    ///   boundary (`reconstruct` per SPEC-19 R38).
    /// - `BorderClean` (Strategy B): workers MAY pop from the free-list only if the
    ///   popped ID is NOT in the partition's `border_entries_shadow`. Border-referenced
    ///   IDs are re-pushed and a fresh allocation is returned instead.
    ///
    /// This field is the grid-level configuration knob. At `build_subnet` time the
    /// coordinator/dispatcher copies this value into `Net.recycle_policy` for each
    /// worker's subnet (SF-003 fix: moves the public control surface to GridConfig
    /// as specified in SPEC-22 R10b).
    #[serde(default)]
    #[cfg_attr(feature = "zero-copy", rkyv(with = rkyv::with::Skip))]
    pub recycle_under_delta: crate::net::core::RecyclePolicy,

    // -----------------------------------------------------------------------
    // SPEC-21 §3.8 A3 — streaming generation fields (append at tail per
    // bincode discriminant-ordering rule; serde(default) preserves
    // backward-compat for old config files and bincode streams).
    // -----------------------------------------------------------------------
    /// Target chunk size for streaming generation (SPEC-21 §3.3 R24).
    ///
    /// Number of agents per `AgentBatch`.  When set to `u32::MAX` (sentinel),
    /// the pipeline takes the R26 short-circuit path: the full stream is
    /// materialised via the default `make_net_stream` impl and delegated to
    /// SPEC-04 `split()` directly.
    ///
    /// **Default: 10_000** (placeholder; calibrated post-SC-024 / Q2).
    #[serde(default = "default_chunk_size")]
    pub chunk_size: u32,

    /// Streaming partition strategy (SPEC-21 §3.4 R25).
    ///
    /// Selects the `StreamingPartitionStrategy` used when the pipeline
    /// operates in streaming mode (i.e., `chunk_size != u32::MAX`).
    /// The factory method [`StreamingStrategyConfig::build`] produces
    /// a boxed `dyn StreamingPartitionStrategy` on demand.
    ///
    /// **Default: `RoundRobin`** (zero-configuration, R11).
    #[serde(default = "default_streaming_strategy")]
    #[cfg_attr(feature = "zero-copy", rkyv(with = rkyv::with::Skip))]
    pub streaming_strategy: StreamingStrategyConfig,

    /// Pull-dispatch mode selector (SPEC-21 §3.6 R34).
    ///
    /// When `Auto`, the coordinator resolves at runtime: `Push` when
    /// `chunk_size == u32::MAX`, `Pull` otherwise (subject to stream-length
    /// estimate). Use `Push` or `Pull` to force a specific mode.
    ///
    /// **Default: `Auto`**.
    #[serde(default = "default_dispatch_mode")]
    #[cfg_attr(feature = "zero-copy", rkyv(with = rkyv::with::Skip))]
    pub dispatch_mode: DispatchMode,

    /// Maximum number of unresolved pending connections before the pipeline
    /// reports an error (SPEC-21 §3.7 R37g).
    ///
    /// A `Pending` directive that remains unresolved after more than
    /// `max_pending_lifetime` batches indicates a malformed stream
    /// (the target agent was never emitted). Default: 16 batches.
    #[serde(default = "default_max_pending_lifetime")]
    pub max_pending_lifetime: u32,
}

impl Default for GridConfig {
    fn default() -> Self {
        Self {
            num_workers: 1,
            max_rounds: None,
            strict_bsp: false,
            // SPEC-19 §3.6 R42: opt-in only — default preserves v1 behaviour.
            delta_mode: false,
            // SPEC-19 §3.1 R4: opt-in only — defaults preserve v1 behaviour.
            coordinator_free_rounds: false,
            hybrid_coordinator: false,
            elastic_departure: false,
            retain_partitions: false,
            elastic_join: false,
            checkpoint_partitions: false,
            initial_wait_timeout: Duration::from_secs(30),
            join_window_min: Duration::from_millis(50),
            join_window_max: Duration::from_millis(500),
            solo_budget: 10_000,
            // SPEC-22 R10b: default is Strategy A (conservative, G1-safe).
            recycle_under_delta: crate::net::core::RecyclePolicy::DisableUnderDelta,
            // SPEC-21 §3.8 A3: streaming fields (additive defaults, R42-compatible).
            chunk_size: 10_000,
            streaming_strategy: StreamingStrategyConfig::RoundRobin,
            dispatch_mode: DispatchMode::Auto,
            max_pending_lifetime: 16,
        }
    }
}

impl GridConfig {
    /// SPEC-19 §3.6 R43 (TASK-0397): enforce the `delta_mode → coordinator_free_rounds`
    /// default coupling. When `delta_mode` is `true` this method sets
    /// `coordinator_free_rounds` to `true` unconditionally (DC-0397-A option c),
    /// emitting a `tracing::debug!` if the previous value was `false` so the
    /// coercion is auditable. When `delta_mode` is `false` the method is a no-op.
    ///
    /// SPEC-20 R33a: enforce derived elastic defaults.
    ///
    /// The method is called automatically by CLI construction paths
    /// (`build_grid_config`, `build_grid_config_from_local`) so users running
    /// `relativist <coord|local> --delta-mode` get R43 semantics transparently.
    /// Programmatic callers constructing a `GridConfig` via struct literal should
    /// call `.normalize()` themselves if they want R43 enforcement; `Default`
    /// alone leaves both fields at their raw default (R42 baseline).
    ///
    /// Idempotent: `cfg.normalize().normalize() == cfg.normalize()`.
    pub fn normalize(mut self) -> Self {
        if self.delta_mode && !self.coordinator_free_rounds {
            tracing::debug!(
                "SPEC-19 R43: coordinator_free_rounds forced to true under delta_mode=true"
            );
            self.coordinator_free_rounds = true;
        }

        // SPEC-20 derived defaults
        if self.elastic_departure && !self.retain_partitions {
            self.retain_partitions = true;
        }
        if (self.hybrid_coordinator || self.elastic_departure) && !self.elastic_join {
            self.elastic_join = true;
        }

        self
    }

    /// Validates the configuration (SPEC-20 §3.4).
    pub fn validate(&self) -> Result<(), crate::error::ConfigError> {
        use crate::error::ConfigError;

        if self.elastic_departure && !self.retain_partitions {
            return Err(ConfigError::RetainRequiredForDeparture);
        }
        if self.join_window_min > self.join_window_max {
            return Err(ConfigError::JoinWindowOrdering {
                min: self.join_window_min,
                max: self.join_window_max,
            });
        }
        if self.solo_budget == 0 {
            return Err(ConfigError::SoloBudgetZero);
        }
        Ok(())
    }

    /// SPEC-20 R0c: mode immutability. delta_mode and strict_bsp MUST NOT mutate
    /// after `run_grid` enters `WaitingForWorkers`.
    pub fn active_mode(&self) -> ExecutionMode {
        match (self.delta_mode, self.strict_bsp) {
            (false, false) => ExecutionMode::V1Lenient,
            (false, true) => ExecutionMode::V1Strict,
            (true, false) => ExecutionMode::DeltaLenient,
            (true, true) => ExecutionMode::DeltaStrict,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === GridMetrics tests (TASK-0060) ===

    // T1: GridMetrics::default() initializes all fields correctly
    #[test]
    fn test_grid_metrics_default() {
        let m = GridMetrics::default();
        assert_eq!(m.rounds, 0);
        assert_eq!(m.total_interactions, 0);
        assert_eq!(m.total_interactions_by_rule, [0; 6]);
        assert!(m.local_interactions_per_round.is_empty());
        assert!(m.border_interactions_per_round.is_empty());
        assert!(m.border_redexes_per_round.is_empty());
        assert!(m.agents_per_round.is_empty());
        assert!(m.partition_time_per_round.is_empty());
        assert!(m.compute_time_per_round.is_empty());
        assert!(m.merge_time_per_round.is_empty());
        assert!(m.border_reduce_time_per_round.is_empty());
        assert!(m.index_rebuild_time_per_round.is_empty());
        assert!(m.bytes_sent_per_round.is_empty());
        assert!(m.bytes_received_per_round.is_empty());
        assert!(m.network_send_time_per_round.is_empty());
        assert!(m.network_recv_time_per_round.is_empty());
        assert_eq!(m.total_time, Duration::ZERO);
        assert!(!m.converged);
        assert!(m.worker_stats_per_round.is_empty());
        // TASK-0350 UT-02: new metric defaults to zero (v1 baseline).
        assert_eq!(m.coordinator_free_rounds, 0);
        // UT-0384-04 — delta-mode fields default to "off" so v1 code
        // paths that build `GridMetrics::default()` do not accidentally
        // engage the delta route.
        assert!(!m.delta_mode);
        assert!(m.delta_max_rounds_hit.is_none());
    }

    // UT-0384-04 (duplicate target — see TEST-SPEC-0384): explicit
    // check for the two new fields so a future refactor that renames
    // them trips a targeted test, not only the big default bundle.
    #[test]
    fn grid_metrics_default_delta_fields_are_off() {
        let m = GridMetrics::default();
        assert!(
            !m.delta_mode,
            "GridMetrics::default().delta_mode MUST be false"
        );
        assert!(
            m.delta_max_rounds_hit.is_none(),
            "GridMetrics::default().delta_max_rounds_hit MUST be None"
        );
    }

    // TASK-0350 UT-01: GridConfig::default() must keep v1 behaviour
    // (coordinator_free_rounds disabled). Any change to this default
    // would silently re-route every default run through the v2 skip
    // path — guard against that here.
    #[test]
    fn grid_config_default_disables_coordinator_free_rounds() {
        let cfg = GridConfig::default();
        assert!(
            !cfg.coordinator_free_rounds,
            "default GridConfig must keep coordinator_free_rounds = false"
        );
    }

    // TASK-0350 UT-03: the field is settable and round-tripped through
    // a clone (sanity check for the public API).
    #[test]
    fn grid_config_coordinator_free_rounds_is_settable() {
        let cfg = GridConfig {
            coordinator_free_rounds: true,
            ..GridConfig::default()
        };
        assert!(cfg.coordinator_free_rounds);
        let cloned = cfg.clone();
        assert!(cloned.coordinator_free_rounds);
    }

    // TASK-0389 UT-01: GridConfig::default() must keep delta_mode = false
    // per SPEC-19 R42 backwards compatibility — a silent flip of this
    // default would route every caller through the delta path.
    #[test]
    fn grid_config_default_disables_delta_mode() {
        let cfg = GridConfig::default();
        assert!(
            !cfg.delta_mode,
            "default GridConfig must keep delta_mode = false (R42)"
        );
    }

    // TASK-0389 UT-02: the field is settable and round-tripped through
    // a clone (sanity check for the public API).
    #[test]
    fn grid_config_delta_mode_is_settable() {
        let cfg = GridConfig {
            delta_mode: true,
            ..GridConfig::default()
        };
        assert!(cfg.delta_mode);
        let cloned = cfg.clone();
        assert!(cloned.delta_mode);
    }

    // TASK-0397 UT-0397-01: Default::default() alone (without normalize)
    // preserves R42 baseline — both `delta_mode` and
    // `coordinator_free_rounds` stay `false`. Normalize is a no-op on the
    // default per DC-0397-B (do NOT fold normalize into Default).
    #[test]
    fn ut_0397_01_default_grid_config_preserves_v1_r42_baseline() {
        let cfg = GridConfig::default();
        assert!(
            !cfg.delta_mode,
            "default GridConfig.delta_mode must be false (R42)"
        );
        assert!(
            !cfg.coordinator_free_rounds,
            "default GridConfig.coordinator_free_rounds must be false (R42 baseline)"
        );
        // Idempotence on default: normalize is a no-op when delta_mode is false.
        let normalized = cfg.clone().normalize();
        assert!(!normalized.delta_mode);
        assert!(
            !normalized.coordinator_free_rounds,
            "normalize on a default GridConfig must not flip coordinator_free_rounds (R42)"
        );
    }

    // TASK-0397 UT-0397-02: R43 primary path — delta_mode=true + default
    // coordinator_free_rounds=false → normalize flips coordinator_free_rounds
    // to true. Other fields pass through untouched.
    #[test]
    fn ut_0397_02_normalize_with_delta_mode_true_sets_coordinator_free_rounds_true() {
        let cfg = GridConfig {
            delta_mode: true,
            coordinator_free_rounds: false,
            ..GridConfig::default()
        };
        let normalized = cfg.normalize();
        assert!(
            normalized.delta_mode,
            "normalize must preserve delta_mode=true"
        );
        assert!(
            normalized.coordinator_free_rounds,
            "SPEC-19 R43: normalize must set coordinator_free_rounds=true under delta_mode"
        );
        // Other fields pass through.
        assert_eq!(normalized.num_workers, 1);
        assert_eq!(normalized.max_rounds, None);
        assert!(!normalized.strict_bsp);
    }

    // TASK-0397 UT-0397-03: DC-0397-A option (c) — unconditional override.
    // Even when caller EXPLICITLY sets coordinator_free_rounds=false,
    // delta_mode=true + normalize forces coordinator_free_rounds=true.
    // The tracing::debug! records the coercion (not asserted here; QA probe).
    #[test]
    fn ut_0397_03_normalize_with_delta_mode_true_and_explicit_coordinator_free_rounds_false_forces_true(
    ) {
        let cfg = GridConfig {
            delta_mode: true,
            coordinator_free_rounds: false, // explicit user-set false
            ..GridConfig::default()
        };
        let normalized = cfg.normalize();
        assert!(normalized.delta_mode);
        assert!(
            normalized.coordinator_free_rounds,
            "DC-0397-A (option c): R43 override wins over explicit coordinator_free_rounds=false"
        );
    }

    // TASK-0397 — Idempotence canary (QA-0397-A): normalize twice is the
    // same as normalize once. Guards against accidental accumulation of
    // side effects in future normalize extensions.
    #[test]
    fn ut_0397_normalize_is_idempotent() {
        let cfg_delta = GridConfig {
            delta_mode: true,
            ..GridConfig::default()
        };
        let once = cfg_delta.clone().normalize();
        let twice = once.clone().normalize();
        assert_eq!(once.delta_mode, twice.delta_mode);
        assert_eq!(
            once.coordinator_free_rounds, twice.coordinator_free_rounds,
            "normalize must be idempotent (DC-0397-A)"
        );
    }

    // TASK-0397 — QA-0397-B: R44 legal combination
    // `coordinator_free_rounds=true + delta_mode=false` is preserved
    // through normalize (normalize only fires on the delta_mode=true arm).
    #[test]
    fn ut_0397_normalize_preserves_r44_coord_free_without_delta_mode() {
        let cfg = GridConfig {
            delta_mode: false,
            coordinator_free_rounds: true, // legal per R44
            ..GridConfig::default()
        };
        let normalized = cfg.normalize();
        assert!(!normalized.delta_mode);
        assert!(
            normalized.coordinator_free_rounds,
            "SPEC-19 R44: coordinator_free_rounds=true + delta_mode=false is legal and must be preserved"
        );
    }

    // T2: GridMetrics fields are writable and accessible
    #[test]
    fn test_grid_metrics_fields_writable() {
        let m = GridMetrics {
            rounds: 5,
            total_interactions: 1000,
            total_interactions_by_rule: [10, 20, 30, 40, 50, 60],
            local_interactions_per_round: vec![100],
            border_redexes_per_round: vec![3],
            converged: true,
            total_time: Duration::from_secs(42),
            ..GridMetrics::default()
        };

        assert_eq!(m.rounds, 5);
        assert_eq!(m.total_interactions, 1000);
        assert_eq!(m.total_interactions_by_rule[1], 20);
        assert_eq!(m.local_interactions_per_round[0], 100);
        assert_eq!(m.border_redexes_per_round[0], 3);
        assert!(m.converged);
        assert_eq!(m.total_time, Duration::from_secs(42));
    }

    // === WorkerRoundStats tests (TASK-0061) ===

    // T1: WorkerRoundStats construction and field access
    #[test]
    fn test_worker_round_stats_construction() {
        let stats = WorkerRoundStats {
            worker_id: 2,
            agents_before: 100,
            agents_after: 50,
            local_redexes: 25,
            reduce_duration_secs: 0.042,
            interactions_by_rule: [5, 3, 7, 2, 4, 4],
            has_border_activity: false,
            is_coordinator_self: false,
        };
        assert_eq!(stats.worker_id, 2);
        assert_eq!(stats.agents_before, 100);
        assert_eq!(stats.agents_after, 50);
        assert_eq!(stats.local_redexes, 25);
        assert!((stats.reduce_duration_secs - 0.042).abs() < f64::EPSILON);
        assert_eq!(stats.interactions_by_rule, [5, 3, 7, 2, 4, 4]);
        assert!(!stats.has_border_activity);
    }

    // T2: WorkerRoundStats serde round-trip (default polarity: false)
    #[test]
    fn test_worker_round_stats_serde() {
        let stats = WorkerRoundStats {
            worker_id: 1,
            agents_before: 200,
            agents_after: 100,
            local_redexes: 50,
            reduce_duration_secs: 1.5,
            interactions_by_rule: [10, 20, 5, 8, 3, 4],
            has_border_activity: false,
            is_coordinator_self: false,
        };
        let bytes = crate::protocol::bincode_v2::encode(&stats).unwrap();
        let deserialized: WorkerRoundStats =
            crate::protocol::bincode_v2::decode_value(&bytes).unwrap();
        assert_eq!(deserialized.worker_id, stats.worker_id);
        assert_eq!(deserialized.agents_before, stats.agents_before);
        assert_eq!(deserialized.agents_after, stats.agents_after);
        assert_eq!(deserialized.local_redexes, stats.local_redexes);
        assert!(
            (deserialized.reduce_duration_secs - stats.reduce_duration_secs).abs() < f64::EPSILON
        );
        assert_eq!(
            deserialized.interactions_by_rule,
            stats.interactions_by_rule
        );
        assert!(!deserialized.has_border_activity);
    }

    // T3: interactions_by_rule has exactly 6 elements
    #[test]
    fn test_worker_round_stats_by_rule_size() {
        let stats = WorkerRoundStats {
            worker_id: 0,
            agents_before: 0,
            agents_after: 0,
            local_redexes: 0,
            reduce_duration_secs: 0.0,
            interactions_by_rule: [0; 6],
            has_border_activity: false,
            is_coordinator_self: false,
        };
        assert_eq!(stats.interactions_by_rule.len(), 6);
    }

    // TASK-0348 UT-06: new field round-trips through bincode v2 with the
    // "active" polarity (true). Serves as the wire-carrier regression for
    // SPEC-19 R2 — the field rides inside Message::PartitionResult's
    // bincode v2 payload (R7 additive, no new variant).
    #[test]
    fn worker_round_stats_serde_roundtrip_with_activity_true() {
        let stats = WorkerRoundStats {
            worker_id: 7,
            agents_before: 12,
            agents_after: 10,
            local_redexes: 3,
            reduce_duration_secs: 0.005,
            interactions_by_rule: [1, 2, 3, 4, 5, 6],
            has_border_activity: true,
            is_coordinator_self: false,
        };
        let bytes = crate::protocol::bincode_v2::encode(&stats).unwrap();
        let decoded: WorkerRoundStats = crate::protocol::bincode_v2::decode_value(&bytes).unwrap();
        assert_eq!(decoded.worker_id, stats.worker_id);
        assert_eq!(decoded.agents_before, stats.agents_before);
        assert_eq!(decoded.agents_after, stats.agents_after);
        assert_eq!(decoded.local_redexes, stats.local_redexes);
        assert_eq!(decoded.interactions_by_rule, stats.interactions_by_rule);
        assert!(
            decoded.has_border_activity,
            "round-tripped value must preserve has_border_activity = true"
        );
    }

    // === GridConfig tests (TASK-0062) ===

    // SPEC-20 UT: GridConfig defaults match R33a (TASK-0415).
    #[test]
    fn grid_config_defaults_match_r33a() {
        let c = GridConfig::default();
        assert_eq!(c.num_workers, 1);
        assert!(!c.hybrid_coordinator);
        assert!(!c.elastic_departure);
        assert!(!c.retain_partitions);
        assert!(!c.elastic_join);
        assert!(!c.checkpoint_partitions);
        assert_eq!(c.initial_wait_timeout, Duration::from_secs(30));
        assert_eq!(c.join_window_min, Duration::from_millis(50));
        assert_eq!(c.join_window_max, Duration::from_millis(500));
        assert_eq!(c.solo_budget, 10_000);
    }

    // SPEC-20 UT: normalize derived retain_partitions (TASK-0415).
    #[test]
    fn grid_config_derived_retain_partitions() {
        let c = GridConfig {
            elastic_departure: true,
            retain_partitions: false,
            ..GridConfig::default()
        }
        .normalize();
        assert!(
            c.retain_partitions,
            "R33a: retain_partitions must be auto-enabled when elastic_departure is true"
        );
    }

    // SPEC-20 UT: normalize derived elastic_join (TASK-0415).
    #[test]
    fn grid_config_derived_elastic_join() {
        let c = GridConfig {
            hybrid_coordinator: true,
            ..GridConfig::default()
        }
        .normalize();
        assert!(
            c.elastic_join,
            "R33a: elastic_join must be auto-enabled when hybrid_coordinator is true"
        );

        let c = GridConfig {
            elastic_departure: true,
            ..GridConfig::default()
        }
        .normalize();
        assert!(
            c.elastic_join,
            "R33a: elastic_join must be auto-enabled when elastic_departure is true"
        );
    }

    // SPEC-20 UT: validate rejects retain_false_with_departure_true (TASK-0415).
    #[test]
    fn validate_rejects_retain_false_with_departure_true() {
        use crate::error::ConfigError;
        let c = GridConfig {
            elastic_departure: true,
            retain_partitions: false, // explicitly disable after default
            ..GridConfig::default()
        };
        assert!(matches!(
            c.validate(),
            Err(ConfigError::RetainRequiredForDeparture)
        ));
    }

    // SPEC-20 UT: validate rejects inverted_join_window_bounds (TASK-0415).
    #[test]
    fn validate_rejects_inverted_join_window_bounds() {
        use crate::error::ConfigError;
        let c = GridConfig {
            join_window_min: Duration::from_millis(500),
            join_window_max: Duration::from_millis(50),
            ..GridConfig::default()
        };
        assert!(matches!(
            c.validate(),
            Err(ConfigError::JoinWindowOrdering { .. })
        ));
    }

    // SPEC-20 UT: validate rejects zero_solo_budget (TASK-0415).
    #[test]
    fn validate_rejects_zero_solo_budget() {
        use crate::error::ConfigError;
        let c = GridConfig {
            solo_budget: 0,
            ..GridConfig::default()
        };
        assert!(matches!(c.validate(), Err(ConfigError::SoloBudgetZero)));
    }

    // SPEC-20 UT: active_mode full matrix (TASK-0415).
    #[test]
    fn active_mode_full_matrix() {
        let c = GridConfig {
            delta_mode: false,
            strict_bsp: false,
            ..GridConfig::default()
        };
        assert_eq!(c.active_mode(), ExecutionMode::V1Lenient);

        let c = GridConfig {
            delta_mode: false,
            strict_bsp: true,
            ..GridConfig::default()
        };
        assert_eq!(c.active_mode(), ExecutionMode::V1Strict);

        let c = GridConfig {
            delta_mode: true,
            strict_bsp: false,
            ..GridConfig::default()
        };
        assert_eq!(c.active_mode(), ExecutionMode::DeltaLenient);

        let c = GridConfig {
            delta_mode: true,
            strict_bsp: true,
            ..GridConfig::default()
        };
        assert_eq!(c.active_mode(), ExecutionMode::DeltaStrict);
    }

    // SPEC-20 UT: GridConfig wire-break validation (TASK-0415).
    //
    // NOTE: Because bincode v2 with standard (varint) encoding is used,
    // adding fields to a struct NOT at the end or changing types is a
    // wire break. Even adding to the end is a break for decoders that
    // expect the full byte stream to be consumed (UnexpectedEnd).
    // This is why TASK-0417 bumps PROTOCOL_VERSION 3 -> 4.
    #[test]
    fn grid_config_v4_roundtrip() {
        let original = GridConfig {
            num_workers: 4,
            max_rounds: Some(10),
            strict_bsp: true,
            delta_mode: true,
            hybrid_coordinator: true,
            ..GridConfig::default()
        };

        let bytes = crate::protocol::bincode_v2::encode(&original).unwrap();
        let back: GridConfig = crate::protocol::bincode_v2::decode_value(&bytes).unwrap();

        assert_eq!(back, original);
    }

    // T1: GridConfig with max_rounds
    #[test]
    fn test_grid_config_with_max_rounds() {
        let config = GridConfig {
            num_workers: 4,
            max_rounds: Some(100),
            ..GridConfig::default()
        };
        assert_eq!(config.num_workers, 4);
        assert_eq!(config.max_rounds, Some(100));
    }

    // T2: GridConfig with no round limit
    #[test]
    fn test_grid_config_no_limit() {
        let config = GridConfig {
            num_workers: 8,
            max_rounds: None,
            ..GridConfig::default()
        };
        assert_eq!(config.num_workers, 8);
        assert_eq!(config.max_rounds, None);
    }

    // T3: GridConfig::default() — lenient BSP, single worker, no round limit
    #[test]
    fn test_grid_config_default() {
        let config = GridConfig::default();
        assert_eq!(config.num_workers, 1);
        assert_eq!(config.max_rounds, None);
        assert!(!config.strict_bsp);
    }

    // T4: GridConfig with strict_bsp
    #[test]
    fn test_grid_config_strict_bsp() {
        let config = GridConfig {
            num_workers: 4,
            strict_bsp: true,
            ..GridConfig::default()
        };
        assert!(config.strict_bsp);
        assert_eq!(config.num_workers, 4);
    }

    // === GridMetrics network extensions (TASK-0094) ===

    // T1: total_network_bytes with known values
    #[test]
    fn test_total_network_bytes() {
        let m = GridMetrics {
            bytes_sent_per_round: vec![100, 200, 300],
            bytes_received_per_round: vec![50, 150, 250],
            ..GridMetrics::default()
        };
        assert_eq!(m.total_network_bytes(), 1050);
    }

    // T2: total_network_bytes with no rounds
    #[test]
    fn test_total_network_bytes_empty() {
        let m = GridMetrics::default();
        assert_eq!(m.total_network_bytes(), 0);
    }

    // T3: network_overhead_fraction with known durations
    #[test]
    fn test_network_overhead_fraction() {
        let m = GridMetrics {
            network_send_time_per_round: vec![Duration::from_secs(1), Duration::from_secs(2)],
            network_recv_time_per_round: vec![Duration::from_secs(3), Duration::from_secs(4)],
            total_time: Duration::from_secs(20),
            ..GridMetrics::default()
        };
        // (1+2+3+4) / 20 = 10/20 = 0.5
        let fraction = m.network_overhead_fraction();
        assert!((fraction - 0.5).abs() < f64::EPSILON);
    }

    // T4: network_overhead_fraction returns 0.0 when total_time is zero
    #[test]
    fn test_network_overhead_fraction_zero_time() {
        let m = GridMetrics::default();
        assert_eq!(m.network_overhead_fraction(), 0.0);
    }

    // T5: network_overhead_fraction with empty Vecs
    #[test]
    fn test_network_overhead_fraction_empty() {
        let m = GridMetrics {
            total_time: Duration::from_secs(10),
            ..GridMetrics::default()
        };
        assert_eq!(m.network_overhead_fraction(), 0.0);
    }

    // -----------------------------------------------------------------------
    // TASK-0353 — rkyv round-trip for WorkerRoundStats (SPEC-18 §3.5).
    //
    // WorkerRoundStats lacks PartialEq (see SPEC-19 history — the f64
    // `reduce_duration_secs` field is the obstacle), so we compare each
    // field individually. The f64 is compared via `to_bits` for exact
    // bitwise equality (rkyv's archived f64 is just a re-aligned native
    // load on little-endian targets).
    //
    // This is the "stats" half of the wire payload that flows back from
    // workers under the zero-copy path (SPEC-18 §4.4 — the
    // `ArchivePartitionResultPayload` wrapper added by TASK-0356).
    // -----------------------------------------------------------------------

    /// UT-0353-08: WorkerRoundStats round-trips through rkyv with both
    /// `has_border_activity` polarities and a non-trivial f64 duration.
    #[cfg(feature = "zero-copy")]
    #[test]
    fn rkyv_round_trip_worker_round_stats() {
        for (_worker_id, _has_border_activity, _duration_secs) in [
            (0u32, false, 0.0_f64),
            (7u32, true, 0.042_f64),
            (u32::MAX, false, 1234.5678_f64),
        ] {
            let original = WorkerRoundStats {
                worker_id: 2,
                agents_before: 100,
                agents_after: 50,
                local_redexes: 25,
                reduce_duration_secs: 0.042,
                interactions_by_rule: [5, 3, 7, 2, 4, 4],
                has_border_activity: true,
                is_coordinator_self: false,
            };

            let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&original).expect("serialize");
            let archived = rkyv::access::<rkyv::Archived<WorkerRoundStats>, rkyv::rancor::Error>(
                bytes.as_ref(),
            )
            .expect("access");
            let back: WorkerRoundStats =
                rkyv::deserialize::<WorkerRoundStats, rkyv::rancor::Error>(archived)
                    .expect("deserialize");

            assert_eq!(back.worker_id, original.worker_id);
            assert_eq!(back.agents_before, original.agents_before);
            assert_eq!(back.agents_after, original.agents_after);
            assert_eq!(back.local_redexes, original.local_redexes);
            assert_eq!(
                back.reduce_duration_secs.to_bits(),
                original.reduce_duration_secs.to_bits(),
                "f64 duration must round-trip bit-exact"
            );
            assert_eq!(back.interactions_by_rule, original.interactions_by_rule);
            assert_eq!(back.has_border_activity, original.has_border_activity);
        }
    }

    // -----------------------------------------------------------------------
    // TASK-0565: GridConfig streaming fields (SPEC-21 §3.8 A3)
    // -----------------------------------------------------------------------

    // UT-0565-01: Default streaming fields match SPEC-21 §3.8 A3 normative
    // defaults: chunk_size=10_000, RoundRobin strategy, Auto dispatch mode,
    // max_pending_lifetime=16.
    #[test]
    fn ut_0565_01_default_streaming_fields() {
        let cfg = GridConfig::default();
        assert_eq!(
            cfg.chunk_size, 10_000,
            "SPEC-21 R24: default chunk_size must be 10_000"
        );
        assert_eq!(
            cfg.streaming_strategy,
            StreamingStrategyConfig::RoundRobin,
            "SPEC-21 R25: default streaming_strategy must be RoundRobin"
        );
        assert_eq!(
            cfg.dispatch_mode,
            DispatchMode::Auto,
            "SPEC-21 R34: default dispatch_mode must be Auto"
        );
        assert_eq!(
            cfg.max_pending_lifetime, 16,
            "SPEC-21 R37g: default max_pending_lifetime must be 16"
        );
    }

    // UT-0565-02: Additive-compat regression — JSON missing the new fields
    // deserializes cleanly and applies defaults (serde(default) guard).
    #[test]
    fn ut_0565_02_json_missing_streaming_fields_applies_defaults() {
        // A JSON object that predates SPEC-21 (contains only the v1 fields).
        // Note: RecyclePolicy serde is PascalCase ("DisableUnderDelta" not "disable_under_delta").
        let v1_json = r#"{"num_workers":4,"max_rounds":null,"strict_bsp":false,"delta_mode":false,"coordinator_free_rounds":false,"hybrid_coordinator":false,"elastic_departure":false,"retain_partitions":false,"elastic_join":false,"checkpoint_partitions":false,"initial_wait_timeout":{"secs":30,"nanos":0},"join_window_min":{"secs":0,"nanos":50000000},"join_window_max":{"secs":0,"nanos":500000000},"solo_budget":10000,"recycle_under_delta":"DisableUnderDelta"}"#;
        let cfg: GridConfig =
            serde_json::from_str(v1_json).expect("old config file MUST deserialize cleanly");
        assert_eq!(
            cfg.chunk_size, 10_000,
            "additive-compat: chunk_size must default to 10_000 when absent from JSON"
        );
        assert_eq!(
            cfg.streaming_strategy,
            StreamingStrategyConfig::RoundRobin,
            "additive-compat: streaming_strategy must default to RoundRobin when absent from JSON"
        );
        assert_eq!(
            cfg.dispatch_mode,
            DispatchMode::Auto,
            "additive-compat: dispatch_mode must default to Auto when absent from JSON"
        );
    }

    // UT-0565-03: streaming_strategy.build(4) for RoundRobin returns a strategy
    // that assigns agents cyclically across 4 workers.
    #[test]
    fn ut_0565_03_build_round_robin_strategy() {
        use crate::net::Symbol;
        use crate::partition::streaming::AgentBatch;
        let mut strat = StreamingStrategyConfig::RoundRobin.build(4);
        let batch = AgentBatch {
            agents: vec![
                (0, Symbol::Con),
                (1, Symbol::Era),
                (2, Symbol::Dup),
                (3, Symbol::Con),
            ],
            connections: vec![],
        };
        let assignments = strat.allocate_batch(&batch, 4);
        // RoundRobin: agent 0→w0, 1→w1, 2→w2, 3→w3 — assignments are (AgentId, WorkerId) tuples
        let worker_ids: Vec<u32> = assignments.iter().map(|&(_, w)| w).collect();
        assert_eq!(
            worker_ids,
            vec![0, 1, 2, 3],
            "RoundRobin must distribute cyclically"
        );
    }

    // UT-0565-04: streaming_strategy.build(2) for Fennel { alpha: 1.0 } returns a Fennel
    // strategy (can accept a batch without panicking).
    #[test]
    fn ut_0565_04_build_fennel_strategy() {
        use crate::net::Symbol;
        use crate::partition::streaming::AgentBatch;
        let mut strat = StreamingStrategyConfig::Fennel { alpha: 1.0 }.build(2);
        let batch = AgentBatch {
            agents: vec![(0, Symbol::Con), (1, Symbol::Era)],
            connections: vec![],
        };
        let assignments = strat.allocate_batch(&batch, 2);
        assert_eq!(
            assignments.len(),
            2,
            "Fennel strategy must assign every agent in the batch"
        );
        for &(_, w) in &assignments {
            assert!(w < 2, "all worker IDs must be < num_workers");
        }
    }

    // UT-0565-05: DispatchMode variants serialize round-trip correctly.
    #[test]
    fn ut_0565_05_dispatch_mode_serde_roundtrip() {
        for mode in [DispatchMode::Auto, DispatchMode::Push, DispatchMode::Pull] {
            let json = serde_json::to_string(&mode).unwrap();
            let back: DispatchMode = serde_json::from_str(&json).unwrap();
            assert_eq!(mode, back, "DispatchMode serde round-trip must be identity");
        }
    }

    // UT-0565-06: StreamingStrategyConfig::Fennel bit-equality for PartialEq/Eq.
    #[test]
    fn ut_0565_06_streaming_strategy_config_eq_bit_identical_floats() {
        let a = StreamingStrategyConfig::Fennel { alpha: 1.5_f32 };
        let b = StreamingStrategyConfig::Fennel { alpha: 1.5_f32 };
        assert_eq!(a, b, "Fennel configs with identical alpha bits must be Eq");
    }

    // UT-0565-07: chunk_size == u32::MAX sentinel: the field is storable/readable.
    #[test]
    fn ut_0565_07_chunk_size_max_sentinel_storable() {
        let cfg = GridConfig {
            chunk_size: u32::MAX,
            ..GridConfig::default()
        };
        assert_eq!(
            cfg.chunk_size,
            u32::MAX,
            "u32::MAX sentinel must round-trip through GridConfig"
        );
        let json = serde_json::to_string(&cfg).unwrap();
        let back: GridConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(
            back.chunk_size,
            u32::MAX,
            "u32::MAX must survive JSON round-trip"
        );
    }
}
