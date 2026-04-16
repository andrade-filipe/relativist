//! Transport abstraction (SPEC-17, Section 3.1).
//!
//! Defines the `Transport` trait that abstracts connection establishment
//! and stream provisioning, and the `TransportStream` type alias for
//! type-erased bidirectional byte streams.

use std::net::SocketAddr;
use std::pin::Pin;

use tokio::io::{AsyncRead, AsyncWrite};

use super::config::{TransportBackend, TransportConfig};
use super::error::ProtocolError;
use super::tcp::TcpTransport;

/// Combined async read+write trait for transport streams.
///
/// Rust trait objects only allow a single non-auto trait, so we cannot write
/// `dyn AsyncRead + AsyncWrite`. This supertrait combines both with `Unpin`
/// and `Send`, and a blanket impl ensures any compatible type (TcpStream,
/// UnixStream, DuplexStream, etc.) automatically implements it.
pub trait AsyncStream: AsyncRead + AsyncWrite + Unpin + Send + Sync {}

/// Blanket impl: any type that is `AsyncRead + AsyncWrite + Unpin + Send + Sync`
/// automatically implements `AsyncStream`.
impl<T: AsyncRead + AsyncWrite + Unpin + Send + Sync> AsyncStream for T {}

/// Type-erased bidirectional byte stream (SPEC-17 R2).
///
/// Compatible with `send_frame`/`recv_frame` in `frame.rs`, which are generic
/// over `AsyncWriteExt + Unpin` / `AsyncReadExt + Unpin`. `Pin<Box<dyn AsyncStream>>`
/// implements both `AsyncRead` and `AsyncWrite` via tokio's blanket impls on
/// `Pin<P: DerefMut + Unpin>`, so it is directly usable with existing framing.
pub type TransportStream = Pin<Box<dyn AsyncStream>>;

/// Abstraction over the connection establishment mechanism (SPEC-17 R1, R3, R5).
///
/// The Transport trait separates *how connections are made* from *what is sent
/// over them*. The framing layer (SPEC-06) and serialization (bincode) operate
/// on the `TransportStream` returned by `accept` and `connect`, regardless of
/// the underlying transport.
///
/// Implementations:
/// - `TcpTransport`: production TCP (with optional TLS)
/// - `UnixTransport`: same-host Unix domain sockets (cfg(unix) only)
/// - `ChannelTransport`: in-memory channels for testing
///
/// This trait uses `async_trait` for object safety (`Box<dyn Transport>`).
/// Native async traits (Rust 1.75+) do not support dynamic dispatch for
/// this use case (SPEC-17 R4, SPEC-13 R51).
#[async_trait::async_trait]
pub trait Transport: Send + Sync {
    /// Bind and start listening for incoming connections.
    ///
    /// For TCP: binds a `TcpListener` on the configured address.
    /// For Unix: binds a `UnixListener` on the configured socket path.
    /// For Channel: no-op (channels are pre-connected via `pair()`).
    async fn listen(&mut self) -> Result<(), ProtocolError>;

    /// Accept a single incoming connection.
    ///
    /// Blocks until a new connection is available. Returns a `TransportStream`
    /// that the caller uses with `send_frame`/`recv_frame`.
    async fn accept(&mut self) -> Result<TransportStream, ProtocolError>;

    /// Establish an outgoing connection to a remote endpoint.
    ///
    /// For TCP: calls `TcpStream::connect()` with tuning applied.
    /// For Unix: calls `UnixStream::connect()` to the configured socket path.
    /// For Channel: returns the pre-created channel endpoint.
    async fn connect(&mut self) -> Result<TransportStream, ProtocolError>;
}

