# SPEC-17: Transport Abstraction and Tuning

**Status:** Draft
**Depends on:** SPEC-06 (Wire Protocol), SPEC-13 (System Architecture)
**References consumed:** REF-001, REF-013
**Discussions consumed:** DISC-005 v2 (cross-boundary protocol, serialization format), DISC-006 v2 (communication overhead, granularity, break-even analysis)
**Briefings consumed:** BRIEF-20260415-v2-codebase-assessment (Sections 2, 4.1, 5)

---

## 1. Purpose

This spec defines the `Transport` trait that abstracts connection establishment and stream provisioning, three concrete implementations (`TcpTransport`, `UnixTransport`, `ChannelTransport`), TCP socket tuning parameters, a `TransportConfig` structure integrated into `NodeConfig`, and the runtime transport selection mechanism -- refactoring the hardcoded TCP code in v1's `coordinator.rs` and `worker.rs` into a pluggable transport layer while preserving the existing `frame.rs` generics over `AsyncReadExt`/`AsyncWriteExt`.

---

## 2. Definitions

Terms defined in SPEC-00 (Glossary) and SPEC-06 (Wire Protocol) are used without redefinition. Terms introduced or refined in this spec:

| Term | Definition |
|------|-----------|
| **Transport** | A trait abstracting the establishment and teardown of bidirectional byte streams between coordinator and workers. Implementations provide `listen`/`accept` (server side) and `connect` (client side) methods that return types implementing `AsyncRead + AsyncWrite`. |
| **TransportStream** | The type-erased bidirectional byte stream returned by a `Transport` implementation: `Pin<Box<dyn AsyncRead + AsyncWrite + Unpin + Send>>`. Consumed by the existing `send_frame`/`recv_frame` functions in `frame.rs`. |
| **TcpTransport** | The production transport implementation that wraps `tokio::net::TcpListener` and `tokio::net::TcpStream`. Extracted from the hardcoded TCP logic currently in `coordinator.rs` and `worker.rs`. |
| **UnixTransport** | A same-host transport implementation using Unix domain sockets (`tokio::net::UnixListener` / `tokio::net::UnixStream`). Provides lower latency and higher throughput than TCP loopback by bypassing the kernel network stack. |
| **ChannelTransport** | An in-memory transport implementation using `tokio::sync::mpsc` channels for testing. Enables the full grid cycle to run in a single process without network I/O. Specified in SPEC-13 R29, R31. |
| **TransportConfig** | A configuration struct within `NodeConfig` that selects the transport backend and holds backend-specific settings (TCP tuning, UDS socket path). |
| **TCP Tuning** | Socket-level configuration applied to TCP connections: `TCP_NODELAY`, `SO_SNDBUF`, `SO_RCVBUF`, and TCP keepalive parameters. These were left at OS defaults in v1. |
| **Same-Host Detection** | A heuristic that determines whether the coordinator and workers are on the same host, enabling automatic recommendation of the `Unix` transport backend for lower overhead. |
| **TransportBackend** | An enum discriminating the transport to use: `Tcp`, `Unix`, or `Channel`. Selected via CLI flag or programmatic configuration. |

---

## 3. Requirements

### 3.1 Transport Trait

**R1.** The `protocol` module MUST define a `Transport` trait in `src/protocol/transport.rs` that abstracts connection establishment. The trait MUST provide methods for both the server side (listen/accept) and the client side (connect). **(MUST)**

**R2.** The `Transport` trait MUST return `TransportStream` (defined as `Pin<Box<dyn AsyncRead + AsyncWrite + Unpin + Send>>`) from its `accept` and `connect` methods. This type MUST be compatible with the existing `send_frame<W: AsyncWriteExt + Unpin>` and `recv_frame<R: AsyncReadExt + Unpin>` signatures in `frame.rs` (SPEC-06 Section 4.2). **(MUST)**

**R3.** The `Transport` trait MUST be object-safe, enabling `Box<dyn Transport>` dispatch as specified in SPEC-13 R28. **(MUST)**

**R4.** The `Transport` trait SHOULD use Rust's native async traits (stabilized in Rust 1.75+) if they support `Box<dyn Transport>` dispatch. If native async traits do not support dynamic dispatch for the required use case, `async_trait` is acceptable (SPEC-13 R51). **(SHOULD)**

**R5.** The `Transport` trait MUST be `Send + Sync` to support the coordinator's concurrent dispatch of partitions to multiple workers (SPEC-06 R21). **(MUST)**

### 3.2 TcpTransport

**R6.** Relativist MUST provide a `TcpTransport` implementation in `src/protocol/tcp.rs` that wraps `tokio::net::TcpListener` and `tokio::net::TcpStream`. This implementation MUST be extracted from the hardcoded TCP logic currently in `coordinator.rs` (lines ~40-120) and `worker.rs` (lines ~40-60). **(MUST)**

**R7.** `TcpTransport` MUST apply the TCP tuning parameters from `TransportConfig` (see R17-R22) to every accepted and connected stream. Tuning MUST be applied before any data is transmitted. **(MUST)**

**R8.** When the `tls` feature is enabled, `TcpTransport` MUST transparently wrap the TCP stream with TLS 1.3 via `tokio-rustls`. The `Transport` trait interface MUST NOT change -- TLS is an implementation detail of `TcpTransport`, not a separate transport (SPEC-13 R30). **(MUST)**

