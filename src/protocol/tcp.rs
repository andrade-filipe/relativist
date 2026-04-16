//! TCP transport with configurable socket tuning (SPEC-17, Section 3.2 + 3.5).
//!
//! Extracted from the hardcoded TCP logic in `coordinator.rs` and `worker.rs`.
//! Applies `TCP_NODELAY`, buffer sizes, and keepalive per `TransportConfig`.

use std::net::SocketAddr;

use socket2::SockRef;
use tokio::net::{TcpListener, TcpStream};

use super::config::TransportConfig;
use super::error::ProtocolError;
use super::transport::{Transport, TransportStream};

/// TCP transport with configurable socket tuning (SPEC-17 R6).
///
/// Wraps `tokio::net::TcpListener` and `tokio::net::TcpStream`.
/// Applies TCP tuning parameters (NODELAY, buffer sizes, keepalive) to every
/// accepted and connected stream before any data is transmitted (R7).
pub struct TcpTransport {
    /// Bind address for the listener (server side) or connect address (client side).
    bind_addr: SocketAddr,
    /// Socket tuning parameters.
    config: TransportConfig,
    /// The TCP listener, created on `listen()`.
    listener: Option<TcpListener>,
}

impl TcpTransport {
    /// Create a new TCP transport with the given bind address and tuning config.
    pub fn new(bind_addr: SocketAddr, config: TransportConfig) -> Self {
        Self {
            bind_addr,
            config,
            listener: None,
        }
    }

    /// Apply TCP tuning to an accepted or connected stream (SPEC-17 R7, R17-R22).
    ///
    /// Called immediately after accept/connect, before any data is sent.
    /// Uses `socket2::SockRef` for keepalive configuration (R21).
    fn apply_tuning(&self, stream: &TcpStream) -> Result<(), ProtocolError> {
        // TCP_NODELAY — disable Nagle's algorithm (R17)
        stream
            .set_nodelay(self.config.tcp_nodelay)
            .map_err(ProtocolError::ConnectionLost)?;

        let sock = SockRef::from(stream);

        // SO_SNDBUF (R18)
        if let Some(size) = self.config.send_buffer_bytes {
            sock.set_send_buffer_size(size)
                .map_err(ProtocolError::ConnectionLost)?;
        }

        // SO_RCVBUF (R19)
        if let Some(size) = self.config.recv_buffer_bytes {
            sock.set_recv_buffer_size(size)
                .map_err(ProtocolError::ConnectionLost)?;
        }

        // TCP keepalive (R20-R21)
        if let Some(idle) = self.config.keepalive_idle {
            let keepalive = socket2::TcpKeepalive::new()
                .with_time(idle)
                .with_interval(self.config.keepalive_interval);

            // with_retries is not available on all platforms (e.g. Windows)
            #[cfg(any(
                target_os = "linux",
                target_os = "macos",
                target_os = "freebsd",
                target_os = "netbsd",
            ))]
            let keepalive = keepalive.with_retries(self.config.keepalive_count);

            sock.set_tcp_keepalive(&keepalive)
                .map_err(ProtocolError::ConnectionLost)?;
        }

        // Log actual buffer sizes — may differ from requested due to kernel behavior (R43)
        tracing::debug!(
            requested_sndbuf = ?self.config.send_buffer_bytes,
            actual_sndbuf = ?sock.send_buffer_size(),
            requested_rcvbuf = ?self.config.recv_buffer_bytes,
            actual_rcvbuf = ?sock.recv_buffer_size(),
            nodelay = self.config.tcp_nodelay,
            "TCP tuning applied"
        );

        Ok(())
    }
}

#[async_trait::async_trait]
impl Transport for TcpTransport {
    async fn listen(&mut self) -> Result<(), ProtocolError> {
        let listener = TcpListener::bind(self.bind_addr)
            .await
            .map_err(ProtocolError::ConnectionLost)?;
        tracing::info!(addr = %self.bind_addr, backend = "tcp", "Transport listening");
        self.listener = Some(listener);
        Ok(())
    }

    async fn accept(&mut self) -> Result<TransportStream, ProtocolError> {
        let listener = self
            .listener
            .as_ref()
            .ok_or_else(|| ProtocolError::UnexpectedMessage {
                expected: "listen() called before accept()",
                received: "TcpTransport::accept called before listen".to_string(),
            })?;
        let (stream, peer_addr) = listener
            .accept()
            .await
            .map_err(ProtocolError::ConnectionLost)?;
        self.apply_tuning(&stream)?;
        tracing::debug!(peer = %peer_addr, "TCP connection accepted");
        Ok(Box::pin(stream))
    }

