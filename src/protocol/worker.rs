//! Worker logic (SPEC-06, Sections 4.7-4.8).
//!
//! Implements the worker side: connect with retry, receive partitions,
//! reduce locally, send results, handle shutdown.

use std::cmp::min;
use std::time::{Duration, Instant};

use tokio::net::TcpStream;

use super::config::NodeConfig;
use super::error::ProtocolError;
use super::frame::{recv_frame, send_frame};
use super::types::Message;
use crate::merge::{rebuild_free_port_index, WorkerRoundStats};
use crate::reduction::reduce_all;

/// Maximum number of connection attempts before giving up.
const MAX_ATTEMPTS: u32 = 10;
/// Initial retry delay (exponential backoff starts here).
const INITIAL_DELAY: Duration = Duration::from_secs(1);
/// Maximum retry delay (backoff caps here).
const MAX_DELAY: Duration = Duration::from_secs(16);

/// Connects to the coordinator with exponential backoff.
///
/// Backoff: 1s, 2s, 4s, 8s, 16s, 16s, 16s, 16s, 16s, 16s (10 attempts).
/// Identical to connectWithRetry in the Haskell prototype (AC-003).
///
/// Returns: connected TcpStream, or ProtocolError after 10 attempts.
pub async fn connect_with_retry(addr: std::net::SocketAddr) -> Result<TcpStream, ProtocolError> {
    let mut delay = INITIAL_DELAY;

    for attempt in 1..=MAX_ATTEMPTS {
        match TcpStream::connect(addr).await {
            Ok(stream) => {
                tracing::info!("Connected to coordinator on attempt {}", attempt);
                return Ok(stream);
            }
            Err(e) => {
                if attempt == MAX_ATTEMPTS {
                    return Err(ProtocolError::ConnectionLost(std::io::Error::new(
                        std::io::ErrorKind::ConnectionRefused,
                        format!("failed to connect after {} attempts: {}", MAX_ATTEMPTS, e),
                    )));
                }
                tracing::warn!(
                    "Attempt {}/{} failed: {}. Retrying in {:?}",
                    attempt,
                    MAX_ATTEMPTS,
                    e,
                    delay
                );
                tokio::time::sleep(delay).await;
                delay = min(delay * 2, MAX_DELAY);
            }
        }
    }

    unreachable!()
}

