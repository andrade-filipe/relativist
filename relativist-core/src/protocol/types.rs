//! Protocol message types (SPEC-06, Section 4.1).
//!
//! Defines the Message enum with 7 variants covering all coordinator-worker
//! communication, including registration handshake (SPEC-10).

use serde::{Deserialize, Serialize};

use crate::merge::WorkerRoundStats;
use crate::partition::{Partition, WorkerId};

/// Messages in the communication protocol between coordinator and workers.
///
/// The enum covers all possible communication. Each variant is annotated
/// with the direction (Coordinator->Worker or Worker->Coordinator) and the
/// FSM state in which it is expected.
///
/// The enum has 7 variants: 4 defined by SPEC-06 (core protocol) and 3
/// defined by SPEC-10 (registration/authentication). SPEC-06 is the canonical
/// owner of the Message enum definition.
///
/// IMPORTANT: New variants MUST be appended at the end of this enum to
/// preserve bincode discriminant stability (R5).
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
}
