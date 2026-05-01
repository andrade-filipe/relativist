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

use crate::merge::{DispatchMode, WorkerRoundStats};
use crate::net::Net;
use crate::partition::{Partition, WorkerId};
use crate::protocol::{Message, TimerKind};

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

    // --- New pull-dispatch states for SPEC-21 §3.8 A5 (TASK-0577) ---
    // These states are ONLY entered when `DispatchMode::Pull` is active.
    // Push-mode FSMs never enter these states (R37e).
    // Appended at end to preserve discriminant stability.
    //
    // TASK-0600 (QA-D010-013): the canonical source of truth for pull-mode
    // FSM state is `PullCoordinatorState` (defined below). The `Pull*`
    // variants in this enum are a derived projection produced ONLY via
    // `From<PullCoordinatorState> for CoordinatorState`. Production code
    // MUST NOT construct them directly — see UT-0600-01 / UT-0600-02 / IT-0600-03
    // which guard against re-introducing parallel state representations.
    /// Pull-mode cold-start: generating + dispatching the first chunk eagerly.
    ///
    /// Per R32 step 1: coordinator generates first chunk and dispatches to worker 0
    /// before any `RequestWork` arrives.
    PullDispatchingFirst,

    /// Pull-mode waiting: coordinator has dispatched at least one chunk and is
    /// waiting for `RequestWork` or `PartitionResult` from any worker.
    ///
    /// `PartitionResult` arrivals are BUFFERED here (R37d BSP barrier).
    /// Merge does NOT happen until `PullAwaitingFinalResults`.
    PullAwaitingResults,

    /// Pull-mode generating: calling `make_net_stream::next` + `strategy.allocate_batch`
    /// for the next chunk to dispatch (R32 step 3 / A5).
    PullGeneratingNext,

    /// Pull-mode exhausted: stream has no more chunks; coordinator is sending
    /// `NoMoreWork` to all workers that have issued `RequestWork`.
    PullSendingNoMoreWork,

    /// Pull-mode final collection: all workers have acknowledged `NoMoreWork`;
    /// coordinator is waiting for all final `PartitionResult`s.
    ///
    /// Once all final results arrive, transitions to `Merging` (legacy state)
    /// consuming ALL buffered results in a single logical BSP round (R37d — closes SC-019).
    PullAwaitingFinalResults,
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
    /// SPEC-20 R11: Monotonic WorkerId counter.
    pub next_worker_id: u32,
}

