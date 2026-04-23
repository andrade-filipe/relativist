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
            Self::Config(_) => 1,
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
        let proto = ProtocolError::AuthFailed;
        let coord = CoordinatorError::Protocol(proto);
        let err = RelativistError::Coordinator(coord);
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn test_exit_code_worker_protocol() {
        let proto = ProtocolError::AuthFailed;
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
