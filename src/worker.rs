//! Worker FSM for distributed IC reduction (SPEC-13 R24-R27).
//!
//! Workers connect to the coordinator, receive partitions, reduce
//! them locally via reduce_all (SPEC-03), and return results.
//! Workers have no knowledge of each other (star topology, SPEC-13 R27).

use crate::partition::Partition;
use crate::protocol::Message;

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
    LogTransition {
        from: WorkerState,
        to: WorkerState,
    },
}

// ---------------------------------------------------------------------------
// FSM transition (TASK-0110, SPEC-13 R25)
// ---------------------------------------------------------------------------

/// Worker FSM context.
pub struct WorkerContext {
    pub state: WorkerState,
    /// Current round number (echoed from AssignPartition).
    pub round: u32,
}

impl Default for WorkerContext {
    fn default() -> Self {
        Self {
            state: WorkerState::Init,
            round: 0,
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
            actions.push(WorkerAction::SendMessage(Box::new(Message::PartitionResult {
                round: ctx.round,
                partition: partition.clone(),
                stats: crate::merge::WorkerRoundStats {
                    worker_id: partition.worker_id,
                    agents_before: 0, // filled by the runtime
                    agents_after: partition.subnet.count_live_agents(),
                    local_redexes: 0, // filled by the runtime
                    reduce_duration_secs: 0.0, // filled by the runtime
                    interactions_by_rule: [0; 6], // filled by the runtime
                },
            })));
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
        let states = vec![
            WorkerState::Init,
            WorkerState::Idle,
            WorkerState::Reducing,
            WorkerState::Returning,
            WorkerState::Error,
            WorkerState::Done,
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
        assert!(actions.iter().any(|a| matches!(a, WorkerAction::LogTransition { .. })));
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
        assert!(actions.iter().any(|a| matches!(a, WorkerAction::SendMessage(_))));
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
        assert!(actions.iter().any(|a| matches!(a, WorkerAction::CloseConnection)));
    }

    #[test]
    fn test_reduction_error() {
        let mut ctx = WorkerContext::new();
        ctx.state = WorkerState::Reducing;
        let actions = transition(&mut ctx, WorkerEvent::ReductionError("bad".into()));
        assert_eq!(ctx.state, WorkerState::Error);
        assert!(actions.iter().any(|a| matches!(a, WorkerAction::SendMessage(m) if matches!(**m, Message::Error { .. }))));
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
            assert!(actions.iter().any(|a| matches!(a, WorkerAction::ShutdownSelf)));
        }
    }
}