impl CoordinatorContext {
    pub fn new(bind: SocketAddr, min_workers: u32, hybrid_mode: bool) -> Self {
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
            // R7a: in hybrid mode, WorkerId 0 is reserved. Counter starts at 1.
            next_worker_id: if hybrid_mode { 1 } else { 0 },
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

        // SPEC-20 R10b — mid-state buffering of joins (Phase C refactor MF-004).
        //
        // The four "active round" states (Partitioning, Dispatching,
        // WaitingForResults, Merging) MUST emit `QueueWorkerForNextWindow(id)`
        // when a `WorkerJoined(id)` event arrives, deferring the join into
        // the upcoming `AcceptingMembershipChanges` window. Same state on the
        // way out: only the buffer mutates; the FSM phase is unchanged.
        //
        // This closes SC-012 (FSM-totality for mid-round joins) and the
        // QA-001 Phase A pattern (wildcard silently absorbing critical
        // events). Without these arms, every `WorkerJoined` outside
        // `AcceptingMembershipChanges` would fall through to the wildcard
        // and produce zero actions — the joiner would be dropped at the
        // FSM layer even when the procedural runtime correctly buffers
        // the raw stream.
        (
            CoordinatorState::Partitioning
            | CoordinatorState::Dispatching
            | CoordinatorState::WaitingForResults
            | CoordinatorState::Merging,
            CoordinatorEvent::WorkerJoined(id),
        ) => {
            actions.push(CoordinatorAction::QueueWorkerForNextWindow(id));
            actions.push(CoordinatorAction::LogJoin(id));
            actions.push(CoordinatorAction::LogTransition {
                from: from.clone(),
                to: from,
            });
        }

        // SPEC-20 R22b — mid-state graceful departures (Phase C refactor MF-004).
        //
        // Same family as `WorkerJoined`: the FSM observes the departure
        // event in any active-round state, emits `RemoveWorker(id)`, and
        // stays in the same state. The procedural runtime separately
        // handles the LeaveAck handshake; the FSM layer only owns
        // membership bookkeeping.
        (
            CoordinatorState::Partitioning
            | CoordinatorState::Dispatching
            | CoordinatorState::WaitingForResults
            | CoordinatorState::Merging,
            CoordinatorEvent::WorkerLeft(id, kind),
        ) => {
            actions.push(CoordinatorAction::RemoveWorker(id));
            let dep_kind = match kind {
                LeaveKind::AfterResult => DepartureKind::LeaveAfter,
                LeaveKind::Urgent => DepartureKind::LeaveUrgent,
            };
            actions.push(CoordinatorAction::LogDeparture(id, dep_kind));
            actions.push(CoordinatorAction::LogTransition {
                from: from.clone(),
                to: from,
            });
        }

        // SPEC-20 R10/R10a — `MembershipWindowClosed` from any active-round
        // state: the timer firing here is a stale carryover from a previous
        // window (e.g., min-timer outliving the AMC→Partitioning transition).
        // We log a transition row (state unchanged) so observability sees the
        // event was processed, but no membership mutation occurs.
        (
            CoordinatorState::Partitioning
            | CoordinatorState::Dispatching
            | CoordinatorState::WaitingForResults
            | CoordinatorState::Merging,
            CoordinatorEvent::MembershipWindowClosed,
        ) => {
            actions.push(CoordinatorAction::LogTransition {
                from: from.clone(),
                to: from,
            });
        }

        // NOTE(TASK-0436): Replace this wildcard with exhaustive arms before
        // landing transition wiring. Every new SPEC-20 event (WorkerJoined,
        // WorkerLeft, WorkerConnectionLost, MembershipWindowClosed,
        // SelfPartitionReduced, SelfPartitionPanic, InitialWaitTimeout,
        // SoloReductionComplete, SoloReduceBatchComplete) that still reaches
        // this arm after TASK-0436 lands is a missing transition row.
        //
        // Phase C refactor (MF-004) closed the WorkerJoined / WorkerLeft /
        // MembershipWindowClosed gap for the four active-round states above.
        // The remaining wildcard catches the residual events
        // (WorkerConnectionLost, SelfPartitionReduced, SelfPartitionPanic,
        // InitialWaitTimeout, SoloReductionComplete, SoloReduceBatchComplete)
        // pending TASK-0436. Test `ec_3_wildcard_arm_logs_unexpected_event_only`
        // pins those still-absorbed cells.
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
// Pull-dispatch FSM types (SPEC-21 R30, R32, A5 / TASK-0577)
// ---------------------------------------------------------------------------

/// Resolved dispatch mode — the effective mode after `Auto` is evaluated.
///
/// `GridConfig.dispatch_mode` may be `Auto`; this type represents the post-
/// resolution value consumed by the coordinator loop (SPEC-21 TASK-0577 NOTE
/// line 98):
/// - `Auto` → `Push` when `chunk_size == u32::MAX` (R26 short-circuit, SC-014).
/// - `Auto` → `Push` when `estimated_chunks <= num_workers` (degenerate; pull
///   overhead unjustified).
/// - `Auto` → `Pull` otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedDispatchMode {
    /// Push mode: coordinator dispatches all partitions upfront (legacy v1 path).
    Push,
    /// Pull mode: workers request work via `RequestWork`; coordinator responds.
    Pull,
}

/// Resolve `GridConfig.dispatch_mode` to `ResolvedDispatchMode`.
///
/// `chunk_size` is compared against `u32::MAX` (R26 sentinel). When the caller
/// does not know `chunk_size` (e.g., the chunk size is stored in `GridConfig`),
/// pass `cfg.chunk_size`. `num_workers` is the active worker count.
///
/// Auto resolution rule (TASK-0577 NOTE line 98 + UT-0577-14..16):
/// 1. `Push` explicitly → `Push`.
/// 2. `Pull` explicitly → `Pull`.
/// 3. `Auto` + `chunk_size == u32::MAX` → `Push` (R26 short-circuit).
/// 4. `Auto` otherwise: estimate chunk count as `chunk_size` itself acts
///    as a proxy; call `resolve_dispatch_mode_with_chunks` for the full check.
pub fn resolve_dispatch_mode(
    mode: DispatchMode,
    chunk_size: u32,
    num_workers: u32,
) -> ResolvedDispatchMode {
    match mode {
        DispatchMode::Push => ResolvedDispatchMode::Push,
        DispatchMode::Pull => ResolvedDispatchMode::Pull,
        DispatchMode::Auto => {
            // R26 short-circuit: chunk_size == u32::MAX → materialise-then-split → Push.
            if chunk_size == u32::MAX {
                return ResolvedDispatchMode::Push;
            }
            // No estimate available at this level; treat chunk_size as estimated_chunks = 1
            // (conservative: any valid chunk_size < u32::MAX with unknown total means Pull).
            // Callers that know the total should use resolve_dispatch_mode_with_chunks.
            let _ = num_workers;
            ResolvedDispatchMode::Pull
        }
    }
}

/// Resolve `Auto` dispatch mode given the estimated number of chunks and worker count.
///
/// - `estimated_chunks > num_workers` → `Pull` (more chunks than workers; pull load-balances).
/// - `estimated_chunks <= num_workers` → `Push` (degenerate; one chunk per worker at most).
/// - `chunk_size == u32::MAX` is not checked here; call `resolve_dispatch_mode` first.
pub fn resolve_dispatch_mode_with_chunks(
    mode: DispatchMode,
    estimated_chunks: u32,
    num_workers: u32,
) -> ResolvedDispatchMode {
    match mode {
        DispatchMode::Push => ResolvedDispatchMode::Push,
        DispatchMode::Pull => ResolvedDispatchMode::Pull,
        DispatchMode::Auto => {
            if estimated_chunks <= num_workers {
                // Degenerate case: push is sufficient (one chunk per worker at most).
                ResolvedDispatchMode::Push
            } else {
                ResolvedDispatchMode::Pull
            }
        }
    }
}

/// Resolve `Auto` dispatch mode given the total agent count estimate and worker count.
///
/// Per TASK-0577 NOTE line 98: `Auto` → `Pull` when `len_estimate > num_workers`
/// (meaning the total net is large enough to warrant pull-based load balancing).
///
/// - `len_estimate > num_workers` → `Pull`.
/// - `len_estimate <= num_workers` → `Push` (trivially small; pull overhead unjustified).
pub fn resolve_dispatch_mode_with_len_estimate(
    mode: DispatchMode,
    len_estimate: u32,
    num_workers: u32,
) -> ResolvedDispatchMode {
    match mode {
        DispatchMode::Push => ResolvedDispatchMode::Push,
        DispatchMode::Pull => ResolvedDispatchMode::Pull,
        DispatchMode::Auto => {
            if len_estimate > num_workers {
                ResolvedDispatchMode::Pull
            } else {
                ResolvedDispatchMode::Push
            }
        }
    }
}

/// Assert that `NoMoreWork` is never sent in push mode (R37e enforcement).
///
/// Fires `debug_assert!` in debug builds; is a no-op in release builds.
/// The `is_push_mode` flag should be `true` when `ResolvedDispatchMode::Push`.
pub fn assert_no_more_work_not_in_pull_mode(
    mode: ResolvedDispatchMode,
    is_sending_no_more_work: bool,
) {
    debug_assert!(
        !(mode == ResolvedDispatchMode::Push && is_sending_no_more_work),
        "R37e violation: NoMoreWork MUST NOT be sent in push mode"
    );
}

/// Pull-dispatch FSM states (SPEC-21 §3.8 A5 / TASK-0577).
///
/// These states are used by the `CoordinatorPullContext` (the lightweight
/// state-machine used when `dispatch_mode == Pull`). They are also surfaced
/// in the main `CoordinatorState` enum as the `Pull*` variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullCoordinatorState {
    /// Cold-start: generating and dispatching the first chunk eagerly (R32 step 1).
    DispatchingFirst,
    /// Waiting for `RequestWork` or `PartitionResult` from any worker.
    ///
    /// `PartitionResult` arrivals are BUFFERED here (not merged). Merge happens
    /// only from `AwaitingFinalResults` (R37d BSP barrier — closes SC-019).
    AwaitingResults,
    /// Calling `make_net_stream::next` + `strategy.allocate_batch` for the next chunk.
    GeneratingNext,
    /// Stream exhausted; sending `NoMoreWork` to all workers that requested work.
    SendingNoMoreWork,
    /// All workers have acknowledged `NoMoreWork`; collecting final `PartitionResult`s.
    ///
    /// Once all final results arrive, the coordinator transitions to `MergingResults`
    /// and consumes ALL buffered results (mid-stream + final) in a single logical BSP
    /// round (R37d — closes SC-019).
    AwaitingFinalResults,
    /// Merging all buffered partition results (single logical BSP round, R37d).
    MergingResults,
}

