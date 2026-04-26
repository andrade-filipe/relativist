//! Protocol message types (SPEC-06 + SPEC-19 §3.4).
//!
//! Defines the Message enum with 12 variants covering all
//! coordinator-worker communication.
//!
//! Variant ownership, by spec:
//!
//! - **SPEC-06** (core protocol) — discriminants 0..=3:
//!   `AssignPartition`, `Shutdown`, `PartitionResult`, `Error`.
//! - **SPEC-10** (registration/authentication) — discriminants 4..=6:
//!   `Register`, `RegisterAck`, `RegisterNack`.
//! - **SPEC-19 §3.4** (delta protocol) — discriminants 7..=11:
//!   `InitialPartition`, `RoundStart`, `RoundResult`,
//!   `FinalStateRequest`, `FinalStateResult`.
//!
//! SPEC-06 is the canonical owner of the Message enum definition. The
//! SPEC-19 §3.4 additions preserve R37 discriminant stability by
//! appending strictly at the tail (R37).

use serde::{Deserialize, Serialize};

use crate::merge::{
    BorderDelta, LocalReconnection, MintedAgent, PendingCommutation, WorkerRoundStats,
};
use crate::net::PortRef;
use crate::partition::{Partition, WorkerId};

/// Messages in the communication protocol between coordinator and workers.
///
/// The enum covers all possible communication. Each variant is annotated
/// with the direction (Coordinator->Worker or Worker->Coordinator) and the
/// FSM state in which it is expected.
///
/// The enum has 12 variants: 4 defined by SPEC-06 (core protocol), 3
/// defined by SPEC-10 (registration/authentication), and 5 defined by
/// SPEC-19 §3.4 (delta protocol). SPEC-06 is the canonical owner of the
/// Message enum definition; the SPEC-19 additions ride the same bincode
/// v2 path established by SPEC-18.
///
/// IMPORTANT: New variants MUST be appended at the end of this enum to
/// preserve bincode discriminant stability (SPEC-06 R5, SPEC-19 R37).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    // === Coordinator -> Worker (SPEC-06 core) ===
    /// Sends a partition for the worker to reduce locally.
    /// Sent in the coordinator's Dispatching state (SPEC-13 R19).
    AssignPartition {
        /// Current round number (0-indexed).
        round: u32,
        /// The complete partition to be reduced.
        partition: Partition,
    },

    /// Signals the worker to terminate.
    /// Sent when the coordinator transitions to Done state (SPEC-13 R19).
    Shutdown,

    // === Worker -> Coordinator (SPEC-06 core) ===
    /// Returns the reduced partition to the coordinator.
    /// Sent after completing reduce_all + rebuild_free_port_index.
    PartitionResult {
        /// Round number (echo of the value received in AssignPartition).
        round: u32,
        /// The partition with the locally-reduced sub-net.
        partition: Partition,
        /// Local reduction statistics for this worker in this round.
        stats: WorkerRoundStats,
    },

    /// Reports an irrecoverable error in the worker.
    /// The coordinator MUST abort the grid loop upon receipt.
    Error {
        /// Round number in which the error occurred.
        round: u32,
        /// Identifier of the worker that reported the error.
        worker_id: WorkerId,
        /// Textual description of the error.
        description: String,
    },

    // === Registration (SPEC-10 Section 4.3) ===
    /// Worker registration request. First message on every new connection
    /// when authentication is enabled (Tier 2/3).
    /// Worker -> Coordinator.
    Register(RegisterPayload),

    /// Registration accepted. Coordinator -> Worker.
    RegisterAck(RegisterAckPayload),

    /// Registration rejected. Coordinator -> Worker.
    /// Connection MUST be closed after sending this.
    RegisterNack(RegisterNackPayload),

    // === SPEC-19 §3.4 — Delta protocol (discriminants 7..=11) ===
    /// Delivers the full stateful partition at round 0. Coordinator -> Worker.
    /// Sent exactly once per worker at grid start-up under delta mode
    /// (SPEC-19 R21.1); the worker's stateful lifecycle (R20-R30) begins
    /// here. `round` MUST be 0.
    ///
    /// Discriminant: 7 (SPEC-19 R37).
    InitialPartition {
        /// Round number. MUST be 0 (R21.1).
        round: u32,
        /// The worker's stateful partition, delivered once at round 0.
        partition: Partition,
    },

    /// Per-round kick-off. Coordinator -> Worker. Carries the coordinator's
    /// view of delta-applicable events the worker MUST fold into its
    /// stateful partition before running `reduce_all` for the round
    /// (SPEC-19 §3.3 R23).
    ///
    /// Discriminant: 8 (SPEC-19 R37).
    RoundStart {
        /// Round number (1-indexed for rounds ≥ 1).
        round: u32,
        /// Border deltas from previous-round border redex resolution
        /// (SPEC-19 R23; R17 DISCONNECTED semantics apply).
        border_deltas: Vec<BorderDelta>,
        /// Borders resolved (annihilated) last round and now removed from
        /// the worker's view (SPEC-19 R23).
        resolved_borders: Vec<u32>,
        /// Newly minted borders produced by CON-DUP dispatch last round
        /// (SPEC-19 R23); each `(border_id, PortRef)` gives the worker its
        /// side of the new border.
        new_borders: Vec<(u32, PortRef)>,
        /// Internal port reconnections the worker MUST apply before
        /// reducing the round (SPEC-19 R23, R33, DC-B3 — 2026-04-17).
        /// Each entry rewires `agent_id.port` to `new_target` locally;
        /// these are interior edits that cannot be expressed via
        /// `border_deltas` or `resolved_borders` / `new_borders`.
        local_reconnections: Vec<LocalReconnection>,
        /// Coordinator-issued AgentId allocation requests the worker MUST
        /// fulfil and echo back on the next `RoundResult.minted_agents`
        /// (SPEC-19 R23, R33, R48, DC-B5 — 2026-04-17). Correlation key
        /// is `PendingCommutation.request_id`.
        pending_commutations: Vec<PendingCommutation>,
    },

    /// Per-round reply. Worker -> Coordinator. Reports the worker's
    /// per-round delta and stats after `reduce_all` (SPEC-19 §3.3 R26).
    ///
    /// Discriminant: 9 (SPEC-19 R37).
    RoundResult {
        /// Round number (echo of the `RoundStart.round` this reply
        /// answers).
        round: u32,
        /// Border deltas the worker produced this round (SPEC-19 R25).
        border_deltas: Vec<BorderDelta>,
        /// Per-round stats including `stats.has_border_activity` (the
        /// canonical source of truth for convergence per DC-A2; the
        /// top-level `has_border_activity` is a wire cache that MUST
        /// agree with `stats.has_border_activity`).
        stats: WorkerRoundStats,
        /// Wire cache of `stats.has_border_activity` (SPEC-19 R26 literal,
        /// DC-A2 — 2026-04-17). Duplicated at the top level for
        /// coordinator pattern-match ergonomics; the canonical source of
        /// truth is `stats.has_border_activity`. Worker-side builder
        /// debug_assert enforces agreement at construction (invariant to
        /// be enabled in sub-bundle 2.26-C).
        has_border_activity: bool,
        /// Worker's echo of last round's `RoundStart.pending_commutations`.
        /// Each entry pairs the coordinator-issued `request_id` with the
        /// worker-allocated `AgentId` (SPEC-19 R26, R33, R48, DC-B5 —
        /// 2026-04-17). The coordinator correlates replies by
        /// `request_id`; an unmatched `request_id` is a protocol
        /// violation (R48).
        minted_agents: Vec<MintedAgent>,
    },

    /// Convergence signal. Coordinator -> Worker. Instructs the worker to
    /// reply with `FinalStateResult` carrying its final partition
    /// (SPEC-19 R29).
    ///
    /// Discriminant: 10 (SPEC-19 R37).
    FinalStateRequest {
        /// Round number at which the coordinator declared convergence.
        round: u32,
    },

    /// Final state reply. Worker -> Coordinator. Carries the worker's
    /// final partition after convergence (SPEC-19 R29).
    ///
    /// Discriminant: 11 (SPEC-19 R37).
    FinalStateResult {
        /// Round number at which the coordinator declared convergence.
        round: u32,
        /// The worker's final partition.
        partition: Partition,
    },

    // === SPEC-20 — Elastic grid protocol (discriminants 12..=16) ===
    /// Worker join request. Worker -> Coordinator.
    ///
    /// Discriminant: 12 (SPEC-20 R35).
    JoinRequest {
        /// Protocol version for fast rejection (MUST match `Register.protocol_version`).
        protocol_version: u8,
        /// Authentication token (MUST match `Register.auth_token`).
        auth_token: Option<[u8; 32]>,
        /// Worker's advertised processing capabilities (reserved for future use).
        capabilities: WorkerCapabilities,
    },

    /// Join request accepted. Coordinator -> Worker.
    ///
    /// Discriminant: 13 (SPEC-20 R35).
    JoinAck {
        /// The WorkerId assigned to the newly joined worker.
        worker_id: WorkerId,
        /// The partition index assigned for the next round.
        partition_index: u32,
        /// The round number at which the worker will join the grid.
        next_round_number: u32,
    },

    /// Worker-initiated departure request. Worker -> Coordinator.
    ///
    /// Discriminant: 14 (SPEC-20 R35).
    LeaveRequest {
        /// Severity/timing of the departure.
        kind: LeaveKind,
    },

    /// Departure request acknowledged. Coordinator -> Worker.
    /// The coordinator MUST send this before closing the TCP stream.
    ///
    /// Discriminant: 15 (SPEC-20 R35).
    LeaveAck,

    /// Join request rejected. Coordinator -> Worker.
    ///
    /// Discriminant: 16 (SPEC-20 R35).
    JoinNack {
        /// Structured reason for rejection.
        reason: JoinNackReason,
    },
}