**R9.** `TcpTransport` MUST support the existing persistent connection model: connections remain open for the entire duration of the grid loop, closed only after `Shutdown` or irrecoverable error (SPEC-06 R19). **(MUST)**

### 3.3 UnixTransport

**R10.** Relativist MUST provide a `UnixTransport` implementation in `src/protocol/unix.rs` that uses `tokio::net::UnixListener` and `tokio::net::UnixStream` for same-host communication. **(MUST)**

**R11.** `UnixTransport` MUST accept a configurable socket path. The default path MUST be `/tmp/relativist.sock` on Linux/macOS. **(MUST)**

**R12.** `UnixTransport` MUST delete any stale socket file at the configured path before calling `bind()`, to handle unclean shutdowns. The deletion MUST be logged at `warn` level. **(MUST)**

**R13.** `UnixTransport` MUST NOT apply TLS wrapping, even when the `tls` feature is enabled. Unix domain sockets provide OS-level access control; application-level encryption on the same host adds overhead with no security benefit. **(MUST NOT)**

**R14.** `UnixTransport` MUST be conditionally compiled: it MUST be available on `cfg(unix)` targets (Linux, macOS) and MUST NOT be compiled on Windows. On Windows, attempting to select the `Unix` transport backend MUST produce a clear compile-time error or a runtime error with message `"Unix domain sockets are not supported on this platform"`. **(MUST)**

### 3.4 ChannelTransport

**R15.** Relativist MUST provide a `ChannelTransport` implementation in `src/protocol/channel.rs` using `tokio::sync::mpsc` channels for in-memory communication (SPEC-13 R29, R31). **(MUST)**

**R16.** `ChannelTransport` MUST provide a `pair()` constructor that returns two connected `TransportStream` instances (one for each end of the channel), enabling in-process coordinator-worker communication without network I/O. **(MUST)**

### 3.5 TCP Tuning

**R17.** `TcpTransport` MUST set `TCP_NODELAY = true` on every stream (accepted and connected). Nagle's algorithm adds up to 40 ms latency on small writes (Register/RegisterAck handshake, flush of the final TCP segment). **(MUST)**

**R18.** `TcpTransport` MUST set `SO_SNDBUF` to a configurable value. Default: 4 MiB (`4_194_304` bytes). For frames reaching the 1 GiB cap (observed in v1 L6 and 20M stress smoke), larger buffers amortize kernel context switches during `write_all`. **(MUST)**

**R19.** `TcpTransport` MUST set `SO_RCVBUF` to a configurable value. Default: 4 MiB (`4_194_304` bytes). Rationale: symmetric with `SO_SNDBUF`; the receiver also benefits from fewer kernel wake-ups during `read_exact` of large frames. **(MUST)**

**R20.** `TcpTransport` MUST enable TCP keepalive with a configurable idle timeout. Default: 30 seconds. The keepalive interval (`TCP_KEEPINTVL`) MUST be set to 10 seconds. The keepalive probe count (`TCP_KEEPCNT`) MUST be set to 3. **(MUST)**

**R21.** The keepalive configuration MUST use `socket2::SockRef` where `tokio::net::TcpSocket` does not expose the required option directly. The `socket2` crate MUST be added as an always-on dependency. **(MUST)**

**R22.** All TCP tuning parameters MUST be configurable via `TransportConfig` (R23). The defaults specified in R17-R20 MUST be used when no explicit configuration is provided. **(MUST)**

### 3.6 TransportConfig

**R23.** The `protocol::config` module MUST define a `TransportConfig` struct containing all transport-related configuration. **(MUST)**

**R24.** `TransportConfig` MUST contain the following fields:

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `backend` | `TransportBackend` | `Tcp` | Which transport to use |
| `tcp_nodelay` | `bool` | `true` | Whether to disable Nagle's algorithm |
| `send_buffer_bytes` | `Option<usize>` | `Some(4_194_304)` | `SO_SNDBUF` size; `None` = OS default |
| `recv_buffer_bytes` | `Option<usize>` | `Some(4_194_304)` | `SO_RCVBUF` size; `None` = OS default |
| `keepalive_idle` | `Option<Duration>` | `Some(30s)` | TCP keepalive idle timeout; `None` = disabled |
| `keepalive_interval` | `Duration` | `10s` | TCP keepalive probe interval |
| `keepalive_count` | `u32` | `3` | TCP keepalive probe count |
| `unix_socket_path` | `Option<PathBuf>` | `None` | Socket path for `Unix` backend |

**(MUST)**

**R25.** `TransportConfig` MUST be embedded as a field in `NodeConfig` (SPEC-06 R36). The existing `bind: SocketAddr` field in `NodeConfig` MUST remain for the TCP bind address; `TransportConfig` adds the transport-specific settings alongside it. **(MUST)**

**R26.** `TransportConfig` MUST derive `Debug` and `Clone`. It MUST implement `Default` with the values specified in R24. **(MUST)**

### 3.7 TransportBackend Enum

