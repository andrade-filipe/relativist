//! Worker FSM for distributed IC reduction (SPEC-13 R24-R27).
//!
//! Workers connect to the coordinator, receive partitions, reduce
//! them locally via reduce_all (SPEC-03), and return results.
//! Workers have no knowledge of each other (star topology, SPEC-13 R27).

use crate::error::WorkerError;
use crate::merge::{BorderDelta, LocalReconnection, MintedAgent, PendingCommutation};
use crate::net::PortRef;
use crate::partition::Partition;
use crate::protocol::Message;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// FSM types (TASK-0109, SPEC-13 R24-R25)
// ---------------------------------------------------------------------------

/// Worker FSM states (SPEC-13 R24).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum WorkerState {
    /// Initial state. Connecting to coordinator.
    Init,
    /// Connected and idle. Waiting for a partition.
    Idle,
    /// Reducing a partition locally.
    Reducing,
    /// Sending the reduced partition back to the coordinator.
    Returning,
    /// Fatal error.
    Error,
    /// Shutdown received. Exiting.
    Done,
    /// SPEC-19 R21.1 (TASK-0380): delta mode, partition stored, waiting
    /// for first `Message::RoundStart`.
    DeltaIdle,
    /// SPEC-19 R24 (TASK-0381): delta mode, at least one delta round
    /// processed. Returned to after every `handle_round_start`.
    DeltaActive,
}

/// Events that drive the worker FSM (SPEC-13 R25).
#[derive(Debug, Clone)]
pub enum WorkerEvent {
    /// TCP connection to coordinator established.
    Connected,
    /// Partition received from coordinator.
    ReceivePartition(Partition),
    /// Local reduction completed.
    ReductionComplete(Partition),
    /// Reduced partition successfully sent back to coordinator.
    SendComplete,
    /// Shutdown message received from coordinator.
    Shutdown,
    /// Reduction failed with an error.
    ReductionError(String),
    /// TCP connection to coordinator lost.
    ConnectionLost,
}

/// Actions the worker runtime must execute (SPEC-13 R25).
#[derive(Debug)]
pub enum WorkerAction {
    /// Send a message to the coordinator.
    ///
    /// Boxed to avoid large size difference between enum variants (clippy::large_enum_variant).
    SendMessage(Box<Message>),
    /// Close the TCP connection gracefully.
    CloseConnection,
    /// Shut down the worker process. Used on ConnectionLost since
    /// the coordinator has already aborted (SPEC-06 R25). No reconnection.
    ShutdownSelf,
    /// Log a state transition at INFO level (SPEC-13 R23).
    LogTransition { from: WorkerState, to: WorkerState },
    /// SPEC-19 R26, DC-B5 (TASK-0394): a recoverable worker-side
    /// protocol error surfaced during round processing (e.g.,
    /// `id_range` exhaustion while fulfilling `pending_commutations`).
    /// The runtime decides whether to report to the coordinator via a
    /// future `Message::Error` variant or terminate; today it is
    /// logged and the worker is expected to abort that round cleanly.
    /// When this action is emitted, no `RoundResult` is emitted in the
    /// same return vector (spec-compliant failure mode).
    Error(WorkerError),
}

// ---------------------------------------------------------------------------
// Delta-mode persistent worker state (TASK-0379, SPEC-19 R22, R25)
// ---------------------------------------------------------------------------

/// SPEC-19 R22, R25: persistent state a stateful worker retains across
/// BSP rounds in delta mode (`GridConfig.delta_mode = true`).
///
/// v1 workers are stateless: each `AssignPartition` delivers a fresh
/// `Partition` and the worker returns the reduced result via
/// `PartitionResult`. Under the delta protocol, the worker receives the
/// partition ONCE at Round 0 (`Message::InitialPartition`) and thereafter
/// receives only `BorderDelta`s in `Message::RoundStart`. The worker
/// reduces its stored partition in-place and reports only changed border
/// endpoints in `Message::RoundResult`.
///
/// The `previous_border_state` map holds the last-reported endpoint for
/// every border ID, used by TASK-0382's delta computation to emit only
/// changed entries (R25). It is seeded at Round 0 from
/// `partition.free_port_index` so that Round 1's first delta-dispatch
/// reports only borders that local reduction actually moved (DC-C4
/// option (b), ratified 2026-04-17).
///
/// This struct is never serialized; only the `partition` field crosses
/// the wire (via `Message::InitialPartition` at Round 0 and
/// `Message::FinalStateResult` at convergence).
#[derive(Debug, Clone)]
pub struct WorkerDeltaState {
    pub partition: Partition,
    pub previous_border_state: HashMap<u32, PortRef>,
    pub round: u32,
}

