//! Relativist error types (SPEC-13 R15-R18).
//!
//! Centralized error handling using `thiserror` (PESQ-023 D2).
//! Each module defines its own error enum. `RelativistError` unifies
//! them all via `#[from]` conversions.
//!
//! Errors are classified as transient (retryable) or fatal (abort).
//! The coordinator FSM uses richer classification; `exit_code()` is
//! a simplification for the CLI layer.

use thiserror::Error;

use crate::net::AgentId;
use crate::partition::WorkerId;
use crate::protocol::ProtocolError;
use crate::security::SecurityError;

// ---------------------------------------------------------------------------
// Per-module error enums (SPEC-13 R16)
// ---------------------------------------------------------------------------

/// Errors from the net representation layer.
#[derive(Debug, Error)]
pub enum NetError {
    #[error("agent {0} not found")]
    AgentNotFound(AgentId),

    #[error("net invariant violated: {0}")]
    InvariantViolation(String),

    #[error("serialization error: {0}")]
    Serialize(String),

    #[error("deserialization error: {0}")]
    Deserialize(String),

    /// SPEC-22 R9: free-list post-condition violated.
    ///
    /// Returned by `Net::validate_free_list` when an entry in `free_list`
    /// corresponds to a `Some` slot in the arena (the slot was not properly
    /// cleared before the ID was recycled, or the free-list was corrupted),
    /// OR when a duplicate entry is detected (R6 violation).
    #[error("free-list invalid: id {id} — {reason}")]
    FreeListInvalid {
        /// The offending AgentId.
        id: AgentId,
        /// Human-readable explanation of the violation.
        reason: &'static str,
    },

    /// SPEC-22 R20 / QA-D009-005: `SparseNet::to_dense` allocation guard.
    ///
    /// Returned when the computed arena length exceeds `MAX_DENSE_ARENA_SLOTS`.
    /// An attacker-controlled `max_id` near `u32::MAX` would otherwise produce
    /// a multi-GiB allocation request (DoS surface). Callers must either
    /// provide a bounded `id_range` or restructure the SparseNet.
    #[error(
        "dense arena allocation would exceed threshold: arena_len={arena_len} > max={max} \
         (live_count={live_count}); use a bounded id_range or reduce the agent ID spread"
    )]
    DenseAllocationExceedsThreshold {
        arena_len: usize,
        max: usize,
        live_count: usize,
    },

    /// SPEC-22 R20 / QA-D009-006: inverted `id_range` passed to `SparseNet::to_dense`.
    ///
    /// Returned when `id_range.start > id_range.end`. An inverted range is a
    /// caller bug or attacker-supplied malformed state; panicking is worse than
    /// returning a graceful error.
    #[error("invalid id_range: start={start} > end={end}")]
    InvalidIdRange { start: u32, end: u32 },

    /// QA-D009-011 / SPEC-22 R3: `AgentId` space exhausted.
    ///
    /// Returned by `create_agent` when `next_id == u32::MAX` (would overflow on
    /// the next fresh allocation). Workers should treat this as a fatal ID-space
    /// exhaustion and signal the coordinator.
    #[error("agent ID space exhausted: next_id would overflow u32::MAX")]
    AgentIdOverflow,
}

/// Errors from the reduction engine.
#[derive(Debug, Error)]
pub enum ReductionError {
    #[error("invalid redex: agents {0} and {1} are not connected via principal ports")]
    InvalidRedex(AgentId, AgentId),

    #[error("reduction invariant violated: {0}")]
    InvariantViolation(String),
}

/// Errors from the partitioning subsystem.
#[derive(Debug, Error)]
pub enum PartitionError {
    #[error("cannot partition net with {agents} agents into {k} partitions")]
    TooFewAgents { agents: usize, k: usize },

    #[error("partition invariant violated: {0}")]
    InvariantViolation(String),

    /// SPEC-20 §3.8 A3 (R18a) — returned by `PartitionPlan::allocate_border_ids`
    /// when `count > u32::MAX - next_border_id` (NF-006 closure).
    ///
    /// # Wire serde note (QA-007)
    ///
    /// `PartitionError` is not serialized over the wire today (it is a
    /// coordinator-local Rust error, not a `Message` variant). If a future
    /// task adds a wire-borne payload that carries a `PartitionError`
    /// (e.g., `RoundResult::partition_error: Option<PartitionError>`),
    /// `Serialize`/`Deserialize` derives must be added to ALL variants at
    /// that time. The `u32` fields in `BorderIdSpaceExhausted` and
    /// `NewRangeTooSmall` are trivially serializable when needed.
    #[error("border_id space exhausted: requested {requested} ids but only {available} remain")]
    BorderIdSpaceExhausted { requested: u32, available: u32 },