**R27.** The `protocol::config` module MUST define a `TransportBackend` enum with the following variants:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportBackend {
    /// TCP transport (production, benchmarks, LAN).
    Tcp,
    /// Unix domain sockets (same-host fast path).
    /// Only available on cfg(unix) platforms.
    Unix,
    /// In-memory channels (testing only).
    /// Not selectable via CLI; used programmatically by test harness.
    Channel,
}
```

**(MUST)**

**R28.** The `Channel` variant MUST NOT be selectable via the CLI. It MUST only be usable programmatically by the test harness and the `relativist local` in-process mode (SPEC-13 R41a). **(MUST)**

### 3.8 Transport Selection

**R29.** The transport backend MUST be selectable at runtime via the CLI flag `--transport={tcp,unix}`. The default MUST be `tcp`. **(MUST)**

**R30.** The CLI MUST accept the following transport-related flags for the `coordinator` and `worker` subcommands:

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--transport` | `tcp` \| `unix` | `tcp` | Transport backend selection |
| `--socket-path` | `PathBuf` | `/tmp/relativist.sock` | UDS path (only with `--transport=unix`) |
| `--tcp-nodelay` | `bool` | `true` | Enable/disable TCP_NODELAY |
| `--send-buffer` | `usize` (bytes) | `4194304` | SO_SNDBUF size |
| `--recv-buffer` | `usize` (bytes) | `4194304` | SO_RCVBUF size |
| `--keepalive` | `Duration` (seconds) | `30` | TCP keepalive idle; `0` = disabled |

**(MUST)**

**R31.** If `--transport=unix` is specified on a non-Unix platform, the CLI MUST reject the argument with an error message and exit with a non-zero status code. **(MUST)**

**R32.** If `--socket-path` is specified without `--transport=unix`, the CLI SHOULD emit a warning that the socket path is ignored for the TCP backend. **(SHOULD)**

### 3.9 Same-Host Detection Heuristic

**R33.** When the coordinator's bind address is `127.0.0.1` or `::1` (loopback) and the selected backend is `Tcp`, the coordinator SHOULD log a message at `info` level recommending `--transport=unix` for lower overhead. **(SHOULD)**

**R34.** The same-host detection MUST NOT automatically switch transports. It is advisory only. Automatic switching could break Docker Compose setups where `127.0.0.1` is used across containers (not truly same-host). **(MUST NOT)**

### 3.10 Coordinator and Worker Refactoring

**R35.** `coordinator.rs::accept_workers()` MUST be refactored to accept a `&dyn Transport` (or equivalent) instead of creating a `TcpListener` directly. The function signature MUST change from returning `(TcpListener, Vec<TcpStream>)` to returning `Vec<TransportStream>` (or equivalent container). The `TcpListener` binding logic MUST move into `TcpTransport`. **(MUST)**

**R36.** `worker.rs::connect_with_retry()` MUST be refactored to accept a `&dyn Transport` (or equivalent) instead of calling `TcpStream::connect()` directly. The function MUST return a `TransportStream`. The retry logic (exponential backoff, SPEC-06 R23) MUST be preserved in the refactored version. **(MUST)**

**R37.** The refactored `coordinator.rs` and `worker.rs` MUST NOT import `tokio::net::TcpListener` or `tokio::net::TcpStream` directly. All network I/O MUST go through the `Transport` trait. **(MUST)**

**R38.** The refactored functions MUST produce identical observable behavior to the v1 implementations: the same `Message` sequences, the same timeout semantics (SPEC-06 R30-R31), and the same retry behavior (SPEC-06 R23). **(MUST)**

### 3.11 Invariants

**R39.** This spec introduces no new invariants and does not affect any existing invariant (T1-T7, D1-D6, I1-I7, G1). The transport layer is below the level at which IC invariants operate: it is a byte-pipe between coordinator and workers, and the correctness properties of the system depend on the framing and serialization layer (SPEC-06) and the BSP cycle (SPEC-05), not on the transport mechanism. **(informative)**

### 3.12 Complexity and Performance

**R40.** Transport selection and TCP tuning MUST be O(1) per connection (applied once at connection establishment). They MUST NOT add per-frame overhead. **(MUST)**

**R41.** `UnixTransport` SHOULD deliver lower latency than `TcpTransport` on loopback for the same frame sizes. Published benchmarks show 2-3x latency reduction for UDS vs TCP loopback. This spec does not mandate a specific speedup factor; the improvement is workload-dependent. **(SHOULD, informative)**

### 3.13 Logging

**R42.** Transport establishment MUST be logged at `info` level, including the selected backend and relevant parameters (bind address for TCP, socket path for UDS). **(MUST)**

**R43.** TCP tuning application MUST be logged at `debug` level, including the actual buffer sizes as reported by the OS (which may differ from requested values due to kernel doubling behavior). **(MUST)**

**R44.** Stale UDS socket file removal (R12) MUST be logged at `warn` level. **(MUST)**

---

## 4. Design

### 4.1 Transport Trait

