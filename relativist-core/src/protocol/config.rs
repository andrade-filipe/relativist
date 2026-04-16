//! Node configuration (SPEC-06, Section 4.5).
//!
//! Defines NodeConfig with bind address, worker count, payload limits,
//! and timeout settings for the coordinator and worker nodes.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use super::frame::DEFAULT_MAX_PAYLOAD_SIZE;

// ---------------------------------------------------------------------------
// Transport configuration (SPEC-17)
// ---------------------------------------------------------------------------

/// Transport backend selection (SPEC-17 R27).
///
/// Discriminates which transport implementation to use for
/// coordinator-worker communication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportBackend {
    /// TCP transport (production, benchmarks, LAN).
    Tcp,
    /// Unix domain sockets (same-host fast path).
    /// Only available on `cfg(unix)` platforms.
    Unix,
    /// In-memory channels (testing only).
    /// Not selectable via CLI; used programmatically by test harness.
    Channel,
}

/// Configuration for the transport layer (SPEC-17 R23-R24).
///
/// Embedded in `NodeConfig` (SPEC-17 R25). Controls which transport
/// backend is used and how TCP sockets are tuned.
#[derive(Debug, Clone)]
pub struct TransportConfig {
    /// Which transport backend to use.
    pub backend: TransportBackend,

    /// Whether to set `TCP_NODELAY` (disable Nagle's algorithm).
    /// Ignored for non-TCP backends.
    pub tcp_nodelay: bool,

    /// `SO_SNDBUF` size in bytes. `None` = OS default.
    /// Ignored for non-TCP backends.
    pub send_buffer_bytes: Option<usize>,

    /// `SO_RCVBUF` size in bytes. `None` = OS default.
    /// Ignored for non-TCP backends.
    pub recv_buffer_bytes: Option<usize>,

    /// TCP keepalive idle timeout. `None` = keepalive disabled.
    /// Ignored for non-TCP backends.
    pub keepalive_idle: Option<Duration>,

    /// TCP keepalive probe interval.
    /// Only used when `keepalive_idle` is `Some`.
    pub keepalive_interval: Duration,

    /// TCP keepalive probe count before declaring connection dead.
    /// Only used when `keepalive_idle` is `Some`.
    pub keepalive_count: u32,

    /// Socket path for Unix domain sockets.
    /// Only used when backend is `Unix`.
    pub unix_socket_path: Option<PathBuf>,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            backend: TransportBackend::Tcp,
            tcp_nodelay: true,
            send_buffer_bytes: Some(4_194_304), // 4 MiB
            recv_buffer_bytes: Some(4_194_304), // 4 MiB
            keepalive_idle: Some(Duration::from_secs(30)),
            keepalive_interval: Duration::from_secs(10),
            keepalive_count: 3,
            unix_socket_path: None,
        }
    }
}

/// Configuration of the coordinator node (SPEC-06 Section 4.5).
///
/// SPEC-13 R44-R45 defines separate `CoordinatorArgs` and `WorkerArgs`
/// for the CLI layer. `NodeConfig` is an internal configuration struct
/// populated after CLI argument parsing. The coordinator and worker know
/// their role from the CLI subcommand (SPEC-13 R43), so a `role` field
/// is unnecessary.
#[derive(Debug, Clone)]
pub struct NodeConfig {
    /// Address and port for the TCP listener (coordinator) or
    /// coordinator address (worker).
    /// Default: 127.0.0.1:9000 (SPEC-10 R5, SPEC-13 R44).
    pub bind: SocketAddr,

    /// Expected number of workers (relevant only for coordinator).
    pub num_workers: u32,

    /// Maximum accepted payload size, in bytes.
    /// Default: DEFAULT_MAX_PAYLOAD_SIZE (1 GiB).
    pub max_payload_size: u32,

    /// Timeout for waiting for all workers to connect (coordinator).
    /// Default: 120 seconds.
    pub worker_connect_timeout: Duration,

    /// Timeout for distributing partitions in a round (SHOULD).
    /// Default: 60 seconds.
    pub distribute_timeout: Duration,

    /// Timeout for collecting results in a round (MUST).
    /// Default: 600 seconds.
    pub collect_timeout: Duration,

    /// Transport layer configuration (SPEC-17 R25).
    /// Controls which transport backend is used and how TCP sockets are tuned.
    pub transport: TransportConfig,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1:9000".parse().unwrap(),
            num_workers: 1,
            max_payload_size: DEFAULT_MAX_PAYLOAD_SIZE,
            worker_connect_timeout: Duration::from_secs(120),
            distribute_timeout: Duration::from_secs(60),
            collect_timeout: Duration::from_secs(600),
            transport: TransportConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // T1: Default values match spec
    #[test]
    fn test_node_config_default() {
        let config = NodeConfig::default();
        assert_eq!(config.bind, "127.0.0.1:9000".parse::<SocketAddr>().unwrap());
        assert_eq!(config.num_workers, 1);
        assert_eq!(config.max_payload_size, 1_073_741_824);
        assert_eq!(config.worker_connect_timeout, Duration::from_secs(120));
        assert_eq!(config.distribute_timeout, Duration::from_secs(60));
        assert_eq!(config.collect_timeout, Duration::from_secs(600));
    }

