//! Node configuration (SPEC-06, Section 4.5).
//!
//! Defines NodeConfig with bind address, worker count, payload limits,
//! and timeout settings for the coordinator and worker nodes.

use std::net::SocketAddr;
use std::time::Duration;

use super::frame::DEFAULT_MAX_PAYLOAD_SIZE;

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
}