/// Departure kind for `LeaveRequest` (SPEC-20 R21).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "zero-copy",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
pub enum LeaveKind {
    /// Graceful departure: worker wants to leave after current round finishes.
    AfterResult,
    /// Urgent departure: worker is leaving immediately (best-effort recovery).
    Urgent,
}

/// Worker processing capabilities (SPEC-20 R35). Placeholder for future use.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "zero-copy",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
pub struct WorkerCapabilities {
    // Reserved for SPEC-31 (intra-worker parallelism, rayon).
}

/// Reasons for rejecting a `JoinRequest` (SPEC-20 R35a).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "zero-copy",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
pub enum JoinNackReason {
    /// Protocol version mismatch (NF-009 shape alignment).
    ProtocolVersionMismatch {
        /// Version expected by the coordinator.
        expected: u8,
        /// Version provided by the worker.
        got: u8,
    },
    /// The grid is not configured to allow dynamic joins.
    ElasticJoinDisabled,
    /// No WorkerId slots remaining (counter reached u32::MAX).
    WorkerIdSpaceExhausted,
    /// Authentication token is missing or invalid.
    AuthenticationFailed,
}

/// Registration payload (SPEC-10 Section 4.3).
/// Direction: Worker -> Coordinator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterPayload {
    /// Protocol version for fast rejection of incompatible clients.
    /// Current version: 1.
    pub protocol_version: u8,
    /// Authentication token. None when running in Tier 1 (no auth).
    /// Some(raw_bytes) when running in Tier 2 or 3.
    pub auth_token: Option<[u8; 32]>,
}