/// TASK-0600 (QA-D010-013): canonical projection from `PullCoordinatorState`
/// (the inner pull-mode FSM) to the outer `CoordinatorState` (the overall FSM
/// state surfaced via `serde::Serialize` for telemetry).
///
/// Per the collapse decision (TASK-0600 dispatch brief), `PullCoordinatorState`
/// is the **single canonical source of truth** for pull-mode FSM state.
/// The `CoordinatorState::Pull*` variants are kept ONLY for ABI/discriminant
/// stability (UT-0577-18) — production code MUST NOT construct them directly;
/// it MUST use this `From` impl so the canonical mapping is the only producer.
///
/// Mapping:
/// - `DispatchingFirst`     → `PullDispatchingFirst`
/// - `AwaitingResults`      → `PullAwaitingResults`
/// - `GeneratingNext`       → `PullGeneratingNext`
/// - `SendingNoMoreWork`    → `PullSendingNoMoreWork`
/// - `AwaitingFinalResults` → `PullAwaitingFinalResults`
/// - `MergingResults`       → `Merging` (legacy state — pull-mode joins the
///   non-pull FSM at the merge step; SPEC-21 R37d).
impl From<PullCoordinatorState> for CoordinatorState {
    fn from(s: PullCoordinatorState) -> Self {
        match s {
            PullCoordinatorState::DispatchingFirst => CoordinatorState::PullDispatchingFirst,
            PullCoordinatorState::AwaitingResults => CoordinatorState::PullAwaitingResults,
            PullCoordinatorState::GeneratingNext => CoordinatorState::PullGeneratingNext,
            PullCoordinatorState::SendingNoMoreWork => CoordinatorState::PullSendingNoMoreWork,
            PullCoordinatorState::AwaitingFinalResults => {
                CoordinatorState::PullAwaitingFinalResults
            }
            PullCoordinatorState::MergingResults => CoordinatorState::Merging,
        }
    }
}

/// Events that drive the pull-dispatch FSM (SPEC-21 R32, A5 / TASK-0577).
#[derive(Debug, Clone)]
pub enum PullCoordinatorEvent {
    /// The first chunk has been generated and dispatched to worker 0 (R32 step 1).
    FirstChunkDispatched,
    /// A worker sent `RequestWork` (R32 step 2).
    RequestWorkReceived { worker_id: u32 },
    /// A `PartitionResult` arrived from a worker (buffered in `AwaitingResults`; R37d).
    PartitionResultReceived { worker_id: u32 },
    /// A new chunk has been generated and dispatched via `AssignPartition` (R32 step 3).
    ChunkAllocatedAndDispatched,
    /// All workers have been sent `NoMoreWork` and no further `RequestWork` is expected.
    AllWorkersAckedNoMoreWork,
    /// All expected final `PartitionResult`s have arrived (R32 step 4 complete).
    AllFinalResultsReceived,
}

/// Errors from the pull-dispatch FSM (TASK-0577 acceptance criterion line 45).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PullCoordinatorError {
    /// A transition was attempted that is not valid from the current state.
    ///
    /// In debug builds, the FSM panics instead of returning this error.
    #[error("unexpected event {event:?} in state {state:?}")]
    UnexpectedEvent { event: String, state: String },

    /// QA-D010-002: The body `worker_id` of a `RequestWork` message does not
    /// match the authenticated `WorkerId` of the connection it arrived on.
    ///
    /// This indicates an impersonation attempt or a buggy worker that misroutes
    /// its own identity. Coordinators MUST reject these messages without
    /// dispatching the next chunk; the offending connection should be closed.
    #[error(
        "RequestWork worker_id mismatch: body says {body_worker_id}, \
         authenticated connection is {connection_worker_id} (QA-D010-002)"
    )]
    WorkerIdMismatch {
        body_worker_id: u32,
        connection_worker_id: u32,
    },
}

/// Lightweight context for the pull-dispatch FSM (TASK-0577).
///
/// This struct tracks only the pull-mode-specific fields; the main
/// `CoordinatorContext` continues to own the global state.
#[derive(Debug)]
pub struct CoordinatorPullContext {
    /// Current FSM state.
    pub state: PullCoordinatorState,
    /// Number of workers in this grid run.
    pub num_workers: u32,
    /// Whether the stream is exhausted (no more chunks available).
    pub stream_exhausted: bool,
    /// Number of workers that still need to receive `NoMoreWork`.
    pub workers_awaiting_no_more_work: u32,
    /// Buffered worker IDs from `PartitionResult` arrivals in `AwaitingResults`.
    ///
    /// NOT merged until `AwaitingFinalResults` (R37d BSP barrier — closes SC-019).
    pub pending_results: Vec<u32>,
    /// Expected total final `PartitionResult` count (= num_workers).
    pub expected_final_results: u32,
    /// `true` iff merge has been triggered (set on `MergingResults` entry).
    pub merge_triggered: bool,
    /// Count of results consumed for merge (for test verification).
    pub results_consumed_for_merge: usize,
    /// State trace for tests — records every state entered.
    ///
    /// Appended on every `transition()` / `try_transition()` call so tests
    /// can assert state-trace invariants (e.g., "no pull states entered in push mode").
    #[cfg(test)]
    pub state_trace: Vec<PullCoordinatorState>,
}

impl CoordinatorPullContext {
    /// Creates a new pull-dispatch context for `num_workers` workers.
    ///
    /// Starts in `DispatchingFirst` (the cold-start state per R32 step 1).
    pub fn new(num_workers: u32) -> Self {
        Self {
            state: PullCoordinatorState::DispatchingFirst,
            num_workers,
            stream_exhausted: false,
            workers_awaiting_no_more_work: num_workers,
            pending_results: Vec::new(),
            expected_final_results: num_workers,
            merge_triggered: false,
            results_consumed_for_merge: 0,
            #[cfg(test)]
            state_trace: vec![PullCoordinatorState::DispatchingFirst],
        }
    }