```rust
use std::pin::Pin;
use tokio::io::{AsyncRead, AsyncWrite};

use super::error::ProtocolError;

/// Type-erased bidirectional byte stream.
///
/// Compatible with `send_frame`/`recv_frame` in `frame.rs`, which are generic
/// over `AsyncWriteExt + Unpin` / `AsyncReadExt + Unpin`. Since
/// `AsyncWriteExt` is a blanket impl for all `AsyncWrite`, and `AsyncReadExt`
/// for all `AsyncRead`, any `TransportStream` is directly usable with the
/// existing framing functions.
pub type TransportStream = Pin<Box<dyn AsyncRead + AsyncWrite + Unpin + Send>>;

/// Abstraction over the connection establishment mechanism.
///
/// The Transport trait separates *how connections are made* from *what is sent
/// over them*. The framing layer (SPEC-06) and serialization (bincode) operate
/// on the `TransportStream` returned by `accept` and `connect`, regardless of
/// the underlying transport.
///
/// Implementations:
/// - `TcpTransport`: production TCP (with optional TLS)
/// - `UnixTransport`: same-host Unix domain sockets
/// - `ChannelTransport`: in-memory channels for testing
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
    ///
    /// For TCP: calls `TcpListener::accept()`, applies TCP tuning (R7),
    /// optionally wraps with TLS (R8).
    /// For Unix: calls `UnixListener::accept()`.
    /// For Channel: returns the next pre-created channel endpoint.
    async fn accept(&mut self) -> Result<TransportStream, ProtocolError>;

    /// Establish an outgoing connection to a remote endpoint.
    ///
    /// For TCP: calls `TcpStream::connect()`, applies TCP tuning (R7),
    /// optionally wraps with TLS (R8).
    /// For Unix: calls `UnixStream::connect()` to the configured socket path.
    /// For Channel: returns the pre-created channel endpoint.
    async fn connect(&mut self) -> Result<TransportStream, ProtocolError>;
}
```

**Note on `async_trait`:** If native async traits support `Box<dyn Transport>` dispatch at the time of implementation, the `#[async_trait]` attribute SHOULD be removed in favor of native syntax (R4, SPEC-13 R51).

### 4.2 TcpTransport

```rust
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};
use socket2::SockRef;

use super::config::TransportConfig;
use super::error::ProtocolError;
use super::transport::{Transport, TransportStream};

/// TCP transport with configurable socket tuning.
///
/// Extracted from the hardcoded TCP logic in coordinator.rs and worker.rs.
/// Applies TCP_NODELAY, buffer sizes, and keepalive per TransportConfig.
pub struct TcpTransport {
    /// Bind address for the listener (server side).
    bind_addr: SocketAddr,
    /// Socket tuning parameters.
    config: TransportConfig,
    /// The TCP listener, created on `listen()`.
    listener: Option<TcpListener>,
}

impl TcpTransport {
    pub fn new(bind_addr: SocketAddr, config: TransportConfig) -> Self {
        Self {
            bind_addr,
            config,
            listener: None,
        }
    }

    /// Apply TCP tuning to an accepted or connected stream.
    ///
    /// Called immediately after accept/connect, before any data is sent (R7).
    fn apply_tuning(&self, stream: &TcpStream) -> Result<(), ProtocolError> {
        // TCP_NODELAY (R17)
        stream
            .set_nodelay(self.config.tcp_nodelay)
            .map_err(ProtocolError::ConnectionLost)?;

        // SO_SNDBUF (R18)
        if let Some(size) = self.config.send_buffer_bytes {
            let sock = SockRef::from(stream);
            sock.set_send_buffer_size(size)
                .map_err(ProtocolError::ConnectionLost)?;
        }

        // SO_RCVBUF (R19)
        if let Some(size) = self.config.recv_buffer_bytes {
            let sock = SockRef::from(stream);
            sock.set_recv_buffer_size(size)
                .map_err(ProtocolError::ConnectionLost)?;
        }

        // TCP keepalive (R20)
        if let Some(idle) = self.config.keepalive_idle {
            let sock = SockRef::from(stream);
            let keepalive = socket2::TcpKeepalive::new()
                .with_time(idle)
                .with_interval(self.config.keepalive_interval)
                .with_retries(self.config.keepalive_count);
            sock.set_tcp_keepalive(&keepalive)
                .map_err(ProtocolError::ConnectionLost)?;
        }

        // Log actual buffer sizes (R43)
        let sock = SockRef::from(stream);
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
            .ok_or_else(|| ProtocolError::InvalidMessage(
                "TcpTransport::accept called before listen".to_string(),
            ))?;
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
```

### 4.3 UnixTransport

```rust
use std::path::PathBuf;
use tokio::net::{UnixListener, UnixStream};

use super::error::ProtocolError;
use super::transport::{Transport, TransportStream};

/// Unix domain socket transport for same-host communication.
///
/// Bypasses the kernel TCP/IP stack entirely. Published benchmarks show
/// 2-3x lower latency and up to 7x higher throughput vs TCP loopback.
#[cfg(unix)]
pub struct UnixTransport {
    /// Path to the Unix domain socket file.
    socket_path: PathBuf,
    /// The listener, created on `listen()`.
    listener: Option<UnixListener>,
}

#[cfg(unix)]
impl UnixTransport {
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
        // Remove stale socket file if it exists (R12)
        if self.socket_path.exists() {
            tracing::warn!(
                path = %self.socket_path.display(),
                "Removing stale Unix socket file"
            );
            std::fs::remove_file(&self.socket_path)
                .map_err(ProtocolError::ConnectionLost)?;
        }

        let listener = UnixListener::bind(&self.socket_path)
            .map_err(ProtocolError::ConnectionLost)?;
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
            .ok_or_else(|| ProtocolError::InvalidMessage(
                "UnixTransport::accept called before listen".to_string(),
            ))?;
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
```

### 4.4 ChannelTransport

