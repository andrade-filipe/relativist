//! Unix domain socket transport for same-host communication (SPEC-17, Section 3.3).
//!
//! Bypasses the kernel TCP/IP stack entirely. Published benchmarks show
//! 2-3x lower latency and up to 7x higher throughput vs TCP loopback.
//!
//! Only available on `cfg(unix)` platforms (Linux, macOS, FreeBSD).

#[cfg(unix)]
use std::path::PathBuf;

#[cfg(unix)]
use tokio::net::{UnixListener, UnixStream};

#[cfg(unix)]
use super::error::ProtocolError;
#[cfg(unix)]
use super::transport::{Transport, TransportStream};

/// Unix domain socket transport for same-host communication (SPEC-17 R10).
///
/// Does NOT apply TLS wrapping, even when the `tls` feature is enabled (R13).
/// Unix domain sockets provide OS-level access control; encryption on the
/// same host adds overhead with no security benefit.
#[cfg(unix)]
pub struct UnixTransport {
    /// Path to the Unix domain socket file.
    socket_path: PathBuf,
    /// The listener, created on `listen()`.
    listener: Option<UnixListener>,
}

#[cfg(unix)]
impl UnixTransport {
    /// Create a new Unix transport with the given socket path.
    pub fn new(socket_path: PathBuf) -> Self {
        Self {
            socket_path,
            listener: None,
        }
    }
}

#[cfg(unix)]
#[async_trait::async_trait]
impl Transport for UnixTransport {
    async fn listen(&mut self) -> Result<(), ProtocolError> {
        // Remove stale socket file if it exists (R12, R44)
        if self.socket_path.exists() {
            tracing::warn!(
                path = %self.socket_path.display(),
                "Removing stale Unix socket file"
            );
            std::fs::remove_file(&self.socket_path).map_err(ProtocolError::ConnectionLost)?;
        }

        let listener =
            UnixListener::bind(&self.socket_path).map_err(ProtocolError::ConnectionLost)?;
        tracing::info!(
            path = %self.socket_path.display(),
            backend = "unix",
            "Transport listening"
        );
        self.listener = Some(listener);
        Ok(())
    }

    async fn accept(&mut self) -> Result<TransportStream, ProtocolError> {
        let listener = self
            .listener
            .as_ref()
            .ok_or_else(|| ProtocolError::UnexpectedMessage {
                expected: "listen() called before accept()",
                received: "UnixTransport::accept called before listen".to_string(),
            })?;
        let (stream, _peer_addr) = listener
            .accept()
            .await
            .map_err(ProtocolError::ConnectionLost)?;
        tracing::debug!(backend = "unix", "Unix connection accepted");
        Ok(Box::pin(stream))
    }

    async fn connect(&mut self) -> Result<TransportStream, ProtocolError> {
        let stream = UnixStream::connect(&self.socket_path)
            .await
            .map_err(ProtocolError::ConnectionLost)?;
        tracing::info!(
            path = %self.socket_path.display(),
            backend = "unix",
            "Transport connected"
        );
        Ok(Box::pin(stream))
    }
}

// Tests are cfg(unix) only — they will not run on Windows.
#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::protocol::frame::{recv_frame, send_frame};
    use crate::protocol::types::Message;

    // TR2: UnixTransport round-trip (R10, R2)
    #[tokio::test]
    async fn test_unix_transport_round_trip() {
        let sock_path = std::env::temp_dir().join("relativist_test_tr2.sock");
        let _ = std::fs::remove_file(&sock_path);

        let mut server = UnixTransport::new(sock_path.clone());
        server.listen().await.unwrap();

        let mut client = UnixTransport::new(sock_path.clone());

        let connect_handle = tokio::spawn(async move {
            let mut stream = client.connect().await.unwrap();
            send_frame(&mut stream, &Message::Shutdown).await.unwrap();
        });

        let mut server_stream = server.accept().await.unwrap();
        let (msg, _) = recv_frame(&mut server_stream, 1_073_741_824).await.unwrap();
        assert!(matches!(msg, Message::Shutdown));

        connect_handle.await.unwrap();
        let _ = std::fs::remove_file(&sock_path);
    }

    // TR3: Stale socket cleanup (R12, R44)
    #[tokio::test]
    async fn test_unix_transport_stale_socket() {
        let sock_path = std::env::temp_dir().join("relativist_test_tr3.sock");
        // Create a stale file
        std::fs::write(&sock_path, "stale").unwrap();
        assert!(sock_path.exists());

        let mut server = UnixTransport::new(sock_path.clone());
        // listen() should succeed despite stale file
        server.listen().await.unwrap();

        let _ = std::fs::remove_file(&sock_path);
    }
}