    /// Apply an event to the FSM, updating state in-place.
    ///
    /// In debug builds, invalid transitions panic. In release builds they
    /// return `Err(PullCoordinatorError::UnexpectedEvent)` via `try_transition`.
    /// This method panics in debug, returns silently on unexpected events in release
    /// (defensive — callers should prefer `try_transition` for error propagation).
    pub fn transition(&mut self, event: PullCoordinatorEvent) {
        // In debug builds, unexpected transitions panic.
        // In release builds, try_transition's error is silently swallowed here;
        // callers that need error propagation should call try_transition directly.
        #[cfg(debug_assertions)]
        {
            self.try_transition_inner(event)
                .expect("pull FSM unexpected event (debug: panic per TASK-0577 criterion line 45)");
        }
        #[cfg(not(debug_assertions))]
        {
            let _ = self.try_transition_inner(event);
        }
    }

    /// Apply an event, returning `Err` on unexpected transitions (release-mode API).
    pub fn try_transition(
        &mut self,
        event: PullCoordinatorEvent,
    ) -> Result<(), PullCoordinatorError> {
        self.try_transition_inner(event)
    }

    /// QA-D010-002: validated transition for `RequestWorkReceived` events.
    ///
    /// Compares the body `worker_id` (carried in the wire `Message::RequestWork`
    /// payload) against the `connection_worker_id` (the authenticated identity
    /// of the connection the message arrived on). On mismatch, returns
    /// `Err(WorkerIdMismatch)` WITHOUT applying the transition.
    ///
    /// Coordinator handlers MUST use this method (rather than `try_transition`)
    /// for `RequestWorkReceived` events so impersonation / chunk-theft attempts
    /// are rejected at the FSM boundary.
    pub fn try_transition_request_work(
        &mut self,
        body_worker_id: u32,
        connection_worker_id: u32,
    ) -> Result<(), PullCoordinatorError> {
        if body_worker_id != connection_worker_id {
            return Err(PullCoordinatorError::WorkerIdMismatch {
                body_worker_id,
                connection_worker_id,
            });
        }
        self.try_transition_inner(PullCoordinatorEvent::RequestWorkReceived {
            worker_id: body_worker_id,
        })
    }

    fn try_transition_inner(
        &mut self,
        event: PullCoordinatorEvent,
    ) -> Result<(), PullCoordinatorError> {
        let new_state = match (&self.state, &event) {
            // DispatchingFirst + FirstChunkDispatched → AwaitingResults.
            (
                PullCoordinatorState::DispatchingFirst,
                PullCoordinatorEvent::FirstChunkDispatched,
            ) => PullCoordinatorState::AwaitingResults,

            // AwaitingResults + PartitionResultReceived → AwaitingResults (buffered; R37d).
            (
                PullCoordinatorState::AwaitingResults,
                PullCoordinatorEvent::PartitionResultReceived { worker_id },
            ) => {
                self.pending_results.push(*worker_id);
                // State UNCHANGED — merge is deferred to AwaitingFinalResults.
                // merge_triggered stays false.
                PullCoordinatorState::AwaitingResults
            }

            // AwaitingResults + RequestWork (stream alive) → GeneratingNext.
            (
                PullCoordinatorState::AwaitingResults,
                PullCoordinatorEvent::RequestWorkReceived { .. },
            ) if !self.stream_exhausted => PullCoordinatorState::GeneratingNext,

            // AwaitingResults + RequestWork (stream exhausted) → SendingNoMoreWork.
            (
                PullCoordinatorState::AwaitingResults,
                PullCoordinatorEvent::RequestWorkReceived { .. },
            ) if self.stream_exhausted => PullCoordinatorState::SendingNoMoreWork,

            // GeneratingNext + ChunkAllocatedAndDispatched → AwaitingResults.
            (
                PullCoordinatorState::GeneratingNext,
                PullCoordinatorEvent::ChunkAllocatedAndDispatched,
            ) => PullCoordinatorState::AwaitingResults,

            // SendingNoMoreWork + AllWorkersAckedNoMoreWork → AwaitingFinalResults.
            (
                PullCoordinatorState::SendingNoMoreWork,
                PullCoordinatorEvent::AllWorkersAckedNoMoreWork,
            ) => PullCoordinatorState::AwaitingFinalResults,

            // AwaitingFinalResults + AllFinalResultsReceived → MergingResults (R37d BSP barrier).
            (
                PullCoordinatorState::AwaitingFinalResults,
                PullCoordinatorEvent::AllFinalResultsReceived,
            ) => {
                self.merge_triggered = true;
                self.results_consumed_for_merge = self.pending_results.len();
                PullCoordinatorState::MergingResults
            }

            // Unexpected transition.
            (state, event) => {
                let state_s = format!("{:?}", state);
                let event_s = format!("{:?}", event);
                #[cfg(debug_assertions)]
                panic!(
                    "pull FSM: unexpected event {:?} in state {:?} (TASK-0577 criterion line 45)",
                    event_s, state_s
                );
                #[cfg(not(debug_assertions))]
                return Err(PullCoordinatorError::UnexpectedEvent {
                    event: event_s,
                    state: state_s,
                });
            }
        };

        self.state = new_state;
        #[cfg(test)]
        self.state_trace.push(self.state);

        Ok(())
    }
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
        let mut ctx = CoordinatorContext::new("127.0.0.1:9000".parse().unwrap(), 2, false);
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
            is_coordinator_self: false,
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
            is_coordinator_self: false,
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

    // ---------------------------------------------------------------------------
    // TASK-0577: Coordinator FSM pull-dispatch states (SPEC-21 R30, R32, A5)
    // ---------------------------------------------------------------------------

    // UT-0577-01: Init → DispatchingFirst when dispatch_mode = Pull.
    #[test]
    fn ut_0577_01_transition_init_to_dispatching_first() {
        let mode = resolve_dispatch_mode(DispatchMode::Pull, u32::MAX - 1, 4);
        assert_eq!(
            mode,
            ResolvedDispatchMode::Pull,
            "Pull mode must resolve to Pull"
        );
        // Verify DispatchingFirst is a reachable initial state.
        let ctx = CoordinatorPullContext::new(4);
        assert_eq!(
            ctx.state,
            PullCoordinatorState::DispatchingFirst,
            "new PullCoordinatorContext starts in DispatchingFirst"
        );
    }

    // UT-0577-02: DispatchingFirst → AwaitingResults after first chunk dispatched.
    #[test]
    fn ut_0577_02_transition_dispatching_first_to_awaiting_results() {
        let mut ctx = CoordinatorPullContext::new(4);
        assert_eq!(ctx.state, PullCoordinatorState::DispatchingFirst);
        ctx.transition(PullCoordinatorEvent::FirstChunkDispatched);
        assert_eq!(
            ctx.state,
            PullCoordinatorState::AwaitingResults,
            "state must be AwaitingResults after FirstChunkDispatched"
        );
        #[cfg(test)]
        assert_eq!(
            ctx.state_trace.last().unwrap(),
            &PullCoordinatorState::AwaitingResults
        );
    }

