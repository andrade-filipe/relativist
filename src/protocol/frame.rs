//! Frame format and framing functions (SPEC-06, Sections 4.2-4.3).
//!
//! Each message is framed with an 8-byte header:
//! [4 bytes length (LE u32)] [4 bytes CRC32 (LE u32)] [payload bytes...]
//!
//! The framing functions (`send_frame`, `recv_frame`) are generic over
//! `AsyncWriteExt` / `AsyncReadExt` to support both TCP and in-memory testing.

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::error::ProtocolError;
use super::types::Message;

/// Header size in bytes: 4 (length) + 4 (CRC32 checksum).
pub const FRAME_HEADER_SIZE: usize = 8;

/// Default maximum payload size (256 MiB).
pub const DEFAULT_MAX_PAYLOAD_SIZE: u32 = 268_435_456;

/// Header of a frame in the wire protocol.
/// Precedes each payload transmitted over TCP.
///
/// Does NOT derive Serialize/Deserialize — it is manually encoded
/// as raw bytes for efficiency and control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameHeader {
    /// Length of the payload in bytes (excluding the header itself).
    pub length: u32,
    /// CRC32C checksum of the payload.
    pub checksum: u32,
}

impl FrameHeader {
    /// Serializes the header to 8 bytes (little-endian).
    pub fn to_bytes(&self) -> [u8; FRAME_HEADER_SIZE] {
        let mut buf = [0u8; FRAME_HEADER_SIZE];
        buf[0..4].copy_from_slice(&self.length.to_le_bytes());
        buf[4..8].copy_from_slice(&self.checksum.to_le_bytes());
        buf
    }

    /// Deserializes the header from 8 bytes (little-endian).
    pub fn from_bytes(bytes: [u8; FRAME_HEADER_SIZE]) -> Self {
        let length = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let checksum = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        Self { length, checksum }
    }
}

/// Serializes a message and sends it as a frame over the TCP socket.
///
/// Returns the total number of bytes written (header + payload).
///
/// Steps:
/// 1. Serialize Message with bincode -> payload bytes.
/// 2. Compute CRC32C of the payload.
/// 3. Write header (length + checksum) as 8 bytes little-endian.
/// 4. Write payload.
/// 5. Flush the buffer.
pub async fn send_frame<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    message: &Message,
) -> Result<usize, ProtocolError> {
    // 1. Serialize
    let payload = bincode::serialize(message).map_err(ProtocolError::Serialize)?;

    // 2. CRC32C checksum
    let checksum = crc32fast::hash(&payload);

    // 3. Write header
    let header = FrameHeader {
        length: payload.len() as u32,
        checksum,
    };
    writer
        .write_all(&header.to_bytes())
        .await
        .map_err(ProtocolError::ConnectionLost)?;

    // 4. Write payload
    writer
        .write_all(&payload)
        .await
        .map_err(ProtocolError::ConnectionLost)?;

    // 5. Flush
    writer
        .flush()
        .await
        .map_err(ProtocolError::ConnectionLost)?;

    Ok(FRAME_HEADER_SIZE + payload.len())
}

