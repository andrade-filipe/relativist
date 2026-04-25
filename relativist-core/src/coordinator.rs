//! Coordinator FSM for distributed IC reduction (SPEC-13 R19-R23).
//!
//! The coordinator implements the BSP (Bulk Synchronous Parallel)
//! programming model (SPEC-13 R1, R4). Each grid round is one BSP
//! superstep: split → dispatch → reduce → collect → merge → check.
//!
//! The FSM follows the stimulus-response pattern (SPEC-13 R20):
//! a pure function `transition(state, event) -> (new_state, actions)`
//! that is testable without tokio.
//!
//! Extended by SPEC-20 §3.8 A2 (TASK-0414): new states
//! `AcceptingMembershipChanges` and `SoloReducing`, new events, new
//! actions, and helper enums `LeaveKind`, `RetainedSlot`, `DepartureKind`.

use std::net::SocketAddr;
use std::time::Duration;

use crate::merge::WorkerRoundStats;
use crate::net::Net;
use crate::partition::{Partition, WorkerId};
use crate::protocol::Message;

/// Unique identifier for a timer managed by the coordinator's async runtime.
///
/// Derived from `TimerKind as u32` per SPEC-20 §4.1.3.
pub type TimerId = u32;

// ---------------------------------------------------------------------------
// Helper enums (SPEC-20 §3.8 A2 / TASK-0414)
// ---------------------------------------------------------------------------

// `LeaveKind` is defined in `crate::partition::LeaveKind` (SPEC-13 R28 / QA-004).
// Re-exported here for ergonomic use within coordinator logic.
pub use crate::partition::LeaveKind;

/// Maximum byte length for `SelfPartitionPanic` payload (QA-002).
///
/// Panic messages exceeding this limit are truncated at construction time to
/// prevent coordinator OOM under fatal conditions. The truncation marker
/// `" [truncated]"` is appended when the input exceeds this cap.
pub const MAX_PANIC_PAYLOAD_BYTES: usize = 4096;

/// Which retained-state slot the coordinator consults during departure
/// recovery (SPEC-20 R23, R24a/R24b).
///
/// SPEC-20 §4.1.2: consumed by `CoordinatorAction::ReclaimPartition`.
///
/// # API stability
///
/// `#[non_exhaustive]` prevents external callers from writing exhaustive
/// matches that silently miss new variants. Do not reorder variants — declaration
/// order is part of the public ABI.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RetainedSlot {
    /// Round-0 dispatch snapshot for worker `w` (R23b / R24a).
    Initial,
    /// Most recently committed worker state (R23c / R24b).
    LastAcked,
}

/// Reason recorded in a departure log entry (SPEC-20 §4.1.2).
///
/// Consumed by `CoordinatorAction::LogDeparture`.
///
/// # API stability
///
/// `#[non_exhaustive]` prevents external callers from writing exhaustive
/// matches that silently miss new variants. Do not reorder variants — declaration
/// order is part of the public ABI.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DepartureKind {
    /// Coordinator-initiated: worker exceeded `collect_timeout` (R18).
    Timeout,
    /// Coordinator-initiated: TCP connection closed unexpectedly (R19).
    ConnLost,
    /// Worker-initiated: `LeaveRequest { kind: AfterResult }` (R22a).
    LeaveAfter,
    /// Worker-initiated: `LeaveRequest { kind: Urgent }` (R22b).
    LeaveUrgent,
}

/// Typed timer identifiers (SPEC-20 §4.1.3; closes SC-022).
///
/// `TimerId` values are derived via `TimerKind as u32` so that the cast is
/// portable and log tooling can decode `TimerId → TimerKind` without
/// per-build metadata.
///
/// # Import path for observability consumers
///
/// Observability crates and log-decoding tooling should import this type as
/// `relativist_core::coordinator::TimerKind`. When `LeaveKind` is moved to a
/// shared types module (TASK-0418 / TASK-0426), `TimerKind` will follow to the
/// same location for consistent import ergonomics.
///
/// # ABI stability
///
/// Explicit `#[repr(u32)]` discriminants 0–3 are normative per SPEC-20 §4.1.3
/// (NF-008). Log-decoding tooling depends on these values. A future variant
/// insertion that shifts these values is a wire-decoder break. Do NOT reorder
/// or insert variants before position 4.
///
/// `#[non_exhaustive]` allows new variants to be added in future SPEC revisions
/// without breaking downstream matches.
#[repr(u32)]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimerKind {
    /// Timer for the initial worker-connection wait (R6).
    InitialWait = 0,
    /// Minimum join-window duration (R10, R10a).
    JoinWindowMin = 1,
    /// Maximum join-window duration (R10a).
    JoinWindowMax = 2,
    /// Per-round collect timeout (SPEC-06 R30, R18).
    Collect = 3,
}

// ---------------------------------------------------------------------------
// FSM types (TASK-0107, SPEC-13 R19-R22)
// ---------------------------------------------------------------------------

/// Coordinator FSM states (SPEC-13 R19; extended by SPEC-20 §3.8 A2 / §4.1.1).
///
/// Enum-based (not typestate) per SPEC-13 R22 for serialization,
/// logging, and testing ergonomics.
///
/// WIRE-SERDE NOTICE: This enum derives `serde::Serialize` only (one-way,
/// for log/observability output). It is NOT used as a wire-format discriminant
/// in `protocol/` messages; therefore adding new variants here does NOT
/// constitute a wire-protocol break and does NOT require a `PROTOCOL_VERSION`
/// bump. Wire-protocol changes are governed by SPEC-20 R37 / `protocol/`.
/// New variants are appended at the end to preserve discriminant stability for
/// any downstream serde consumers (TASK-0414 note).
///
/// # ABI stability
///
/// Variant declaration order is part of the public ABI: `serde::Serialize`
/// uses the variant name (not the discriminant integer), but `std::mem::discriminant`
/// ordering follows declaration order. Do NOT reorder variants; append only.
///
/// `#[non_exhaustive]` prevents external callers from writing exhaustive
/// matches that silently miss new variants added in future SPEC revisions.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum CoordinatorState {
    // --- Existing states from SPEC-13 R19 (order preserved for discriminant stability) ---
    /// Initial state. Loading configuration, binding TCP listener.
    Init,
    /// Waiting for the minimum number of workers to connect.
    WaitingForWorkers,
    /// Partitioning the net into sub-nets for distribution.
    Partitioning,
    /// Sending partitions to workers.
    Dispatching,
    /// Waiting for all workers to return their reduced partitions.
    WaitingForResults,
    /// Merging returned partitions and running reduce_all on the
    /// merged net to resolve border redexes (SPEC-05 R15-R18).
    Merging,
    /// Checking if the merged-and-reduced net is in Normal Form.
    /// If yes → Done. If no → Partitioning (next round).
    CheckTermination,
    /// Reduction complete. Writing output, sending Shutdown to all workers.
    Done,
    /// Fatal error. Shutting down.
    Error,

    // --- New states for SPEC-20 §4.1.1 (appended after existing variants) ---
    /// Accepting new worker connections and processing graceful departures
    /// between BSP rounds (SPEC-20 R10, R10a, R10b).
    ///
    /// Entered from `CheckTermination` when `is_normal_form == false` AND
    /// `(elastic_join || elastic_departure) == true`. Transitions to
    /// `Partitioning` on `MembershipWindowClosed`.
    AcceptingMembershipChanges,

    /// Coordinator reduces the entire net alone via `reduce_n(solo_budget)`
    /// in a loop, polling the async event loop for `WorkerJoined` events
    /// between successive batches (SPEC-20 R5, R5a). Closes SC-018.
    ///
    /// Entered when `K == 0 && hybrid_coordinator == true`. Terminates on
    /// `SoloReductionComplete` (→ `Done`) or `WorkerJoined(id)` (→
    /// `CheckTermination`).
    SoloReducing,
}