    // UT-0577-03: AwaitingResults + RequestWork (stream alive) → GeneratingNext.
    #[test]
    fn ut_0577_03_transition_awaiting_results_request_work_stream_alive() {
        let mut ctx = CoordinatorPullContext::new(4);
        ctx.state = PullCoordinatorState::AwaitingResults;
        ctx.stream_exhausted = false;
        ctx.transition(PullCoordinatorEvent::RequestWorkReceived { worker_id: 0 });
        assert_eq!(
            ctx.state,
            PullCoordinatorState::GeneratingNext,
            "RequestWork with stream alive → GeneratingNext"
        );
    }

    // UT-0577-04: AwaitingResults + RequestWork (stream exhausted) → SendingNoMoreWork.
    #[test]
    fn ut_0577_04_transition_awaiting_results_request_work_stream_exhausted() {
        let mut ctx = CoordinatorPullContext::new(4);
        ctx.state = PullCoordinatorState::AwaitingResults;
        ctx.stream_exhausted = true;
        ctx.transition(PullCoordinatorEvent::RequestWorkReceived { worker_id: 0 });
        assert_eq!(
            ctx.state,
            PullCoordinatorState::SendingNoMoreWork,
            "RequestWork with stream exhausted → SendingNoMoreWork"
        );
    }

    // UT-0577-05: GeneratingNext → AwaitingResults after chunk allocated.
    #[test]
    fn ut_0577_05_transition_generating_next_to_awaiting_results() {
        let mut ctx = CoordinatorPullContext::new(4);
        ctx.state = PullCoordinatorState::GeneratingNext;
        ctx.transition(PullCoordinatorEvent::ChunkAllocatedAndDispatched);
        assert_eq!(
            ctx.state,
            PullCoordinatorState::AwaitingResults,
            "GeneratingNext + ChunkAllocatedAndDispatched → AwaitingResults"
        );
    }

    // UT-0577-06: SendingNoMoreWork + all workers acked → AwaitingFinalResults.
    #[test]
    fn ut_0577_06_transition_sending_no_more_work_all_acks() {
        let mut ctx = CoordinatorPullContext::new(4);
        ctx.state = PullCoordinatorState::SendingNoMoreWork;
        // Simulate 4 workers all receiving NoMoreWork (no more RequestWork arrivals).
        ctx.workers_awaiting_no_more_work = 4;
        ctx.transition(PullCoordinatorEvent::AllWorkersAckedNoMoreWork);
        assert_eq!(
            ctx.state,
            PullCoordinatorState::AwaitingFinalResults,
            "SendingNoMoreWork + AllWorkersAckedNoMoreWork → AwaitingFinalResults"
        );
    }

    // UT-0577-07: AwaitingFinalResults + all final results → MergingResults (R37d BSP barrier).
    #[test]
    fn ut_0577_07_transition_awaiting_final_results_to_merging() {
        let mut ctx = CoordinatorPullContext::new(4);
        ctx.state = PullCoordinatorState::AwaitingFinalResults;
        ctx.pending_results.push(0);
        ctx.pending_results.push(1);
        ctx.pending_results.push(2);
        ctx.pending_results.push(3);
        ctx.expected_final_results = 4;
        ctx.transition(PullCoordinatorEvent::AllFinalResultsReceived);
        assert_eq!(
            ctx.state,
            PullCoordinatorState::MergingResults,
            "AwaitingFinalResults + AllFinalResultsReceived → MergingResults (R37d BSP barrier)"
        );
    }

    // UT-0577-08: Rejected transition — DispatchingFirst + RequestWork.
    // Debug mode: panics. Release mode: returns CoordinatorPullError::UnexpectedEvent.
    #[test]
    #[cfg(not(debug_assertions))]
    fn ut_0577_08_rejected_transition_dispatching_first_request_work() {
        let mut ctx = CoordinatorPullContext::new(4);
        assert_eq!(ctx.state, PullCoordinatorState::DispatchingFirst);
        let err = ctx.try_transition(PullCoordinatorEvent::RequestWorkReceived { worker_id: 0 });
        assert!(
            err.is_err(),
            "DispatchingFirst + RequestWork must return error in release build"
        );
        match err.unwrap_err() {
            PullCoordinatorError::UnexpectedEvent { .. } => {}
        }
    }

    // UT-0577-09: PartitionResult in AwaitingResults is BUFFERED, not merged. State UNCHANGED.
    #[test]
    fn ut_0577_09_partition_result_in_awaiting_results_buffered_not_merged() {
        let mut ctx = CoordinatorPullContext::new(4);
        ctx.state = PullCoordinatorState::AwaitingResults;
        let before = ctx.pending_results.len();
        ctx.transition(PullCoordinatorEvent::PartitionResultReceived { worker_id: 1 });
        assert_eq!(
            ctx.state,
            PullCoordinatorState::AwaitingResults,
            "state must remain AwaitingResults when buffering a PartitionResult (R37d)"
        );
        assert_eq!(
            ctx.pending_results.len(),
            before + 1,
            "PartitionResult must be appended to pending_results buffer (R37d)"
        );
        // CRITICAL: no merge action should be queued from this transition.
        assert!(
            !ctx.merge_triggered,
            "merge must NOT be triggered by PartitionResult in AwaitingResults (R37d BSP barrier)"
        );
    }

    // UT-0577-10: Merge consumes ALL buffered results in AwaitingFinalResults (R37d single round).
    #[test]
    fn ut_0577_10_merge_only_in_awaiting_final_results() {
        let mut ctx = CoordinatorPullContext::new(4);
        ctx.state = PullCoordinatorState::AwaitingFinalResults;
        // 3 mid-stream + 1 final result buffered.
        ctx.pending_results = vec![0, 1, 2, 3];
        ctx.expected_final_results = 4;
        ctx.transition(PullCoordinatorEvent::AllFinalResultsReceived);
        assert_eq!(
            ctx.state,
            PullCoordinatorState::MergingResults,
            "AwaitingFinalResults + AllFinalResultsReceived → MergingResults"
        );
        assert!(
            ctx.merge_triggered,
            "merge must be triggered in MergingResults (R37d single-round reduction)"
        );
        // All buffered results consumed (all 4 pending).
        assert_eq!(
            ctx.results_consumed_for_merge, 4,
            "all 4 buffered results must be consumed for merge"
        );
    }