/// Registration accepted payload (SPEC-10 Section 4.3).
/// Direction: Coordinator -> Worker (success).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterAckPayload {
    /// The WorkerId assigned to this worker by the coordinator.
    pub worker_id: WorkerId,
}

/// Registration rejected payload (SPEC-10 Section 4.3).
/// Direction: Coordinator -> Worker (failure).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterNackPayload {
    /// Human-readable reason for rejection.
    /// MUST be generic (e.g., "authentication failed") and MUST NOT
    /// reveal internal state (SPEC-10 R35).
    pub reason: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::{Net, PortRef, Symbol};
    use crate::partition::IdRange;
    use crate::protocol::bincode_v2;
    use std::collections::HashMap;

    fn make_test_partition() -> Partition {
        Partition {
            subnet: Net::new(),
            worker_id: 0,
            free_port_index: HashMap::new(),
            id_range: IdRange {
                start: 0,
                end: 100_000,
            },
            border_id_start: 0,
            border_id_end: 0,
        }
    }

    fn make_test_stats() -> WorkerRoundStats {
        WorkerRoundStats {
            worker_id: 0,
            agents_before: 10,
            agents_after: 5,
            local_redexes: 5,
            reduce_duration_secs: 0.001,
            interactions_by_rule: [1, 1, 1, 1, 1, 0],
            has_border_activity: false,
            is_coordinator_self: false,
        }
    }

    /// TASK-0369 helper — `make_test_stats` with explicit activity flag so
    /// `Message::RoundResult { has_border_activity, stats }` pairs can be
    /// constructed in-agreement or adversarially.
    fn make_test_stats_with_activity(activity: bool) -> WorkerRoundStats {
        WorkerRoundStats {
            worker_id: 0,
            agents_before: 10,
            agents_after: 5,
            local_redexes: 5,
            reduce_duration_secs: 0.001,
            interactions_by_rule: [1, 1, 1, 1, 1, 0],
            has_border_activity: activity,
            is_coordinator_self: false,
        }
    }

    /// TASK-0367 T3 helper — partition with `n` CON agents for
    /// size-monotone tests.
    fn make_partition_with_n_agents(n: usize) -> Partition {
        let mut subnet = Net::new();
        for _ in 0..n {
            subnet.create_agent(Symbol::Con);
        }
        Partition {
            subnet,
            worker_id: 0,
            free_port_index: HashMap::new(),
            id_range: IdRange {
                start: 0,
                end: 100_000,
            },
            border_id_start: 0,
            border_id_end: 0,
        }
    }

    // T1: Each variant can be created and serialized with bincode
    #[test]
    fn test_all_variants_serde_roundtrip() {
        let variants: Vec<Message> = vec![
            Message::AssignPartition {
                round: 0,
                partition: make_test_partition(),
            },
            Message::Shutdown,
            Message::PartitionResult {
                round: 1,
                partition: make_test_partition(),
                stats: make_test_stats(),
            },
            Message::Error {
                round: 2,
                worker_id: 3,
                description: "test".into(),
            },
            Message::Register(RegisterPayload {
                protocol_version: 1,
                auth_token: None,
            }),
            Message::RegisterAck(RegisterAckPayload { worker_id: 0 }),
            Message::RegisterNack(RegisterNackPayload {
                reason: "no".into(),
            }),
            // SPEC-19 §3.4 additions (discriminants 7..=11)
            Message::InitialPartition {
                round: 0,
                partition: make_test_partition(),
            },
            Message::RoundStart {
                round: 0,
                border_deltas: Vec::new(),
                resolved_borders: Vec::new(),
                new_borders: Vec::new(),
                local_reconnections: Vec::new(),
                pending_commutations: Vec::new(),
            },
            Message::RoundResult {
                round: 0,
                border_deltas: Vec::new(),
                stats: make_test_stats(),
                has_border_activity: false,
                minted_agents: Vec::new(),
            },
            Message::FinalStateRequest { round: 0 },
            Message::FinalStateResult {
                round: 0,
                partition: make_test_partition(),
            },
            Message::JoinRequest {
                protocol_version: 4,
                auth_token: None,
                capabilities: WorkerCapabilities::default(),
            },
            Message::JoinAck {
                worker_id: 1,
                partition_index: 0,
                next_round_number: 1,
            },
            Message::LeaveRequest {
                kind: LeaveKind::AfterResult,
            },
            Message::LeaveAck,
            Message::JoinNack {
                reason: JoinNackReason::ElasticJoinDisabled,
            },
        ];

        for msg in &variants {
            let bytes = bincode_v2::encode(msg).unwrap();
            let restored: Message = bincode_v2::decode_value(&bytes).unwrap();
            // Can't use PartialEq (Net doesn't derive it for this use),
            // but assert it doesn't panic.
            let _ = format!("{:?}", restored);
        }
    }

    // T2: Shutdown variant is small (just an enum discriminant)
    #[test]
    fn test_shutdown_size() {
        let bytes = bincode_v2::encode(&Message::Shutdown).unwrap();
        // bincode v2 (varint) encodes the Shutdown discriminant in 1 byte.
        // Allow up to 8 to remain robust against discriminant reordering.
        assert!(bytes.len() <= 8);
    }

    // T3: AssignPartition fields
    #[test]
    fn test_assign_partition_fields() {
        let msg = Message::AssignPartition {
            round: 42,
            partition: make_test_partition(),
        };
        match msg {
            Message::AssignPartition { round, partition } => {
                assert_eq!(round, 42);
                assert_eq!(partition.worker_id, 0);
            }
            _ => panic!("wrong variant"),
        }
    }

    // T4: PartitionResult fields
    #[test]
    fn test_partition_result_fields() {
        let msg = Message::PartitionResult {
            round: 7,
            partition: make_test_partition(),
            stats: make_test_stats(),
        };
        match msg {
            Message::PartitionResult {
                round,
                partition,
                stats,
            } => {
                assert_eq!(round, 7);
                assert_eq!(partition.worker_id, 0);
                assert_eq!(stats.agents_before, 10);
            }
            _ => panic!("wrong variant"),
        }
    }

    // T5: Error fields
    #[test]
    fn test_error_fields() {
        let msg = Message::Error {
            round: 3,
            worker_id: 1,
            description: "oops".into(),
        };
        match msg {
            Message::Error {
                round,
                worker_id,
                description,
            } => {
                assert_eq!(round, 3);
                assert_eq!(worker_id, 1);
                assert_eq!(description, "oops");
            }
            _ => panic!("wrong variant"),
        }
    }

    // T6: Register with auth token
    #[test]
    fn test_register_with_token() {
        let token = [0xABu8; 32];
        let msg = Message::Register(RegisterPayload {
            protocol_version: 1,
            auth_token: Some(token),
        });
        let bytes = bincode_v2::encode(&msg).unwrap();
        let restored: Message = bincode_v2::decode_value(&bytes).unwrap();
        match restored {
            Message::Register(payload) => {
                assert_eq!(payload.protocol_version, 1);
                assert_eq!(payload.auth_token, Some(token));
            }
            _ => panic!("wrong variant"),
        }
    }

    // T7: Register without auth token (Tier 1)
    #[test]
    fn test_register_no_token() {
        let msg = Message::Register(RegisterPayload {
            protocol_version: 1,
            auth_token: None,
        });
        let bytes = bincode_v2::encode(&msg).unwrap();
        let restored: Message = bincode_v2::decode_value(&bytes).unwrap();
        match restored {
            Message::Register(payload) => {
                assert_eq!(payload.auth_token, None);
            }
            _ => panic!("wrong variant"),
        }
    }

    // T8: RegisterAck preserves worker_id
    #[test]
    fn test_register_ack_serde() {
        let msg = Message::RegisterAck(RegisterAckPayload { worker_id: 99 });
        let bytes = bincode_v2::encode(&msg).unwrap();
        let restored: Message = bincode_v2::decode_value(&bytes).unwrap();
        match restored {
            Message::RegisterAck(payload) => assert_eq!(payload.worker_id, 99),
            _ => panic!("wrong variant"),
        }
    }

    // T9: RegisterNack preserves reason
    #[test]
    fn test_register_nack_serde() {
        let msg = Message::RegisterNack(RegisterNackPayload {
            reason: "bad token".into(),
        });
        let bytes = bincode_v2::encode(&msg).unwrap();
        let restored: Message = bincode_v2::decode_value(&bytes).unwrap();
        match restored {
            Message::RegisterNack(payload) => assert_eq!(payload.reason, "bad token"),
            _ => panic!("wrong variant"),
        }
    }

    // T10: Message with real agents in partition
    #[test]
    fn test_message_with_agents() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));

        let partition = Partition {
            subnet: net,
            worker_id: 1,
            free_port_index: HashMap::new(),
            id_range: IdRange {
                start: 0,
                end: 1000,
            },
            border_id_start: 0,
            border_id_end: 0,
        };

        let msg = Message::AssignPartition {
            round: 0,
            partition,
        };
        let bytes = bincode_v2::encode(&msg).unwrap();
        let restored: Message = bincode_v2::decode_value(&bytes).unwrap();
        match restored {
            Message::AssignPartition { partition, .. } => {
                assert_eq!(partition.subnet.count_live_agents(), 1);
                assert_eq!(partition.worker_id, 1);
            }
            _ => panic!("wrong variant"),
        }
    }

    // -----------------------------------------------------------------
    // SPEC-19 §3.4 — delta protocol variant tests (TASK-0367..0371)
    // -----------------------------------------------------------------

    // TASK-0367 T1 — InitialPartition (disc 7) round-trip identity.
    #[test]
    fn test_initial_partition_bincode_roundtrip_identity() {
        let original = Message::InitialPartition {
            round: 0,
            partition: make_test_partition(),
        };
        let bytes = bincode_v2::encode(&original).expect("encode");
        let decoded: Message = bincode_v2::decode_value(&bytes).expect("decode");
        match decoded {
            Message::InitialPartition { round, partition } => {
                assert_eq!(round, 0, "R21.1: InitialPartition round MUST be 0");
                assert_eq!(partition.worker_id, make_test_partition().worker_id);
                let expected_dbg = format!("{:?}", make_test_partition());
                let actual_dbg = format!("{:?}", partition);
                assert_eq!(
                    actual_dbg, expected_dbg,
                    "full partition Debug-format must match"
                );
            }
            other => panic!("expected InitialPartition, got {:?}", other),
        }
    }

    // TASK-0367 T2 — FinalStateResult (disc 11) preserves worker_id.
    #[test]
    fn test_final_state_result_bincode_roundtrip_preserves_worker_id() {
        let mut partition = make_test_partition();
        partition.worker_id = 7;
        let original = Message::FinalStateResult {
            round: 42,
            partition,
        };
        let bytes = bincode_v2::encode(&original).expect("encode");
        let decoded: Message = bincode_v2::decode_value(&bytes).expect("decode");
        match decoded {
            Message::FinalStateResult { round, partition } => {
                assert_eq!(round, 42);
                assert_eq!(
                    partition.worker_id, 7,
                    "FinalStateResult must preserve partition.worker_id"
                );
            }
            other => panic!("expected FinalStateResult, got {:?}", other),
        }
    }

    // TASK-0367 T3 — InitialPartition encoded size scales with agent count.
    #[test]
    fn test_initial_partition_bincode_size_monotone_in_agent_count() {
        let small = Message::InitialPartition {
            round: 0,
            partition: make_partition_with_n_agents(10),
        };
        let large = Message::InitialPartition {
            round: 0,
            partition: make_partition_with_n_agents(20),
        };
        let small_bytes = bincode_v2::encode(&small).expect("encode small");
        let large_bytes = bincode_v2::encode(&large).expect("encode large");
        assert!(
            small_bytes.len() < large_bytes.len(),
            "bincode must scale with agent count (small={} bytes, large={} bytes)",
            small_bytes.len(),
            large_bytes.len(),
        );
    }

    // TASK-0367 T4 — FinalStateResult preserves a non-trivial partition.
    #[test]
    fn test_final_state_result_preserves_non_trivial_partition() {
        let original_partition = make_partition_with_n_agents(4);
        let original_agent_count = original_partition.subnet.count_live_agents();
        let msg = Message::FinalStateResult {
            round: 99,
            partition: original_partition,
        };
        let bytes = bincode_v2::encode(&msg).expect("encode");
        let decoded: Message = bincode_v2::decode_value(&bytes).expect("decode");
        match decoded {
            Message::FinalStateResult { round, partition } => {
                assert_eq!(round, 99);
                assert_eq!(
                    partition.subnet.count_live_agents(),
                    original_agent_count,
                    "agent count must be preserved through round-trip",
                );
            }
            other => panic!("expected FinalStateResult, got {:?}", other),
        }
    }

    // TASK-0368 T1 — RoundStart (disc 8) with all 5 vecs populated.
    #[test]
    fn test_round_start_bincode_roundtrip_populated_vecs() {
        let original = Message::RoundStart {
            round: 5,
            border_deltas: vec![
                BorderDelta {
                    border_id: 5,
                    new_target: PortRef::AgentPort(42, 0),
                },
                BorderDelta {
                    border_id: 7,
                    new_target: PortRef::FreePort(9),
                },
            ],
            resolved_borders: vec![1, 3, 8],
            new_borders: vec![(20, PortRef::FreePort(20)), (21, PortRef::FreePort(21))],
            local_reconnections: vec![LocalReconnection {
                agent_id: 7,
                port: 2,
                new_target: PortRef::AgentPort(11, 1),
            }],
            pending_commutations: vec![PendingCommutation {
                request_id: 42,
                target_symbols: vec![Symbol::Dup],
                local_wiring: Vec::new(),
            }],
        };
        let bytes = bincode_v2::encode(&original).expect("encode");
        let decoded: Message = bincode_v2::decode_value(&bytes).expect("decode");
        match decoded {
            Message::RoundStart {
                round,
                border_deltas,
                resolved_borders,
                new_borders,
                local_reconnections,
                pending_commutations,
            } => {
                assert_eq!(round, 5);
                assert_eq!(border_deltas.len(), 2);
                assert_eq!(border_deltas[0].border_id, 5);
                assert_eq!(border_deltas[0].new_target, PortRef::AgentPort(42, 0));
                assert_eq!(border_deltas[1].border_id, 7);
                assert_eq!(border_deltas[1].new_target, PortRef::FreePort(9));
                assert_eq!(resolved_borders, vec![1, 3, 8]);
                assert_eq!(new_borders.len(), 2);
                assert_eq!(new_borders[0], (20, PortRef::FreePort(20)));
                assert_eq!(new_borders[1], (21, PortRef::FreePort(21)));
                assert_eq!(local_reconnections.len(), 1);
                assert_eq!(local_reconnections[0].agent_id, 7);
                assert_eq!(local_reconnections[0].port, 2);
                assert_eq!(local_reconnections[0].new_target, PortRef::AgentPort(11, 1),);
                assert_eq!(pending_commutations.len(), 1);
                assert_eq!(pending_commutations[0].request_id, 42);
                assert_eq!(pending_commutations[0].target_symbols, vec![Symbol::Dup]);
                assert!(pending_commutations[0].local_wiring.is_empty());
            }
            other => panic!("expected RoundStart, got {:?}", other),
        }
    }

    // TASK-0368 T2 — RoundStart with all 5 vecs empty (idle-worker case).
    #[test]
    fn test_round_start_bincode_roundtrip_empty_vecs() {
        let original = Message::RoundStart {
            round: 0,
            border_deltas: Vec::new(),
            resolved_borders: Vec::new(),
            new_borders: Vec::new(),
            local_reconnections: Vec::new(),
            pending_commutations: Vec::new(),
        };
        let bytes = bincode_v2::encode(&original).expect("encode");
        let decoded: Message = bincode_v2::decode_value(&bytes).expect("decode");
        match decoded {
            Message::RoundStart {
                round,
                border_deltas,
                resolved_borders,
                new_borders,
                local_reconnections,
                pending_commutations,
            } => {
                assert_eq!(round, 0);
                assert_eq!(border_deltas.len(), 0);
                assert_eq!(resolved_borders.len(), 0);
                assert_eq!(new_borders.len(), 0);
                assert_eq!(local_reconnections.len(), 0);
                assert_eq!(pending_commutations.len(), 0);
            }
            other => panic!("expected RoundStart, got {:?}", other),
        }
    }

    // TASK-0368 T3 — FinalStateRequest (disc 10) minimal round-trip.
    #[test]
    fn test_final_state_request_bincode_roundtrip_minimal() {
        let original = Message::FinalStateRequest { round: 42 };
        let bytes = bincode_v2::encode(&original).expect("encode");
        let decoded: Message = bincode_v2::decode_value(&bytes).expect("decode");
        match decoded {
            Message::FinalStateRequest { round } => {
                assert_eq!(round, 42);
            }
            other => panic!("expected FinalStateRequest, got {:?}", other),
        }
        // 1 byte disc + 1..=5 bytes varint for round = 2..=6 bytes total.
        assert!(
            bytes.len() <= 6,
            "FinalStateRequest must encode in <= 6 bytes; got {}",
            bytes.len(),
        );
    }

    // TASK-0368 T5 (DC-B3) — RoundStart local_reconnections round-trip.
    #[test]
    fn test_round_start_local_reconnections_populated_only() {
        let original = Message::RoundStart {
            round: 1,
            border_deltas: Vec::new(),
            resolved_borders: Vec::new(),
            new_borders: Vec::new(),
            local_reconnections: vec![
                LocalReconnection {
                    agent_id: 5,
                    port: 0,
                    new_target: PortRef::AgentPort(6, 1),
                },
                LocalReconnection {
                    agent_id: 6,
                    port: 1,
                    new_target: PortRef::FreePort(42),
                },
            ],
            pending_commutations: Vec::new(),
        };
        let bytes = bincode_v2::encode(&original).expect("encode");
        let decoded: Message = bincode_v2::decode_value(&bytes).expect("decode");
        match decoded {
            Message::RoundStart {
                round,
                local_reconnections,
                ..
            } => {
                assert_eq!(round, 1);
                assert_eq!(local_reconnections.len(), 2);
                assert_eq!(local_reconnections[0].agent_id, 5);
                assert_eq!(local_reconnections[0].port, 0);
                assert_eq!(local_reconnections[0].new_target, PortRef::AgentPort(6, 1),);
                assert_eq!(local_reconnections[1].agent_id, 6);
                assert_eq!(local_reconnections[1].port, 1);
                assert_eq!(local_reconnections[1].new_target, PortRef::FreePort(42),);
            }
            other => panic!("expected RoundStart, got {:?}", other),
        }
    }

    // TASK-0368 T6 (DC-B5 + R48) — RoundStart pending_commutations order
    // preservation.
    #[test]
    fn test_round_start_pending_commutations_multi_request_id_order_preserved() {
        let original = Message::RoundStart {
            round: 2,
            border_deltas: Vec::new(),
            resolved_borders: Vec::new(),
            new_borders: Vec::new(),
            local_reconnections: Vec::new(),
            pending_commutations: vec![
                PendingCommutation {
                    request_id: 100,
                    target_symbols: vec![Symbol::Con],
                    local_wiring: Vec::new(),
                },
                PendingCommutation {
                    request_id: 101,
                    target_symbols: vec![Symbol::Era],
                    local_wiring: Vec::new(),
                },
            ],
        };
        let bytes = bincode_v2::encode(&original).expect("encode");
        let decoded: Message = bincode_v2::decode_value(&bytes).expect("decode");
        match decoded {
            Message::RoundStart {
                round,
                pending_commutations,
                ..
            } => {
                assert_eq!(round, 2);
                assert_eq!(pending_commutations.len(), 2);
                assert_eq!(pending_commutations[0].request_id, 100);
                assert_eq!(pending_commutations[0].target_symbols, vec![Symbol::Con]);
                assert!(pending_commutations[0].local_wiring.is_empty());
                assert_eq!(pending_commutations[1].request_id, 101);
                assert_eq!(pending_commutations[1].target_symbols, vec![Symbol::Era]);
                assert!(pending_commutations[1].local_wiring.is_empty());
            }
            other => panic!("expected RoundStart, got {:?}", other),
        }
    }

    // TASK-0369 T1 — RoundResult (disc 9) populated round-trip.
    #[test]
    fn test_round_result_bincode_roundtrip_populated() {
        let stats = make_test_stats_with_activity(true);
        let original = Message::RoundResult {
            round: 3,
            border_deltas: vec![
                BorderDelta {
                    border_id: 5,
                    new_target: PortRef::AgentPort(42, 0),
                },
                BorderDelta {
                    border_id: 7,
                    new_target: PortRef::FreePort(9),
                },
            ],
            stats,
            has_border_activity: true,
            minted_agents: vec![MintedAgent {
                request_id: 42,
                minted_agent_id: 103,
            }],
        };
        let bytes = bincode_v2::encode(&original).expect("encode");
        let decoded: Message = bincode_v2::decode_value(&bytes).expect("decode");
        match decoded {
            Message::RoundResult {
                round,
                border_deltas,
                stats,
                has_border_activity,
                minted_agents,
            } => {
                assert_eq!(round, 3);
                assert_eq!(border_deltas.len(), 2);
                assert_eq!(border_deltas[0].border_id, 5);
                assert!(has_border_activity);
                assert!(stats.has_border_activity);
                assert_eq!(minted_agents.len(), 1);
                assert_eq!(minted_agents[0].request_id, 42);
                assert_eq!(minted_agents[0].minted_agent_id, 103);
            }
            other => panic!("expected RoundResult, got {:?}", other),
        }
    }

    // TASK-0369 T2 — RoundResult converged case (empty deltas, flag false).
    #[test]
    fn test_round_result_bincode_roundtrip_empty_deltas_converged_case() {
        let stats = make_test_stats_with_activity(false);
        let original = Message::RoundResult {
            round: 10,
            border_deltas: Vec::new(),
            stats,
            has_border_activity: false,
            minted_agents: Vec::new(),
        };
        let bytes = bincode_v2::encode(&original).expect("encode");
        let decoded: Message = bincode_v2::decode_value(&bytes).expect("decode");
        match decoded {
            Message::RoundResult {
                round,
                border_deltas,
                stats,
                has_border_activity,
                minted_agents,
            } => {
                assert_eq!(round, 10);
                assert_eq!(border_deltas.len(), 0);
                assert!(!has_border_activity);
                assert!(!stats.has_border_activity);
                assert_eq!(minted_agents.len(), 0);
            }
            other => panic!("expected RoundResult, got {:?}", other),
        }
    }

    // TASK-0369 T3 — `WorkerRoundStats.interactions_by_rule: [u64; 6]`
    // fixed-size array preserved.
    #[test]
    fn test_round_result_preserves_interactions_by_rule_array() {
        let mut stats = make_test_stats();
        stats.interactions_by_rule = [0, 1, 2, 3, 4, 5];
        stats.has_border_activity = false;
        let original = Message::RoundResult {
            round: 0,
            border_deltas: Vec::new(),
            stats,
            has_border_activity: false,
            minted_agents: Vec::new(),
        };
        let bytes = bincode_v2::encode(&original).expect("encode");
        let decoded: Message = bincode_v2::decode_value(&bytes).expect("decode");
        match decoded {
            Message::RoundResult { stats, .. } => {
                assert_eq!(
                    stats.interactions_by_rule,
                    [0, 1, 2, 3, 4, 5],
                    "interactions_by_rule array must survive intact",
                );
            }
            other => panic!("expected RoundResult, got {:?}", other),
        }
    }

    // TASK-0369 T4 — top-level `has_border_activity` actually serialised
    // (not optimised away).
    #[test]
    fn test_round_result_activity_flag_independence_from_stats() {
        let stats = make_test_stats_with_activity(false);
        let msg_true = Message::RoundResult {
            round: 0,
            border_deltas: Vec::new(),
            stats: stats.clone(),
            has_border_activity: true,
            minted_agents: Vec::new(),
        };
        let msg_false = Message::RoundResult {
            round: 0,
            border_deltas: Vec::new(),
            stats,
            has_border_activity: false,
            minted_agents: Vec::new(),
        };
        let bytes_true = bincode_v2::encode(&msg_true).expect("encode true");
        let bytes_false = bincode_v2::encode(&msg_false).expect("encode false");
        assert_ne!(
            bytes_true, bytes_false,
            "top-level has_border_activity MUST be serialised — \
             different values MUST produce different byte streams",
        );
    }

    // TASK-0369 T5 (DC-A2) — serde preserves agreement on well-formed pairs.
    #[test]
    fn test_round_result_activity_matches_stats_activity() {
        for top_level in [true, false] {
            let stats = make_test_stats_with_activity(top_level);
            let original = Message::RoundResult {
                round: 0,
                border_deltas: Vec::new(),
                stats,
                has_border_activity: top_level,
                minted_agents: Vec::new(),
            };
            let bytes = bincode_v2::encode(&original).expect("encode");
            let decoded: Message = bincode_v2::decode_value(&bytes).expect("decode");
            match decoded {
                Message::RoundResult {
                    has_border_activity: msg_flag,
                    stats: decoded_stats,
                    ..
                } => {
                    assert_eq!(
                        msg_flag, decoded_stats.has_border_activity,
                        "DC-A2: after bincode round-trip, top-level \
                         has_border_activity MUST equal stats.has_border_activity \
                         (top_level input was {})",
                        top_level,
                    );
                }
                other => panic!("expected RoundResult, got {:?}", other),
            }
        }
    }

    // TASK-0369 T6 (DC-A2, `#[ignore]` stub) — enabled in 2.26-C when the
    // worker-side `RoundResult` builder lands. See TEST-SPEC-0369 T6 for
    // the expected invariant. Body kept as `panic!` so that a future
    // `#[ignore]` removal without wiring the builder fails loudly.
    #[test]
    #[ignore = "TODO(2.26-C): enable once worker_emit_round_result builder lands"]
    #[should_panic(expected = "RoundResult invariant")]
    fn test_round_result_activity_invariant_runtime() {
        panic!("stub — enable in 2.26-C");
    }

    // TASK-0369 T8 (DC-B5 + R48) — minted_agents multi-entry order preserved.
    #[test]
    fn test_round_result_minted_agents_multi_order_preserved() {
        let stats = make_test_stats_with_activity(false);
        let original = Message::RoundResult {
            round: 4,
            border_deltas: Vec::new(),
            stats,
            has_border_activity: false,
            minted_agents: vec![
                MintedAgent {
                    request_id: 100,
                    minted_agent_id: 1001,
                },
                MintedAgent {
                    request_id: 101,
                    minted_agent_id: 1002,
                },
                MintedAgent {
                    request_id: 102,
                    minted_agent_id: 1003,
                },
            ],
        };
        let bytes = bincode_v2::encode(&original).expect("encode");
        let decoded: Message = bincode_v2::decode_value(&bytes).expect("decode");
        match decoded {
            Message::RoundResult { minted_agents, .. } => {
                assert_eq!(minted_agents.len(), 3);
                assert_eq!(minted_agents[0].request_id, 100);
                assert_eq!(minted_agents[0].minted_agent_id, 1001);
                assert_eq!(minted_agents[1].request_id, 101);
                assert_eq!(minted_agents[1].minted_agent_id, 1002);
                assert_eq!(minted_agents[2].request_id, 102);
                assert_eq!(minted_agents[2].minted_agent_id, 1003);
            }
            other => panic!("expected RoundResult, got {:?}", other),
        }
    }

    // TASK-0371 — byte-level discriminant stability for all 12 variants.
    //
    // SPEC-06 R5 + SPEC-18 R3 + SPEC-19 R37: variants MUST be append-only
    // with stable discriminants. Under bincode v2 varint encoding,
    // discriminants 0..=250 encode as exactly one byte equal to the
    // discriminant value. This test is the authoritative regression guard
    // for the entire enum.
    //
    // R37 coordination note (spec-critic Bonus-2 2026-04-17): SPEC-18
    // (wire format v2) appended NO Message variants; discriminants 7..=11
    // are therefore the FIRST post-SPEC-06 assignments under R37's
    // deferred-to-coordinated-numbering rule — a no-op coordination.
    //
    // TODO: when a new Message variant is appended, add its
    // (expected_disc, fixture) entry here AND bump the cardinality
    // assertion at the tail. Keep variants append-only per SPEC-06 R5 /
    // SPEC-19 R37.
    #[test]
    fn test_message_discriminant_stability() {
        let cases: Vec<(u8, Message)> = vec![
            (
                0,
                Message::AssignPartition {
                    round: 0,
                    partition: make_test_partition(),
                },
            ),
            (1, Message::Shutdown),
            (
                2,
                Message::PartitionResult {
                    round: 0,
                    partition: make_test_partition(),
                    stats: make_test_stats(),
                },
            ),
            (
                3,
                Message::Error {
                    round: 0,
                    worker_id: 0,
                    description: String::new(),
                },
            ),
            (
                4,
                Message::Register(RegisterPayload {
                    protocol_version: 1,
                    auth_token: None,
                }),
            ),
            (5, Message::RegisterAck(RegisterAckPayload { worker_id: 0 })),
            (
                6,
                Message::RegisterNack(RegisterNackPayload {
                    reason: String::new(),
                }),
            ),
            (
                7,
                Message::InitialPartition {
                    round: 0,
                    partition: make_test_partition(),
                },
            ),
            (
                8,
                Message::RoundStart {
                    round: 0,
                    border_deltas: Vec::new(),
                    resolved_borders: Vec::new(),
                    new_borders: Vec::new(),
                    local_reconnections: Vec::new(),
                    pending_commutations: Vec::new(),
                },
            ),
            (
                9,
                Message::RoundResult {
                    round: 0,
                    border_deltas: Vec::new(),
                    stats: make_test_stats(),
                    has_border_activity: false,
                    minted_agents: Vec::new(),
                },
            ),
            (10, Message::FinalStateRequest { round: 0 }),
            (
                11,
                Message::FinalStateResult {
                    round: 0,
                    partition: make_test_partition(),
                },
            ),
            (
                12,
                Message::JoinRequest {
                    protocol_version: 4,
                    auth_token: None,
                    capabilities: WorkerCapabilities::default(),
                },
            ),
            (
                13,
                Message::JoinAck {
                    worker_id: 1,
                    partition_index: 0,
                    next_round_number: 1,
                },
            ),
            (
                14,
                Message::LeaveRequest {
                    kind: LeaveKind::AfterResult,
                },
            ),
            (15, Message::LeaveAck),
            (
                16,
                Message::JoinNack {
                    reason: JoinNackReason::ElasticJoinDisabled,
                },
            ),
        ];

        for (expected_disc, msg) in &cases {
            let bytes = bincode_v2::encode(msg).expect("encode must succeed");
            assert!(
                !bytes.is_empty(),
                "encoded bytes must be non-empty for {:?}",
                msg,
            );
            assert_eq!(
                bytes[0], *expected_disc,
                "variant {:?} expected discriminant {} but got {}",
                msg, expected_disc, bytes[0],
            );
        }

        // Cardinality contract: `cases` MUST cover every current variant.
        // Extend this assertion when the enum grows.
        assert_eq!(
            cases.len(),
            17,
            "R37: the discriminant-stability test MUST cover all current \
             Message variants; update `cases` when a variant is appended",
        );
    }
}
