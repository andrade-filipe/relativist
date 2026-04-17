//! Worker logic (SPEC-06, Sections 4.7-4.8).
//!
//! Implements the worker side: connect with retry, receive partitions,
//! reduce locally, send results, handle shutdown.

use std::cmp::min;
use std::time::{Duration, Instant};

use super::config::NodeConfig;
use super::coordinator::PROTOCOL_VERSION;
use super::error::ProtocolError;
use super::frame::{recv_frame, send_frame};
use super::transport::{Transport, TransportStream};
use super::types::{Message, RegisterPayload};
use crate::merge::helpers::compute_border_activity;
use crate::merge::{rebuild_free_port_index, WorkerRoundStats};
use crate::reduction::reduce_all;
use crate::security::AuthToken;

/// Parses a `RegisterNack` reason string for the canonical
/// "protocol version mismatch: expected N, got M" phrasing emitted by
/// `coordinator::accept_workers` (SPEC-18 R29). Returns `Some(received)`
/// when the phrase is present and the received-version field can be
/// parsed as a `u8`; `None` otherwise. Callers fall back to `AuthFailed`
/// on `None` to preserve the SPEC-10 contract for non-version nacks.
///
/// Note: the wire word is `"got"` (R29 literal) but the variable name
/// `received` matches `ProtocolError::VersionMismatch { received }`
/// (R35). The two intentionally use different terms — the wire string is
/// the spec-mandated literal, the field name is the local Rust name.
fn parse_version_mismatch_nack(reason: &str) -> Option<u8> {
    if !reason.contains("protocol version mismatch") {
        return None;
    }
    let got_idx = reason.find("got ")?;
    let tail = &reason[got_idx + "got ".len()..];
    let end = tail
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(tail.len());
    tail[..end].parse::<u8>().ok()
}

/// Maximum number of connection attempts before giving up (single-shot mode).
const MAX_ATTEMPTS: u32 = 10;
/// Initial retry delay (exponential backoff starts here).
const INITIAL_DELAY: Duration = Duration::from_secs(1);
/// Maximum retry delay (backoff caps here).
const MAX_DELAY: Duration = Duration::from_secs(16);
/// Delay after a successful job in daemon mode (SPEC-16 R5).
const DAEMON_SUCCESS_DELAY: Duration = Duration::from_secs(2);
/// Delay after a failed job in daemon mode (SPEC-16 R6).
const DAEMON_FAILURE_DELAY: Duration = Duration::from_secs(5);

/// Connects to the coordinator with exponential backoff (SPEC-06 R23, SPEC-16 R4, SPEC-17 R36).
///
/// Backoff: 1s, 2s, 4s, 8s, 16s, 16s, ... (capped at 16s).
///
/// `max_attempts`: `Some(n)` limits to n attempts (default single-shot behavior).
/// `None` retries indefinitely (daemon mode, SPEC-16 R4).
pub async fn connect_with_retry(
    transport: &mut dyn Transport,
    max_attempts: Option<u32>,
) -> Result<TransportStream, ProtocolError> {
    let mut delay = INITIAL_DELAY;
    let mut attempt: u32 = 0;

    loop {
        attempt += 1;

        match transport.connect().await {
            Ok(stream) => {
                tracing::info!("Connected to coordinator on attempt {}", attempt);
                return Ok(stream);
            }
            Err(e) => {
                if let Some(max) = max_attempts {
                    if attempt >= max {
                        return Err(ProtocolError::ConnectionLost(std::io::Error::new(
                            std::io::ErrorKind::ConnectionRefused,
                            format!("failed to connect after {} attempts: {}", max, e),
                        )));
                    }
                    tracing::warn!(
                        "Attempt {}/{} failed: {}. Retrying in {:?}",
                        attempt,
                        max,
                        e,
                        delay
                    );
                } else {
                    tracing::warn!("Attempt {} failed: {}. Retrying in {:?}", attempt, e, delay);
                }
                tokio::time::sleep(delay).await;
                delay = min(delay * 2, MAX_DELAY);
            }
        }
    }
}