    async fn connect(&mut self) -> Result<TransportStream, ProtocolError> {
        let stream = TcpStream::connect(self.bind_addr)
            .await
            .map_err(ProtocolError::ConnectionLost)?;
        self.apply_tuning(&stream)?;
        tracing::info!(addr = %self.bind_addr, backend = "tcp", "Transport connected");
        Ok(Box::pin(stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::frame::{recv_frame, send_frame};
    use crate::protocol::types::Message;

    /// Helper: create a TcpTransport with default tuning bound to an ephemeral port.
    async fn make_tcp_transport() -> (TcpTransport, SocketAddr) {
        // Bind to port 0 to get an ephemeral port
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);

        let config = TransportConfig::default();
        (TcpTransport::new(addr, config), addr)
    }

    // TT1: TCP_NODELAY is applied via apply_tuning (R17)
    #[tokio::test]
    async fn test_tcp_nodelay_applied() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let connect_handle = tokio::spawn(async move { TcpStream::connect(addr).await.unwrap() });
        let (server_stream, _) = listener.accept().await.unwrap();
        let client_stream = connect_handle.await.unwrap();

        let transport = TcpTransport::new(addr, TransportConfig::default());
        transport.apply_tuning(&server_stream).unwrap();
        transport.apply_tuning(&client_stream).unwrap();

        assert!(server_stream.nodelay().unwrap());
        assert!(client_stream.nodelay().unwrap());
    }

    // TT2: Buffer sizes applied (R18, R19)
    // Note: Linux kernels double the requested buffer size. We accept >= configured.
    #[tokio::test]
    async fn test_buffer_sizes_applied() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let connect_handle = tokio::spawn(async move { TcpStream::connect(addr).await.unwrap() });
        let (stream, _) = listener.accept().await.unwrap();
        let _ = connect_handle.await.unwrap();

        let config = TransportConfig {
            send_buffer_bytes: Some(1_048_576), // 1 MiB
            recv_buffer_bytes: Some(1_048_576),
            ..TransportConfig::default()
        };
        let transport = TcpTransport::new(addr, config);
        transport.apply_tuning(&stream).unwrap();

        let sock = SockRef::from(&stream);
        let sndbuf = sock.send_buffer_size().unwrap();
        let rcvbuf = sock.recv_buffer_size().unwrap();
        // Kernel may double the value; accept >= requested
        assert!(sndbuf >= 1_048_576, "sndbuf {} < 1MiB", sndbuf);
        assert!(rcvbuf >= 1_048_576, "rcvbuf {} < 1MiB", rcvbuf);
    }

    // TT3: Keepalive is enabled after tuning (R20)
    #[tokio::test]
    async fn test_keepalive_applied() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let connect_handle = tokio::spawn(async move { TcpStream::connect(addr).await.unwrap() });
        let (stream, _) = listener.accept().await.unwrap();
        let _ = connect_handle.await.unwrap();

        let transport = TcpTransport::new(addr, TransportConfig::default());
        transport.apply_tuning(&stream).unwrap();

        let sock = SockRef::from(&stream);
        assert!(sock.keepalive().unwrap(), "keepalive should be enabled");
    }

    // TT4: None buffer sizes don't cause errors (R22)
    #[tokio::test]
    async fn test_tuning_with_none_buffers() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let connect_handle = tokio::spawn(async move { TcpStream::connect(addr).await.unwrap() });
        let (stream, _) = listener.accept().await.unwrap();
        let _ = connect_handle.await.unwrap();

        let config = TransportConfig {
            send_buffer_bytes: None,
            recv_buffer_bytes: None,
            ..TransportConfig::default()
        };
        let transport = TcpTransport::new(addr, config);
        // Should not error — OS defaults are used
        transport.apply_tuning(&stream).unwrap();
    }

    // TT5: Keepalive disabled when idle is None (R22)
    #[tokio::test]
    async fn test_tuning_keepalive_disabled() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let connect_handle = tokio::spawn(async move { TcpStream::connect(addr).await.unwrap() });
        let (stream, _) = listener.accept().await.unwrap();
        let _ = connect_handle.await.unwrap();

        let config = TransportConfig {
            keepalive_idle: None,
            ..TransportConfig::default()
        };
        let transport = TcpTransport::new(addr, config);
        transport.apply_tuning(&stream).unwrap();
        // When keepalive_idle is None, we don't set keepalive — OS default (off) is preserved.
    }

    // TR1: TcpTransport round-trip with send_frame/recv_frame (R6, R2)
    #[tokio::test]
    async fn test_tcp_transport_round_trip() {
        let (mut server, addr) = make_tcp_transport().await;
        server.listen().await.unwrap();

        let mut client = TcpTransport::new(addr, TransportConfig::default());

        let connect_handle = tokio::spawn(async move {
            let mut stream = client.connect().await.unwrap();
            // Client sends Shutdown
            send_frame(&mut stream, &Message::Shutdown).await.unwrap();
            stream
        });

        let mut server_stream = server.accept().await.unwrap();
        let (msg, _nbytes) = recv_frame(&mut server_stream, 1_073_741_824).await.unwrap();
        assert!(matches!(msg, Message::Shutdown));

        let _ = connect_handle.await.unwrap();
    }

    // Transport trait is object-safe with TcpTransport
    #[test]
    fn test_tcp_transport_object_safe() {
        let t = TcpTransport::new("127.0.0.1:0".parse().unwrap(), TransportConfig::default());
        let _boxed: Box<dyn Transport> = Box::new(t);
    }
}