impl WorkerDeltaState {
    /// Initialize from the Round-0 `InitialPartition` payload. Stores
    /// the partition and seeds `previous_border_state` from its
    /// `free_port_index` (DC-C4 option (b)).
    pub fn from_initial_partition(partition: Partition) -> Self {
        let previous_border_state = partition.free_port_index.clone();
        Self {
            partition,
            previous_border_state,
            round: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// FSM transition (TASK-0110, SPEC-13 R25)
// ---------------------------------------------------------------------------

/// Worker FSM context.
pub struct WorkerContext {
    pub state: WorkerState,
    /// Current round number (echoed from AssignPartition).
    pub round: u32,
    /// SPEC-19 R22 (TASK-0380): delta-mode persistent state. `None` in
    /// v1 full-partition mode; `Some(_)` after Round 0 `InitialPartition`.
    pub delta_state: Option<WorkerDeltaState>,
}

impl Default for WorkerContext {
    fn default() -> Self {
        Self {
            state: WorkerState::Init,
            round: 0,
            delta_state: None,
        }
    }
}

impl WorkerContext {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Pure transition function for the worker FSM (SPEC-13 R25).
pub fn transition(ctx: &mut WorkerContext, event: WorkerEvent) -> Vec<WorkerAction> {
    let from = ctx.state.clone();
    let mut actions = Vec::new();

    match (&ctx.state, event) {
        // Init + Connected → Idle
        (WorkerState::Init, WorkerEvent::Connected) => {
            ctx.state = WorkerState::Idle;
            actions.push(WorkerAction::LogTransition {
                from,
                to: ctx.state.clone(),
            });
        }

        // Idle + ReceivePartition → Reducing
        (WorkerState::Idle, WorkerEvent::ReceivePartition(_partition)) => {
            ctx.state = WorkerState::Reducing;
            actions.push(WorkerAction::LogTransition {
                from,
                to: ctx.state.clone(),
            });
            // The actual reduce_all call is done by the async runtime
            // (spawn_blocking), not by the FSM. The runtime fires
            // ReductionComplete when done.
        }

        // Reducing + ReductionComplete → Returning
        (WorkerState::Reducing, WorkerEvent::ReductionComplete(partition)) => {
            ctx.state = WorkerState::Returning;
            actions.push(WorkerAction::SendMessage(Box::new(
                Message::PartitionResult {
                    round: ctx.round,
                    partition: partition.clone(),
                    stats: crate::merge::WorkerRoundStats {
                        worker_id: partition.worker_id,
                        agents_before: 0, // filled by the runtime
                        agents_after: partition.subnet.count_live_agents(),
                        local_redexes: 0,             // filled by the runtime
                        reduce_duration_secs: 0.0,    // filled by the runtime
                        interactions_by_rule: [0; 6], // filled by the runtime
                        // SPEC-19 R2: TASK-0349 wires the real value via
                        // the async runtime path; this FSM stub stays at
                        // false to keep the v1 contract bit-identical.
                        has_border_activity: false,
                    },
                },
            )));
            actions.push(WorkerAction::LogTransition {
                from,
                to: ctx.state.clone(),
            });
        }

        // Returning + SendComplete → Idle
        (WorkerState::Returning, WorkerEvent::SendComplete) => {
            ctx.state = WorkerState::Idle;
            actions.push(WorkerAction::LogTransition {
                from,
                to: ctx.state.clone(),
            });
        }

        // Idle + Shutdown → Done
        (WorkerState::Idle, WorkerEvent::Shutdown) => {
            ctx.state = WorkerState::Done;
            actions.push(WorkerAction::CloseConnection);
            actions.push(WorkerAction::LogTransition {
                from,
                to: ctx.state.clone(),
            });
        }

        // Reducing + ReductionError → Error
        (WorkerState::Reducing, WorkerEvent::ReductionError(msg)) => {
            ctx.state = WorkerState::Error;
            actions.push(WorkerAction::SendMessage(Box::new(Message::Error {
                round: ctx.round,
                worker_id: 0, // filled by runtime
                description: msg,
            })));
            actions.push(WorkerAction::LogTransition {
                from,
                to: ctx.state.clone(),
            });
        }

        // Any + ConnectionLost → Error
        (_, WorkerEvent::ConnectionLost) => {
            ctx.state = WorkerState::Error;
            actions.push(WorkerAction::LogTransition {
                from,
                to: ctx.state.clone(),
            });
            actions.push(WorkerAction::ShutdownSelf);
        }

        // Unexpected event in current state
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
// Delta-mode Round 0 handler (TASK-0380, SPEC-19 R21.1, R22)
// ---------------------------------------------------------------------------

/// SPEC-19 R21.1, R22 (TASK-0380): handle the Round 0
/// `Message::InitialPartition { round: 0, partition }` message. Stores
/// the partition in `ctx.delta_state`, transitions to `DeltaIdle`, and
/// returns log actions. **Does NOT send an ack** (DC-C1 option (b)
/// locked-in 2026-04-17 — the coordinator treats `InitialPartition`
/// dispatch as fire-and-forget; the worker's first `RoundResult` at
/// Round 1 is the implicit ack.)
pub fn handle_initial_partition(
    ctx: &mut WorkerContext,
    round: u32,
    partition: Partition,
) -> Vec<WorkerAction> {
    debug_assert!(
        round == 0,
        "InitialPartition MUST arrive at round 0 (R21.1)"
    );
    let from = ctx.state.clone();
    ctx.delta_state = Some(WorkerDeltaState::from_initial_partition(partition));
    ctx.state = WorkerState::DeltaIdle;
    ctx.round = 0;
    vec![WorkerAction::LogTransition {
        from,
        to: ctx.state.clone(),
    }]
}

// ---------------------------------------------------------------------------
// Delta-mode round handler (TASK-0381, SPEC-19 R23, R24, R26)
// ---------------------------------------------------------------------------

/// SPEC-19 R23, R24, R26 (TASK-0381): per-round handler for delta-mode
/// workers. Called by the wire layer on receipt of
/// `Message::RoundStart`; returns the actions the runtime must execute
/// (log + `SendMessage(RoundResult)`).
///
/// The handler implements the five-step R24 pipeline:
/// 1. **Apply** coordinator-provided `border_deltas`, `resolved_borders`
///    and `new_borders` to the stored partition
///    (`apply_border_deltas_to_partition`).
/// 2. **Reduce** the local partition (`reduce_all`), recording stats.
/// 3. **Rebuild** the `free_port_index` so it reflects the post-reduction
///    subnet (`rebuild_free_port_index`).
/// 4. **Diff** the previous-round border state against the rebuilt
///    index and emit only changed entries (`compute_outgoing_deltas`);
///    snapshot the new index into `previous_border_state` for next round.
/// 5. **Report** the round outcome via `Message::RoundResult` (R26).
///
/// On first call the worker state MUST be `DeltaIdle` (just after
/// `handle_initial_partition` at Round 0); on subsequent calls it MUST
/// be `DeltaActive`. Either way the handler transitions to
/// `DeltaActive`.
///
/// DC-C3 (option (a), 2026-04-17): delta mode relies on `BorderResolver`
/// running at the coordinator, not on `reduce_border_once`. This handler
/// therefore calls only `reduce_all` on the local subnet — no merge.
///
/// DC-C4 (option (b), 2026-04-17): `state.previous_border_state` is
/// seeded from `partition.free_port_index` at Round 0 (by
/// `WorkerDeltaState::from_initial_partition`). The first delta emitted
/// by this handler (in Round 1) therefore reports only borders that
/// Round-1 local reduction actually moved.
#[allow(clippy::too_many_arguments)]
pub fn handle_round_start(
    ctx: &mut WorkerContext,
    round: u32,
    border_deltas: Vec<BorderDelta>,
    resolved_borders: Vec<u32>,
    new_borders: Vec<(u32, PortRef)>,
    local_reconnections: Vec<LocalReconnection>,
    pending_commutations: Vec<PendingCommutation>,
) -> Vec<WorkerAction> {
    use crate::merge::helpers::{
        apply_border_deltas_to_partition, compute_border_activity, compute_outgoing_deltas,
        rebuild_free_port_index,
    };
    use crate::reduction::reduce_all;
    use std::time::Instant;

    debug_assert!(
        matches!(ctx.state, WorkerState::DeltaIdle | WorkerState::DeltaActive),
        "handle_round_start requires DeltaIdle or DeltaActive (R24); got {:?}",
        ctx.state
    );

    let from = ctx.state.clone();
    let state = ctx
        .delta_state
        .as_mut()
        .expect("handle_round_start requires prior handle_initial_partition (R21.1)");

    // R24.1 — fold coordinator deltas into the stored partition.
    apply_border_deltas_to_partition(
        &mut state.partition,
        &border_deltas,
        &resolved_borders,
        &new_borders,
    );

    // R24.1.5 — TASK-0394 DC-B3: apply coordinator-emitted
    // `local_reconnections` BEFORE local reduction. Each entry rewires
    // `agent_id.port` to `new_target` on the stored subnet. Self-loops
    // (`new_target == AgentPort(agent_id, port)`) and DISCONNECTED
    // targets are applied verbatim — `Net::connect` handles both cases.
    // The resolver contract (DC-B3) guarantees the pairs describe valid
    // post-resolution topology; the worker applies them transparently.
    for lr in &local_reconnections {
        let local = PortRef::AgentPort(lr.agent_id, lr.port);
        // Self-loop defence: skip a pair where source equals target on
        // the same port. This cannot arise from a well-formed resolver
        // emission but we guard against it rather than tripping
        // Net::connect's debug_assert.
        if lr.new_target == local {
            tracing::trace!(
                agent_id = lr.agent_id,
                port = lr.port,
                "handle_round_start: skipping LocalReconnection self-loop (DC-B3)"
            );
            continue;
        }
        state.partition.subnet.connect(local, lr.new_target);
    }

    // R24.1.6 — TASK-0394 DC-B5 second half: fulfil coordinator-issued
    // `pending_commutations`. For each entry, mint a fresh agent from
    // the worker's own `id_range` via `Net::create_agent` (which uses
    // `next_id`, seeded at partition split to `id_range.start`). If the
    // range is exhausted mid-loop, partial-progress semantics apply:
    // already-minted agents remain committed; the handler returns ONLY
    // a `WorkerAction::Error` and does NOT emit a `Message::RoundResult`
    // for this round. The coordinator (per R48) is responsible for
    // deciding whether to repartition or abort.
    let mut minted_agents: Vec<MintedAgent> = Vec::with_capacity(pending_commutations.len());
    for pc in &pending_commutations {
        // Exhaustion check: the next `create_agent` call would produce
        // an id == `next_id`, which MUST remain strictly less than
        // `id_range.end` (exclusive upper bound, SPEC-04 IdRange).
        if state.partition.subnet.next_id >= state.partition.id_range.end {
            tracing::warn!(
                request_id = pc.request_id,
                worker_id = state.partition.worker_id,
                next_id = state.partition.subnet.next_id,
                id_range_end = state.partition.id_range.end,
                "handle_round_start: id_range exhausted while fulfilling PendingCommutation (SPEC-19 R26, DC-B5)"
            );
            return vec![
                WorkerAction::LogTransition {
                    from,
                    to: ctx.state.clone(),
                },
                WorkerAction::Error(WorkerError::IdRangeExhausted {
                    request_id: pc.request_id,
                }),
            ];
        }
        // Arity check: the resolver MUST emit arity consistent with
        // the symbol's natural arity (Con/Dup => 2, Era => 0). A
        // mismatch is a resolver bug; debug_assert surfaces it in
        // development. In release builds we trust the resolver.
        debug_assert_eq!(
            pc.arity,
            crate::net::arity(pc.symbol_type),
            "DC-B5: PendingCommutation arity {} mismatches symbol {:?} natural arity {}",
            pc.arity,
            pc.symbol_type,
            crate::net::arity(pc.symbol_type)
        );
        let minted_agent_id = state.partition.subnet.create_agent(pc.symbol_type);
        minted_agents.push(MintedAgent {
            request_id: pc.request_id,
            minted_agent_id,
        });
    }

    let agents_before = state.partition.subnet.count_live_agents();

    // R24.2 — local reduction to quiescence.
    let t_reduce = Instant::now();
    let reduction_stats = reduce_all(&mut state.partition.subnet);
    let reduce_duration = t_reduce.elapsed();

    // R24.3 — lazy index reconstruction against the post-reduction net.
    state.partition.free_port_index = rebuild_free_port_index(
        &state.partition.subnet,
        state.partition.border_id_start,
        state.partition.border_id_end,
    );

    // R24.4 — diff vs last-reported snapshot, then promote current to
    // the new baseline (R25).
    let outgoing = compute_outgoing_deltas(
        &state.previous_border_state,
        &state.partition.free_port_index,
    );
    state.previous_border_state = state.partition.free_port_index.clone();

    let has_border_activity = compute_border_activity(&state.partition);
    let agents_after = state.partition.subnet.count_live_agents();
    let worker_id = state.partition.worker_id;
    state.round = round;

    // R26 — RoundResult payload.
    let stats = crate::merge::WorkerRoundStats {
        worker_id,
        agents_before,
        agents_after,
        local_redexes: reduction_stats.total_interactions as usize,
        reduce_duration_secs: reduce_duration.as_secs_f64(),
        interactions_by_rule: reduction_stats.interactions_by_rule,
        has_border_activity,
    };

    ctx.round = round;
    ctx.state = WorkerState::DeltaActive;

    vec![
        WorkerAction::LogTransition {
            from,
            to: ctx.state.clone(),
        },
        WorkerAction::SendMessage(Box::new(Message::RoundResult {
            round,
            border_deltas: outgoing,
            stats,
            has_border_activity,
            minted_agents,
        })),
    ]
}

// ---------------------------------------------------------------------------
// Delta-mode final-state handler (TASK-0383, SPEC-19 R21 phase 3, R28)
// ---------------------------------------------------------------------------

/// SPEC-19 R21 phase 3, R28 (TASK-0383): handle the coordinator's
/// `Message::FinalStateRequest { round }` issued once the coordinator
/// declares Global Normal Form (R4). Extracts the stored partition
/// from `ctx.delta_state` via `.take()` (freeing the worker's copy)
/// and emits `Message::FinalStateResult { round, partition }`.
///
/// Transitions `ctx.state = WorkerState::Returning`. The subsequent
/// `Message::Shutdown` from the coordinator is handled by the existing
/// v1 FSM (`transition(ctx, WorkerEvent::Shutdown)`).
///
/// Accepts both `DeltaActive` (normal multi-round convergence) and
/// `DeltaIdle` (Round-0-only convergence — the input net was already
/// in Normal Form so the coordinator declared GNF before any
/// `RoundStart` was emitted).
///
/// **Caller invariant:** `ctx.delta_state` MUST be `Some(_)` — the
/// coordinator never sends `FinalStateRequest` without first sending
/// `InitialPartition`. Violation panics.
pub fn handle_final_state_request(ctx: &mut WorkerContext, round: u32) -> Vec<WorkerAction> {
    debug_assert!(
        matches!(ctx.state, WorkerState::DeltaIdle | WorkerState::DeltaActive),
        "handle_final_state_request requires DeltaIdle or DeltaActive (R21 phase 3); got {:?}",
        ctx.state
    );
    let from = ctx.state.clone();
    let partition = ctx
        .delta_state
        .take()
        .expect("handle_final_state_request requires prior handle_initial_partition (R21.1)")
        .partition;
    ctx.round = round;
    ctx.state = WorkerState::Returning;
    vec![
        WorkerAction::LogTransition {
            from,
            to: ctx.state.clone(),
        },
        WorkerAction::SendMessage(Box::new(Message::FinalStateResult { round, partition })),
    ]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::Net;
    use crate::partition::IdRange;
    use std::collections::HashMap;

    // === TASK-0109: type tests ===

    #[test]
    fn test_worker_state_serialize() {
        let json = serde_json::to_string(&WorkerState::Init).unwrap();
        assert!(json.contains("Init"));
    }

    #[test]
    fn test_worker_state_equality() {
        assert_eq!(WorkerState::Init, WorkerState::Init);
        assert_ne!(WorkerState::Init, WorkerState::Done);
    }

    #[test]
    fn test_all_states_distinct() {
        let states = [
            WorkerState::Init,
            WorkerState::Idle,
            WorkerState::Reducing,
            WorkerState::Returning,
            WorkerState::Error,
            WorkerState::Done,
            WorkerState::DeltaIdle,
            WorkerState::DeltaActive,
        ];
        for i in 0..states.len() {
            for j in (i + 1)..states.len() {
                assert_ne!(states[i], states[j]);
            }
        }
    }

    // === TASK-0110: transition tests ===

    fn make_partition() -> Partition {
        Partition {
            subnet: Net::new(),
            worker_id: 0,
            free_port_index: HashMap::new(),
            id_range: IdRange { start: 0, end: 100 },
            border_id_start: 0,
            border_id_end: 0,
        }
    }

    #[test]
    fn test_init_to_idle() {
        let mut ctx = WorkerContext::new();
        let actions = transition(&mut ctx, WorkerEvent::Connected);
        assert_eq!(ctx.state, WorkerState::Idle);
        assert!(actions
            .iter()
            .any(|a| matches!(a, WorkerAction::LogTransition { .. })));
    }

    #[test]
    fn test_idle_to_reducing() {
        let mut ctx = WorkerContext::new();
        ctx.state = WorkerState::Idle;
        let actions = transition(&mut ctx, WorkerEvent::ReceivePartition(make_partition()));
        assert_eq!(ctx.state, WorkerState::Reducing);
        assert!(!actions.is_empty());
    }

    #[test]
    fn test_reducing_to_returning() {
        let mut ctx = WorkerContext::new();
        ctx.state = WorkerState::Reducing;
        let actions = transition(&mut ctx, WorkerEvent::ReductionComplete(make_partition()));
        assert_eq!(ctx.state, WorkerState::Returning);
        assert!(actions
            .iter()
            .any(|a| matches!(a, WorkerAction::SendMessage(_))));
    }

    #[test]
    fn test_returning_to_idle() {
        let mut ctx = WorkerContext::new();
        ctx.state = WorkerState::Returning;
        let actions = transition(&mut ctx, WorkerEvent::SendComplete);
        assert_eq!(ctx.state, WorkerState::Idle);
        assert!(!actions.is_empty());
    }

    #[test]
    fn test_idle_shutdown() {
        let mut ctx = WorkerContext::new();
        ctx.state = WorkerState::Idle;
        let actions = transition(&mut ctx, WorkerEvent::Shutdown);
        assert_eq!(ctx.state, WorkerState::Done);
        assert!(actions
            .iter()
            .any(|a| matches!(a, WorkerAction::CloseConnection)));
    }

    #[test]
    fn test_reduction_error() {
        let mut ctx = WorkerContext::new();
        ctx.state = WorkerState::Reducing;
        let actions = transition(&mut ctx, WorkerEvent::ReductionError("bad".into()));
        assert_eq!(ctx.state, WorkerState::Error);
        assert!(actions.iter().any(
            |a| matches!(a, WorkerAction::SendMessage(m) if matches!(**m, Message::Error { .. }))
        ));
    }

    #[test]
    fn test_connection_lost_from_any_state() {
        for initial in &[
            WorkerState::Init,
            WorkerState::Idle,
            WorkerState::Reducing,
            WorkerState::Returning,
        ] {
            let mut ctx = WorkerContext::new();
            ctx.state = initial.clone();
            let actions = transition(&mut ctx, WorkerEvent::ConnectionLost);
            assert_eq!(ctx.state, WorkerState::Error);
            assert!(actions
                .iter()
                .any(|a| matches!(a, WorkerAction::ShutdownSelf)));
        }
    }

    // === TASK-0379: WorkerDeltaState tests (SPEC-19 R22, R25) ===

    use crate::net::Symbol;

    fn make_delta_partition_with_borders() -> Partition {
        let mut subnet = Net::new();
        let a = subnet.create_agent(Symbol::Con);
        let b = subnet.create_agent(Symbol::Dup);
        let _c = subnet.create_agent(Symbol::Era);
        let mut free_port_index = HashMap::new();
        free_port_index.insert(0, crate::net::PortRef::AgentPort(a, 1));
        free_port_index.insert(1, crate::net::PortRef::AgentPort(b, 2));
        Partition {
            subnet,
            worker_id: 0,
            free_port_index,
            id_range: IdRange { start: 0, end: 100 },
            border_id_start: 0,
            border_id_end: 2,
        }
    }

    #[test]
    fn workerdeltastate_from_initial_partition_stores_partition() {
        let partition = make_delta_partition_with_borders();
        let state = WorkerDeltaState::from_initial_partition(partition);
        assert_eq!(state.partition.subnet.count_live_agents(), 3);
        assert_eq!(state.round, 0);
    }

    #[test]
    fn workerdeltastate_from_initial_partition_seeds_previous_border_state() {
        let partition = make_delta_partition_with_borders();
        let expected_seed = partition.free_port_index.clone();
        let state = WorkerDeltaState::from_initial_partition(partition);
        assert_eq!(state.previous_border_state, expected_seed);
    }

    #[test]
    fn workerdeltastate_from_initial_partition_empty_freeports() {
        let partition = make_partition();
        let state = WorkerDeltaState::from_initial_partition(partition);
        assert!(state.previous_border_state.is_empty());
    }

    #[test]
    fn workerdeltastate_clone_is_deep() {
        let partition = make_delta_partition_with_borders();
        let state = WorkerDeltaState::from_initial_partition(partition);
        let mut clone = state.clone();
        clone.previous_border_state.clear();
        assert_eq!(state.previous_border_state.len(), 2);
        assert!(clone.previous_border_state.is_empty());
    }

    // === TASK-0380: handle_initial_partition tests (SPEC-19 R21.1, R22) ===

    #[test]
    fn handle_initial_partition_stores_state() {
        let partition = make_delta_partition_with_borders();
        let expected_count = partition.subnet.count_live_agents();
        let mut ctx = WorkerContext::new();
        ctx.state = WorkerState::Idle;
        let _actions = handle_initial_partition(&mut ctx, 0, partition);
        assert!(ctx.delta_state.is_some());
        let stored = ctx.delta_state.as_ref().unwrap();
        assert_eq!(stored.partition.subnet.count_live_agents(), expected_count);
    }

    #[test]
    fn handle_initial_partition_transitions_to_delta_idle() {
        let partition = make_delta_partition_with_borders();
        let mut ctx = WorkerContext::new();
        ctx.state = WorkerState::Idle;
        let _ = handle_initial_partition(&mut ctx, 0, partition);
        assert_eq!(ctx.state, WorkerState::DeltaIdle);
    }

    #[test]
    fn handle_initial_partition_emits_log_transition() {
        let partition = make_delta_partition_with_borders();
        let mut ctx = WorkerContext::new();
        ctx.state = WorkerState::Idle;
        let actions = handle_initial_partition(&mut ctx, 0, partition);
        assert_eq!(actions.len(), 1);
        assert!(actions.iter().any(|a| matches!(
            a,
            WorkerAction::LogTransition {
                from: WorkerState::Idle,
                to: WorkerState::DeltaIdle
            }
        )));
        assert!(!actions
            .iter()
            .any(|a| matches!(a, WorkerAction::SendMessage(_))));
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "InitialPartition MUST arrive at round 0")]
    fn handle_initial_partition_round_nonzero_panics_in_debug() {
        let partition = make_delta_partition_with_borders();
        let mut ctx = WorkerContext::new();
        ctx.state = WorkerState::Idle;
        let _ = handle_initial_partition(&mut ctx, 1, partition);
    }

    #[test]
    fn handle_initial_partition_seeds_previous_border_state() {
        let partition = make_delta_partition_with_borders();
        let expected = partition.free_port_index.clone();
        let mut ctx = WorkerContext::new();
        ctx.state = WorkerState::Idle;
        let _ = handle_initial_partition(&mut ctx, 0, partition);
        let stored = ctx.delta_state.as_ref().unwrap();
        assert_eq!(stored.previous_border_state, expected);
    }

    // === TASK-0381: handle_round_start tests (SPEC-19 R23, R24, R26) ===
    //
    // The tests seed a worker with `handle_initial_partition` (so that
    // `ctx.state == DeltaIdle` and `ctx.delta_state` is populated
    // exactly as the production path requires at Round 1 entry) and
    // then exercise `handle_round_start` with crafted inputs.

    /// Builds a partition whose subnet has no active pairs (no
    /// principal↔principal wires) and an empty border list. Used by
    /// tests that need a deterministic, reduction-free handler call.
    fn make_quiet_partition() -> Partition {
        Partition {
            subnet: Net::new(),
            worker_id: 0,
            free_port_index: HashMap::new(),
            id_range: IdRange { start: 0, end: 100 },
            border_id_start: 100,
            border_id_end: 200,
        }
    }

    /// Builds a partition with a single CON agent whose aux port 1 is
    /// bound to a boundary `FreePort(border_id)`. Principal + aux-2 go
    /// to Lafont FreePorts so the subnet has no local redex.
    fn make_partition_with_border(border_id: u32) -> (Partition, crate::net::AgentId) {
        let mut subnet = Net::new();
        let a = subnet.create_agent(Symbol::Con);
        subnet.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(0));
        subnet.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(border_id));
        subnet.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));

        let free_port_index = crate::merge::helpers::rebuild_free_port_index(&subnet, 100, 200);
        let partition = Partition {
            subnet,
            worker_id: 0,
            free_port_index,
            id_range: IdRange { start: 0, end: 100 },
            border_id_start: 100,
            border_id_end: 200,
        };
        (partition, a)
    }

    /// Seeds a `WorkerContext` into `DeltaIdle` with the given partition.
    fn seed_delta_worker(partition: Partition) -> WorkerContext {
        let mut ctx = WorkerContext::new();
        ctx.state = WorkerState::Idle;
        let _ = handle_initial_partition(&mut ctx, 0, partition);
        ctx
    }

    // UT-0381-01 — empty deltas on a quiet partition: no reduction, no
    // outgoing deltas, no activity.
    #[test]
    fn handle_round_start_empty_deltas_no_reduction() {
        let mut ctx = seed_delta_worker(make_quiet_partition());
        let actions = handle_round_start(&mut ctx, 1, vec![], vec![], vec![], vec![], vec![]);

        // Stats must report zero local reductions.
        let send = actions
            .iter()
            .find_map(|a| match a {
                WorkerAction::SendMessage(m) => Some(m),
                _ => None,
            })
            .expect("RoundResult must be sent");
        match send.as_ref() {
            Message::RoundResult {
                border_deltas,
                stats,
                has_border_activity,
                ..
            } => {
                assert_eq!(stats.local_redexes, 0, "no redexes on a quiet partition");
                assert!(border_deltas.is_empty(), "no border deltas on empty input");
                assert!(!has_border_activity, "no principal-port borders");
            }
            other => panic!("expected RoundResult, got {:?}", other),
        }
    }

    // UT-0381-02 — a BorderDelta re-points the local side. The
    // post-rebuild `free_port_index` must reflect the new target.
    #[test]
    fn handle_round_start_applies_border_delta() {
        let (partition, _a) = make_partition_with_border(105);
        let mut subnet_b_partition = partition;
        let b = subnet_b_partition.subnet.create_agent(Symbol::Con);
        let mut ctx = seed_delta_worker(subnet_b_partition);

        let delta = BorderDelta {
            border_id: 105,
            new_target: PortRef::AgentPort(b, 1),
        };
        let _ = handle_round_start(&mut ctx, 1, vec![delta], vec![], vec![], vec![], vec![]);

        let stored = ctx.delta_state.as_ref().unwrap();
        assert_eq!(
            stored.partition.free_port_index[&105],
            PortRef::AgentPort(b, 1),
            "border 105 must re-point at (b, 1) after the delta"
        );
    }

    // UT-0381-03 — resolved_borders drops the entry from the index.
    #[test]
    fn handle_round_start_removes_resolved_border() {
        let (partition, _a) = make_partition_with_border(107);
        let mut ctx = seed_delta_worker(partition);

        let _ = handle_round_start(&mut ctx, 1, vec![], vec![107], vec![], vec![], vec![]);

        let stored = ctx.delta_state.as_ref().unwrap();
        assert!(
            !stored.partition.free_port_index.contains_key(&107),
            "border 107 must be removed from the index"
        );
    }

    // UT-0381-04 — new_borders inserts a fresh wire + index entry.
    #[test]
    fn handle_round_start_adds_new_border() {
        let mut partition = make_quiet_partition();
        let c = partition.subnet.create_agent(Symbol::Con);
        let mut ctx = seed_delta_worker(partition);

        let _ = handle_round_start(
            &mut ctx,
            1,
            vec![],
            vec![],
            vec![(109, PortRef::AgentPort(c, 1))],
            vec![],
            vec![],
        );

        let stored = ctx.delta_state.as_ref().unwrap();
        assert_eq!(
            stored.partition.free_port_index[&109],
            PortRef::AgentPort(c, 1),
            "border 109 must point at (c, 1) after new_borders insertion"
        );
    }

    // UT-0381-05 — a CON-CON active pair is reduced; stats reflect one
    // interaction with the CON-CON rule incremented.
    #[test]
    fn handle_round_start_reduces_and_reports_interactions() {
        use crate::reduction::SpecificRule;

        let mut subnet = Net::new();
        let a = subnet.create_agent(Symbol::Con);
        let b = subnet.create_agent(Symbol::Con);
        // Active pair on principal ports.
        subnet.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        // Aux ports: tie them together so reduce_all can finish cleanly.
        subnet.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 1));
        subnet.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(b, 2));

        let partition = Partition {
            subnet,
            worker_id: 0,
            free_port_index: HashMap::new(),
            id_range: IdRange { start: 0, end: 100 },
            border_id_start: 100,
            border_id_end: 200,
        };
        let mut ctx = seed_delta_worker(partition);

        let actions = handle_round_start(&mut ctx, 1, vec![], vec![], vec![], vec![], vec![]);
        let send = actions
            .iter()
            .find_map(|a| match a {
                WorkerAction::SendMessage(m) => Some(m),
                _ => None,
            })
            .expect("RoundResult must be sent");
        match send.as_ref() {
            Message::RoundResult { stats, .. } => {
                assert_eq!(stats.local_redexes, 1, "CON-CON pair reduces exactly once");
                assert_eq!(
                    stats.interactions_by_rule[SpecificRule::ConCon as usize],
                    1,
                    "CON-CON rule counter must be 1"
                );
            }
            other => panic!("expected RoundResult, got {:?}", other),
        }
    }

    // UT-0381-06 — previous_border_state is snapshotted to current
    // after every round (R25).
    #[test]
    fn handle_round_start_updates_previous_border_state() {
        let (partition, _a) = make_partition_with_border(103);
        let mut ctx = seed_delta_worker(partition);

        let _ = handle_round_start(&mut ctx, 1, vec![], vec![], vec![], vec![], vec![]);

        let stored = ctx.delta_state.as_ref().unwrap();
        assert_eq!(
            stored.previous_border_state, stored.partition.free_port_index,
            "previous_border_state must mirror current free_port_index"
        );
    }

    // UT-0381-07 — the handler transitions DeltaIdle -> DeltaActive.
    #[test]
    fn handle_round_start_transitions_to_delta_active() {
        let (partition, _a) = make_partition_with_border(104);
        let mut ctx = seed_delta_worker(partition);
        assert_eq!(ctx.state, WorkerState::DeltaIdle);

        let _ = handle_round_start(&mut ctx, 1, vec![], vec![], vec![], vec![], vec![]);
        assert_eq!(ctx.state, WorkerState::DeltaActive);
    }

    // UT-0381-08 — the handler emits exactly one SendMessage wrapping
    // RoundResult whose round echoes the input.
    #[test]
    fn handle_round_start_emits_send_message_round_result() {
        let mut ctx = seed_delta_worker(make_quiet_partition());

        let actions = handle_round_start(&mut ctx, 42, vec![], vec![], vec![], vec![], vec![]);
        let sends: Vec<_> = actions
            .iter()
            .filter_map(|a| match a {
                WorkerAction::SendMessage(m) => Some(m),
                _ => None,
            })
            .collect();
        assert_eq!(sends.len(), 1, "exactly one SendMessage must be emitted");
        match sends[0].as_ref() {
            Message::RoundResult { round, .. } => {
                assert_eq!(*round, 42, "RoundResult.round must echo input round");
            }
            other => panic!("expected RoundResult, got {:?}", other),
        }
    }

    // ============================================================
    // TASK-0394: handle_round_start full R23/R26 completion tests
    // (SPEC-19 §3.3 R23 full payload, R26 minted_agents echo, DC-B5
    // 2-phase allocation second half). Baseline: UT-0381-01..09 remain
    // valid with the 2-arg signature extension (all pass vec![], vec![]).
    // Adds 11 new #[test] fns exercising:
    //   - local_reconnections application (UT-0394-01..04)
    //   - pending_commutations → minted_agents echo (UT-0394-05..09)
    //   - error paths / invariants (UT-0394-10, UT-0394-11)
    // ============================================================

    // UT-0394-01 — backward-compat sanity: empty new fields → output is
    // equivalent to UT-0381-01's 5-arg baseline behavior.
    #[test]
    fn ut_0394_01_empty_local_reconnections_empty_pending_commutations_unchanged() {
        let mut ctx = seed_delta_worker(make_quiet_partition());
        let actions = handle_round_start(&mut ctx, 1, vec![], vec![], vec![], vec![], vec![]);
        let send = actions
            .iter()
            .find_map(|a| match a {
                WorkerAction::SendMessage(m) => Some(m),
                _ => None,
            })
            .expect("RoundResult must be emitted (no pending exhaustion)");
        match send.as_ref() {
            Message::RoundResult {
                border_deltas,
                stats,
                has_border_activity,
                minted_agents,
                ..
            } => {
                assert_eq!(stats.local_redexes, 0);
                assert!(border_deltas.is_empty());
                assert!(!has_border_activity);
                assert!(
                    minted_agents.is_empty(),
                    "empty pending_commutations must yield empty minted_agents"
                );
            }
            other => panic!("expected RoundResult, got {:?}", other),
        }
        assert_eq!(ctx.state, WorkerState::DeltaActive);
    }

    // UT-0394-02 — single LocalReconnection mutates subnet before reduce.
    #[test]
    fn ut_0394_02_applies_single_local_reconnection() {
        // Two CON agents, ports 1 and 2 unconnected; resolver directs
        // rewire of a.1 → b.2.
        let mut subnet = Net::new();
        let a = subnet.create_agent(Symbol::Con);
        let b = subnet.create_agent(Symbol::Con);
        // Principal ports: bind to distinct FreePorts so reduce_all
        // finds no local redex (avoid accidental reduction during the test).
        subnet.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(0));
        subnet.connect(PortRef::AgentPort(b, 0), PortRef::FreePort(1));
        // Aux pairs: leave (a,2) and (b,1) dangling so the rewire is
        // observable on (a,1) and (b,2).
        subnet.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(2));
        subnet.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(3));

        let partition = Partition {
            subnet,
            worker_id: 0,
            free_port_index: HashMap::new(),
            id_range: IdRange { start: 0, end: 100 },
            border_id_start: 100,
            border_id_end: 200,
        };
        let mut ctx = seed_delta_worker(partition);

        let lr = LocalReconnection {
            agent_id: a,
            port: 1,
            new_target: PortRef::AgentPort(b, 2),
        };
        let _ = handle_round_start(&mut ctx, 1, vec![], vec![], vec![], vec![lr], vec![]);

        let stored = ctx.delta_state.as_ref().unwrap();
        assert_eq!(
            stored.partition.subnet.get_target(PortRef::AgentPort(a, 1)),
            PortRef::AgentPort(b, 2),
            "local_reconnection must rewire a.1 to b.2"
        );
        assert_eq!(
            stored.partition.subnet.get_target(PortRef::AgentPort(b, 2)),
            PortRef::AgentPort(a, 1),
            "Net::connect symmetry: b.2 must point back at a.1"
        );
    }

    // UT-0394-03 — multiple LocalReconnections applied sequentially. Two
    // reconnections on disjoint ports should both take effect.
    #[test]
    fn ut_0394_03_applies_multiple_local_reconnections() {
        let mut subnet = Net::new();
        let a = subnet.create_agent(Symbol::Con);
        let b = subnet.create_agent(Symbol::Con);
        let c = subnet.create_agent(Symbol::Con);
        // Seed all ports DISCONNECTED (Net::create_agent initializes to DISCONNECTED).
        let partition = Partition {
            subnet,
            worker_id: 0,
            free_port_index: HashMap::new(),
            id_range: IdRange { start: 0, end: 100 },
            border_id_start: 100,
            border_id_end: 200,
        };
        let mut ctx = seed_delta_worker(partition);

        let lrs = vec![
            LocalReconnection {
                agent_id: a,
                port: 1,
                new_target: PortRef::AgentPort(b, 1),
            },
            LocalReconnection {
                agent_id: a,
                port: 2,
                new_target: PortRef::AgentPort(c, 1),
            },
        ];
        let _ = handle_round_start(&mut ctx, 1, vec![], vec![], vec![], lrs, vec![]);

        let stored = ctx.delta_state.as_ref().unwrap();
        assert_eq!(
            stored.partition.subnet.get_target(PortRef::AgentPort(a, 1)),
            PortRef::AgentPort(b, 1),
            "first LR: a.1 → b.1"
        );
        assert_eq!(
            stored.partition.subnet.get_target(PortRef::AgentPort(a, 2)),
            PortRef::AgentPort(c, 1),
            "second LR: a.2 → c.1"
        );
    }

    // UT-0394-04 — self-loop in LocalReconnection (new_target == source
    // port) is skipped without panic; tracing::trace! captures it.
    #[test]
    fn ut_0394_04_skips_self_loop_in_local_reconnections() {
        let mut subnet = Net::new();
        let a = subnet.create_agent(Symbol::Con);
        let b = subnet.create_agent(Symbol::Con);
        let partition = Partition {
            subnet,
            worker_id: 0,
            free_port_index: HashMap::new(),
            id_range: IdRange { start: 0, end: 100 },
            border_id_start: 100,
            border_id_end: 200,
        };
        let mut ctx = seed_delta_worker(partition);

        let lrs = vec![
            // self-loop: a.1 → a.1 — skipped
            LocalReconnection {
                agent_id: a,
                port: 1,
                new_target: PortRef::AgentPort(a, 1),
            },
            // valid: b.1 → b.2 — applied
            LocalReconnection {
                agent_id: b,
                port: 1,
                new_target: PortRef::AgentPort(b, 2),
            },
        ];
        let _ = handle_round_start(&mut ctx, 1, vec![], vec![], vec![], lrs, vec![]);

        let stored = ctx.delta_state.as_ref().unwrap();
        // Self-loop NOT applied: a.1 stays DISCONNECTED.
        assert_eq!(
            stored.partition.subnet.get_target(PortRef::AgentPort(a, 1)),
            crate::net::DISCONNECTED,
            "self-loop must be skipped (DC-B3 defensive)"
        );
        // Valid pair applied.
        assert_eq!(
            stored.partition.subnet.get_target(PortRef::AgentPort(b, 1)),
            PortRef::AgentPort(b, 2),
            "second LR must apply after skip"
        );
    }

    // UT-0394-05 — single PendingCommutation mints 1 agent, minted_agents
    // carries matching request_id + fresh AgentId from id_range.
    #[test]
    fn ut_0394_05_mints_agent_for_single_pending_commutation() {
        let partition = Partition {
            subnet: Net::new(),
            worker_id: 0,
            free_port_index: HashMap::new(),
            // Worker 0's range starts at 100 to test that id_range is honored.
            id_range: IdRange {
                start: 100,
                end: 200,
            },
            border_id_start: 200,
            border_id_end: 300,
        };
        // Partition split sets subnet.next_id = id_range.start when the
        // partition has no prior agents; here we simulate that.
        let mut partition = partition;
        partition.subnet.next_id = 100;
        let mut ctx = seed_delta_worker(partition);

        let pc = PendingCommutation {
            request_id: 42,
            symbol_type: Symbol::Con,
            arity: 2,
        };
        let actions = handle_round_start(&mut ctx, 1, vec![], vec![], vec![], vec![], vec![pc]);
        let send = actions
            .iter()
            .find_map(|a| match a {
                WorkerAction::SendMessage(m) => Some(m),
                _ => None,
            })
            .expect("RoundResult must be emitted");
        match send.as_ref() {
            Message::RoundResult { minted_agents, .. } => {
                assert_eq!(minted_agents.len(), 1);
                assert_eq!(minted_agents[0].request_id, 42);
                assert_eq!(minted_agents[0].minted_agent_id, 100);
            }
            other => panic!("expected RoundResult, got {:?}", other),
        }
        let stored = ctx.delta_state.as_ref().unwrap();
        assert_eq!(stored.partition.subnet.next_id, 101);
    }

    // UT-0394-06 — multiple PendingCommutations: AgentIds are allocated
    // contiguously from id_range.start, minted_agents preserves order.
    #[test]
    fn ut_0394_06_mints_multiple_agents_with_contiguous_ids() {
        let mut partition = Partition {
            subnet: Net::new(),
            worker_id: 0,
            free_port_index: HashMap::new(),
            id_range: IdRange {
                start: 100,
                end: 200,
            },
            border_id_start: 200,
            border_id_end: 300,
        };
        partition.subnet.next_id = 100;
        let mut ctx = seed_delta_worker(partition);

        let pcs = vec![
            PendingCommutation {
                request_id: 1,
                symbol_type: Symbol::Con,
                arity: 2,
            },
            PendingCommutation {
                request_id: 2,
                symbol_type: Symbol::Dup,
                arity: 2,
            },
            PendingCommutation {
                request_id: 3,
                symbol_type: Symbol::Era,
                arity: 0,
            },
        ];
        let actions = handle_round_start(&mut ctx, 2, vec![], vec![], vec![], vec![], pcs);
        let send = actions
            .iter()
            .find_map(|a| match a {
                WorkerAction::SendMessage(m) => Some(m),
                _ => None,
            })
            .expect("RoundResult must be emitted");
        match send.as_ref() {
            Message::RoundResult { minted_agents, .. } => {
                assert_eq!(minted_agents.len(), 3);
                assert_eq!(minted_agents[0].request_id, 1);
                assert_eq!(minted_agents[0].minted_agent_id, 100);
                assert_eq!(minted_agents[1].request_id, 2);
                assert_eq!(minted_agents[1].minted_agent_id, 101);
                assert_eq!(minted_agents[2].request_id, 3);
                assert_eq!(minted_agents[2].minted_agent_id, 102);
            }
            other => panic!("expected RoundResult, got {:?}", other),
        }
    }

    // UT-0394-07 — DC-B5 non-overlap: independent id_ranges on two
    // workers do not collide.
    #[test]
    fn ut_0394_07_id_ranges_do_not_collide_across_workers() {
        fn seed(range_start: u32, worker_id: u32) -> WorkerContext {
            let mut partition = Partition {
                subnet: Net::new(),
                worker_id,
                free_port_index: HashMap::new(),
                id_range: IdRange {
                    start: range_start,
                    end: range_start + 100,
                },
                border_id_start: range_start + 100,
                border_id_end: range_start + 200,
            };
            partition.subnet.next_id = range_start;
            seed_delta_worker(partition)
        }
        let mut ctx_a = seed(100, 0);
        let mut ctx_b = seed(200, 1);
        let pc = PendingCommutation {
            request_id: 1,
            symbol_type: Symbol::Con,
            arity: 2,
        };
        let actions_a = handle_round_start(
            &mut ctx_a,
            1,
            vec![],
            vec![],
            vec![],
            vec![],
            vec![pc.clone()],
        );
        let actions_b = handle_round_start(&mut ctx_b, 1, vec![], vec![], vec![], vec![], vec![pc]);

        let id_a = match actions_a
            .iter()
            .find_map(|a| match a {
                WorkerAction::SendMessage(m) => Some(m.as_ref()),
                _ => None,
            })
            .unwrap()
        {
            Message::RoundResult { minted_agents, .. } => minted_agents[0].minted_agent_id,
            _ => unreachable!(),
        };
        let id_b = match actions_b
            .iter()
            .find_map(|a| match a {
                WorkerAction::SendMessage(m) => Some(m.as_ref()),
                _ => None,
            })
            .unwrap()
        {
            Message::RoundResult { minted_agents, .. } => minted_agents[0].minted_agent_id,
            _ => unreachable!(),
        };
        assert_eq!(id_a, 100);
        assert_eq!(id_b, 200);
        assert_ne!(id_a, id_b, "DC-B5 non-overlap: id_ranges must not collide");
    }

    // UT-0394-08 — symbol transparency: the agent created has the exact
    // Symbol the PendingCommutation requested.
    #[test]
    fn ut_0394_08_minted_agent_symbol_matches_request() {
        let mut partition = Partition {
            subnet: Net::new(),
            worker_id: 0,
            free_port_index: HashMap::new(),
            id_range: IdRange { start: 0, end: 100 },
            border_id_start: 100,
            border_id_end: 200,
        };
        partition.subnet.next_id = 0;
        let mut ctx = seed_delta_worker(partition);

        let pcs = vec![
            PendingCommutation {
                request_id: 1,
                symbol_type: Symbol::Con,
                arity: 2,
            },
            PendingCommutation {
                request_id: 2,
                symbol_type: Symbol::Dup,
                arity: 2,
            },
        ];
        let _ = handle_round_start(&mut ctx, 1, vec![], vec![], vec![], vec![], pcs);

        let stored = ctx.delta_state.as_ref().unwrap();
        // Agents at IDs 0 and 1 must exist with the requested symbols.
        let a0 = stored.partition.subnet.agents[0].as_ref().unwrap();
        let a1 = stored.partition.subnet.agents[1].as_ref().unwrap();
        assert_eq!(a0.symbol, Symbol::Con);
        assert_eq!(a1.symbol, Symbol::Dup);
    }

    // UT-0394-09 — DC-0394-B: minted_agents preserves pending_commutations
    // input order even when request_ids are non-monotonic.
    #[test]
    fn ut_0394_09_minted_agents_preserve_input_order() {
        let mut partition = Partition {
            subnet: Net::new(),
            worker_id: 0,
            free_port_index: HashMap::new(),
            id_range: IdRange { start: 0, end: 100 },
            border_id_start: 100,
            border_id_end: 200,
        };
        partition.subnet.next_id = 0;
        let mut ctx = seed_delta_worker(partition);

        let pcs = vec![
            PendingCommutation {
                request_id: 7,
                symbol_type: Symbol::Con,
                arity: 2,
            },
            PendingCommutation {
                request_id: 3,
                symbol_type: Symbol::Con,
                arity: 2,
            },
            PendingCommutation {
                request_id: 11,
                symbol_type: Symbol::Con,
                arity: 2,
            },
            PendingCommutation {
                request_id: 2,
                symbol_type: Symbol::Con,
                arity: 2,
            },
            PendingCommutation {
                request_id: 5,
                symbol_type: Symbol::Con,
                arity: 2,
            },
        ];
        let actions = handle_round_start(&mut ctx, 3, vec![], vec![], vec![], vec![], pcs);
        let ids: Vec<u32> = match actions
            .iter()
            .find_map(|a| match a {
                WorkerAction::SendMessage(m) => Some(m.as_ref()),
                _ => None,
            })
            .unwrap()
        {
            Message::RoundResult { minted_agents, .. } => {
                minted_agents.iter().map(|m| m.request_id).collect()
            }
            _ => unreachable!(),
        };
        assert_eq!(
            ids,
            vec![7, 3, 11, 2, 5],
            "DC-0394-B: input order MUST be preserved"
        );
    }

    // UT-0394-10 — id_range exhaustion mid-loop: returns Error action,
    // NO RoundResult, partial progress preserved.
    #[test]
    fn ut_0394_10_id_range_exhaustion_returns_error_no_round_result() {
        use crate::error::WorkerError;

        // id_range has room for 2 AgentIds only; 3 PendingCommutations
        // should allocate 2 and error on the 3rd.
        let mut partition = Partition {
            subnet: Net::new(),
            worker_id: 0,
            free_port_index: HashMap::new(),
            id_range: IdRange {
                start: 100,
                end: 102,
            },
            border_id_start: 200,
            border_id_end: 300,
        };
        partition.subnet.next_id = 100;
        let mut ctx = seed_delta_worker(partition);

        let pcs = vec![
            PendingCommutation {
                request_id: 1,
                symbol_type: Symbol::Con,
                arity: 2,
            },
            PendingCommutation {
                request_id: 2,
                symbol_type: Symbol::Dup,
                arity: 2,
            },
            PendingCommutation {
                request_id: 3,
                symbol_type: Symbol::Era,
                arity: 0,
            },
        ];
        let actions = handle_round_start(&mut ctx, 1, vec![], vec![], vec![], vec![], pcs);

        // No RoundResult SendMessage.
        let sends = actions
            .iter()
            .filter(|a| matches!(a, WorkerAction::SendMessage(_)))
            .count();
        assert_eq!(
            sends, 0,
            "on exhaustion handler MUST NOT emit Message::RoundResult"
        );
        // Exactly one Error action with request_id=3 (the one that
        // exceeded the range).
        let err_request_ids: Vec<u32> = actions
            .iter()
            .filter_map(|a| match a {
                WorkerAction::Error(WorkerError::IdRangeExhausted { request_id }) => {
                    Some(*request_id)
                }
                _ => None,
            })
            .collect();
        assert_eq!(err_request_ids, vec![3]);

        // Partial progress: first two agents committed, third never allocated.
        let stored = ctx.delta_state.as_ref().unwrap();
        assert_eq!(stored.partition.subnet.next_id, 102);
        assert!(stored.partition.subnet.agents[100].is_some());
        assert!(stored.partition.subnet.agents[101].is_some());
        assert_eq!(
            stored.partition.subnet.agents.len(),
            102,
            "arena must not extend past the two committed agents"
        );
    }

    // UT-0394-11 — arity consistency: PendingCommutation.arity matching
    // the symbol's natural arity does not panic; the minted agent has
    // the correct natural arity of ports.
    #[test]
    fn ut_0394_11_arity_consistent_with_symbol_succeeds() {
        let mut partition = Partition {
            subnet: Net::new(),
            worker_id: 0,
            free_port_index: HashMap::new(),
            id_range: IdRange { start: 0, end: 100 },
            border_id_start: 100,
            border_id_end: 200,
        };
        partition.subnet.next_id = 0;
        let mut ctx = seed_delta_worker(partition);

        // Era is arity 0 (natural); the pending_commutation's arity field
        // must match. If it didn't, the debug_assert in handle_round_start
        // would fire (guarded by debug_assertions).
        let pc_era = PendingCommutation {
            request_id: 1,
            symbol_type: Symbol::Era,
            arity: 0,
        };
        let pc_con = PendingCommutation {
            request_id: 2,
            symbol_type: Symbol::Con,
            arity: 2,
        };
        let _ = handle_round_start(
            &mut ctx,
            1,
            vec![],
            vec![],
            vec![],
            vec![],
            vec![pc_era, pc_con],
        );

        let stored = ctx.delta_state.as_ref().unwrap();
        // Era agent: 0 aux ports, but the net arena still holds the
        // principal slot. Con agent: 2 aux ports.
        let era_agent = stored.partition.subnet.agents[0].as_ref().unwrap();
        let con_agent = stored.partition.subnet.agents[1].as_ref().unwrap();
        assert_eq!(era_agent.symbol, Symbol::Era);
        assert_eq!(con_agent.symbol, Symbol::Con);
    }

    // ============================================================
    // QA adversarial probes (Stage 5 partial — Option B rigor pass,
    // post-DEV of TASK-0394..0397, pre-commit). Each probe closes a
    // gap flagged in REVIEW-SPEC-19-section-3.3-3.5-3.6-item-2.26-BCD-2026-04-23
    // or TEST-SPEC-0394 that the DEV-stage unit tests did not exercise.
    // ============================================================

    // QA-0394-A — duplicate `request_id` in pending_commutations.
    // Spec R48 states the coordinator rejects stray request_ids; it
    // does NOT mandate the worker detect duplicates. Current worker
    // contract is LENIENT: it mints both agents, each with the same
    // echoed request_id in `minted_agents`. This probe PINS that
    // behavior so a future refactor that silently tightens it (or
    // loosens a tightening) is caught.
    #[test]
    fn qa_0394_a_duplicate_request_id_in_pending_commutations_is_lenient() {
        let mut partition = Partition {
            subnet: Net::new(),
            worker_id: 0,
            free_port_index: HashMap::new(),
            id_range: IdRange { start: 0, end: 100 },
            border_id_start: 100,
            border_id_end: 200,
        };
        partition.subnet.next_id = 0;
        let mut ctx = seed_delta_worker(partition);

        // Both entries share request_id = 42 — a resolver-side bug or
        // a malicious coordinator. The worker's current contract is
        // NOT to detect this; it mints both agents.
        let pcs = vec![
            PendingCommutation {
                request_id: 42,
                symbol_type: Symbol::Con,
                arity: 2,
            },
            PendingCommutation {
                request_id: 42,
                symbol_type: Symbol::Dup,
                arity: 2,
            },
        ];
        let actions = handle_round_start(&mut ctx, 1, vec![], vec![], vec![], vec![], pcs);
        let send = actions
            .iter()
            .find_map(|a| match a {
                WorkerAction::SendMessage(m) => Some(m),
                _ => None,
            })
            .expect("RoundResult must be emitted (lenient contract)");
        match send.as_ref() {
            Message::RoundResult { minted_agents, .. } => {
                assert_eq!(
                    minted_agents.len(),
                    2,
                    "lenient contract: worker mints one agent per pending_commutation \
                     regardless of request_id duplication"
                );
                assert_eq!(minted_agents[0].request_id, 42);
                assert_eq!(minted_agents[1].request_id, 42);
                // IDs must still be distinct — id_range allocator
                // doesn't care about request_id.
                assert_ne!(
                    minted_agents[0].minted_agent_id,
                    minted_agents[1].minted_agent_id,
                    "id_range allocator must produce distinct AgentIds even under request_id collision"
                );
            }
            other => panic!("expected RoundResult, got {:?}", other),
        }
    }

    // QA-0394-F — exhaustion check treats `next_id == id_range.end`
    // as already-exhausted (off-by-one guard on the boundary check).
    //
    // UT-0394-10 covers mid-loop exhaustion; this probe covers the
    // stricter case where the FIRST PendingCommutation is rejected
    // because the worker enters the round with the range already
    // full. Protects the `>=` semantics of the check against an
    // accidental `>` refactor that would allow one over-allocation.
    //
    // NOTE: this intentionally uses small id_range boundaries to
    // avoid triggering `Net::agents.resize(id + 1, None)` with a
    // massive dense arena (the arena is O(max_AgentId); testing at
    // u32::MAX would attempt a ~100 GB allocation and OOM the test
    // process). The semantic property being tested — the boundary
    // `next_id >= id_range.end` — is independent of the absolute
    // values, so a single-slot range near 100 captures the same
    // logic as a single-slot range near u32::MAX.
    #[test]
    fn qa_0394_f_exhaustion_check_treats_next_id_equals_end_as_exhausted() {
        use crate::error::WorkerError;

        let mut partition = Partition {
            subnet: Net::new(),
            worker_id: 0,
            free_port_index: HashMap::new(),
            // Single-slot range [100, 101): exactly one mintable id = 100.
            id_range: IdRange {
                start: 100,
                end: 101,
            },
            border_id_start: 200,
            border_id_end: 300,
        };
        // Seed next_id already at id_range.end — already-exhausted
        // on entry. An off-by-one `>` check would incorrectly permit
        // one more allocation here.
        partition.subnet.next_id = 101;
        let mut ctx = seed_delta_worker(partition);

        let pc = PendingCommutation {
            request_id: 7,
            symbol_type: Symbol::Con,
            arity: 2,
        };
        let actions = handle_round_start(&mut ctx, 1, vec![], vec![], vec![], vec![], vec![pc]);

        // Exhaustion fires on the FIRST iteration — no RoundResult, one Error.
        assert!(
            !actions
                .iter()
                .any(|a| matches!(a, WorkerAction::SendMessage(_))),
            "handler MUST NOT emit RoundResult when the first PC cannot be minted"
        );
        let errs: Vec<u32> = actions
            .iter()
            .filter_map(|a| match a {
                WorkerAction::Error(WorkerError::IdRangeExhausted { request_id }) => {
                    Some(*request_id)
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            errs,
            vec![7],
            "boundary check must treat next_id == id_range.end as already-exhausted"
        );

        // No mint committed — partition.next_id unchanged.
        let stored = ctx.delta_state.as_ref().unwrap();
        assert_eq!(
            stored.partition.subnet.next_id, 101,
            "next_id must not advance when exhausted at entry"
        );
    }

    // UT-0381-09 (helper) — apply_border_deltas on all-empty inputs
    // leaves the subnet's agent count unchanged.
    #[test]
    fn apply_border_deltas_to_partition_preserves_agent_count_empty() {
        use crate::merge::helpers::apply_border_deltas_to_partition;

        let mut partition = make_quiet_partition();
        let _ = partition.subnet.create_agent(Symbol::Con);
        let _ = partition.subnet.create_agent(Symbol::Dup);
        let before = partition.subnet.count_live_agents();

        apply_border_deltas_to_partition(&mut partition, &[], &[], &[]);
        let after = partition.subnet.count_live_agents();

        assert_eq!(before, after, "empty apply MUST preserve agent count");
    }

    // === TASK-0383: handle_final_state_request tests (SPEC-19 R21 phase 3, R28) ===

    // UT-0383-01 — from DeltaActive: transitions to Returning and emits
    // FinalStateResult carrying the stored partition.
    #[test]
    fn handle_final_state_request_from_delta_active() {
        let partition = make_delta_partition_with_borders();
        let expected_agents = partition.subnet.count_live_agents();
        let mut ctx = seed_delta_worker(partition);
        ctx.state = WorkerState::DeltaActive;

        let actions = handle_final_state_request(&mut ctx, 7);

        assert_eq!(ctx.state, WorkerState::Returning);
        let send = actions
            .iter()
            .find_map(|a| match a {
                WorkerAction::SendMessage(m) => Some(m),
                _ => None,
            })
            .expect("FinalStateResult must be sent");
        match send.as_ref() {
            Message::FinalStateResult { round, partition } => {
                assert_eq!(*round, 7);
                assert_eq!(partition.subnet.count_live_agents(), expected_agents);
            }
            other => panic!("expected FinalStateResult, got {:?}", other),
        }
    }

    // UT-0383-02 — from DeltaIdle (Round-0-only convergence): same
    // transitions and payload as DeltaActive.
    #[test]
    fn handle_final_state_request_from_delta_idle() {
        let partition = make_delta_partition_with_borders();
        let expected_agents = partition.subnet.count_live_agents();
        let mut ctx = seed_delta_worker(partition);
        assert_eq!(ctx.state, WorkerState::DeltaIdle);

        let actions = handle_final_state_request(&mut ctx, 0);

        assert_eq!(ctx.state, WorkerState::Returning);
        let send = actions
            .iter()
            .find_map(|a| match a {
                WorkerAction::SendMessage(m) => Some(m),
                _ => None,
            })
            .expect("FinalStateResult must be sent");
        match send.as_ref() {
            Message::FinalStateResult { round, partition } => {
                assert_eq!(*round, 0);
                assert_eq!(partition.subnet.count_live_agents(), expected_agents);
            }
            other => panic!("expected FinalStateResult, got {:?}", other),
        }
    }

    // UT-0383-03 — after the call, ctx.delta_state is None (state was
    // `.take()`n and moved into the outgoing message).
    #[test]
    fn handle_final_state_request_clears_delta_state() {
        let mut ctx = seed_delta_worker(make_delta_partition_with_borders());
        let _ = handle_final_state_request(&mut ctx, 1);
        assert!(
            ctx.delta_state.is_none(),
            "delta_state MUST be cleared after FinalStateResult (memory freed)"
        );
    }

    // UT-0383-04 — calling without delta_state panics (caller invariant).
    #[test]
    #[should_panic(expected = "handle_final_state_request requires prior handle_initial_partition")]
    fn handle_final_state_request_without_delta_state_panics() {
        let mut ctx = WorkerContext::new();
        ctx.state = WorkerState::DeltaIdle; // state set, but delta_state None
        let _ = handle_final_state_request(&mut ctx, 0);
    }

    // UT-0383-05 — the emitted FinalStateResult.round echoes the input.
    #[test]
    fn handle_final_state_request_echoes_round() {
        let mut ctx = seed_delta_worker(make_delta_partition_with_borders());
        ctx.state = WorkerState::DeltaActive;

        let actions = handle_final_state_request(&mut ctx, 42);
        let send = actions
            .iter()
            .find_map(|a| match a {
                WorkerAction::SendMessage(m) => Some(m),
                _ => None,
            })
            .expect("FinalStateResult must be sent");
        match send.as_ref() {
            Message::FinalStateResult { round, .. } => assert_eq!(*round, 42),
            other => panic!("expected FinalStateResult, got {:?}", other),
        }
    }
}