```rust
use tokio::io::{DuplexStream, duplex};

use super::error::ProtocolError;
use super::transport::{Transport, TransportStream};

/// In-memory channel transport for testing (SPEC-13 R29, R31).
///
/// Uses `tokio::io::duplex()` to create paired bidirectional streams.
/// No serialization overhead beyond what `send_frame`/`recv_frame` perform.
pub struct ChannelTransport {
    /// Pre-created stream endpoints to hand out on `accept()`/`connect()`.
    /// Each `pair()` call pushes two endpoints: one for accept, one for connect.
    accept_streams: Vec<TransportStream>,
    connect_streams: Vec<TransportStream>,
}

impl ChannelTransport {
    /// Create a pair of connected ChannelTransport instances.
    ///
    /// Returns `(server_transport, client_transport)` where:
    /// - `server_transport.accept()` returns one end of each channel
    /// - `client_transport.connect()` returns the other end
    ///
    /// The `buffer_size` parameter controls the tokio duplex buffer capacity.
    /// Default recommendation: 64 KiB for small tests, 1 MiB for integration tests
    /// with realistic partition sizes.
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
            .ok_or_else(|| ProtocolError::InvalidMessage(
                "ChannelTransport: no more pre-created accept streams".to_string(),
            ))
    }

    async fn connect(&mut self) -> Result<TransportStream, ProtocolError> {
        self.connect_streams
            .pop()
            .ok_or_else(|| ProtocolError::InvalidMessage(
                "ChannelTransport: no more pre-created connect streams".to_string(),
            ))
    }
}
```

### 4.5 TransportConfig

```rust
use std::path::PathBuf;
use std::time::Duration;

/// Transport backend selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportBackend {
    /// TCP transport (production, benchmarks, LAN).
    Tcp,
    /// Unix domain sockets (same-host fast path).
    /// Only available on cfg(unix) platforms.
    Unix,
    /// In-memory channels (testing only).
    /// Not selectable via CLI.
    Channel,
}

/// Configuration for the transport layer.
///
/// Embedded in `NodeConfig` (SPEC-06 R36). Controls which transport
/// backend is used and how TCP sockets are tuned.
#[derive(Debug, Clone)]
pub struct TransportConfig {
    /// Which transport backend to use.
    pub backend: TransportBackend,

    /// Whether to set TCP_NODELAY (disable Nagle's algorithm).
    /// Ignored for non-TCP backends.
    pub tcp_nodelay: bool,

    /// SO_SNDBUF size in bytes. None = OS default.
    /// Ignored for non-TCP backends.
    pub send_buffer_bytes: Option<usize>,

    /// SO_RCVBUF size in bytes. None = OS default.
    /// Ignored for non-TCP backends.
    pub recv_buffer_bytes: Option<usize>,

    /// TCP keepalive idle timeout. None = keepalive disabled.
    /// Ignored for non-TCP backends.
    pub keepalive_idle: Option<Duration>,

    /// TCP keepalive probe interval.
    /// Only used when keepalive_idle is Some.
    pub keepalive_interval: Duration,

    /// TCP keepalive probe count before declaring connection dead.
    /// Only used when keepalive_idle is Some.
    pub keepalive_count: u32,

    /// Socket path for Unix domain sockets.
    /// Only used when backend is Unix.
    pub unix_socket_path: Option<PathBuf>,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            backend: TransportBackend::Tcp,
            tcp_nodelay: true,
            send_buffer_bytes: Some(4_194_304),  // 4 MiB
            recv_buffer_bytes: Some(4_194_304),  // 4 MiB
            keepalive_idle: Some(Duration::from_secs(30)),
            keepalive_interval: Duration::from_secs(10),
            keepalive_count: 3,
            unix_socket_path: None,
        }
    }
}
```

### 4.6 NodeConfig Integration

The existing `NodeConfig` in `src/protocol/config.rs` gains a `transport` field:

```rust
#[derive(Debug, Clone)]
pub struct NodeConfig {
    /// Address and port for the TCP listener / coordinator address.
    pub bind: SocketAddr,
    /// Expected number of workers.
    pub num_workers: u32,
    /// Maximum accepted payload size, in bytes.
    pub max_payload_size: u32,
    /// Timeout for waiting for all workers to connect.
    pub worker_connect_timeout: Duration,
    /// Timeout for distributing partitions in a round.
    pub distribute_timeout: Duration,
    /// Timeout for collecting results in a round.
    pub collect_timeout: Duration,
    /// Transport layer configuration (NEW: SPEC-17).
    pub transport: TransportConfig,
}
```

The `Default` implementation MUST set `transport: TransportConfig::default()`.

### 4.7 Module Layout

After SPEC-17, the `protocol/` module contains:

```
src/protocol/
├── mod.rs          # Re-exports
├── config.rs       # NodeConfig, TransportConfig, TransportBackend
├── error.rs        # ProtocolError (unchanged)
├── frame.rs        # send_frame, recv_frame (unchanged — already generic)
├── types.rs        # Message enum (unchanged)
├── transport.rs    # Transport trait, TransportStream type alias (NEW)
├── tcp.rs          # TcpTransport (NEW — extracted from coordinator.rs/worker.rs)
├── unix.rs         # UnixTransport (NEW — cfg(unix) only)
├── channel.rs      # ChannelTransport (NEW)
├── coordinator.rs  # Refactored: uses &dyn Transport instead of TcpListener/TcpStream
└── worker.rs       # Refactored: uses &dyn Transport instead of TcpStream
```

This layout is consistent with SPEC-13 R5 (module structure) and adds the files foreseen in SPEC-13 (`transport.rs`, `tcp.rs`, `channel.rs`) plus `unix.rs`.

### 4.8 Transport Factory