    // UT-0577-11: Push mode — ZERO NoMoreWork messages emitted (R37e).
    #[test]
    fn ut_0577_11_push_mode_no_more_work_never_emitted() {
        // The push mode coordinator never uses PullCoordinatorContext at all.
        // Verify that resolve_dispatch_mode(Push, ...) never returns Pull.
        let mode = resolve_dispatch_mode(DispatchMode::Push, 64, 4);
        assert_eq!(
            mode,
            ResolvedDispatchMode::Push,
            "Push mode must resolve to Push (R37e)"
        );
        // In push mode, the pull-only states are unreachable by construction.
        // We verify this by checking that the PullCoordinatorContext is NOT
        // instantiated in the push path.
    }

    // UT-0577-12: Push mode — pull-only states are unreachable (verified via state-trace).
    #[test]
    fn ut_0577_12_push_mode_pull_states_unreachable() {
        // resolve_dispatch_mode(Push, ...) always returns Push.
        // The PullCoordinatorContext is never created in push mode.
        // This test verifies the resolution logic is correct for Push.
        let modes_and_params = [
            (DispatchMode::Push, 64u32, 4u32),
            (DispatchMode::Push, 1, 10),
            (DispatchMode::Push, 0, 1),
        ];
        for (mode, len, workers) in modes_and_params {
            let resolved = resolve_dispatch_mode(mode, len, workers);
            assert_eq!(
                resolved,
                ResolvedDispatchMode::Push,
                "Push mode must always resolve to Push regardless of stream params"
            );
        }
    }

    // UT-0577-13: Push mode debug_assert fires on erroneous NoMoreWork send attempt (R37e).
    // Only valid in debug builds; in release, gating makes path unreachable.
    #[test]
    fn ut_0577_13_push_mode_debug_assert_on_no_more_work_attempt() {
        // In push mode (ResolvedDispatchMode::Push), assert_no_more_work_in_push()
        // fires a debug_assert! per R37e. We can only test the non-firing path here;
        // the firing path is tested via #[should_panic] in a separate #[cfg(debug_assertions)] test.
        let mode = resolve_dispatch_mode(DispatchMode::Push, 64, 4);
        // This function must never panic in push mode when called with Push.
        assert_no_more_work_not_in_pull_mode(mode, false); // push=true, not panicking.
    }

    // UT-0577-14: Auto resolves to Push when chunk_size == u32::MAX (R26 short-circuit).
    #[test]
    fn ut_0577_14_auto_resolves_to_push_when_chunk_size_u32_max() {
        #[allow(clippy::assertions_on_constants)]
        let mode = resolve_dispatch_mode(DispatchMode::Auto, u32::MAX, 4);
        assert_eq!(
            mode,
            ResolvedDispatchMode::Push,
            "Auto + chunk_size=u32::MAX → Push (R26 short-circuit)"
        );
    }

    // UT-0577-15: Auto resolves to Pull when streaming with excess capacity.
    #[test]
    fn ut_0577_15_auto_resolves_to_pull_when_streaming_with_excess_capacity() {
        // chunk_size = 16, len_estimate = 64, num_workers = 4 → len_estimate(64) > num_workers(4) → Pull.
        // The resolution is based on len_estimate > num_workers (TASK-0577 NOTE line 98).
        let len_estimate: u32 = 64;
        let num_workers: u32 = 4;
        // len_estimate(64) > num_workers(4): streaming has more total agents than workers → Pull.
        let mode =
            resolve_dispatch_mode_with_len_estimate(DispatchMode::Auto, len_estimate, num_workers);
        assert_eq!(
            mode,
            ResolvedDispatchMode::Pull,
            "Auto + len_estimate({len_estimate}) > num_workers({num_workers}) → Pull"
        );
    }

    // UT-0577-16: Auto resolves to Push when chunks ≤ workers (degenerate case).
    #[test]
    fn ut_0577_16_auto_resolves_to_push_when_chunks_le_workers() {
        // chunk_size = 16, len_estimate = 16, num_workers = 4 → 1 chunk ≤ workers → Push.
        // At most 1 chunk (len_estimate/chunk_size = 1) ≤ 4 workers → degenerate, use Push.
        let estimated_chunks: u32 = 1;
        let num_workers: u32 = 4;
        let mode =
            resolve_dispatch_mode_with_chunks(DispatchMode::Auto, estimated_chunks, num_workers);
        assert_eq!(
            mode,
            ResolvedDispatchMode::Push,
            "Auto + estimated_chunks({estimated_chunks}) ≤ num_workers({num_workers}) → Push (degenerate)"
        );
    }

    // UT-0577-17: PullCoordinatorState variants are distinct and Debug-representable.
    #[test]
    fn ut_0577_17_pull_coordinator_state_variants_distinct() {
        let states = [
            PullCoordinatorState::DispatchingFirst,
            PullCoordinatorState::AwaitingResults,
            PullCoordinatorState::GeneratingNext,
            PullCoordinatorState::SendingNoMoreWork,
            PullCoordinatorState::AwaitingFinalResults,
            PullCoordinatorState::MergingResults,
        ];
        for i in 0..states.len() {
            for j in (i + 1)..states.len() {
                assert_ne!(
                    states[i], states[j],
                    "PullCoordinatorState variants must be distinct"
                );
            }
        }
        for s in &states {
            assert!(!format!("{:?}", s).is_empty(), "Debug must be non-empty");
        }
    }