/// Runs the worker loop: connect, receive partitions, reduce, send results.
///
/// Implements the worker FSM (SPEC-13 R24):
/// Init -> Idle -> Reducing -> Returning -> Idle -> ... -> Done.
pub async fn run_worker(config: &NodeConfig) -> Result<(), ProtocolError> {
    let mut stream = connect_with_retry(config.bind).await?;

    loop {
        let (msg, _nbytes) = recv_frame(&mut stream, config.max_payload_size).await?;

        match msg {
            Message::AssignPartition {
                round,
                mut partition,
            } => {
                tracing::info!(
                    "Round {}: received partition worker_id={}",
                    round,
                    partition.worker_id
                );

                // Local reduction with timing
                let agents_before = partition.subnet.count_live_agents();
                let t_reduce = Instant::now();
                let reduction_stats = reduce_all(&mut partition.subnet);
                let reduce_duration = t_reduce.elapsed();
                let agents_after = partition.subnet.count_live_agents();

                // Reconstruct free_port_index (SPEC-05, Section 4.3)
                partition.free_port_index = rebuild_free_port_index(
                    &partition.subnet,
                    partition.border_id_start,
                    partition.border_id_end,
                );

                // Send result with full 6-field WorkerRoundStats
                let stats = WorkerRoundStats {
                    worker_id: partition.worker_id,
                    agents_before,
                    agents_after,
                    local_redexes: reduction_stats.total_interactions as usize,
                    reduce_duration_secs: reduce_duration.as_secs_f64(),
                    interactions_by_rule: reduction_stats.interactions_by_rule,
                };

                tracing::info!(
                    "Round {}: worker_id={} agents {}->{} interactions={}",
                    round,
                    partition.worker_id,
                    agents_before,
                    agents_after,
                    reduction_stats.total_interactions
                );

                let result_msg = Message::PartitionResult {
                    round,
                    partition,
                    stats,
                };
                send_frame(&mut stream, &result_msg).await?;
            }

            Message::Shutdown => {
                tracing::info!("Received shutdown, terminating worker.");
                break;
            }

            other => {
                return Err(ProtocolError::UnexpectedMessage {
                    expected: "AssignPartition or Shutdown",
                    received: format!("{:?}", other),
                });
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // T1: Backoff sequence is correct
    #[test]
    fn test_backoff_sequence() {
        let mut delay = INITIAL_DELAY;
        let expected = [1, 2, 4, 8, 16, 16, 16, 16, 16, 16];
        for &exp_secs in &expected {
            assert_eq!(delay, Duration::from_secs(exp_secs));
            delay = min(delay * 2, MAX_DELAY);
        }
    }

    // T2: connect_with_retry succeeds on first attempt with a real listener
    #[tokio::test]
    async fn test_connect_with_retry_immediate_success() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let connect_handle = tokio::spawn(async move { connect_with_retry(addr).await });

        // Accept the connection
        let (_stream, _addr) = listener.accept().await.unwrap();

        let result = connect_handle.await.unwrap();
        assert!(result.is_ok());
    }

    // T3: run_worker processes AssignPartition + Shutdown
    #[tokio::test]
    async fn test_run_worker_single_round() {
        use crate::net::{Net, PortRef, Symbol};
        use crate::partition::{IdRange, Partition};
        use std::collections::HashMap;

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let max_payload = NodeConfig::default().max_payload_size;
        let config = NodeConfig {
            bind: addr,
            ..NodeConfig::default()
        };

        // Spawn the worker
        let worker_handle = tokio::spawn(async move { run_worker(&config).await });

        // Accept connection as "coordinator"
        let (mut stream, _) = listener.accept().await.unwrap();

        // Build a simple net with one redex: Con(0) <-> Dup(1) principal-principal
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        // Auxiliary ports to FreePort
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(3));

        let partition = Partition {
            subnet: net,
            worker_id: 0,
            free_port_index: HashMap::new(),
            id_range: IdRange {
                start: 100,
                end: 200_000,
            },
            border_id_start: 0,
            border_id_end: 0,
        };

        // Send AssignPartition
        let msg = Message::AssignPartition {
            round: 0,
            partition,
        };
        send_frame(&mut stream, &msg).await.unwrap();

        // Receive PartitionResult
        let (response, _) = recv_frame(&mut stream, max_payload).await.unwrap();
        match &response {
            Message::PartitionResult { round, stats, .. } => {
                assert_eq!(*round, 0);
                assert_eq!(stats.worker_id, 0);
                assert_eq!(stats.agents_before, 2);
                // CON-DUP commutation creates 4 new agents, removes 2 => 4 after
                assert_eq!(stats.agents_after, 4);
                assert!(stats.local_redexes > 0);
                assert!(stats.reduce_duration_secs >= 0.0);
            }
            other => panic!("expected PartitionResult, got {:?}", other),
        }

        // Send Shutdown
        send_frame(&mut stream, &Message::Shutdown).await.unwrap();

        // Worker should exit cleanly
        let result = worker_handle.await.unwrap();
        assert!(result.is_ok());
    }

    // T4: run_worker handles multiple rounds
    #[tokio::test]
    async fn test_run_worker_multiple_rounds() {
        use crate::net::Net;
        use crate::partition::{IdRange, Partition};
        use std::collections::HashMap;

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let config = NodeConfig {
            bind: addr,
            ..NodeConfig::default()
        };

        let worker_handle = tokio::spawn(async move { run_worker(&config).await });
        let (mut stream, _) = listener.accept().await.unwrap();

        // Send 3 empty rounds + shutdown
        for round in 0..3 {
            let partition = Partition {
                subnet: Net::new(),
                worker_id: 0,
                free_port_index: HashMap::new(),
                id_range: IdRange {
                    start: 0,
                    end: 100_000,
                },
                border_id_start: 0,
                border_id_end: 0,
            };
            send_frame(&mut stream, &Message::AssignPartition { round, partition })
                .await
                .unwrap();

            let (response, _) = recv_frame(&mut stream, NodeConfig::default().max_payload_size)
                .await
                .unwrap();
            match response {
                Message::PartitionResult { round: r, .. } => assert_eq!(r, round),
                other => panic!("expected PartitionResult, got {:?}", other),
            }
        }

        send_frame(&mut stream, &Message::Shutdown).await.unwrap();
        assert!(worker_handle.await.unwrap().is_ok());
    }

    // T5: run_worker returns UnexpectedMessage for wrong message type
    #[tokio::test]
    async fn test_run_worker_unexpected_message() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let config = NodeConfig {
            bind: addr,
            ..NodeConfig::default()
        };

        let worker_handle = tokio::spawn(async move { run_worker(&config).await });
        let (mut stream, _) = listener.accept().await.unwrap();

        // Send a PartitionResult (wrong direction — coordinator->worker should
        // only send AssignPartition or Shutdown)
        use crate::net::Net;
        use crate::partition::{IdRange, Partition};
        use std::collections::HashMap;

        let bad_msg = Message::PartitionResult {
            round: 0,
            partition: Partition {
                subnet: Net::new(),
                worker_id: 0,
                free_port_index: HashMap::new(),
                id_range: IdRange {
                    start: 0,
                    end: 100_000,
                },
                border_id_start: 0,
                border_id_end: 0,
            },
            stats: WorkerRoundStats {
                worker_id: 0,
                agents_before: 0,
                agents_after: 0,
                local_redexes: 0,
                reduce_duration_secs: 0.0,
                interactions_by_rule: [0; 6],
            },
        };
        send_frame(&mut stream, &bad_msg).await.unwrap();

        let result = worker_handle.await.unwrap();
        assert!(matches!(
            result,
            Err(ProtocolError::UnexpectedMessage { .. })
        ));
    }
}
