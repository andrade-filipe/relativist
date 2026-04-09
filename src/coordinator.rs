//! Coordinator FSM for distributed IC reduction (SPEC-13 R19-R23).
//!
//! The coordinator implements the BSP (Bulk Synchronous Parallel)
//! programming model (SPEC-13 R1, R4). Each grid round is one BSP
//! superstep: split → dispatch → reduce → collect → merge → check.
//!
//! The FSM follows the stimulus-response pattern (SPEC-13 R20):
//! a pure function `transition(state, event) -> (new_state, actions)`
//! that is testable without tokio.

use std::net::SocketAddr;
use std::time::Duration;

use crate::net::Net;
use crate::partition::{Partition, WorkerId};
use crate::protocol::Message;

/// Unique identifier for a timer managed by the coordinator's async runtime.
pub type TimerId = u32;

// ---------------------------------------------------------------------------
// FSM types (TASK-0107, SPEC-13 R19-R22)
// ---------------------------------------------------------------------------

/// Coordinator FSM states (SPEC-13 R19).
///
/// Enum-based (not typestate) per SPEC-13 R22 for serialization,
/// logging, and testing ergonomics.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum CoordinatorState {
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
}

/// Events that drive the coordinator FSM (SPEC-13 R20).
#[derive(Debug, Clone)]
pub enum CoordinatorEvent {
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
}

/// Actions the coordinator runtime must execute (SPEC-13 R20).
///
/// `InvokeSplit` and `InvokeMergeAndReduce` preserve stimulus-response
/// purity: the runtime executes them (possibly via `spawn_blocking`)
/// and fires `SplitComplete` / `MergeComplete` events back into the FSM.
#[derive(Debug)]
pub enum CoordinatorAction {
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
}

// ---------------------------------------------------------------------------
// FSM transition (TASK-0108, SPEC-13 R21)
// ---------------------------------------------------------------------------

/// Mutable context for the coordinator FSM.
///
/// Holds the state that the pure transition function needs to read
/// but that is not part of the event itself.
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
            actions.push(CoordinatorAction::StartTimer(0, Duration::from_secs(600)));
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
                actions.push(CoordinatorAction::CancelTimer(0));
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
        ];
        for i in 0..states.len() {
            for j in (i + 1)..states.len() {
                assert_ne!(states[i], states[j]);
            }
        }
    }

    // === TASK-0108: transition tests ===

    fn make_ctx() -> CoordinatorContext {
        let mut ctx = CoordinatorContext::new("127.0.0.1:9000".parse().unwrap(), 2);
        ctx.net = Some(Net::new());
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
}