/// Runs the worker loop (single-shot): connect, register, reduce, exit.
///
/// This is the public API; internal logic is in `run_worker_inner`.
/// Preserves backward compatibility: max 10 connection attempts.
pub async fn run_worker(
    config: &NodeConfig,
    token: Option<&AuthToken>,
    transport: &mut dyn Transport,
) -> Result<(), ProtocolError> {
    run_worker_inner(config, token, Some(MAX_ATTEMPTS), transport).await
}

/// Runs the worker in daemon mode: reconnects after each job (SPEC-16 R3).
///
/// Loops forever (until Ctrl+C) calling `run_worker_inner` with infinite
/// retries. After each job, waits a brief delay before reconnecting.
pub async fn run_worker_daemon(
    config: &NodeConfig,
    token: Option<&AuthToken>,
    transport: &mut dyn Transport,
) -> Result<(), ProtocolError> {
    let mut job_number: u64 = 0;

    loop {
        job_number += 1;
        tracing::info!("Daemon job #{}: connecting to coordinator...", job_number);

        tokio::select! {
            result = run_worker_inner(config, token, None, transport) => {
                match result {
                    Ok(()) => {
                        tracing::info!(
                            "Daemon job #{}: complete. Reconnecting in {:?}...",
                            job_number, DAEMON_SUCCESS_DELAY
                        );
                        tokio::time::sleep(DAEMON_SUCCESS_DELAY).await;
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Daemon job #{}: failed: {}. Reconnecting in {:?}...",
                            job_number, e, DAEMON_FAILURE_DELAY
                        );
                        tokio::time::sleep(DAEMON_FAILURE_DELAY).await;
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!(
                    "Ctrl+C received. Shutting down daemon after {} job(s).",
                    job_number - 1
                );
                break;
            }
        }
    }

    Ok(())
}

/// Internal worker loop with configurable connection attempts (SPEC-16).
///
/// Implements the worker FSM (SPEC-13 R24):
/// Init -> Idle -> Reducing -> Returning -> Idle -> ... -> Done.
///
/// After connecting, sends a Register message (SPEC-10 R14). If auth
/// is required, includes the token bytes. Waits for RegisterAck before
/// entering the main loop.
async fn run_worker_inner(
    config: &NodeConfig,
    token: Option<&AuthToken>,
    max_connect_attempts: Option<u32>,
    transport: &mut dyn Transport,
) -> Result<(), ProtocolError> {
    let mut stream = connect_with_retry(transport, max_connect_attempts).await?;

    // Send Register (SPEC-10 R14)
    let register = Message::Register(RegisterPayload {
        protocol_version: PROTOCOL_VERSION,
        auth_token: token.map(|t| *t.as_bytes()),
    });
    send_frame(&mut stream, &register).await?;

    // Wait for RegisterAck/RegisterNack
    let (response, _) = recv_frame(&mut stream, config.max_payload_size).await?;
    match response {
        Message::RegisterAck(ack) => {
            tracing::info!("Registered with coordinator as worker_id={}", ack.worker_id);
        }
        Message::RegisterNack(nack) => {
            tracing::error!("Registration rejected: {}", nack.reason);
            // SPEC-18 R30: a coordinator-issued nack carrying the canonical
            // "protocol version mismatch" phrase must surface as
            // `VersionMismatch` so daemon-mode workers fail fast (no point
            // retrying against an incompatible peer). Other nacks remain
            // `AuthFailed` for backwards compatibility with SPEC-10.
            if let Some(received) = parse_version_mismatch_nack(&nack.reason) {
                return Err(ProtocolError::VersionMismatch {
                    expected: PROTOCOL_VERSION,
                    received,
                });
            }
            return Err(ProtocolError::AuthFailed);
        }
        other => {
            return Err(ProtocolError::UnexpectedMessage {
                expected: "RegisterAck or RegisterNack",
                received: format!("{:?}", other),
            });
        }
    }

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

                // Reconstruct free_port_index (SPEC-05, Section 4.3).
                // SPEC-19 R1 ordering: rebuild_free_port_index MUST run
                // before compute_border_activity below — the activity
                // flag reflects the post-reduction, post-rebuild state.
                partition.free_port_index = rebuild_free_port_index(
                    &partition.subnet,
                    partition.border_id_start,
                    partition.border_id_end,
                );

                // SPEC-19 R2: report whether any local border endpoint is
                // a principal port. The coordinator uses this (TASK-0351,
                // out of scope for this wire path until Delta-Only
                // Protocol ships) to skip merge when every worker reports
                // false. Computed AFTER rebuild_free_port_index (R1).
                let has_border_activity = compute_border_activity(&partition);

                let stats = WorkerRoundStats {
                    worker_id: partition.worker_id,
                    agents_before,
                    agents_after,
                    local_redexes: reduction_stats.total_interactions as usize,
                    reduce_duration_secs: reduce_duration.as_secs_f64(),
                    interactions_by_rule: reduction_stats.interactions_by_rule,
                    has_border_activity,
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
    use crate::protocol::channel::ChannelTransport;
    use crate::protocol::config::TransportConfig;
    use crate::protocol::tcp::TcpTransport;
    use crate::protocol::types::{RegisterAckPayload, RegisterNackPayload};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

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

    // T2: connect_with_retry succeeds on first attempt (ChannelTransport)
    #[tokio::test]
    async fn test_connect_with_retry_immediate_success() {
        let (_server, mut client) = ChannelTransport::pair(1, 65536);
        let result = connect_with_retry(&mut client, Some(10)).await;
        assert!(result.is_ok());
    }

    /// Simulate coordinator handshake: receive Register, send RegisterAck.
    async fn coordinator_handshake<S: AsyncReadExt + AsyncWriteExt + Unpin>(
        stream: &mut S,
        worker_id: u32,
    ) {
        let max = NodeConfig::default().max_payload_size;
        let (msg, _) = recv_frame(stream, max).await.unwrap();
        assert!(matches!(msg, Message::Register(_)));
        let ack = Message::RegisterAck(RegisterAckPayload { worker_id });
        send_frame(stream, &ack).await.unwrap();
    }

    // T3: run_worker processes AssignPartition + Shutdown (ChannelTransport)
    #[tokio::test]
    async fn test_run_worker_single_round() {
        use crate::net::{Net, PortRef, Symbol};
        use crate::partition::{IdRange, Partition};
        use std::collections::HashMap;

        let (mut server, mut client) = ChannelTransport::pair(1, 65536);
        let max_payload = NodeConfig::default().max_payload_size;
        let config = NodeConfig::default();

        // Spawn the worker (Tier 1, no token)
        let worker_handle =
            tokio::spawn(async move { run_worker(&config, None, &mut client).await });

        // Accept connection as "coordinator"
        let mut stream = server.accept().await.unwrap();

        // Complete registration handshake
        coordinator_handshake(&mut stream, 0).await;

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

    // T4: run_worker handles multiple rounds (ChannelTransport)
    #[tokio::test]
    async fn test_run_worker_multiple_rounds() {
        use crate::net::Net;
        use crate::partition::{IdRange, Partition};
        use std::collections::HashMap;

        let (mut server, mut client) = ChannelTransport::pair(1, 65536);
        let config = NodeConfig::default();

        let worker_handle =
            tokio::spawn(async move { run_worker(&config, None, &mut client).await });
        let mut stream = server.accept().await.unwrap();

        // Complete registration handshake
        coordinator_handshake(&mut stream, 0).await;

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

    // T5: run_worker returns UnexpectedMessage for wrong message type (ChannelTransport)
    #[tokio::test]
    async fn test_run_worker_unexpected_message() {
        use crate::net::Net;
        use crate::partition::{IdRange, Partition};
        use std::collections::HashMap;

        let (mut server, mut client) = ChannelTransport::pair(1, 65536);
        let config = NodeConfig::default();

        let worker_handle =
            tokio::spawn(async move { run_worker(&config, None, &mut client).await });
        let mut stream = server.accept().await.unwrap();

        // Complete registration handshake
        coordinator_handshake(&mut stream, 0).await;

        // Send a PartitionResult (wrong direction — coordinator->worker should
        // only send AssignPartition or Shutdown)
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
                has_border_activity: false,
            },
        };
        send_frame(&mut stream, &bad_msg).await.unwrap();

        let result = worker_handle.await.unwrap();
        assert!(matches!(
            result,
            Err(ProtocolError::UnexpectedMessage { .. })
        ));
    }

    // TASK-0347 R3: worker receives a version-mismatch RegisterNack and
    // surfaces ProtocolError::VersionMismatch (NOT AuthFailed). This lets
    // daemon-mode workers fail fast against an incompatible peer.
    #[tokio::test]
    async fn worker_terminates_on_version_mismatch_nack() {
        let (mut server, mut client) = ChannelTransport::pair(1, 65536);
        let config = NodeConfig::default();

        let worker_handle =
            tokio::spawn(async move { run_worker(&config, None, &mut client).await });
        let mut stream = server.accept().await.unwrap();

        // Drain the worker's Register, then reply with the canonical NACK
        // (matches the format coordinator::accept_workers emits).
        let max = NodeConfig::default().max_payload_size;
        let (msg, _) = recv_frame(&mut stream, max).await.unwrap();
        assert!(matches!(msg, Message::Register(_)));

        let nack = Message::RegisterNack(RegisterNackPayload {
            reason: "protocol version mismatch: expected 2, got 1".into(),
        });
        send_frame(&mut stream, &nack).await.unwrap();

        let result = worker_handle.await.unwrap();
        match result {
            Err(ProtocolError::VersionMismatch { expected, received }) => {
                assert_eq!(expected, PROTOCOL_VERSION);
                assert_eq!(received, 1);
            }
            other => panic!(
                "worker did not terminate with VersionMismatch, got {:?}",
                other
            ),
        }
    }

    // TASK-0347 R3 (negative case): non-version nacks must still surface as
    // AuthFailed so SPEC-10's tier-2/3 contract is preserved.
    #[tokio::test]
    async fn worker_returns_auth_failed_for_non_version_nack() {
        let (mut server, mut client) = ChannelTransport::pair(1, 65536);
        let config = NodeConfig::default();

        let worker_handle =
            tokio::spawn(async move { run_worker(&config, None, &mut client).await });
        let mut stream = server.accept().await.unwrap();

        let max = NodeConfig::default().max_payload_size;
        let (msg, _) = recv_frame(&mut stream, max).await.unwrap();
        assert!(matches!(msg, Message::Register(_)));

        let nack = Message::RegisterNack(RegisterNackPayload {
            reason: "authentication failed".into(),
        });
        send_frame(&mut stream, &nack).await.unwrap();

        let result = worker_handle.await.unwrap();
        assert!(matches!(result, Err(ProtocolError::AuthFailed)));
    }

    // TASK-0347 R3 unit: version-mismatch parser handles the canonical phrase
    // and rejects non-matching reasons. Pure helper, no async.
    #[test]
    fn parse_version_mismatch_nack_recognises_canonical_phrase() {
        assert_eq!(
            super::parse_version_mismatch_nack("protocol version mismatch: expected 2, got 1"),
            Some(1)
        );
        assert_eq!(
            super::parse_version_mismatch_nack("protocol version mismatch: expected 2, got 17"),
            Some(17)
        );
        assert_eq!(
            super::parse_version_mismatch_nack("authentication failed"),
            None
        );
        // Phrase present but received-version unparseable → None.
        assert_eq!(
            super::parse_version_mismatch_nack("protocol version mismatch: garbage"),
            None
        );
    }

    // === SPEC-18 §3.6 — QA stage adversarial probe ===

    /// QA probe #6: a stray `RegisterNack` arriving mid-stream (after a
    /// successful handshake, while the worker is awaiting `AssignPartition`
    /// or `Shutdown`) must surface as `UnexpectedMessage`. Guards against
    /// the worker silently treating a late nack as a fatal auth failure
    /// and against any panic on an unmatched arm.
    #[tokio::test]
    async fn qa_probe_6_stray_nack_mid_stream_surfaces_unexpected_message() {
        let (mut server, mut client) = ChannelTransport::pair(1, 65536);
        let config = NodeConfig::default();

        let worker_handle =
            tokio::spawn(async move { run_worker(&config, None, &mut client).await });
        let mut stream = server.accept().await.unwrap();

        // Successful handshake — worker now in main loop expecting Assign/Shutdown.
        coordinator_handshake(&mut stream, 0).await;

        // Stray nack mid-stream.
        let nack = Message::RegisterNack(RegisterNackPayload {
            reason: "stray nack mid-stream".into(),
        });
        send_frame(&mut stream, &nack).await.unwrap();

        let result = worker_handle.await.unwrap();
        assert!(
            matches!(result, Err(ProtocolError::UnexpectedMessage { .. })),
            "stray nack must surface as UnexpectedMessage, got {:?}",
            result,
        );
    }

    // T6: run_worker receives RegisterNack and returns AuthFailed (ChannelTransport)
    #[tokio::test]
    async fn test_run_worker_auth_rejected() {
        let (mut server, mut client) = ChannelTransport::pair(1, 65536);
        let config = NodeConfig::default();

        let worker_handle =
            tokio::spawn(async move { run_worker(&config, None, &mut client).await });
        let mut stream = server.accept().await.unwrap();

        // Read Register, then send RegisterNack
        let max = NodeConfig::default().max_payload_size;
        let (msg, _) = recv_frame(&mut stream, max).await.unwrap();
        assert!(matches!(msg, Message::Register(_)));

        let nack = Message::RegisterNack(RegisterNackPayload {
            reason: "authentication failed".into(),
        });
        send_frame(&mut stream, &nack).await.unwrap();

        let result = worker_handle.await.unwrap();
        assert!(matches!(result, Err(ProtocolError::AuthFailed)));
    }

    // T7: connect_with_retry with max_attempts=Some(1) fails (TcpTransport — needs real failure)
    #[tokio::test]
    async fn test_connect_with_retry_max_one() {
        // Connect to a port where nothing is listening
        let addr: std::net::SocketAddr = "127.0.0.1:1".parse().unwrap();
        let mut transport = TcpTransport::new(addr, TransportConfig::default());
        let result = connect_with_retry(&mut transport, Some(1)).await;
        assert!(result.is_err());
        match result {
            Err(ProtocolError::ConnectionLost(_)) => {} // expected
            Ok(_) => panic!("expected ConnectionLost, got Ok"),
            Err(e) => panic!("expected ConnectionLost, got {:?}", e),
        }
    }

    // T8: connect_with_retry with None succeeds when listener appears (TcpTransport)
    #[tokio::test]
    async fn test_connect_with_retry_infinite_succeeds() {
        // Bind to get a port, then drop the listener to make it initially unavailable
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);

        // Spawn connector with infinite retries
        let mut transport = TcpTransport::new(addr, TransportConfig::default());
        let connect_handle =
            tokio::spawn(async move { connect_with_retry(&mut transport, None).await });

        // Wait a bit, then start listening again
        tokio::time::sleep(Duration::from_millis(500)).await;
        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        let (_stream, _) = listener.accept().await.unwrap();

        let result = connect_handle.await.unwrap();
        assert!(result.is_ok());
    }

    // T9: daemon completes 2 sequential jobs (ChannelTransport)
    #[tokio::test]
    async fn test_daemon_two_sequential_jobs() {
        use crate::net::Net;
        use crate::partition::{IdRange, Partition};
        use std::collections::HashMap;

        let (mut server, mut client) = ChannelTransport::pair(2, 65536);
        let config = NodeConfig::default();

        // Spawn daemon worker (2 jobs via run_worker_inner to avoid ctrl_c complexity)
        let daemon_handle = tokio::spawn(async move {
            for _ in 0..2 {
                run_worker_inner(&config, None, Some(MAX_ATTEMPTS), &mut client)
                    .await
                    .unwrap();
            }
        });

        for job in 0..2u32 {
            // Accept connection
            let mut stream = server.accept().await.unwrap();

            // Handshake
            coordinator_handshake(&mut stream, job).await;

            // Send one empty partition + shutdown
            let partition = Partition {
                subnet: Net::new(),
                worker_id: job,
                free_port_index: HashMap::new(),
                id_range: IdRange {
                    start: 0,
                    end: 100_000,
                },
                border_id_start: 0,
                border_id_end: 0,
            };
            send_frame(
                &mut stream,
                &Message::AssignPartition {
                    round: 0,
                    partition,
                },
            )
            .await
            .unwrap();

            // Receive result
            let (response, _) = recv_frame(&mut stream, NodeConfig::default().max_payload_size)
                .await
                .unwrap();
            assert!(matches!(response, Message::PartitionResult { .. }));

            // Shutdown
            send_frame(&mut stream, &Message::Shutdown).await.unwrap();
        }

        // Daemon should complete both jobs and exit
        let result = daemon_handle.await;
        assert!(result.is_ok());
    }
}