/// Events that drive the coordinator FSM (SPEC-13 R20; extended by SPEC-20
/// §3.8 A2 / §4.1.2).
///
/// New SPEC-20 variants are appended after the existing SPEC-13 variants to
/// preserve discriminant order for any serialization-dependent consumers.
///
/// # API stability
///
/// `#[non_exhaustive]` is set so that external callers are forced to include a
/// wildcard arm in any `match` on this enum. The internal `transition()` function
/// already uses a wildcard at the end of its match block. New variants MAY be
/// added in any SPEC revision; downstream matchers MUST include `_ =>` arms.
///
/// # Payload bounds
///
/// Variants that carry string payloads (`SelfPartitionPanic`) enforce an upper
/// bound of `MAX_PANIC_PAYLOAD_BYTES` at construction time — use
/// `CoordinatorEvent::self_partition_panic(msg)` rather than the raw variant
/// constructor to ensure truncation.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum CoordinatorEvent {
    // --- Existing events from SPEC-13 R21 ---
    /// Configuration loaded and validated.
    ConfigLoaded,
    /// A worker connected (TCP accept succeeded).
    WorkerConnected(WorkerId),
    /// Partition split completed. Contains the resulting partitions.
    SplitComplete(Vec<Partition>),
    /// All partitions have been dispatched to workers.
    AllDispatched,
    /// A worker returned its reduced partition.
    PartitionReturned {
        worker_id: WorkerId,
        partition: Partition,
    },
    /// Phase-level inactivity timeout: no PartitionResult received from
    /// this worker within the configured `collect_timeout` (SPEC-06 R30).
    PhaseTimeout(WorkerId),
    /// Merge and post-merge reduce_all completed. `is_normal_form`
    /// indicates whether the merged-and-reduced net has an empty redex queue.
    MergeComplete { net: Net, is_normal_form: bool },
    /// Fatal error — triggers immediate shutdown.
    FatalError(String),

    // --- New events for SPEC-20 §4.1.2 (appended after existing variants) ---
    /// A worker completed the `Register`/`JoinRequest` handshake and has been
    /// accepted into `W_active` (SPEC-20 R10a, R11).
    WorkerJoined(WorkerId),

    /// A worker sent `LeaveRequest`; `LeaveKind` distinguishes clean
    /// (`AfterResult`) vs urgent departure (SPEC-20 R22a, R22b).
    WorkerLeft(WorkerId, LeaveKind),

    /// The TCP connection to a worker was lost unexpectedly (SPEC-20 R19).
    WorkerConnectionLost(WorkerId),

    /// The join-window timer (`JoinWindowMin` or `JoinWindowMax`) fired,
    /// closing the `AcceptingMembershipChanges` window (SPEC-20 R10, R10a).
    MembershipWindowClosed,

    /// The coordinator's self-partition task completed successfully and
    /// returned its round statistics (SPEC-20 R3, R3b, R7).
    SelfPartitionReduced(WorkerRoundStats),

    /// The coordinator's self-partition `spawn_blocking` task panicked or
    /// terminated unexpectedly (SPEC-20 R3a). The `String` carries the
    /// panic message.
    SelfPartitionPanic(String),

    /// The `initial_wait_timeout` timer fired before any worker connected
    /// (SPEC-20 R6). Transitions to `SoloReducing` when `hybrid_coordinator
    /// = true`, or to `Error` otherwise.
    InitialWaitTimeout,

    /// The `reduce_n(solo_budget)` loop detected an empty redex queue;
    /// reduction is complete (SPEC-20 R5a case (i)).
    SoloReductionComplete,

    /// One `reduce_n(solo_budget)` batch completed but redexes remain;
    /// the coordinator polls for `WorkerJoined` before the next batch
    /// (SPEC-20 R5, R5a case (ii) polling checkpoint).
    SoloReduceBatchComplete,
}

impl CoordinatorEvent {
    /// Constructs a `SelfPartitionPanic` event with a bounded payload (QA-002).
    ///
    /// If `msg` exceeds `MAX_PANIC_PAYLOAD_BYTES`, it is truncated to that
    /// limit and a `" [truncated]"` suffix is appended. This prevents
    /// coordinator OOM under fatal conditions where the panic message may
    /// carry a full backtrace or a large `Net` debug dump.
    ///
    /// # SPEC-20 R3a
    ///
    /// SPEC-20 R3a says "the `String` carries the panic message" without a
    /// bound; QA-002 adds the cap. A follow-up spec amendment to SPEC-20 R3a
    /// is tracked separately (`especialista-specs`).
    pub fn self_partition_panic(msg: impl Into<String>) -> Self {
        let mut s = msg.into();
        if s.len() > MAX_PANIC_PAYLOAD_BYTES {
            s.truncate(MAX_PANIC_PAYLOAD_BYTES);
            s.push_str(" [truncated]");
        }
        CoordinatorEvent::SelfPartitionPanic(s)
    }
}

/// Effective worker count for a round (remote workers + 1 if hybrid).
///
/// SPEC-20 §4.1.2: the `K_eff` value passed to `InvokeSplitAndDispatch`.
/// MUST be ≥ 1; a value of `0` is a SPEC-20 §4.1.4 logic violation — when
/// `K = 0 && hybrid_coordinator`, the FSM routes to `SoloReducing` instead
/// of invoking split. Document this contract at every call site.
pub type EffectiveWorkerCount = u32;

/// Actions the coordinator runtime must execute (SPEC-13 R20; extended by
/// SPEC-20 §3.8 A2 / §4.1.2).
///
/// `InvokeSplit` and `InvokeMergeAndReduce` preserve stimulus-response
/// purity: the runtime executes them (possibly via `spawn_blocking`)
/// and fires `SplitComplete` / `MergeComplete` events back into the FSM.
///
/// New SPEC-20 actions are appended after the existing SPEC-13 actions.
///
/// # API stability
///
/// `#[non_exhaustive]` is set so that external callers are forced to include a
/// wildcard arm in any `match` on this enum. The protocol layer (TASK-0418,
/// TASK-0436) MUST match on `CoordinatorAction` variants; adding `#[non_exhaustive]`
/// here ensures those match sites are updated when new actions land.
#[non_exhaustive]
#[derive(Debug)]
pub enum CoordinatorAction {
    // --- Existing actions from SPEC-13 R21 ---
    /// Bind the TCP listener on the given address.
    BindListener(SocketAddr),
    /// Send a message to a specific worker.
    SendMessage(WorkerId, Message),
    /// Invoke split(net, k). Fires SplitComplete when done.
    InvokeSplit { net: Net, num_workers: usize },
    /// Invoke merge(partitions) + reduce_all(merged_net).
    /// Fires MergeComplete { net, is_normal_form } when done.
    InvokeMergeAndReduce(Vec<Partition>),
    /// Start a named timer. Fires PhaseTimeout on expiry.
    StartTimer(TimerId, Duration),
    /// Cancel a previously started timer.
    CancelTimer(TimerId),
    /// Log a state transition at INFO level (SPEC-13 R23).
    LogTransition {
        from: CoordinatorState,
        to: CoordinatorState,
    },
    /// Write the final reduced network to the output file.
    WriteOutput(Net),
    /// Send Shutdown to all workers and close connections.
    ShutdownAll,