```rust
/// Create a Transport instance from configuration.
///
/// This is the single point where TransportBackend is matched and the
/// corresponding implementation is constructed. Called by main.rs
/// after CLI parsing.
pub fn create_transport(
    bind_addr: SocketAddr,
    config: &TransportConfig,
) -> Result<Box<dyn Transport>, ProtocolError> {
    match config.backend {
        TransportBackend::Tcp => {
            Ok(Box::new(TcpTransport::new(bind_addr, config.clone())))
        }
        #[cfg(unix)]
        TransportBackend::Unix => {
            let path = config
                .unix_socket_path
                .clone()
                .unwrap_or_else(|| PathBuf::from("/tmp/relativist.sock"));
            Ok(Box::new(UnixTransport::new(path)))
        }
        #[cfg(not(unix))]
        TransportBackend::Unix => {
            Err(ProtocolError::InvalidMessage(
                "Unix domain sockets are not supported on this platform".to_string(),
            ))
        }
        TransportBackend::Channel => {
            Err(ProtocolError::InvalidMessage(
                "Channel transport cannot be created via create_transport; \
                 use ChannelTransport::pair() directly".to_string(),
            ))
        }
    }
}
```

---

## 5. Rationale

### 5.1 Why a Transport Trait (Not Just TCP Tuning)

ROADMAP 2.22 (TCP tuning) could be implemented by simply adding `set_nodelay()`/`set_send_buffer_size()` calls to the existing hardcoded TCP code in `coordinator.rs` and `worker.rs`. However, ROADMAP 2.25 (same-host fast path) requires a fundamentally different connection mechanism (UDS), and SPEC-13 R28-R31 already mandate a `Transport` trait with `TcpTransport` and `ChannelTransport`. Implementing the trait now, rather than adding tuning to hardcoded TCP and then refactoring for the trait later, avoids a double refactoring pass. The total effort is marginally higher than tuning alone (~200 LoC for the trait + UDS vs ~100 LoC for tuning only) but produces a cleaner architecture.

### 5.2 Why TransportStream Uses Type Erasure

`frame.rs` is already generic over `AsyncReadExt`/`AsyncWriteExt` (confirmed in briefing: line 66). Using `Pin<Box<dyn AsyncRead + AsyncWrite + Unpin + Send>>` as the return type from `Transport` methods means:

1. **Zero changes to `frame.rs`.** The existing `send_frame<W: AsyncWriteExt + Unpin>` and `recv_frame<R: AsyncReadExt + Unpin>` signatures accept `TransportStream` without modification, because `AsyncReadExt` is auto-implemented for all `AsyncRead` types and `AsyncWriteExt` for all `AsyncWrite` types.

2. **Object safety.** The `Transport` trait can be used as `Box<dyn Transport>` (R3, SPEC-13 R28), which is required for the coordinator's `WorkerHandle` (SPEC-13 Section 3.6).

3. **TLS transparency.** `tokio_rustls::TlsStream<TcpStream>` implements `AsyncRead + AsyncWrite`, so it fits into `TransportStream` without changing the trait (R8, SPEC-13 R30).

The cost is one vtable dispatch per read/write call, which is negligible compared to the I/O syscall latency and the bincode serialization cost.

### 5.3 Why UDS but Not Shared Memory (Yet)

ROADMAP 2.25 describes three levels: TCP, UDS, and shared memory (SHM). This spec covers TCP and UDS only. SHM (ring-buffer over `memfd_create` with futex wakeups) is significantly more complex (~200 additional LoC, careful synchronization, platform-specific) and provides marginal benefit over UDS for the frame sizes typical in the TCC benchmarks. UDS already eliminates the kernel network stack overhead; SHM would additionally eliminate kernel buffer copies but adds implementation risk. SHM MAY be added as a fourth `TransportBackend` variant in a future spec without changing the trait interface.

### 5.4 Why TCP Tuning Defaults Are Aggressive

- **`TCP_NODELAY = true`:** Relativist sends large frames (often > 1 MB) followed by a flush. Nagle's algorithm is designed for interactive protocols with many small writes; it only adds latency here. The Register/RegisterAck handshake is a small-message case where Nagle adds up to 40 ms delay per message.

- **`SO_SNDBUF/SO_RCVBUF = 4 MiB`:** The Linux default (~208 KB) forces thousands of context switches during a single `write_all` of a 1 GiB frame. 4 MiB amortizes this dramatically. The memory cost is 8 MiB per connection (send + receive), which is negligible given that each worker also holds a partition of potentially hundreds of MB.

- **Keepalive idle = 30 seconds:** The Linux default (~2 hours) is far too slow to detect a stalled connection in a grid computing scenario. 30 seconds with 3 probes at 10-second intervals means a dead connection is detected within 60 seconds, which is well within the `collect_timeout` default of 600 seconds.

### 5.5 Alternatives Considered

| Alternative | Why Rejected |
|-------------|-------------|
| Associated types instead of `TransportStream` | Prevents object safety (`Box<dyn Transport>`); would require generics throughout coordinator/worker. |
| Separate `Listener` and `Connector` traits | Adds complexity without clear benefit; coordinator uses both `listen`+`accept` and workers use `connect`. A single trait is simpler. |
| Generic coordinator/worker over `T: Transport` | Would make coordinator and worker generic, increasing binary size (monomorphization) and reducing ergonomics. Dynamic dispatch via `Box<dyn Transport>` is sufficient given the negligible cost relative to I/O. |
| Named pipes on Windows instead of UDS | Would require a separate `NamedPipeTransport` with different semantics. UDS on Windows (available since Windows 10 1803) is technically possible via `uds_windows` crate but is not well-tested with tokio. Deferred to future work. |

