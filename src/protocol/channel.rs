//! In-memory channel transport for testing (SPEC-17, Section 3.4).
//!
//! Uses `tokio::io::duplex()` to create paired bidirectional streams.
//! No serialization overhead beyond what `send_frame`/`recv_frame` perform.
//! Specified in SPEC-13 R29, R31.

use tokio::io::duplex;

use super::error::ProtocolError;
use super::transport::{Transport, TransportStream};

/// In-memory channel transport for testing (SPEC-17 R15, SPEC-13 R29).
///
/// Uses `tokio::io::duplex()` to create paired bidirectional streams.
/// Not selectable via CLI (R28); only usable programmatically by the
/// test harness and `relativist local` in-process mode.
pub struct ChannelTransport {
    /// Pre-created stream endpoints to hand out on `accept()`.
    accept_streams: Vec<TransportStream>,
    /// Pre-created stream endpoints to hand out on `connect()`.
    connect_streams: Vec<TransportStream>,
}

impl ChannelTransport {
    /// Create a pair of connected ChannelTransport instances (SPEC-17 R16).
    ///
    /// Returns `(server_transport, client_transport)` where:
    /// - `server_transport.accept()` returns one end of each channel
    /// - `client_transport.connect()` returns the other end
    ///
    /// The `buffer_size` parameter controls the tokio duplex buffer capacity.
    /// Recommendation: 64 KiB for small tests, 1 MiB for integration tests.
    pub fn pair(num_channels: usize, buffer_size: usize) -> (Self, Self) {
        let mut accept_streams = Vec::with_capacity(num_channels);
        let mut connect_streams = Vec::with_capacity(num_channels);

        for _ in 0..num_channels {
            let (server_half, client_half) = duplex(buffer_size);
            accept_streams.push(Box::pin(server_half) as TransportStream);
            connect_streams.push(Box::pin(client_half) as TransportStream);
        }

        // Reverse so that pop() returns them in creation order
        accept_streams.reverse();
        connect_streams.reverse();

        (
            Self {
                accept_streams,
                connect_streams: Vec::new(),
            },
            Self {
                accept_streams: Vec::new(),
                connect_streams,
            },
        )
    }
}

#[async_trait::async_trait]
impl Transport for ChannelTransport {
    async fn listen(&mut self) -> Result<(), ProtocolError> {
        // No-op for channels: streams are pre-created via pair()
        Ok(())
    }

    async fn accept(&mut self) -> Result<TransportStream, ProtocolError> {
        self.accept_streams
            .pop()
            .ok_or_else(|| ProtocolError::UnexpectedMessage {
                expected: "pre-created accept stream",
                received: "ChannelTransport: no more pre-created accept streams".to_string(),
            })
    }

    async fn connect(&mut self) -> Result<TransportStream, ProtocolError> {
        self.connect_streams
            .pop()
            .ok_or_else(|| ProtocolError::UnexpectedMessage {
                expected: "pre-created connect stream",
                received: "ChannelTransport: no more pre-created connect streams".to_string(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::frame::{recv_frame, send_frame};
    use crate::protocol::types::Message;

    // TR4: ChannelTransport round-trip (R15, R16)
    #[tokio::test]
    async fn test_channel_transport_round_trip() {
        let (mut server, mut client) = ChannelTransport::pair(1, 65536);

        server.listen().await.unwrap(); // no-op

        let mut server_stream = server.accept().await.unwrap();
        let mut client_stream = client.connect().await.unwrap();

        // Client sends Shutdown
        send_frame(&mut client_stream, &Message::Shutdown)
            .await
            .unwrap();

        // Server receives it
        let (msg, _) = recv_frame(&mut server_stream, 1_073_741_824).await.unwrap();
        assert!(matches!(msg, Message::Shutdown));
    }

    // TR5: ChannelTransport exhaustion returns error (R16)
    #[tokio::test]
    async fn test_channel_transport_exhaustion() {
        let (mut server, mut client) = ChannelTransport::pair(1, 65536);

        // First accept succeeds
        let _s1 = server.accept().await.unwrap();

        // Second accept should fail — only 1 channel was created
        let result = server.accept().await;
        assert!(result.is_err());

        // Same for client side
        let _c1 = client.connect().await.unwrap();
        let result = client.connect().await;
        assert!(result.is_err());
    }

    // ChannelTransport with multiple channels
    #[tokio::test]
    async fn test_channel_transport_multiple() {
        let (mut server, mut client) = ChannelTransport::pair(3, 65536);

        for _ in 0..3 {
            let _s = server.accept().await.unwrap();
            let _c = client.connect().await.unwrap();
        }

        // Fourth should fail
        assert!(server.accept().await.is_err());
        assert!(client.connect().await.is_err());
    }

    // Object safety
    #[test]
    fn test_channel_transport_object_safe() {
        let (server, _client) = ChannelTransport::pair(1, 1024);
        let _boxed: Box<dyn Transport> = Box::new(server);
    }
}