    // --- New actions for SPEC-20 §4.1.2 (appended after existing actions) ---
    /// Move worker `id` from the pending-connections queue into `W_active`
    /// (SPEC-20 R10a, R11). Fired during `AcceptingMembershipChanges`.
    RegisterWorker(WorkerId),

    /// Remove worker `id` from `W_active` immediately (SPEC-20 R22b, R24).
    RemoveWorker(WorkerId),

    /// Buffer worker `id` for registration at the next join window (SPEC-20
    /// R10b). Fired in any state other than `AcceptingMembershipChanges`.
    QueueWorkerForNextWindow(WorkerId),

    /// Reclaim the retained-state slot `slot` for departed worker `id` and
    /// queue it for re-introduction at the next `split()` (SPEC-20 R24a/b).
    ReclaimPartition(WorkerId, RetainedSlot),

    /// Emit an INFO-level log entry recording a worker join event (SPEC-20
    /// R17). Carries the joining worker's id.
    LogJoin(WorkerId),

    /// Emit a WARN-level log entry recording a worker departure event
    /// (SPEC-20 R28). Carries the worker id and departure reason.
    LogDeparture(WorkerId, DepartureKind),

    /// Invoke `split(net, K_eff)` followed by dispatch to all members of
    /// `W_active`, including the coordinator self-partition if `hybrid_coordinator
    /// = true`. Wraps the multi-step split + retained-state seeding + dispatch
    /// sequence into a single action for FSM readability (SPEC-20 §4.1.4).
    /// Fires `SplitComplete` / `AllDispatched` events back into the FSM.
    /// `K_eff` MUST be ≥ 1 per SPEC-20 §4.1.4; passing `0` is a logic violation.
    /// See `EffectiveWorkerCount` for the contract. When `K = 0 && hybrid_coordinator`,
    /// the FSM routes to `SoloReducing` instead (not here).
    InvokeSplitAndDispatch(EffectiveWorkerCount),

    /// Spawn the coordinator's in-process self-partition worker via
    /// `ChannelTransport` (SPEC-17 R15; SPEC-20 R3). The `Partition` is the
    /// self-partition selected during `InvokeSplitAndDispatch`. Fires
    /// `SelfPartitionReduced(stats)` on completion or `SelfPartitionPanic`
    /// on task failure.
    SpawnSelfPartition(Partition),

    /// Drain buffered pending connections from the coordinator's
    /// `pending_connections_queue` and begin the `Register`/`JoinRequest`
    /// handshake for each (SPEC-20 R10a). For each successfully completed
    /// handshake, fires `WorkerJoined(id)` back into the FSM.
    PollPendingConnections,
}

// ---------------------------------------------------------------------------
// FSM transition (TASK-0108, SPEC-13 R21)
// ---------------------------------------------------------------------------

/// Mutable context for the coordinator FSM.
///
/// Holds the state that the pure transition function needs to read
/// but that is not part of the event itself.
///
/// # Forward-readiness note
///
/// NOTE(TASK-0430/TASK-0436): Before any §4.1.4 guarded transition can be
/// implemented, this struct must be extended with the following fields:
/// - `hybrid_coordinator: bool` — coordinator reduces self-partition (SPEC-20 R3)
/// - `elastic_join: bool` — accept new workers mid-round (SPEC-20 R10)
/// - `elastic_departure: bool` — allow graceful departures mid-round (SPEC-20 R22)
/// - `solo_budget: u32` — max interactions per `reduce_n` solo batch (SPEC-20 R5a)
///
/// # Deduplication contract
///
/// No deduplication: callers MUST NOT deliver `WorkerConnected(id)` twice for
/// the same `id`. SPEC-20 R0d full-rejoin requires the runtime to suppress
/// duplicate connect events at the transport layer (TASK-0432). See QA-005.
#[derive(Debug)]
pub struct CoordinatorContext {
    /// Current FSM state.
    pub state: CoordinatorState,
    /// Number of workers required to start.
    pub min_workers: u32,
    /// Number of workers currently connected.
    pub connected_workers: u32,
    /// Bind address for the TCP listener.
    pub bind: SocketAddr,
    /// Number of partitions returned in the current round.
    pub partitions_received: u32,
    /// Total workers expected for partition collection.
    pub total_workers: u32,
    /// Collected partitions for the current round.
    pub collected_partitions: Vec<Partition>,
    /// Current BSP round number.
    pub round: u32,
    /// The current net (held between rounds).
    pub net: Option<Net>,
}

impl CoordinatorContext {
    pub fn new(bind: SocketAddr, min_workers: u32) -> Self {
        Self {
            state: CoordinatorState::Init,
            min_workers,
            connected_workers: 0,
            bind,
            partitions_received: 0,
            total_workers: min_workers,
            collected_partitions: Vec::new(),
            round: 0,
            net: None,
        }
    }
}