---

## 6. Migration Path

### 6.1 v1 State

In v1 (`v1-feature-complete`), TCP is hardcoded:

- `coordinator.rs::accept_workers()` creates a `TcpListener` via `TcpListener::bind()`, accepts connections in a loop, and returns `(TcpListener, Vec<TcpStream>)`. ~80 lines of TCP-specific code.
- `worker.rs::connect_with_retry()` calls `TcpStream::connect()` directly with exponential backoff. ~30 lines of TCP-specific code.
- `frame.rs::send_frame<W>` and `recv_frame<R>` are already generic over `AsyncWriteExt`/`AsyncReadExt`. No changes needed.
- No socket tuning is applied. All sockets use OS defaults.
- `NodeConfig` contains `bind: SocketAddr` but no transport configuration.

### 6.2 Migration Steps

1. **Add new files:** `transport.rs`, `tcp.rs`, `unix.rs`, `channel.rs` under `src/protocol/`.

2. **Add `TransportConfig` and `TransportBackend`** to `config.rs`. Add `transport: TransportConfig` field to `NodeConfig`. Update `Default` impl.

3. **Implement `TcpTransport`:** Extract `TcpListener::bind()` from `coordinator.rs` into `TcpTransport::listen()`. Extract `TcpStream::connect()` from `worker.rs` into `TcpTransport::connect()`. Add tuning in `apply_tuning()`.

4. **Implement `UnixTransport`** and `ChannelTransport`.

5. **Refactor `coordinator.rs`:** Replace `TcpListener::bind()` + accept loop with `transport.listen()` + `transport.accept()` loop. Replace `Vec<TcpStream>` with `Vec<TransportStream>`. Remove direct `tokio::net` imports.

6. **Refactor `worker.rs`:** Replace `TcpStream::connect()` with `transport.connect()`. Preserve retry logic. Remove direct `tokio::net` imports.

7. **Add CLI flags** (`--transport`, `--socket-path`, `--tcp-nodelay`, `--send-buffer`, `--recv-buffer`, `--keepalive`) to `CoordinatorArgs` and `WorkerArgs` in `main.rs`.

8. **Add `socket2` dependency** to `Cargo.toml` (always-on).

9. **Update existing tests:** Tests in `coordinator.rs` and `worker.rs` that create `TcpListener`/`TcpStream` directly MUST be updated to use `TcpTransport` or `ChannelTransport`. All 690 v1 tests MUST continue to pass.

### 6.3 Estimated Refactoring Surface

| File | Lines Affected | Nature of Change |
|------|---------------|------------------|
| `protocol/transport.rs` | ~40 (new) | Trait definition, type alias |
| `protocol/tcp.rs` | ~120 (new) | TcpTransport impl, apply_tuning |
| `protocol/unix.rs` | ~70 (new, cfg(unix)) | UnixTransport impl |
| `protocol/channel.rs` | ~60 (new) | ChannelTransport impl |
| `protocol/config.rs` | ~50 (modified) | TransportConfig, TransportBackend, NodeConfig field |
| `protocol/coordinator.rs` | ~80 (modified) | Replace TcpListener/TcpStream with Transport trait |
| `protocol/worker.rs` | ~30 (modified) | Replace TcpStream::connect with Transport::connect |
| `main.rs` | ~30 (modified) | CLI flags, create_transport call |
| `Cargo.toml` | ~3 (modified) | socket2 dependency |
| **Total** | **~483** | |

---

## 7. Test Strategy

### 7.1 Unit Tests

**UT1 (TransportConfig defaults).** Verify that `TransportConfig::default()` returns the values specified in R24: `backend = Tcp`, `tcp_nodelay = true`, `send_buffer_bytes = Some(4_194_304)`, `recv_buffer_bytes = Some(4_194_304)`, `keepalive_idle = Some(30s)`, `keepalive_interval = 10s`, `keepalive_count = 3`, `unix_socket_path = None`. **(R24, R26)**

**UT2 (TransportBackend variants).** Verify that `TransportBackend` has exactly three variants: `Tcp`, `Unix`, `Channel`. Verify `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq` derive. **(R27)**

**UT3 (NodeConfig transport field).** Verify that `NodeConfig::default().transport` equals `TransportConfig::default()`. Verify that the `transport` field can be overridden independently of other NodeConfig fields. **(R25)**

### 7.2 TCP Tuning Tests

**TT1 (TCP_NODELAY applied).** Spin up a `TcpTransport` listener and connector. After `accept()`/`connect()`, assert `tcp_stream.nodelay() == true`. **(R17)**

**TT2 (Buffer sizes applied).** Same setup as TT1. Assert that `SockRef::from(&stream).recv_buffer_size()` is >= the configured value. Note: Linux kernels double the requested buffer size; the test MUST accept values >= configured and <= 2x configured. Document this kernel behavior in the test assertion. **(R18, R19)**

**TT3 (Keepalive applied).** Same setup as TT1. Assert that `SockRef::from(&stream).keepalive()` returns `Ok(true)` (keepalive is enabled). Platform-specific: on Linux, additionally verify `keepalive_time()` if the API exposes it. **(R20)**

**TT4 (Tuning with None buffer sizes).** Configure `send_buffer_bytes = None, recv_buffer_bytes = None`. Verify that the stream is usable (no error) and that buffer sizes are at the OS default. **(R22)**