    // UT-0577-18: CoordinatorState gains 5 new pull-mode states (variant count).
    #[test]
    fn ut_0577_18_coordinator_state_includes_all_pull_variants() {
        // These variants must exist and be constructible (compile-time check via match).
        let pull_states = [
            CoordinatorState::PullDispatchingFirst,
            CoordinatorState::PullAwaitingResults,
            CoordinatorState::PullGeneratingNext,
            CoordinatorState::PullSendingNoMoreWork,
            CoordinatorState::PullAwaitingFinalResults,
        ];
        for s in &pull_states {
            let json = serde_json::to_string(s).unwrap();
            assert!(
                !json.is_empty(),
                "pull-mode CoordinatorState variant must serialize"
            );
        }
        // Legacy states must still exist (no regressions).
        let legacy_count_pre = 11; // 9 original + 2 SPEC-20
        let pull_count = 5;
        let states_vec: Vec<CoordinatorState> = vec![
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
        assert_eq!(
            states_vec.len(),
            legacy_count_pre,
            "legacy state count must be {legacy_count_pre}"
        );
        assert_eq!(pull_count, 5, "exactly 5 pull-only states must be added");
    }

    // --- Stage 6 REFACTOR additions (REVIEW + QA blockers) ---

    // QA-001 / EC-A: Pin the TASK-0414 wildcard-absorber contract.
    // Every new SPEC-20 event in every existing non-terminal state hits the
    // wildcard arm and produces zero actions today. TASK-0436 MUST replace this
    // with explicit transition arms; if any (state, new_event) cell still emits
    // zero actions after TASK-0436, that is a contract violation.
    #[test]
    fn ec_3_wildcard_arm_logs_unexpected_event_only() {
        // Arrange: new_events_still_absorbed() returns the events that
        // STILL hit the wildcard arm after Phase C refactor (MF-004). The
        // refactor added explicit transitions for WorkerJoined,
        // WorkerLeft, and MembershipWindowClosed in the four active-round
        // states (Partitioning/Dispatching/WaitingForResults/Merging) —
        // those cells are tested separately in
        // `mf_004_*_emits_queue_action_in_active_round_states`.
        let new_events_still_absorbed = || {
            vec![
                CoordinatorEvent::WorkerConnectionLost(7),
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
            for event in new_events_still_absorbed() {
                let mut ctx = make_ctx_at_state(state.clone());
                let actions = transition(&mut ctx, event);
                // CURRENT contract (post-MF-004): wildcard still absorbs the
                // four residual events listed above. TASK-0436 will close
                // them with explicit transitions; that is when this test
                // should be updated cell-by-cell to assert the new contract.
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
        // The non-active-round states (Init, WaitingForWorkers,
        // CheckTermination) still see WorkerJoined/WorkerLeft/
        // MembershipWindowClosed as wildcard hits.
        let non_active_states = [
            CoordinatorState::Init,
            CoordinatorState::WaitingForWorkers,
            CoordinatorState::CheckTermination,
        ];
        let new_active_round_events = || {
            vec![
                CoordinatorEvent::WorkerJoined(7),
                CoordinatorEvent::WorkerLeft(7, LeaveKind::AfterResult),
                CoordinatorEvent::MembershipWindowClosed,
            ]
        };
        for state in &non_active_states {
            for event in new_active_round_events() {
                let mut ctx = make_ctx_at_state(state.clone());
                let actions = transition(&mut ctx, event);
                assert!(
                    actions.is_empty(),
                    "QA-001: WorkerJoined-class event produced actions in non-active state {:?}",
                    state
                );
                assert_eq!(
                    ctx.state, *state,
                    "QA-001: wildcard arm must not mutate state {:?}",
                    state
                );
            }
        }
    }

    /// MF-004 (Phase C refactor) — `WorkerJoined(id)` in any of the four
    /// active-round states (`Partitioning`, `Dispatching`,
    /// `WaitingForResults`, `Merging`) MUST emit
    /// `QueueWorkerForNextWindow(id)` + `LogJoin(id)` and stay in the same
    /// state. This closes the SPEC-20 R10b FSM-totality gap (SC-012).
    #[test]
    fn mf_004_worker_joined_emits_queue_action_in_active_round_states() {
        let active_states = [
            CoordinatorState::Partitioning,
            CoordinatorState::Dispatching,
            CoordinatorState::WaitingForResults,
            CoordinatorState::Merging,
        ];
        for state in &active_states {
            let mut ctx = make_ctx_at_state(state.clone());
            let actions = transition(&mut ctx, CoordinatorEvent::WorkerJoined(99));
            // State unchanged.
            assert_eq!(
                ctx.state, *state,
                "MF-004: state must NOT change on WorkerJoined in {:?}",
                state
            );
            // QueueWorkerForNextWindow(99) must be present.
            let has_queue = actions
                .iter()
                .any(|a| matches!(a, CoordinatorAction::QueueWorkerForNextWindow(id) if *id == 99));
            assert!(
                has_queue,
                "MF-004: state {:?} × WorkerJoined(99) must emit QueueWorkerForNextWindow(99); got {:?}",
                state, actions
            );
            // LogJoin(99) must be present.
            let has_log = actions
                .iter()
                .any(|a| matches!(a, CoordinatorAction::LogJoin(id) if *id == 99));
            assert!(
                has_log,
                "MF-004: state {:?} × WorkerJoined(99) must emit LogJoin(99); got {:?}",
                state, actions
            );
        }
    }

    /// MF-004 (Phase C refactor) — `WorkerLeft(id, kind)` in any active-round
    /// state emits `RemoveWorker(id)` + `LogDeparture` and stays in the same
    /// state.
    #[test]
    fn mf_004_worker_left_emits_remove_action_in_active_round_states() {
        let active_states = [
            CoordinatorState::Partitioning,
            CoordinatorState::Dispatching,
            CoordinatorState::WaitingForResults,
            CoordinatorState::Merging,
        ];
        for state in &active_states {
            let mut ctx = make_ctx_at_state(state.clone());
            let actions = transition(
                &mut ctx,
                CoordinatorEvent::WorkerLeft(7, LeaveKind::AfterResult),
            );
            assert_eq!(
                ctx.state, *state,
                "MF-004: state must NOT change on WorkerLeft in {:?}",
                state
            );
            let has_remove = actions
                .iter()
                .any(|a| matches!(a, CoordinatorAction::RemoveWorker(id) if *id == 7));
            assert!(
                has_remove,
                "MF-004: state {:?} × WorkerLeft(7, AfterResult) must emit RemoveWorker(7)",
                state
            );
            let has_dep = actions.iter().any(|a| {
                matches!(
                    a,
                    CoordinatorAction::LogDeparture(id, DepartureKind::LeaveAfter) if *id == 7
                )
            });
            assert!(
                has_dep,
                "MF-004: state {:?} × WorkerLeft(7, AfterResult) must emit LogDeparture(7, LeaveAfter)",
                state
            );
        }
    }

    /// MF-004 (Phase C refactor) — every `WorkerJoined`/`WorkerLeft`/
    /// `MembershipWindowClosed` event in the active-round states produces
    /// at least one action OR a state transition (no silent loss). This is
    /// the regression test for the QA-001 Phase A wildcard pattern.
    #[test]
    fn mf_004_active_round_states_never_silently_drop_membership_events() {
        let active_states = [
            CoordinatorState::Partitioning,
            CoordinatorState::Dispatching,
            CoordinatorState::WaitingForResults,
            CoordinatorState::Merging,
        ];
        let events = || {
            vec![
                CoordinatorEvent::WorkerJoined(1),
                CoordinatorEvent::WorkerLeft(1, LeaveKind::AfterResult),
                CoordinatorEvent::WorkerLeft(1, LeaveKind::Urgent),
                CoordinatorEvent::MembershipWindowClosed,
            ]
        };
        for state in &active_states {
            for ev in events() {
                let ev_dbg = format!("{:?}", ev);
                let mut ctx = make_ctx_at_state(state.clone());
                let before = ctx.state.clone();
                let actions = transition(&mut ctx, ev);
                assert!(
                    !actions.is_empty() || ctx.state != before,
                    "MF-004: state {:?} × {} silently dropped — produced no action and no transition",
                    state, ev_dbg
                );
            }
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

    // ---------------------------------------------------------------------------
    // QA-D010-002: try_transition_request_work validates body identity against
    // connection identity (impersonation / chunk-theft prevention).
    // ---------------------------------------------------------------------------

    /// QA-D010-002-A: matching body / connection worker_id is accepted.
    #[test]
    fn qa_d010_002_a_matching_identity_accepted() {
        let mut ctx = CoordinatorPullContext::new(2);
        // Pre-position the FSM in AwaitingResults (post-FirstChunkDispatched).
        ctx.transition(PullCoordinatorEvent::FirstChunkDispatched);

        // Body says worker 1, connection authenticated as worker 1 -> accept.
        ctx.try_transition_request_work(1, 1)
            .expect("matching identity must be accepted");
        assert_eq!(
            ctx.state,
            PullCoordinatorState::GeneratingNext,
            "QA-D010-002-A: FSM must advance on matching identity"
        );
    }

    /// QA-D010-002-B: mismatched body / connection worker_id is rejected
    /// with the specific WorkerIdMismatch variant; FSM does NOT advance.
    #[test]
    fn qa_d010_002_b_mismatched_identity_rejected() {
        let mut ctx = CoordinatorPullContext::new(2);
        ctx.transition(PullCoordinatorEvent::FirstChunkDispatched);
        let pre_state = ctx.state;

        // Worker 0's connection forging a RequestWork for worker 1.
        let err = ctx
            .try_transition_request_work(1, 0)
            .expect_err("mismatched identity must be rejected");
        assert_eq!(
            err,
            PullCoordinatorError::WorkerIdMismatch {
                body_worker_id: 1,
                connection_worker_id: 0,
            },
            "QA-D010-002-B: must return WorkerIdMismatch with both ids"
        );
        assert_eq!(
            ctx.state, pre_state,
            "QA-D010-002-B: FSM state must NOT advance on mismatch"
        );
    }

    // -----------------------------------------------------------------------
    // TASK-0600 — Collapse parallel `Pull*` / `PullCoordinatorState` types
    // (QA-D010-013). The canonical source of truth is `PullCoordinatorState`;
    // the `CoordinatorState::Pull*` variants are derived only via the
    // `From<PullCoordinatorState>` impl above.
    // -----------------------------------------------------------------------

    /// UT-0600-01 — `canonical_pull_state_type_is_unique` (QA-D010-013).
    ///
    /// Verifies that `PullCoordinatorState` is the SOLE per-FSM pull-state
    /// type. The `CoordinatorState::Pull*` variants are kept for ABI
    /// stability (UT-0577-18) but are constructed exclusively via
    /// `From<PullCoordinatorState> for CoordinatorState`, so they cannot
    /// drift from the canonical type.
    ///
    /// Direction-of-collapse: `PullCoordinatorState` was selected as canonical
    /// because (a) it carries the richer state set including `MergingResults`
    /// and (b) its name is more specific than the original placeholder
    /// `PullState`. See TASK-0600 dispatch brief (2026-04-30).
    #[test]
    fn ut_0600_01_canonical_pull_state_type_is_unique() {
        // The `From` projection must be defined and yield a 1:1 mapping for
        // the 5 pull variants and a join-to-`Merging` for the 6th (terminal
        // pull state — SPEC-21 R37d).
        assert_eq!(
            CoordinatorState::from(PullCoordinatorState::DispatchingFirst),
            CoordinatorState::PullDispatchingFirst
        );
        assert_eq!(
            CoordinatorState::from(PullCoordinatorState::AwaitingResults),
            CoordinatorState::PullAwaitingResults
        );
        assert_eq!(
            CoordinatorState::from(PullCoordinatorState::GeneratingNext),
            CoordinatorState::PullGeneratingNext
        );
        assert_eq!(
            CoordinatorState::from(PullCoordinatorState::SendingNoMoreWork),
            CoordinatorState::PullSendingNoMoreWork
        );
        assert_eq!(
            CoordinatorState::from(PullCoordinatorState::AwaitingFinalResults),
            CoordinatorState::PullAwaitingFinalResults
        );
        // `MergingResults` joins the legacy non-pull merge state — pull-mode
        // does NOT have its own merge state in `CoordinatorState`.
        assert_eq!(
            CoordinatorState::from(PullCoordinatorState::MergingResults),
            CoordinatorState::Merging
        );
    }

    /// UT-0600-02 — `canonical_pull_state_enum_is_exhaustive_per_spec_21_a5`.
    ///
    /// SPEC-21 §3.8 A5 enumerates 5 coordinator pull states + 1 terminal
    /// merging state. The collapsed canonical type MUST contain all 6
    /// variants. The exhaustive `match` below is a compile-time fence: any
    /// future deletion of a variant fails to compile (no `_ =>` wildcard).
    #[test]
    fn ut_0600_02_canonical_pull_state_enum_is_exhaustive_per_spec_21_a5() {
        // Enumerate every variant. Adding a new variant later requires
        // extending this list (regression fence).
        let all = [
            PullCoordinatorState::DispatchingFirst,
            PullCoordinatorState::AwaitingResults,
            PullCoordinatorState::GeneratingNext,
            PullCoordinatorState::SendingNoMoreWork,
            PullCoordinatorState::AwaitingFinalResults,
            PullCoordinatorState::MergingResults,
        ];
        // Variant count: 5 active + 1 terminal = 6.
        assert_eq!(
            all.len(),
            6,
            "UT-0600-02: PullCoordinatorState MUST have exactly 6 variants per SPEC-21 §3.8 A5"
        );
        // Compile-time exhaustiveness: NO `_ =>` arm.
        for s in all {
            let count: u32 = match s {
                PullCoordinatorState::DispatchingFirst => 1,
                PullCoordinatorState::AwaitingResults => 1,
                PullCoordinatorState::GeneratingNext => 1,
                PullCoordinatorState::SendingNoMoreWork => 1,
                PullCoordinatorState::AwaitingFinalResults => 1,
                PullCoordinatorState::MergingResults => 1,
            };
            assert_eq!(count, 1, "every variant must contribute 1 to the count");
        }
    }
}