    /// SPEC-20 §3.8 A4 (R19a) — returned by `remap_partition_ids`
    /// when `partition.agent_count() > new_range.len()` (NF-006 closure).
    #[error(
        "new_range too small: partition has {partition_size} agents but range holds only {range_size}"
    )]
    NewRangeTooSmall {
        partition_size: u32,
        range_size: u32,
    },

    /// SPEC-22 §3.4 R30 — returned by `build_subnet_with_config` when
    /// `sparse_build == false` AND `id_range_size > 4 × live_count`.
    ///
    /// Guards against the M5 memory pathology: a dense arena of size
    /// `id_range_size` with very few live agents wastes up to 800 MiB
    /// per partition. Callers must either set `sparse_build: true` or
    /// reduce the id_range to satisfy the 4× threshold.
    #[error(
        "dense allocation exceeds threshold: partition {partition_index} has id_range_size={id_range_size} but live_count={live_count} (threshold: id_range_size <= 4 × live_count)"
    )]
    DenseAllocationExceedsThreshold {
        /// Zero-based index of the partition that triggered the guard.
        partition_index: usize,
        /// Size of the id_range (`id_range.end - id_range.start`) as `u64`
        /// to avoid overflow when the range spans the full u32 space.
        id_range_size: u64,
        /// Number of live agents in the partition's worker_agents list.
        live_count: u64,
    },

    /// SPEC-21 R19 — returned by `generate_and_partition_chunked` when the
    /// pending-connection store is non-empty after the stream is exhausted.
    ///
    /// Indicates that a generator emitted a `Pending` directive whose target
    /// agent was never generated. This is a generator bug: every `Pending`
    /// directive MUST have its target agent appear in a subsequent batch.
    #[error(
        "unresolved forward reference: pending connection targeting agent {agent_id} was never resolved"
    )]
    UnresolvedForwardReferences {
        /// AgentId referenced in a Pending directive that was never emitted.
        agent_id: AgentId,
    },

    /// SPEC-21 R26 (R26 short-circuit path) — returned when the arena
    /// conversion from `SparseNet` to dense `Net` fails during materialisation.
    ///
    /// This error is only reachable from the R26 path (`chunk_size == u32::MAX`).
    #[error("arena conversion failed during R26 materialise-then-split path: {0}")]
    ArenaConversionFailed(String),
}

/// Errors from the merge subsystem.
#[derive(Debug, Error)]
pub enum MergeError {
    #[error("unresolved border: FreePort({0}) has no matching partner")]
    UnresolvedBorder(u32),

    #[error("merge invariant violated: {0}")]
    InvariantViolation(String),
}

/// SPEC-19 R20, R21 (TASK-0384): errors produced by the delta-mode
/// grid loop (`run_grid_delta`) and `WorkerDispatch` implementations.
/// Reserved for transport / protocol failures surfaced by the dispatch
/// trait; pure-core logic errors continue to use `MergeError`.
#[derive(Debug, Error)]
pub enum GridError {
    #[error("worker dispatch failed at round {round}: {message}")]
    DispatchFailed { round: u32, message: String },

    #[error("worker {worker_id} did not reply at round {round}")]
    WorkerTimeout { worker_id: WorkerId, round: u32 },

    #[error("max rounds ({max}) exceeded without convergence")]
    MaxRoundsExceeded { max: u32 },

    /// SPEC-19 R48 / DC-B5 (TASK-0398): the coordinator detected a
    /// protocol violation — e.g., a `MintedAgent.request_id` in a
    /// `RoundResult` that does not correlate to any outstanding
    /// `PendingPortRef::Pending` in `BorderGraph.pending_new_borders`
    /// (stray request_id). Diagnostic string carries the guilty worker
    /// + decoded (commutation_id, agent_slot) pair.
    #[error("R48 protocol violation: {0}")]
    ProtocolViolation(String),

    #[error(transparent)]
    Merge(#[from] MergeError),
}

/// Errors from the coordinator.
#[derive(Debug, Error)]
pub enum CoordinatorError {
    #[error("worker {0} failed: {1}")]
    WorkerFailed(WorkerId, String),

    #[error("no workers registered within timeout")]
    NoWorkers,

    /// SPEC-20 R11 / SC-023: no WorkerId slots remaining.
    #[error("WorkerId space exhausted (R11)")]
    WorkerIdSpaceExhausted,

    #[error(transparent)]
    Protocol(#[from] ProtocolError),
}

/// Errors from the worker.
#[derive(Debug, Error)]
pub enum WorkerError {
    #[error(transparent)]
    Protocol(#[from] ProtocolError),

    #[error("reduction failed: {0}")]
    ReductionFailed(String),

    /// SPEC-19 R26, DC-B5 second half (TASK-0394): the worker's
    /// `id_range` ran out of `AgentId`s while fulfilling the current
    /// round's `pending_commutations`. The `request_id` identifies which
    /// `PendingCommutation` the worker could not satisfy. The coordinator
    /// treats this as a recoverable protocol error: on receipt it MAY
    /// repartition or abort cleanly. Partial-progress semantics apply —
    /// already-minted agents from earlier entries of the same
    /// `pending_commutations` vector remain committed in the partition.
    #[error("worker id_range exhausted while minting agent for request {request_id}")]
    IdRangeExhausted { request_id: u32 },
}

/// Errors from grid configuration (SPEC-20 §3.4 R33a).
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("elastic_departure=true requires retain_partitions=true (SPEC-20 R32)")]
    RetainRequiredForDeparture,