**TT5 (Tuning with keepalive disabled).** Configure `keepalive_idle = None`. Verify that keepalive is not enabled on the stream. **(R22)**

### 7.3 Transport Trait Tests

**TR1 (TcpTransport round-trip).** Create a `TcpTransport`, call `listen()`, then spawn a task that calls `connect()`. On the listener side, call `accept()`. Send a `Message::Shutdown` via `send_frame` on one stream, receive it via `recv_frame` on the other. Verify the message is correctly round-tripped. **(R6, R2)**

**TR2 (UnixTransport round-trip, cfg(unix) only).** Same as TR1 but with `UnixTransport` using a temporary socket path. Verify the message round-trips correctly through the Unix domain socket. **(R10, R2)**

**TR3 (UnixTransport stale socket cleanup).** Create a file at the socket path before calling `listen()`. Verify that `listen()` succeeds (the stale file was removed) and that a warning was logged. **(R12, R44)**

**TR4 (ChannelTransport round-trip).** Create a `ChannelTransport::pair(1, 65536)`. Call `accept()` on the server side, `connect()` on the client side. Send/receive a `Message::AssignPartition` with a test partition. Verify correctness. **(R15, R16)**

**TR5 (ChannelTransport exhaustion).** Create a `ChannelTransport::pair(1, 65536)`. Call `accept()` once (succeeds). Call `accept()` again (MUST return an error). **(R16)**

### 7.4 Transport Selection Tests

**TS1 (create_transport TCP).** Call `create_transport` with `TransportBackend::Tcp`. Verify it returns `Ok` and the resulting transport can `listen()`. **(R27, Section 4.8)**

**TS2 (create_transport Unix, cfg(unix)).** Call `create_transport` with `TransportBackend::Unix`. Verify it returns `Ok`. **(R27, Section 4.8)**

**TS3 (create_transport Unix, cfg(not(unix))).** On Windows, call `create_transport` with `TransportBackend::Unix`. Verify it returns `Err` with message containing "not supported on this platform". **(R14, R31)**

**TS4 (create_transport Channel).** Call `create_transport` with `TransportBackend::Channel`. Verify it returns `Err` (Channel cannot be created via the factory). **(R28, Section 4.8)**

### 7.5 Coordinator/Worker Refactoring Tests

**CW1 (Existing test suite passes).** All 690 v1 tests MUST pass after the refactoring. This is the primary regression gate. **(R38)**

**CW2 (Coordinator accept via Transport).** Verify that the refactored `accept_workers()` correctly accepts `num_workers` connections via the `Transport` trait, using `ChannelTransport` for the test. **(R35)**

**CW3 (Worker connect via Transport).** Verify that the refactored `connect_with_retry()` correctly establishes a connection via the `Transport` trait, using `ChannelTransport` for the test. Verify that retry behavior is preserved (test with a delayed listener). **(R36)**

**CW4 (No direct TcpListener/TcpStream imports).** A `grep` test (or manual code review checkpoint) verifying that `coordinator.rs` and `worker.rs` do not import `tokio::net::TcpListener` or `tokio::net::TcpStream`. **(R37)**

### 7.6 CLI Tests

**CL1 (--transport=tcp accepted).** Verify the CLI parses `--transport=tcp` without error and produces `TransportBackend::Tcp`. **(R29)**

**CL2 (--transport=unix accepted, cfg(unix)).** Verify the CLI parses `--transport=unix` without error and produces `TransportBackend::Unix`. **(R29)**

**CL3 (--transport=unix rejected, cfg(not(unix))).** On Windows, verify the CLI (or `create_transport`) produces an error for `--transport=unix`. **(R31)**

**CL4 (--socket-path without --transport=unix warns).** Verify that `--socket-path=/tmp/foo.sock --transport=tcp` produces a warning. **(R32)**

**CL5 (TCP tuning flags).** Verify that `--tcp-nodelay=false --send-buffer=1048576 --recv-buffer=2097152 --keepalive=60` correctly populates `TransportConfig`. **(R30)**

### 7.7 Integration Tests

**IT1 (Full grid cycle via TcpTransport).** Run a small grid cycle (e.g., `ep_annihilation_con` with 100 agents, 2 workers) using `TcpTransport` with tuning enabled. Verify the result is isomorphic to `reduce_all` (G1). **(R6, R7, R38)**

**IT2 (Full grid cycle via UnixTransport, cfg(unix)).** Same as IT1 but with `UnixTransport`. Verify G1 is preserved. **(R10)**

**IT3 (Full grid cycle via ChannelTransport).** Same as IT1 but with `ChannelTransport`. This is the existing `relativist local` test path (SPEC-13 R41a). **(R15)**

### 7.8 Same-Host Detection Test

**SD1 (Loopback advisory).** Configure coordinator with `bind = 127.0.0.1:9000` and `backend = Tcp`. Start the coordinator. Verify that an `info`-level log message recommending `--transport=unix` is emitted. **(R33)**

**SD2 (Non-loopback no advisory).** Configure coordinator with `bind = 0.0.0.0:9000` and `backend = Tcp`. Verify that no advisory message is emitted. **(R33)**

---

## 8. Open Questions

*None. All questions have been resolved during spec drafting by consulting ROADMAP 2.22, ROADMAP 2.25, SPEC-13 R28-R31/R51-R52, and the codebase assessment briefing.*