    // T2: Fields can be overridden after construction
    #[test]
    fn test_node_config_override() {
        let config = NodeConfig {
            bind: "0.0.0.0:8080".parse().unwrap(),
            num_workers: 16,
            max_payload_size: 1024,
            worker_connect_timeout: Duration::from_secs(30),
            distribute_timeout: Duration::from_secs(10),
            collect_timeout: Duration::from_secs(120),
            ..NodeConfig::default()
        };

        assert_eq!(config.bind, "0.0.0.0:8080".parse::<SocketAddr>().unwrap());
        assert_eq!(config.num_workers, 16);
        assert_eq!(config.max_payload_size, 1024);
        assert_eq!(config.worker_connect_timeout, Duration::from_secs(30));
    }

    // T3: Clone works
    #[test]
    fn test_node_config_clone() {
        let config = NodeConfig::default();
        let cloned = config.clone();
        assert_eq!(config.bind, cloned.bind);
        assert_eq!(config.num_workers, cloned.num_workers);
    }

    // T4: Debug formatting
    #[test]
    fn test_node_config_debug() {
        let config = NodeConfig::default();
        let debug = format!("{:?}", config);
        assert!(debug.contains("NodeConfig"));
        assert!(debug.contains("127.0.0.1:9000"));
    }

    // --- SPEC-17 Transport Config Tests ---

    // UT1: TransportConfig defaults match SPEC-17 R24
    #[test]
    fn test_transport_config_defaults() {
        let config = TransportConfig::default();
        assert_eq!(config.backend, TransportBackend::Tcp);
        assert!(config.tcp_nodelay);
        assert_eq!(config.send_buffer_bytes, Some(4_194_304));
        assert_eq!(config.recv_buffer_bytes, Some(4_194_304));
        assert_eq!(config.keepalive_idle, Some(Duration::from_secs(30)));
        assert_eq!(config.keepalive_interval, Duration::from_secs(10));
        assert_eq!(config.keepalive_count, 3);
        assert!(config.unix_socket_path.is_none());
    }

    // UT2: TransportBackend has exactly 3 variants with required derives
    #[test]
    fn test_transport_backend_variants() {
        // Verify Debug
        let tcp = TransportBackend::Tcp;
        let unix = TransportBackend::Unix;
        let channel = TransportBackend::Channel;
        assert_eq!(format!("{:?}", tcp), "Tcp");
        assert_eq!(format!("{:?}", unix), "Unix");
        assert_eq!(format!("{:?}", channel), "Channel");

        // Verify Clone + Copy
        let cloned = tcp;
        assert_eq!(tcp, cloned);

        // Verify PartialEq + Eq
        assert_eq!(TransportBackend::Tcp, TransportBackend::Tcp);
        assert_ne!(TransportBackend::Tcp, TransportBackend::Unix);
        assert_ne!(TransportBackend::Unix, TransportBackend::Channel);
    }

    // UT3: NodeConfig.transport field defaults to TransportConfig::default()
    #[test]
    fn test_node_config_transport_field() {
        let node = NodeConfig::default();
        let transport = TransportConfig::default();
        assert_eq!(node.transport.backend, transport.backend);
        assert_eq!(node.transport.tcp_nodelay, transport.tcp_nodelay);
        assert_eq!(
            node.transport.send_buffer_bytes,
            transport.send_buffer_bytes
        );
        assert_eq!(
            node.transport.recv_buffer_bytes,
            transport.recv_buffer_bytes
        );
        assert_eq!(node.transport.keepalive_idle, transport.keepalive_idle);
        assert_eq!(
            node.transport.keepalive_interval,
            transport.keepalive_interval
        );
        assert_eq!(node.transport.keepalive_count, transport.keepalive_count);
        assert_eq!(node.transport.unix_socket_path, transport.unix_socket_path);
    }

    // TransportConfig derives Debug + Clone
    #[test]
    fn test_transport_config_debug_clone() {
        let config = TransportConfig::default();
        let debug = format!("{:?}", config);
        assert!(debug.contains("TransportConfig"));
        assert!(debug.contains("Tcp"));

        let cloned = config.clone();
        assert_eq!(cloned.backend, TransportBackend::Tcp);
        assert!(cloned.tcp_nodelay);
    }
}