/// Reads a frame from the TCP socket and deserializes the message.
///
/// Returns the deserialized message and the total number of bytes read.
///
/// Steps:
/// 1. Read exactly 8 header bytes.
/// 2. Extract length and checksum from the header.
/// 3. Reject if length > max_payload_size (defense against OOM).
/// 4. Read exactly `length` bytes of payload.
/// 5. Verify CRC32C of payload against header checksum.
/// 6. Deserialize payload with bincode -> Message.
pub async fn recv_frame<R: AsyncReadExt + Unpin>(
    reader: &mut R,
    max_payload_size: u32,
) -> Result<(Message, usize), ProtocolError> {
    // 1. Read header
    let mut header_buf = [0u8; FRAME_HEADER_SIZE];
    reader
        .read_exact(&mut header_buf)
        .await
        .map_err(ProtocolError::ConnectionLost)?;

    // 2. Extract fields
    let header = FrameHeader::from_bytes(header_buf);

    // 3. Validate size BEFORE allocating (R9 — defense against OOM)
    if header.length > max_payload_size {
        return Err(ProtocolError::PayloadTooLarge {
            size: header.length,
            max: max_payload_size,
        });
    }

    // 4. Read payload
    let mut payload = vec![0u8; header.length as usize];
    reader
        .read_exact(&mut payload)
        .await
        .map_err(ProtocolError::ConnectionLost)?;

    // 5. Verify checksum (R29)
    let computed_crc = crc32fast::hash(&payload);
    if computed_crc != header.checksum {
        return Err(ProtocolError::ChecksumMismatch {
            expected: header.checksum,
            computed: computed_crc,
        });
    }

    // 6. Deserialize
    let message: Message = bincode::deserialize(&payload).map_err(ProtocolError::Deserialize)?;

    let total_bytes = FRAME_HEADER_SIZE + header.length as usize;
    Ok((message, total_bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merge::WorkerRoundStats;
    use crate::net::{Net, PortRef, Symbol};
    use crate::partition::{IdRange, Partition};
    use std::collections::HashMap;
    use tokio::io::duplex;

    /// Creates an in-memory bidirectional channel for testing.
    fn create_test_channel() -> (tokio::io::DuplexStream, tokio::io::DuplexStream) {
        duplex(1_048_576) // 1 MiB buffer
    }

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
        }
    }

    // --- FrameHeader tests ---

    // T1: round-trip from_bytes(to_bytes()) preserves values
    #[test]
    fn test_frame_header_roundtrip() {
        let header = FrameHeader {
            length: 1234,
            checksum: 0xDEADBEEF,
        };
        let restored = FrameHeader::from_bytes(header.to_bytes());
        assert_eq!(header, restored);
    }

    // T2: to_bytes produces correct little-endian byte order
    #[test]
    fn test_frame_header_byte_order() {
        let header = FrameHeader {
            length: 0x04030201,
            checksum: 0x08070605,
        };
        let bytes = header.to_bytes();
        assert_eq!(bytes, [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
    }

    // T3: constants have expected values
    #[test]
    fn test_framing_constants() {
        assert_eq!(FRAME_HEADER_SIZE, 8);
        assert_eq!(DEFAULT_MAX_PAYLOAD_SIZE, 268_435_456);
    }

    // T4: edge case — length = 0
    #[test]
    fn test_frame_header_zero_length() {
        let header = FrameHeader {
            length: 0,
            checksum: 0,
        };
        let restored = FrameHeader::from_bytes(header.to_bytes());
        assert_eq!(restored.length, 0);
        assert_eq!(restored.checksum, 0);
    }

    // T5: edge case — max values
    #[test]
    fn test_frame_header_max_values() {
        let header = FrameHeader {
            length: u32::MAX,
            checksum: u32::MAX,
        };
        let restored = FrameHeader::from_bytes(header.to_bytes());
        assert_eq!(header, restored);
    }

    // --- send_frame / recv_frame tests ---

    // T6: round-trip Shutdown
    #[tokio::test]
    async fn test_round_trip_shutdown() {
        let (mut client, mut server) = create_test_channel();
        let bytes_written = send_frame(&mut client, &Message::Shutdown).await.unwrap();
        assert!(bytes_written > FRAME_HEADER_SIZE);

        let (msg, bytes_read) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap();
        assert_eq!(bytes_written, bytes_read);
        assert!(matches!(msg, Message::Shutdown));
    }

    // T7: round-trip AssignPartition
    #[tokio::test]
    async fn test_round_trip_assign_partition() {
        let (mut client, mut server) = create_test_channel();
        let msg = Message::AssignPartition {
            round: 42,
            partition: make_test_partition(),
        };
        send_frame(&mut client, &msg).await.unwrap();

        let (received, _) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap();
        match received {
            Message::AssignPartition { round, partition } => {
                assert_eq!(round, 42);
                assert_eq!(partition.worker_id, 0);
            }
            _ => panic!("wrong variant"),
        }
    }

    // T8: round-trip PartitionResult
    #[tokio::test]
    async fn test_round_trip_partition_result() {
        let (mut client, mut server) = create_test_channel();
        let msg = Message::PartitionResult {
            round: 7,
            partition: make_test_partition(),
            stats: make_test_stats(),
        };
        send_frame(&mut client, &msg).await.unwrap();

        let (received, _) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap();
        match received {
            Message::PartitionResult { round, stats, .. } => {
                assert_eq!(round, 7);
                assert_eq!(stats.agents_before, 10);
                assert_eq!(stats.agents_after, 5);
            }
            _ => panic!("wrong variant"),
        }
    }

    // T9: round-trip Error
    #[tokio::test]
    async fn test_round_trip_error() {
        let (mut client, mut server) = create_test_channel();
        let msg = Message::Error {
            round: 3,
            worker_id: 1,
            description: "test error".into(),
        };
        send_frame(&mut client, &msg).await.unwrap();

        let (received, _) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap();
        match received {
            Message::Error {
                round,
                worker_id,
                description,
            } => {
                assert_eq!(round, 3);
                assert_eq!(worker_id, 1);
                assert_eq!(description, "test error");
            }
            _ => panic!("wrong variant"),
        }
    }

    // T10: round-trip Register
    #[tokio::test]
    async fn test_round_trip_register() {
        let (mut client, mut server) = create_test_channel();
        let msg = Message::Register(super::super::types::RegisterPayload {
            protocol_version: 1,
            auth_token: None,
        });
        send_frame(&mut client, &msg).await.unwrap();

        let (received, _) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap();
        assert!(matches!(received, Message::Register(_)));
    }

    // T11: round-trip RegisterAck
    #[tokio::test]
    async fn test_round_trip_register_ack() {
        let (mut client, mut server) = create_test_channel();
        let msg = Message::RegisterAck(super::super::types::RegisterAckPayload { worker_id: 5 });
        send_frame(&mut client, &msg).await.unwrap();

        let (received, _) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap();
        match received {
            Message::RegisterAck(payload) => assert_eq!(payload.worker_id, 5),
            _ => panic!("wrong variant"),
        }
    }

    // T12: round-trip RegisterNack
    #[tokio::test]
    async fn test_round_trip_register_nack() {
        let (mut client, mut server) = create_test_channel();
        let msg = Message::RegisterNack(super::super::types::RegisterNackPayload {
            reason: "auth failed".into(),
        });
        send_frame(&mut client, &msg).await.unwrap();

        let (received, _) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap();
        match received {
            Message::RegisterNack(payload) => assert_eq!(payload.reason, "auth failed"),
            _ => panic!("wrong variant"),
        }
    }

    // T13: checksum mismatch — corrupt payload byte
    #[tokio::test]
    async fn test_checksum_mismatch() {
        let (mut client, mut server) = create_test_channel();

        // Manually write a frame with corrupted payload
        let payload = bincode::serialize(&Message::Shutdown).unwrap();
        let checksum = crc32fast::hash(&payload);
        let header = FrameHeader {
            length: payload.len() as u32,
            checksum,
        };
        client.write_all(&header.to_bytes()).await.unwrap();

        // Write corrupted payload (flip first byte)
        let mut corrupted = payload;
        corrupted[0] ^= 0xFF;
        client.write_all(&corrupted).await.unwrap();
        client.flush().await.unwrap();

        let result = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE).await;
        assert!(matches!(
            result,
            Err(ProtocolError::ChecksumMismatch { .. })
        ));
    }

    // T14: payload too large — rejected before allocating
    #[tokio::test]
    async fn test_payload_too_large() {
        let (mut client, mut server) = create_test_channel();

        // Write a header claiming a huge payload
        let header = FrameHeader {
            length: 1_000_000,
            checksum: 0,
        };
        client.write_all(&header.to_bytes()).await.unwrap();
        client.flush().await.unwrap();

        // Set max to something small
        let result = recv_frame(&mut server, 1024).await;
        match result {
            Err(ProtocolError::PayloadTooLarge { size, max }) => {
                assert_eq!(size, 1_000_000);
                assert_eq!(max, 1024);
            }
            other => panic!("expected PayloadTooLarge, got {:?}", other),
        }
    }

    // T15: bytes_written equals FRAME_HEADER_SIZE + payload length
    #[tokio::test]
    async fn test_bytes_count() {
        let (mut client, mut server) = create_test_channel();
        let msg = Message::Shutdown;
        let payload_size = bincode::serialize(&msg).unwrap().len();
        let bytes_written = send_frame(&mut client, &msg).await.unwrap();
        assert_eq!(bytes_written, FRAME_HEADER_SIZE + payload_size);

        let (_, bytes_read) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap();
        assert_eq!(bytes_written, bytes_read);
    }

    // T16: multiple messages in sequence
    #[tokio::test]
    async fn test_multiple_messages_sequence() {
        let (mut client, mut server) = create_test_channel();

        // Send 3 messages
        send_frame(&mut client, &Message::Shutdown).await.unwrap();
        send_frame(
            &mut client,
            &Message::Error {
                round: 0,
                worker_id: 0,
                description: "oops".into(),
            },
        )
        .await
        .unwrap();
        send_frame(&mut client, &Message::Shutdown).await.unwrap();

        // Receive 3 messages in order
        let (m1, _) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap();
        assert!(matches!(m1, Message::Shutdown));

        let (m2, _) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap();
        assert!(matches!(m2, Message::Error { .. }));

        let (m3, _) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap();
        assert!(matches!(m3, Message::Shutdown));
    }

    // T17: AssignPartition with real agents
    #[tokio::test]
    async fn test_round_trip_partition_with_agents() {
        let (mut client, mut server) = create_test_channel();

        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let _b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));

        let partition = Partition {
            subnet: net,
            worker_id: 2,
            free_port_index: HashMap::new(),
            id_range: IdRange {
                start: 100,
                end: 200,
            },
            border_id_start: 50,
            border_id_end: 60,
        };

        let msg = Message::AssignPartition {
            round: 0,
            partition,
        };
        send_frame(&mut client, &msg).await.unwrap();

        let (received, _) = recv_frame(&mut server, DEFAULT_MAX_PAYLOAD_SIZE)
            .await
            .unwrap();
        match received {
            Message::AssignPartition { round, partition } => {
                assert_eq!(round, 0);
                assert_eq!(partition.worker_id, 2);
                assert_eq!(partition.subnet.count_live_agents(), 2);
                assert_eq!(partition.id_range.start, 100);
                assert_eq!(partition.border_id_start, 50);
            }
            _ => panic!("wrong variant"),
        }
    }
}