/// Pure transition function (SPEC-13 R20-R21).
///
/// Takes the coordinator context and an event, updates the state,
/// and returns a list of side-effectful actions for the async runtime
/// to execute.
pub fn transition(ctx: &mut CoordinatorContext, event: CoordinatorEvent) -> Vec<CoordinatorAction> {
    let from = ctx.state.clone();
    let mut actions = Vec::new();

    match (&ctx.state, event) {
        // Init → WaitingForWorkers
        (CoordinatorState::Init, CoordinatorEvent::ConfigLoaded) => {
            ctx.state = CoordinatorState::WaitingForWorkers;
            actions.push(CoordinatorAction::BindListener(ctx.bind));
            actions.push(CoordinatorAction::LogTransition {
                from,
                to: ctx.state.clone(),
            });
        }

        // WaitingForWorkers + WorkerConnected
        (CoordinatorState::WaitingForWorkers, CoordinatorEvent::WorkerConnected(_id)) => {
            ctx.connected_workers += 1;
            if ctx.connected_workers >= ctx.min_workers {
                ctx.total_workers = ctx.connected_workers;
                ctx.state = CoordinatorState::Partitioning;
                if let Some(net) = ctx.net.take() {
                    actions.push(CoordinatorAction::InvokeSplit {
                        net,
                        num_workers: ctx.total_workers as usize,
                    });
                }
                actions.push(CoordinatorAction::LogTransition {
                    from,
                    to: ctx.state.clone(),
                });
            } else {
                actions.push(CoordinatorAction::LogTransition {
                    from: from.clone(),
                    to: from,
                });
            }
        }

        // Partitioning + SplitComplete → Dispatching
        (CoordinatorState::Partitioning, CoordinatorEvent::SplitComplete(partitions)) => {
            ctx.state = CoordinatorState::Dispatching;
            for (i, part) in partitions.into_iter().enumerate() {
                actions.push(CoordinatorAction::SendMessage(
                    i as WorkerId,
                    Message::AssignPartition {
                        round: ctx.round,
                        partition: part,
                    },
                ));
            }
            actions.push(CoordinatorAction::LogTransition {
                from,
                to: ctx.state.clone(),
            });
        }

        // Dispatching + AllDispatched → WaitingForResults
        (CoordinatorState::Dispatching, CoordinatorEvent::AllDispatched) => {
            ctx.state = CoordinatorState::WaitingForResults;
            ctx.partitions_received = 0;
            ctx.collected_partitions.clear();
            // SPEC-20 §4.1.3: use TimerKind::Collect as TimerId (== 3) — NOT 0
            // (0 == TimerKind::InitialWait). Using the wrong id here would silently
            // collide with TASK-0436's InitialWait timer management.
            actions.push(CoordinatorAction::StartTimer(
                TimerKind::Collect as TimerId,
                Duration::from_secs(600),
            ));
            actions.push(CoordinatorAction::LogTransition {
                from,
                to: ctx.state.clone(),
            });
        }

        // WaitingForResults + PartitionReturned
        (
            CoordinatorState::WaitingForResults,
            CoordinatorEvent::PartitionReturned {
                worker_id: _,
                partition,
            },
        ) => {
            ctx.partitions_received += 1;
            ctx.collected_partitions.push(partition);

            if ctx.partitions_received >= ctx.total_workers {
                ctx.state = CoordinatorState::Merging;
                // SPEC-20 §4.1.3: cancel by TimerKind::Collect (== 3), not raw 0.
                actions.push(CoordinatorAction::CancelTimer(
                    TimerKind::Collect as TimerId,
                ));
                let partitions = std::mem::take(&mut ctx.collected_partitions);
                actions.push(CoordinatorAction::InvokeMergeAndReduce(partitions));
                actions.push(CoordinatorAction::LogTransition {
                    from,
                    to: ctx.state.clone(),
                });
            }
        }

        // WaitingForResults + PhaseTimeout → Error
        (CoordinatorState::WaitingForResults, CoordinatorEvent::PhaseTimeout(worker_id)) => {
            tracing::error!(worker_id, "phase timeout — worker did not respond");
            ctx.state = CoordinatorState::Error;
            actions.push(CoordinatorAction::LogTransition {
                from,
                to: ctx.state.clone(),
            });
            actions.push(CoordinatorAction::ShutdownAll);
        }

        // Merging + MergeComplete → CheckTermination
        (
            CoordinatorState::Merging,
            CoordinatorEvent::MergeComplete {
                net,
                is_normal_form,
            },
        ) => {
            ctx.state = CoordinatorState::CheckTermination;
            actions.push(CoordinatorAction::LogTransition {
                from: from.clone(),
                to: ctx.state.clone(),
            });

            if is_normal_form {
                ctx.state = CoordinatorState::Done;
                actions.push(CoordinatorAction::WriteOutput(net));
                actions.push(CoordinatorAction::ShutdownAll);
                actions.push(CoordinatorAction::LogTransition {
                    from: CoordinatorState::CheckTermination,
                    to: ctx.state.clone(),
                });
            } else {
                ctx.round += 1;
                ctx.state = CoordinatorState::Partitioning;
                actions.push(CoordinatorAction::InvokeSplit {
                    net,
                    num_workers: ctx.total_workers as usize,
                });
                actions.push(CoordinatorAction::LogTransition {
                    from: CoordinatorState::CheckTermination,
                    to: ctx.state.clone(),
                });
            }
        }

        // Any + FatalError → Error
        (_, CoordinatorEvent::FatalError(msg)) => {
            tracing::error!(error = %msg, "fatal error");
            ctx.state = CoordinatorState::Error;
            actions.push(CoordinatorAction::LogTransition {
                from,
                to: ctx.state.clone(),
            });
            actions.push(CoordinatorAction::ShutdownAll);
        }

        // NOTE(TASK-0436): Replace this wildcard with exhaustive arms before
        // landing transition wiring. Every new SPEC-20 event (WorkerJoined,
        // WorkerLeft, WorkerConnectionLost, MembershipWindowClosed,
        // SelfPartitionReduced, SelfPartitionPanic, InitialWaitTimeout,
        // SoloReductionComplete, SoloReduceBatchComplete) that still reaches
        // this arm after TASK-0436 lands is a missing transition row.
        // Test `ec_3_wildcard_arm_logs_unexpected_event_only` pins the
        // current (TASK-0414) contract: wildcard absorbs, no actions emitted.
        // SPEC-20 R10b requires explicit buffering of WorkerJoined in
        // WaitingForResults — that must NOT fall through here after TASK-0436.
        //
        // Unexpected event in current state — log and ignore
        (state, event) => {
            tracing::warn!(
                state = ?state,
                event = ?event,
                "unexpected event in current state"
            );
        }
    }

    actions
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // === TASK-0107: type tests ===

    #[test]
    fn test_coordinator_state_serialize() {
        let json = serde_json::to_string(&CoordinatorState::Init).unwrap();
        assert!(json.contains("Init"));
    }

    #[test]
    fn test_coordinator_state_equality() {
        assert_eq!(CoordinatorState::Init, CoordinatorState::Init);
        assert_ne!(CoordinatorState::Init, CoordinatorState::Done);
    }

    #[test]
    fn test_all_states_distinct() {
        // Updated by TASK-0414 to include SPEC-20 new states (appended at end).
        let states = vec![
            CoordinatorState::Init,
            CoordinatorState::WaitingForWorkers,
            CoordinatorState::Partitioning,
            CoordinatorState::Dispatching,
            CoordinatorState::WaitingForResults,
            CoordinatorState::Merging,
            CoordinatorState::CheckTermination,
            CoordinatorState::Done,
            CoordinatorState::Error,
            CoordinatorState::AcceptingMembershipChanges,
            CoordinatorState::SoloReducing,
        ];
        for i in 0..states.len() {
            for j in (i + 1)..states.len() {
                assert_ne!(states[i], states[j]);
            }
        }
    }

    // === TASK-0108: transition tests ===

    /// Shared test fixture: coordinator with min_workers=2 and a fresh Net.
    pub(super) fn make_ctx() -> CoordinatorContext {
        let mut ctx = CoordinatorContext::new("127.0.0.1:9000".parse().unwrap(), 2);
        ctx.net = Some(Net::new());
        ctx
    }

    /// Shared test fixture: coordinator in a specified starting state.
    pub(super) fn make_ctx_at_state(state: CoordinatorState) -> CoordinatorContext {
        let mut ctx = make_ctx();
        ctx.state = state;
        ctx
    }

    #[test]
    fn test_init_to_waiting() {
        let mut ctx = make_ctx();
        let actions = transition(&mut ctx, CoordinatorEvent::ConfigLoaded);
        assert_eq!(ctx.state, CoordinatorState::WaitingForWorkers);
        assert!(actions
            .iter()
            .any(|a| matches!(a, CoordinatorAction::BindListener(_))));
    }

    #[test]
    fn test_waiting_partial_workers() {
        let mut ctx = make_ctx();
        transition(&mut ctx, CoordinatorEvent::ConfigLoaded);
        transition(&mut ctx, CoordinatorEvent::WorkerConnected(0));
        // Only 1 of 2 workers connected — still waiting
        assert_eq!(ctx.state, CoordinatorState::WaitingForWorkers);
    }

    #[test]
    fn test_waiting_all_workers_triggers_split() {
        let mut ctx = make_ctx();
        transition(&mut ctx, CoordinatorEvent::ConfigLoaded);
        transition(&mut ctx, CoordinatorEvent::WorkerConnected(0));
        let actions = transition(&mut ctx, CoordinatorEvent::WorkerConnected(1));
        assert_eq!(ctx.state, CoordinatorState::Partitioning);
        assert!(actions
            .iter()
            .any(|a| matches!(a, CoordinatorAction::InvokeSplit { .. })));
    }

    #[test]
    fn test_split_complete_dispatches() {
        let mut ctx = make_ctx();
        ctx.state = CoordinatorState::Partitioning;
        ctx.total_workers = 2;

        let p1 = Partition {
            subnet: Net::new(),
            worker_id: 0,
            free_port_index: Default::default(),
            id_range: crate::partition::IdRange { start: 0, end: 100 },
            border_id_start: 0,
            border_id_end: 0,
        };
        let p2 = Partition {
            subnet: Net::new(),
            worker_id: 1,
            free_port_index: Default::default(),
            id_range: crate::partition::IdRange {
                start: 100,
                end: 200,
            },
            border_id_start: 0,
            border_id_end: 0,
        };

        let actions = transition(&mut ctx, CoordinatorEvent::SplitComplete(vec![p1, p2]));
        assert_eq!(ctx.state, CoordinatorState::Dispatching);
        let send_count = actions
            .iter()
            .filter(|a| matches!(a, CoordinatorAction::SendMessage(..)))
            .count();
        assert_eq!(send_count, 2);
    }

    #[test]
    fn test_all_dispatched_starts_timer() {
        let mut ctx = make_ctx();
        ctx.state = CoordinatorState::Dispatching;

        let actions = transition(&mut ctx, CoordinatorEvent::AllDispatched);
        assert_eq!(ctx.state, CoordinatorState::WaitingForResults);
        assert!(actions
            .iter()
            .any(|a| matches!(a, CoordinatorAction::StartTimer(..))));
    }

    #[test]
    fn test_all_dispatched_starts_collect_timer_with_correct_id() {
        // MF-001 regression: the collect timer MUST use TimerKind::Collect (== 3),
        // NOT 0 (== TimerKind::InitialWait). This test pins the corrected behaviour.
        let mut ctx = make_ctx();
        ctx.state = CoordinatorState::Dispatching;

        let actions = transition(&mut ctx, CoordinatorEvent::AllDispatched);
        let collect_id = TimerKind::Collect as TimerId;
        assert!(
            actions
                .iter()
                .any(|a| matches!(a, CoordinatorAction::StartTimer(id, _) if *id == collect_id)),
            "AllDispatched must start the Collect timer (id={collect_id}) per SPEC-20 §4.1.3, not timer 0"
        );
    }

    #[test]
    fn test_partition_returned_collects() {
        let mut ctx = make_ctx();
        ctx.state = CoordinatorState::WaitingForResults;
        ctx.total_workers = 2;

        let p = Partition {
            subnet: Net::new(),
            worker_id: 0,
            free_port_index: Default::default(),
            id_range: crate::partition::IdRange { start: 0, end: 100 },
            border_id_start: 0,
            border_id_end: 0,
        };

        transition(
            &mut ctx,
            CoordinatorEvent::PartitionReturned {
                worker_id: 0,
                partition: p.clone(),
            },
        );
        assert_eq!(ctx.state, CoordinatorState::WaitingForResults);

        let actions = transition(
            &mut ctx,
            CoordinatorEvent::PartitionReturned {
                worker_id: 1,
                partition: p,
            },
        );
        assert_eq!(ctx.state, CoordinatorState::Merging);
        assert!(actions
            .iter()
            .any(|a| matches!(a, CoordinatorAction::InvokeMergeAndReduce(_))));
    }

    #[test]
    fn test_all_partitions_returned_cancels_collect_timer_with_correct_id() {
        // MF-001 regression: CancelTimer must use TimerKind::Collect (== 3).
        let mut ctx = make_ctx();
        ctx.state = CoordinatorState::WaitingForResults;
        ctx.total_workers = 1;

        let p = Partition {
            subnet: Net::new(),
            worker_id: 0,
            free_port_index: Default::default(),
            id_range: crate::partition::IdRange { start: 0, end: 100 },
            border_id_start: 0,
            border_id_end: 0,
        };

        let actions = transition(
            &mut ctx,
            CoordinatorEvent::PartitionReturned {
                worker_id: 0,
                partition: p,
            },
        );
        let collect_id = TimerKind::Collect as TimerId;
        assert!(
            actions
                .iter()
                .any(|a| matches!(a, CoordinatorAction::CancelTimer(id) if *id == collect_id)),
            "all-partitions-returned must cancel the Collect timer (id={collect_id}) per SPEC-20 §4.1.3"
        );
    }

    #[test]
    fn test_merge_complete_normal_form() {
        let mut ctx = make_ctx();
        ctx.state = CoordinatorState::Merging;

        let actions = transition(
            &mut ctx,
            CoordinatorEvent::MergeComplete {
                net: Net::new(),
                is_normal_form: true,
            },
        );
        assert_eq!(ctx.state, CoordinatorState::Done);
        assert!(actions
            .iter()
            .any(|a| matches!(a, CoordinatorAction::WriteOutput(_))));
        assert!(actions
            .iter()
            .any(|a| matches!(a, CoordinatorAction::ShutdownAll)));
    }

    #[test]
    fn test_merge_complete_not_normal_form() {
        let mut ctx = make_ctx();
        ctx.state = CoordinatorState::Merging;

        let actions = transition(
            &mut ctx,
            CoordinatorEvent::MergeComplete {
                net: Net::new(),
                is_normal_form: false,
            },
        );
        assert_eq!(ctx.state, CoordinatorState::Partitioning);
        assert!(actions
            .iter()
            .any(|a| matches!(a, CoordinatorAction::InvokeSplit { .. })));
        assert_eq!(ctx.round, 1);
    }

    #[test]
    fn test_phase_timeout_goes_to_error() {
        let mut ctx = make_ctx();
        ctx.state = CoordinatorState::WaitingForResults;

        let actions = transition(&mut ctx, CoordinatorEvent::PhaseTimeout(0));
        assert_eq!(ctx.state, CoordinatorState::Error);
        assert!(actions
            .iter()
            .any(|a| matches!(a, CoordinatorAction::ShutdownAll)));
    }

    #[test]
    fn test_fatal_error_from_any_state() {
        for initial in &[
            CoordinatorState::Init,
            CoordinatorState::WaitingForWorkers,
            CoordinatorState::Partitioning,
            CoordinatorState::Dispatching,
            CoordinatorState::WaitingForResults,
            CoordinatorState::Merging,
        ] {
            let mut ctx = make_ctx();
            ctx.state = initial.clone();
            let actions = transition(&mut ctx, CoordinatorEvent::FatalError("crash".into()));
            assert_eq!(ctx.state, CoordinatorState::Error);
            assert!(actions
                .iter()
                .any(|a| matches!(a, CoordinatorAction::ShutdownAll)));
        }
    }

    // === TASK-0414: SPEC-20 §3.8 A2 enum surface tests ===
    // TEST-SPEC-0414 (UT-0414-01 through UT-0414-09 + EC-1, EC-2 + Stage 6 additions)

    // UT-0414-01: New CoordinatorState variants construct and round-trip Debug.
    #[test]
    fn coordinator_state_new_variants_construct() {
        let amc = CoordinatorState::AcceptingMembershipChanges;
        let solo = CoordinatorState::SoloReducing;
        // Debug round-trip: format must contain the variant name.
        assert!(
            format!("{:?}", amc).contains("AcceptingMembershipChanges"),
            "AcceptingMembershipChanges Debug repr unexpected"
        );
        assert!(
            format!("{:?}", solo).contains("SoloReducing"),
            "SoloReducing Debug repr unexpected"
        );
    }

    // UT-0414-02: New CoordinatorState variants satisfy PartialEq reflexivity.
    #[test]
    fn coordinator_state_partial_eq_self() {
        assert_eq!(
            CoordinatorState::AcceptingMembershipChanges,
            CoordinatorState::AcceptingMembershipChanges,
            "AcceptingMembershipChanges must equal itself"
        );
        assert_eq!(
            CoordinatorState::SoloReducing,
            CoordinatorState::SoloReducing,
            "SoloReducing must equal itself"
        );
    }

    // UT-0414-03: New CoordinatorState variants are not equal to any
    // existing variant — no accidental aliasing.
    #[test]
    fn coordinator_state_inequality_with_existing_variants() {
        let new_states = [
            CoordinatorState::AcceptingMembershipChanges,
            CoordinatorState::SoloReducing,
        ];
        let existing_states = [
            CoordinatorState::Init,
            CoordinatorState::WaitingForWorkers,
            CoordinatorState::Partitioning,
            CoordinatorState::Dispatching,
            CoordinatorState::WaitingForResults,
            CoordinatorState::Merging,
            CoordinatorState::CheckTermination,
            CoordinatorState::Done,
            CoordinatorState::Error,
        ];
        for ns in &new_states {
            for es in &existing_states {
                assert_ne!(
                    ns, es,
                    "new state {:?} must not alias existing state {:?}",
                    ns, es
                );
            }
        }
        // Also assert the two new states are distinct from each other.
        assert_ne!(
            CoordinatorState::AcceptingMembershipChanges,
            CoordinatorState::SoloReducing,
            "AcceptingMembershipChanges and SoloReducing must be distinct"
        );
    }

    // UT-0414-04: All 9 new CoordinatorEvent variants construct without panic.
    #[test]
    fn coordinator_event_all_new_variants_construct() {
        use crate::merge::WorkerRoundStats;

        let stats = WorkerRoundStats {
            worker_id: 7,
            agents_before: 10,
            agents_after: 5,
            local_redexes: 5,
            reduce_duration_secs: 0.001,
            interactions_by_rule: [1, 0, 0, 0, 0, 0],
            has_border_activity: false,
        };

        // Construct each of the 9 new events.
        let events: Vec<CoordinatorEvent> = vec![
            CoordinatorEvent::WorkerJoined(7),
            CoordinatorEvent::WorkerLeft(7, LeaveKind::AfterResult),
            CoordinatorEvent::WorkerConnectionLost(7),
            CoordinatorEvent::MembershipWindowClosed,
            CoordinatorEvent::SelfPartitionReduced(stats),
            CoordinatorEvent::SelfPartitionPanic("boom".into()),
            CoordinatorEvent::InitialWaitTimeout,
            CoordinatorEvent::SoloReductionComplete,
            CoordinatorEvent::SoloReduceBatchComplete,
        ];
        // Verify all 9 are constructible (no compile or runtime error).
        assert_eq!(events.len(), 9, "expected exactly 9 new SPEC-20 events");
    }

    // UT-0414-05: Debug format of each new event starts with the variant name.
    #[test]
    fn coordinator_event_debug_format_contains_variant_name() {
        use crate::merge::WorkerRoundStats;

        let stats = WorkerRoundStats {
            worker_id: 1,
            agents_before: 0,
            agents_after: 0,
            local_redexes: 0,
            reduce_duration_secs: 0.0,
            interactions_by_rule: [0; 6],
            has_border_activity: false,
        };

        let cases: Vec<(&str, CoordinatorEvent)> = vec![
            ("WorkerJoined", CoordinatorEvent::WorkerJoined(1)),
            (
                "WorkerLeft",
                CoordinatorEvent::WorkerLeft(1, LeaveKind::AfterResult),
            ),
            (
                "WorkerConnectionLost",
                CoordinatorEvent::WorkerConnectionLost(1),
            ),
            (
                "MembershipWindowClosed",
                CoordinatorEvent::MembershipWindowClosed,
            ),
            (
                "SelfPartitionReduced",
                CoordinatorEvent::SelfPartitionReduced(stats),
            ),
            (
                "SelfPartitionPanic",
                CoordinatorEvent::SelfPartitionPanic("test".into()),
            ),
            ("InitialWaitTimeout", CoordinatorEvent::InitialWaitTimeout),
            (
                "SoloReductionComplete",
                CoordinatorEvent::SoloReductionComplete,
            ),
            (
                "SoloReduceBatchComplete",
                CoordinatorEvent::SoloReduceBatchComplete,
            ),
        ];

        for (expected_prefix, event) in &cases {
            let dbg = format!("{:?}", event);
            assert!(
                dbg.starts_with(*expected_prefix),
                "expected Debug of {dbg:?} to start with {expected_prefix:?}"
            );
        }
    }

    // UT-0414-06: All 9 new CoordinatorAction variants construct and
    // round-trip Debug.
    #[test]
    fn coordinator_action_all_new_variants_construct() {
        use crate::net::Net;
        use crate::partition::{IdRange, Partition};

        let p = Partition {
            subnet: Net::new(),
            worker_id: 7,
            free_port_index: Default::default(),
            id_range: IdRange { start: 0, end: 100 },
            border_id_start: 0,
            border_id_end: 0,
        };

        let actions: Vec<CoordinatorAction> = vec![
            CoordinatorAction::RegisterWorker(7),
            CoordinatorAction::RemoveWorker(7),
            CoordinatorAction::QueueWorkerForNextWindow(7),
            CoordinatorAction::ReclaimPartition(7, RetainedSlot::Initial),
            CoordinatorAction::LogJoin(7),
            CoordinatorAction::LogDeparture(7, DepartureKind::Timeout),
            CoordinatorAction::InvokeSplitAndDispatch(3),
            CoordinatorAction::SpawnSelfPartition(p),
            CoordinatorAction::PollPendingConnections,
        ];

        // 9 new SPEC-20 actions must all be constructible.
        assert_eq!(actions.len(), 9, "expected exactly 9 new SPEC-20 actions");

        // Debug round-trip must not panic and must be non-empty.
        for action in &actions {
            let dbg = format!("{:?}", action);
            assert!(!dbg.is_empty(), "Debug of action must be non-empty");
        }
    }

    // UT-0414-07: RetainedSlot has exactly two variants; PartialEq across
    // cases is correct.
    #[test]
    fn retained_slot_two_variants() {
        assert_eq!(RetainedSlot::Initial, RetainedSlot::Initial);
        assert_eq!(RetainedSlot::LastAcked, RetainedSlot::LastAcked);
        assert_ne!(
            RetainedSlot::Initial,
            RetainedSlot::LastAcked,
            "Initial and LastAcked must be distinct"
        );
        // Debug must contain variant names.
        assert!(format!("{:?}", RetainedSlot::Initial).contains("Initial"));
        assert!(format!("{:?}", RetainedSlot::LastAcked).contains("LastAcked"));
    }

    // UT-0414-08: DepartureKind has exactly four variants; PartialEq across
    // all pairs is correct.
    #[test]
    fn departure_kind_four_variants() {
        let kinds = [
            DepartureKind::Timeout,
            DepartureKind::ConnLost,
            DepartureKind::LeaveAfter,
            DepartureKind::LeaveUrgent,
        ];
        // Reflexive equality.
        for k in &kinds {
            assert_eq!(k, k, "{:?} must equal itself", k);
        }
        // Cross-pair inequality.
        for i in 0..kinds.len() {
            for j in (i + 1)..kinds.len() {
                assert_ne!(
                    kinds[i], kinds[j],
                    "DepartureKind variants {:?} and {:?} must be distinct",
                    kinds[i], kinds[j]
                );
            }
        }
        // Debug must contain variant name for each.
        assert!(format!("{:?}", DepartureKind::Timeout).contains("Timeout"));
        assert!(format!("{:?}", DepartureKind::ConnLost).contains("ConnLost"));
        assert!(format!("{:?}", DepartureKind::LeaveAfter).contains("LeaveAfter"));
        assert!(format!("{:?}", DepartureKind::LeaveUrgent).contains("LeaveUrgent"));
    }

    // UT-0414-09: Existing AND new CoordinatorState variants serialize to their
    // expected JSON string values — verifies discriminant stability and serde
    // format for all 11 variants (SF-003 / QA-010 fix: new variants in the loop,
    // not as separate spot-checks).
    #[test]
    fn existing_variants_unchanged() {
        // serde::Serialize uses variant-name strings for unit variants.
        // Including ALL 11 variants (9 existing + 2 new) in the same systematic
        // loop ensures that renames of ANY variant are caught — not just the
        // existing 9.
        let expected: Vec<(&str, CoordinatorState)> = vec![
            ("\"Init\"", CoordinatorState::Init),
            ("\"WaitingForWorkers\"", CoordinatorState::WaitingForWorkers),
            ("\"Partitioning\"", CoordinatorState::Partitioning),
            ("\"Dispatching\"", CoordinatorState::Dispatching),
            ("\"WaitingForResults\"", CoordinatorState::WaitingForResults),
            ("\"Merging\"", CoordinatorState::Merging),
            ("\"CheckTermination\"", CoordinatorState::CheckTermination),
            ("\"Done\"", CoordinatorState::Done),
            ("\"Error\"", CoordinatorState::Error),
            // New SPEC-20 variants (appended, preserving discriminant stability):
            (
                "\"AcceptingMembershipChanges\"",
                CoordinatorState::AcceptingMembershipChanges,
            ),
            ("\"SoloReducing\"", CoordinatorState::SoloReducing),
        ];
        for (json_str, state) in &expected {
            let serialized =
                serde_json::to_string(state).expect("CoordinatorState must be serializable");
            assert_eq!(
                serialized, *json_str,
                "serialization of {:?} changed — discriminant stability violated",
                state
            );
        }
    }

    // EC-1: SelfPartitionPanic with an empty-string payload is constructible;
    // it must not trigger a special case or panic.
    #[test]
    fn coordinator_event_self_partition_panic_empty_payload() {
        let event = CoordinatorEvent::SelfPartitionPanic(String::new());
        let dbg = format!("{:?}", event);
        assert!(
            dbg.contains("SelfPartitionPanic"),
            "empty-payload SelfPartitionPanic must still format correctly"
        );
    }

    // EC-2: WorkerLeft with LeaveKind::Urgent is constructible and carries
    // the kind correctly.
    #[test]
    fn coordinator_event_worker_left_urgent() {
        let event = CoordinatorEvent::WorkerLeft(42, LeaveKind::Urgent);
        let dbg = format!("{:?}", event);
        assert!(
            dbg.contains("WorkerLeft"),
            "WorkerLeft Debug must contain variant name"
        );
        assert!(
            dbg.contains("Urgent"),
            "WorkerLeft(Urgent) Debug must contain LeaveKind::Urgent"
        );
    }

    // --- Stage 6 REFACTOR additions (REVIEW + QA blockers) ---

    // QA-001 / EC-A: Pin the TASK-0414 wildcard-absorber contract.
    // Every new SPEC-20 event in every existing non-terminal state hits the
    // wildcard arm and produces zero actions today. TASK-0436 MUST replace this
    // with explicit transition arms; if any (state, new_event) cell still emits
    // zero actions after TASK-0436, that is a contract violation.
    #[test]
    fn ec_3_wildcard_arm_logs_unexpected_event_only() {
        // Arrange: new_events() returns a freshly-constructed vec of the 6 simple
        // unit/single-value new events. WorkerLeft (requires LeaveKind) is tested
        // in the separate loop below. SelfPartitionReduced and SelfPartitionPanic
        // are not in this loop (move-only / requires test construction), but the
        // constructor tests above exercise them.
        let new_events = || {
            vec![
                CoordinatorEvent::WorkerJoined(7),
                CoordinatorEvent::WorkerConnectionLost(7),
                CoordinatorEvent::MembershipWindowClosed,
                CoordinatorEvent::InitialWaitTimeout,
                CoordinatorEvent::SoloReductionComplete,
                CoordinatorEvent::SoloReduceBatchComplete,
            ]
        };
        let states = [
            CoordinatorState::Init,
            CoordinatorState::WaitingForWorkers,
            CoordinatorState::Partitioning,
            CoordinatorState::Dispatching,
            CoordinatorState::WaitingForResults,
            CoordinatorState::Merging,
            CoordinatorState::CheckTermination,
            // Done / Error excluded — terminal states accept FatalError only.
        ];
        for state in &states {
            for event in new_events() {
                let mut ctx = make_ctx_at_state(state.clone());
                let actions = transition(&mut ctx, event);
                // CURRENT contract (TASK-0414 baseline): wildcard absorbs, no
                // actions are emitted and state is unchanged.
                // TASK-0436 MUST flip this to non-empty for every owned cell.
                // If this assertion fails it means TASK-0436 partially landed —
                // update the assertion to check the new contract instead.
                assert!(
                    actions.is_empty(),
                    "QA-001: wildcard absorbed new event but produced actions in state {:?}; \
                     did TASK-0436 partially land? Update this test to assert the new contract.",
                    state
                );
                // State must also be unchanged — wildcard must not mutate ctx.
                assert_eq!(
                    ctx.state, *state,
                    "QA-001: wildcard arm must not mutate state (was {:?})",
                    state
                );
            }
        }
        // Also cover LeaveKind-carrying variant (WorkerLeft).
        for state in &states {
            let mut ctx = make_ctx_at_state(state.clone());
            let actions = transition(
                &mut ctx,
                CoordinatorEvent::WorkerLeft(42, LeaveKind::AfterResult),
            );
            assert!(
                actions.is_empty(),
                "QA-001: WorkerLeft wildcard produced actions in state {:?}",
                state
            );
            assert_eq!(
                ctx.state, *state,
                "QA-001: WorkerLeft wildcard must not mutate state {:?}",
                state
            );
        }
    }

    // QA-002 / EC-B: SelfPartitionPanic payload is truncated to MAX_PANIC_PAYLOAD_BYTES
    // when constructed via the safe constructor. Direct variant construction still
    // allows unbounded payloads (by design, for tests), so we test the constructor.
    #[test]
    fn self_partition_panic_constructor_truncates_large_payload() {
        // Arrange: an 8 KiB string — double the cap.
        let large = "X".repeat(8192);
        let event = CoordinatorEvent::self_partition_panic(large.clone());

        // Assert: payload is truncated to <= MAX_PANIC_PAYLOAD_BYTES + "[truncated]".len()
        match event {
            CoordinatorEvent::SelfPartitionPanic(ref s) => {
                assert!(
                    s.len() <= MAX_PANIC_PAYLOAD_BYTES + " [truncated]".len(),
                    "QA-002: panic payload must be <= MAX_PANIC_PAYLOAD_BYTES ({}) + truncation marker; got {}",
                    MAX_PANIC_PAYLOAD_BYTES,
                    s.len()
                );
                assert!(
                    s.ends_with("[truncated]"),
                    "QA-002: truncated payload must end with '[truncated]'; got: {:?}",
                    &s[s.len().saturating_sub(20)..]
                );
            }
            _ => panic!("expected SelfPartitionPanic variant"),
        }
    }

    // QA-002 companion: small payload is not truncated.
    #[test]
    fn self_partition_panic_constructor_preserves_small_payload() {
        let small = "oom in reduce_n: out of memory";
        let event = CoordinatorEvent::self_partition_panic(small);
        match event {
            CoordinatorEvent::SelfPartitionPanic(s) => {
                assert_eq!(
                    s, small,
                    "QA-002: small payload must pass through unchanged"
                );
            }
            _ => panic!("expected SelfPartitionPanic variant"),
        }
    }

    // QA-003 / EC-C: TimerKind discriminants are pinned to their normative values
    // per SPEC-20 §4.1.3. A future variant insertion that shifts these values is
    // a wire-decoder break — fail loudly here.
    #[test]
    fn timer_kind_discriminant_stability() {
        assert_eq!(
            TimerKind::InitialWait as u32,
            0,
            "QA-003: TimerKind::InitialWait must be 0 per SPEC-20 §4.1.3"
        );
        assert_eq!(
            TimerKind::JoinWindowMin as u32,
            1,
            "QA-003: TimerKind::JoinWindowMin must be 1 per SPEC-20 §4.1.3"
        );
        assert_eq!(
            TimerKind::JoinWindowMax as u32,
            2,
            "QA-003: TimerKind::JoinWindowMax must be 2 per SPEC-20 §4.1.3"
        );
        assert_eq!(
            TimerKind::Collect as u32,
            3,
            "QA-003: TimerKind::Collect must be 3 per SPEC-20 §4.1.3"
        );
    }

    // QA-005: Pin the current no-dedup behaviour for WorkerConnected.
    // A duplicate WorkerConnected event increments connected_workers twice
    // (no dedup). SPEC-20 R0d full-rejoin requires the *runtime* to suppress
    // duplicates at the transport layer (TASK-0432). This test documents the
    // current FSM-level contract so that a future dedup fix in the FSM is
    // a conscious, visible change (not a silent regression).
    #[test]
    fn worker_connected_duplicate_id_increments_naively() {
        let mut ctx = make_ctx();
        transition(&mut ctx, CoordinatorEvent::ConfigLoaded);
        // Deliver worker 0 twice — naive increment, no dedup.
        transition(&mut ctx, CoordinatorEvent::WorkerConnected(0));
        transition(&mut ctx, CoordinatorEvent::WorkerConnected(0));
        assert_eq!(
            ctx.connected_workers, 2,
            "QA-005: duplicate WorkerConnected(0) increments naively (no dedup). \
             If this fails, dedup was added — update to assert the new contract."
        );
    }

    // QA-006: Pin exact Debug format for new events to lock the shape against
    // a future tuple→struct variant refactor. Log scrapers that parse Debug
    // output depend on this format. If this test fails after a refactor, update
    // all log-consumer regexes before merging.
    #[test]
    fn coordinator_event_debug_format_exact() {
        assert_eq!(
            format!("{:?}", CoordinatorEvent::WorkerJoined(7)),
            "WorkerJoined(7)",
            "QA-006: WorkerJoined Debug format must be 'WorkerJoined(7)'"
        );
        assert_eq!(
            format!("{:?}", CoordinatorEvent::WorkerLeft(42, LeaveKind::Urgent)),
            "WorkerLeft(42, Urgent)",
            "QA-006: WorkerLeft Debug format must be 'WorkerLeft(42, Urgent)'"
        );
        assert_eq!(
            format!("{:?}", CoordinatorEvent::SelfPartitionPanic("x".into())),
            "SelfPartitionPanic(\"x\")",
            "QA-006: SelfPartitionPanic Debug format must be 'SelfPartitionPanic(\"x\")'"
        );
        assert_eq!(
            format!("{:?}", CoordinatorEvent::WorkerConnectionLost(3)),
            "WorkerConnectionLost(3)",
            "QA-006: WorkerConnectionLost Debug format must be 'WorkerConnectionLost(3)'"
        );
    }

    // QA-010 / SF-003: Pin the serde format as bare unit-variant strings —
    // no #[serde(tag = "...")] or #[serde(rename_all)] — so that any future
    // maintainer who adds those attributes gets a failing test and must
    // consciously migrate all log consumers.
    #[test]
    fn coordinator_state_serde_format_is_unit_variant_string() {
        let json = serde_json::to_string(&CoordinatorState::Init).unwrap();
        assert_eq!(
            json, "\"Init\"",
            "QA-010: CoordinatorState must serialize as a bare JSON string — \
             no #[serde(tag = ...)] or rename; changing this is a log-consumer break."
        );
        // Spot-check a second variant to confirm it is not a single-variant fluke.
        let json2 = serde_json::to_string(&CoordinatorState::WaitingForResults).unwrap();
        assert_eq!(json2, "\"WaitingForResults\"");
    }
}