/// Create a transport from configuration (SPEC-17 R27, R28).
///
/// Maps `TransportBackend` to the concrete `Transport` implementation:
/// - `Tcp` → `TcpTransport` with the given bind address and tuning config.
/// - `Unix` → `UnixTransport` (cfg(unix) only; error on other platforms).
/// - `Channel` → error: use `ChannelTransport::pair()` directly (R28).
///
/// Default Unix socket path: `/tmp/relativist.sock` when `unix_socket_path` is `None`.
pub fn create_transport(
    bind_addr: SocketAddr,
    config: &TransportConfig,
) -> Result<Box<dyn Transport>, ProtocolError> {
    match config.backend {
        TransportBackend::Tcp => Ok(Box::new(TcpTransport::new(bind_addr, config.clone()))),
        TransportBackend::Unix => create_unix_transport(config),
        TransportBackend::Channel => Err(ProtocolError::UnexpectedMessage {
            expected: "Tcp or Unix backend",
            received: "Channel backend cannot be created via create_transport; \
                       use ChannelTransport::pair() directly"
                .to_string(),
        }),
    }
}

/// Unix transport creation, platform-gated.
#[cfg(unix)]
fn create_unix_transport(config: &TransportConfig) -> Result<Box<dyn Transport>, ProtocolError> {
    use super::unix::UnixTransport;
    use std::path::PathBuf;

    let path = config
        .unix_socket_path
        .clone()
        .unwrap_or_else(|| PathBuf::from("/tmp/relativist.sock"));
    Ok(Box::new(UnixTransport::new(path)))
}

/// Unix transport stub — returns error on non-Unix platforms (SPEC-17 R14).
#[cfg(not(unix))]
fn create_unix_transport(_config: &TransportConfig) -> Result<Box<dyn Transport>, ProtocolError> {
    Err(ProtocolError::UnexpectedMessage {
        expected: "Unix-compatible platform",
        received: "Unix domain sockets are not supported on this platform".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Compile-time check: TransportStream is compatible with send_frame/recv_frame generics.
    // send_frame requires W: AsyncWriteExt + Unpin, recv_frame requires R: AsyncReadExt + Unpin.
    // AsyncWriteExt is auto-impl for AsyncWrite, AsyncReadExt for AsyncRead.
    // Pin<Box<dyn AsyncStream>> implements both via tokio's blanket impls.
    fn _assert_stream_compat(stream: &mut TransportStream) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        fn _requires_write<W: AsyncWriteExt + Unpin>(_w: &mut W) {}
        fn _requires_read<R: AsyncReadExt + Unpin>(_r: &mut R) {}
        _requires_write(stream);
        _requires_read(stream);
    }

    // Compile-time check: Transport trait is object-safe (Box<dyn Transport> works)
    fn _assert_object_safe(_t: Box<dyn Transport>) {}

    // TS1: create_transport with Tcp backend returns TcpTransport
    #[test]
    fn test_create_transport_tcp() {
        let config = TransportConfig::default(); // backend = Tcp
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let transport = create_transport(addr, &config);
        assert!(transport.is_ok());
    }

    // TS3: create_transport with Unix backend on Windows returns error (R14)
    #[cfg(not(unix))]
    #[test]
    fn test_create_transport_unix_on_windows() {
        let config = TransportConfig {
            backend: TransportBackend::Unix,
            ..TransportConfig::default()
        };
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let result = create_transport(addr, &config);
        assert!(result.is_err());
    }

    // TS3 (unix): create_transport with Unix backend on Unix succeeds
    #[cfg(unix)]
    #[test]
    fn test_create_transport_unix_on_unix() {
        let config = TransportConfig {
            backend: TransportBackend::Unix,
            ..TransportConfig::default()
        };
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let result = create_transport(addr, &config);
        assert!(result.is_ok());
    }

    // TS4: create_transport with Channel backend returns error (R28)
    #[test]
    fn test_create_transport_channel_error() {
        let config = TransportConfig {
            backend: TransportBackend::Channel,
            ..TransportConfig::default()
        };
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let result = create_transport(addr, &config);
        assert!(result.is_err());
    }
}