    #[error("join_window_min ({min:?}) cannot be greater than join_window_max ({max:?})")]
    JoinWindowOrdering {
        min: std::time::Duration,
        max: std::time::Duration,
    },

    #[error("solo_budget cannot be 0")]
    SoloBudgetZero,
}

// ---------------------------------------------------------------------------
// Top-level error type (SPEC-13 R17)
// ---------------------------------------------------------------------------

/// Top-level error type for the Relativist binary.
///
/// Unifies all per-module errors via `#[from]` conversions (SPEC-13 R17).
#[derive(Debug, Error)]
pub enum RelativistError {
    #[error(transparent)]
    Net(#[from] NetError),

    #[error(transparent)]
    Reduction(#[from] ReductionError),

    #[error(transparent)]
    Partition(#[from] PartitionError),

    #[error(transparent)]
    Merge(#[from] MergeError),

    #[error(transparent)]
    Coordinator(#[from] CoordinatorError),

    #[error(transparent)]
    Worker(#[from] WorkerError),

    #[error(transparent)]
    Security(#[from] SecurityError),

    #[error(transparent)]
    ConfigValidation(#[from] ConfigError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("encoding error: {0}")]
    Encoding(String),
}

impl From<crate::encoding::RegistryError> for RelativistError {
    fn from(err: crate::encoding::RegistryError) -> Self {
        RelativistError::Encoding(err.to_string())
    }
}

impl RelativistError {
    /// Map error to process exit code.
    ///
    /// - 1: configuration errors
    /// - 2: communication / I/O errors
    /// - 3: internal errors (invariant violations, logic bugs)
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::ConfigValidation(_) | Self::Config(_) => 1,
            Self::Io(_) => 2,
            Self::Coordinator(CoordinatorError::Protocol(_)) => 2,
            Self::Worker(WorkerError::Protocol(_)) => 2,
            _ => 3,
        }
    }
}

/// Backwards-compatibility alias for code that still uses `RelError`.
pub type RelError = RelativistError;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exit_code_config() {
        let err = RelativistError::Config("bad arg".into());
        assert_eq!(err.exit_code(), 1);
    }

    #[test]
    fn test_exit_code_io() {
        let err = RelativistError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "not found",
        ));
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn test_exit_code_net() {
        let err = RelativistError::Net(NetError::AgentNotFound(42));
        assert_eq!(err.exit_code(), 3);
    }

    #[test]
    fn test_exit_code_coordinator_protocol() {
        let proto = ProtocolError::AuthFailed {
            reason: "test".into(),
        };
        let coord = CoordinatorError::Protocol(proto);
        let err = RelativistError::Coordinator(coord);
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn test_exit_code_worker_protocol() {
        let proto = ProtocolError::AuthFailed {
            reason: "test".into(),
        };
        let worker = WorkerError::Protocol(proto);
        let err = RelativistError::Worker(worker);
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn test_all_errors_display() {
        let errors: Vec<Box<dyn std::error::Error>> = vec![
            Box::new(NetError::AgentNotFound(1)),
            Box::new(NetError::InvariantViolation("test".into())),
            Box::new(NetError::Serialize("ser".into())),
            Box::new(NetError::Deserialize("de".into())),
            Box::new(ReductionError::InvalidRedex(1, 2)),
            Box::new(ReductionError::InvariantViolation("test".into())),
            Box::new(PartitionError::TooFewAgents { agents: 1, k: 4 }),
            Box::new(PartitionError::InvariantViolation("test".into())),
            Box::new(PartitionError::BorderIdSpaceExhausted {
                requested: 10,
                available: 5,
            }),
            Box::new(PartitionError::NewRangeTooSmall {
                partition_size: 4,
                range_size: 2,
            }),
            Box::new(MergeError::UnresolvedBorder(99)),
            Box::new(MergeError::InvariantViolation("test".into())),
            Box::new(CoordinatorError::NoWorkers),
            Box::new(CoordinatorError::WorkerFailed(0, "crash".into())),
            Box::new(WorkerError::ReductionFailed("bad".into())),
        ];
        for err in &errors {
            let msg = format!("{}", err);
            assert!(!msg.is_empty());
        }
    }

    #[test]
    fn test_from_conversions() {
        let net_err = NetError::AgentNotFound(1);
        let _: RelativistError = net_err.into();

        let red_err = ReductionError::InvalidRedex(1, 2);
        let _: RelativistError = red_err.into();

        let part_err = PartitionError::TooFewAgents { agents: 1, k: 4 };
        let _: RelativistError = part_err.into();

        let merge_err = MergeError::UnresolvedBorder(1);
        let _: RelativistError = merge_err.into();

        let io_err = std::io::Error::other("test");
        let _: RelativistError = io_err.into();
    }
}
