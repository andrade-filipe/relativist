//! Coordinator logic (SPEC-06, Sections 4.6, 4.12).
//!
//! Implements the coordinator side of the distributed grid loop:
//! accept workers, distribute partitions, collect results, shutdown.

use std::time::{Duration, Instant};

use super::transport::{Transport, TransportStream};
use futures::future::join_all;

use super::config::NodeConfig;
use super::error::ProtocolError;
use super::frame::{recv_frame, send_frame};
use super::types::{Message, RegisterAckPayload, RegisterNackPayload};
use crate::merge::{drain_stale_redexes, merge, GridConfig, GridMetrics, WorkerRoundStats};
use crate::partition::{split, Partition, PartitionStrategy};
use crate::reduction::reduce_all;
use crate::security::AuthToken;

// ---------------------------------------------------------------------------
// Phase 0: Accept workers (TASK-0088)
// ---------------------------------------------------------------------------

/// Current wire protocol version for Register handshake (SPEC-10 R36, SPEC-18 R28).
///
/// Bumped 1 → 2 by TASK-0347 to mark the atomic v2 wire break:
/// bincode v2 + Compact PortRef + 9-byte frame header + LZ4. v1 workers
/// must be rejected with a `RegisterNack` whose reason carries the
/// canonical phrasing parsed by `worker::run_worker_inner` to surface
/// `ProtocolError::VersionMismatch` (item 2.23 §3.6).
pub const PROTOCOL_VERSION: u8 = 2;

/// Accepts and authenticates workers (SPEC-06 R17, R24; SPEC-10 R14-R17).
///
/// Calls `transport.listen()` then waits for `config.num_workers` connections,
/// performs the Register/RegisterAck handshake with optional token
/// validation, and returns authenticated streams (SPEC-17 R35).
///
/// - Tier 1 (token=None): accepts Register without checking auth_token.
/// - Tier 2/3 (token=Some): validates auth_token with constant-time
///   comparison; rejects with RegisterNack on failure (SPEC-10 R16).
pub async fn accept_workers(
    config: &NodeConfig,
    token: Option<&AuthToken>,
    transport: &mut dyn Transport,
) -> Result<Vec<TransportStream>, ProtocolError> {
    transport.listen().await?;

    tracing::info!("Coordinator listening on {}", config.bind);

    let mut streams = Vec::with_capacity(config.num_workers as usize);

    let accept_future = async {
        while streams.len() < config.num_workers as usize {
            let mut stream = transport.accept().await?;

            // Read Register message from worker (SPEC-10 R14)
            let (msg, _) = recv_frame(&mut stream, config.max_payload_size).await?;

            match msg {
                Message::Register(payload) => {
                    // Validate protocol version (SPEC-10 R36, SPEC-18 R29).
                    // The reason phrasing is contractual — `worker::run_worker_inner`
                    // parses it to distinguish version mismatches from auth failures
                    // and surface them as `ProtocolError::VersionMismatch`.
                    if payload.protocol_version != PROTOCOL_VERSION {
                        tracing::warn!(
                            version = payload.protocol_version,
                            "Rejected: protocol version mismatch"
                        );
                        // Wire literal mandated by SPEC-18 R29:
                        //   "protocol version mismatch: expected N, got M"
                        // The local Rust field on `ProtocolError::VersionMismatch`
                        // is named `received` (R35), and the Display impl renders
                        // "received M" — those are intentionally distinct from the
                        // on-wire word `got`. The worker's parser keys on `"got "`
                        // (see `worker::parse_version_mismatch_nack`).
                        let nack = Message::RegisterNack(RegisterNackPayload {
                            reason: format!(
                                "protocol version mismatch: expected {}, got {}",
                                PROTOCOL_VERSION, payload.protocol_version
                            ),
                        });
                        let _ = send_frame(&mut stream, &nack).await;
                        continue;
                    }

                    // Validate token if configured (SPEC-10 R15-R17)
                    if let Some(expected_token) = token {
                        match payload.auth_token {
                            Some(provided_bytes) => {
                                let provided = AuthToken::from_bytes(provided_bytes);
                                if !expected_token.verify(&provided) {
                                    tracing::warn!("Rejected: authentication failed");
                                    let nack = Message::RegisterNack(RegisterNackPayload {
                                        reason: "authentication failed".into(),
                                    });
                                    let _ = send_frame(&mut stream, &nack).await;
                                    continue;
                                }
                            }
                            None => {
                                tracing::warn!("Rejected: token required but not provided");
                                let nack = Message::RegisterNack(RegisterNackPayload {
                                    reason: "authentication failed".into(),
                                });
                                let _ = send_frame(&mut stream, &nack).await;
                                continue;
                            }
                        }
                    }

                    // Authentication passed — assign worker ID and send Ack
                    let worker_id = streams.len() as u32;
                    let ack = Message::RegisterAck(RegisterAckPayload { worker_id });
                    send_frame(&mut stream, &ack).await?;

                    tracing::info!(
                        "Worker {}/{} registered (id={})",
                        streams.len() + 1,
                        config.num_workers,
                        worker_id
                    );
                    streams.push(stream);
                }
                other => {
                    tracing::warn!("Rejected: expected Register, got {:?}", other);
                    let nack = Message::RegisterNack(RegisterNackPayload {
                        reason: "protocol error".into(),
                    });
                    let _ = send_frame(&mut stream, &nack).await;
                    continue;
                }
            }
        }
        Ok::<_, ProtocolError>(())
    };

    tokio::time::timeout(config.worker_connect_timeout, accept_future)
        .await
        .map_err(|_| ProtocolError::Timeout {
            phase: "accept_workers",
            elapsed: config.worker_connect_timeout,
        })??;

    Ok(streams)
}

// ---------------------------------------------------------------------------
// Phase 2a: Distribute partitions (TASK-0089)
// ---------------------------------------------------------------------------

/// Sends partitions concurrently to all workers (SPEC-06 R21).
///
/// Constructs a `Message::AssignPartition` for each (partition, stream) pair
/// and sends all via `send_frame` in parallel using `join_all`.
/// Returns the total bytes sent.
pub async fn distribute_partitions(
    worker_streams: &mut [TransportStream],
    partitions: Vec<Partition>,
    round: u32,
    distribute_timeout: Duration,
) -> Result<usize, ProtocolError> {
    // Build messages first (owned), then create futures that borrow them.
    let messages: Vec<Message> = partitions
        .into_iter()
        .map(|partition| Message::AssignPartition { round, partition })
        .collect();

    let mut send_futures = Vec::with_capacity(worker_streams.len());
    for (stream, msg) in worker_streams.iter_mut().zip(messages.iter()) {
        send_futures.push(send_frame(stream, msg));
    }

    let results = tokio::time::timeout(distribute_timeout, join_all(send_futures))
        .await
        .map_err(|_| ProtocolError::Timeout {
            phase: "distribute_partitions",
            elapsed: distribute_timeout,
        })?;

    let mut total_bytes = 0;
    for result in results {
        total_bytes += result?;
    }

    Ok(total_bytes)
}

// ---------------------------------------------------------------------------
// Phase 2b: Collect results (TASK-0090)
// ---------------------------------------------------------------------------

/// Collects partition results from all workers (SPEC-06 R22).
///
/// Reads one Message per worker via `recv_frame`, validates that each is
/// a `PartitionResult` with the expected round number, and returns
/// the reduced partitions with stats + total bytes received.
pub async fn collect_results(
    worker_streams: &mut [TransportStream],
    round: u32,
    max_payload_size: u32,
    collect_timeout: Duration,
) -> Result<(Vec<(Partition, WorkerRoundStats)>, usize), ProtocolError> {
    let collect_future = async {
        let mut results = Vec::with_capacity(worker_streams.len());
        let mut total_bytes = 0;

        for stream in worker_streams.iter_mut() {
            let (msg, nbytes) = recv_frame(stream, max_payload_size).await?;
            total_bytes += nbytes;

            match msg {
                Message::PartitionResult {
                    round: r,
                    partition,
                    stats,
                } => {
                    if r != round {
                        return Err(ProtocolError::UnexpectedMessage {
                            expected: "PartitionResult with matching round",
                            received: format!("PartitionResult round={} (expected {})", r, round),
                        });
                    }
                    results.push((partition, stats));
                }
                Message::Error {
                    worker_id,
                    round: r,
                    description,
                } => {
                    return Err(ProtocolError::UnexpectedMessage {
                        expected: "PartitionResult",
                        received: format!(
                            "Error from worker {} in round {}: {}",
                            worker_id, r, description
                        ),
                    });
                }
                other => {
                    return Err(ProtocolError::UnexpectedMessage {
                        expected: "PartitionResult",
                        received: format!("{:?}", other),
                    });
                }
            }
        }

        Ok((results, total_bytes))
    };

    tokio::time::timeout(collect_timeout, collect_future)
        .await
        .map_err(|_| ProtocolError::Timeout {
            phase: "collect_results",
            elapsed: collect_timeout,
        })?
}

// ---------------------------------------------------------------------------
// Shutdown (TASK-0091)
// ---------------------------------------------------------------------------

/// Sends Shutdown to all workers and closes connections (SPEC-06 Section 4.12).
///
/// Best-effort: individual send failures are logged, not propagated.
pub async fn shutdown_workers(worker_streams: &mut [TransportStream]) {
    for (i, stream) in worker_streams.iter_mut().enumerate() {
        if let Err(e) = send_frame(stream, &Message::Shutdown).await {
            tracing::warn!("Failed to send Shutdown to worker {}: {}", i, e);
        }
    }
    tracing::info!("Shutdown sent to all workers.");
}

// ---------------------------------------------------------------------------
// run_coordinator: distributed grid loop (TASK-0092)
// ---------------------------------------------------------------------------

/// Orchestrates the distributed grid loop as the coordinator (SPEC-06 Section 4.6).
///
/// Accepts worker connections, then iterates: partition -> distribute ->
/// collect -> merge until normal form or round limit. Sends Shutdown
/// to all workers on completion.
///
/// This is the distributed equivalent of `run_grid` (SPEC-05, Section 4.4).
pub async fn run_coordinator(
    net: crate::net::Net,
    config: &NodeConfig,
    grid_config: &GridConfig,
    strategy: &dyn PartitionStrategy,
    token: Option<&AuthToken>,
    transport: &mut dyn Transport,
) -> Result<(crate::net::Net, GridMetrics), ProtocolError> {
    // Phase 0: Accept and authenticate worker connections (SPEC-17 R35)
    let mut worker_streams = accept_workers(config, token, transport).await?;

    let mut current_net = net;
    let mut metrics = GridMetrics::default();
    let start_time = Instant::now();

    loop {
        // Check Normal Form
        drain_stale_redexes(&mut current_net);
        if current_net.redex_queue.is_empty() {
            metrics.converged = true;
            break;
        }

        // Check round limit
        if let Some(max) = grid_config.max_rounds {
            if metrics.rounds >= max {
                metrics.converged = false;
                break;
            }
        }

        metrics
            .agents_per_round
            .push(current_net.count_live_agents());

        // === PHASE 1: PARTITION ===
        let t_partition = Instant::now();
        let plan = split(current_net, config.num_workers, strategy);
        metrics.partition_time_per_round.push(t_partition.elapsed());

        // === PHASE 2a: DISTRIBUTE ===
        let t_send = Instant::now();
        let bytes_sent = distribute_partitions(
            &mut worker_streams,
            plan.partitions,
            metrics.rounds,
            config.distribute_timeout,
        )
        .await?;
        metrics.network_send_time_per_round.push(t_send.elapsed());
        metrics.bytes_sent_per_round.push(bytes_sent);

        // === PHASE 2b: COLLECT ===
        let t_recv = Instant::now();
        let (results, bytes_received) = collect_results(
            &mut worker_streams,
            metrics.rounds,
            config.max_payload_size,
            config.collect_timeout,
        )
        .await?;
        metrics.network_recv_time_per_round.push(t_recv.elapsed());
        metrics.bytes_received_per_round.push(bytes_received);

        // Separate partitions and stats
        let mut reduced_partitions = Vec::with_capacity(results.len());
        let mut worker_stats = Vec::with_capacity(results.len());
        for (partition, stats) in results {
            reduced_partitions.push(partition);
            worker_stats.push(stats);
        }
        metrics.worker_stats_per_round.push(worker_stats.clone());

        // === PHASE 3: MERGE + RESOLVE BORDERS ===
        let t_merge = Instant::now();
        let merge_plan = crate::partition::PartitionPlan {
            partitions: reduced_partitions,
            borders: plan.borders,
        };
        let (mut merged_net, border_redex_count) = merge(merge_plan);
        metrics.border_redexes_per_round.push(border_redex_count);

        // Accumulate local interactions from worker stats
        let local_interactions: u64 = worker_stats.iter().map(|s| s.local_redexes as u64).sum();
        metrics
            .local_interactions_per_round
            .push(local_interactions);

        // Accumulate per-rule interactions from workers
        for s in &worker_stats {
            for (i, &count) in s.interactions_by_rule.iter().enumerate() {
                metrics.total_interactions_by_rule[i] += count;
            }
        }

        // Resolve border redexes
        let t_border = Instant::now();
        let border_stats = reduce_all(&mut merged_net);
        let border_interactions = border_stats.total_interactions;
        metrics
            .border_reduce_time_per_round
            .push(t_border.elapsed());
        metrics
            .border_interactions_per_round
            .push(border_interactions);

        // Accumulate border per-rule
        for (i, &count) in border_stats.interactions_by_rule.iter().enumerate() {
            metrics.total_interactions_by_rule[i] += count;
        }

        metrics.merge_time_per_round.push(t_merge.elapsed());
        metrics.total_interactions += local_interactions + border_interactions;

        current_net = merged_net;
        metrics.rounds += 1;
    }

    // SHUTDOWN
    shutdown_workers(&mut worker_streams).await;

    metrics.total_time = start_time.elapsed();
    Ok((current_net, metrics))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merge::WorkerRoundStats;
    use crate::net::{Net, PortRef, Symbol};
    use crate::partition::{ContiguousIdStrategy, IdRange, Partition, WorkerId};
    use crate::protocol::channel::ChannelTransport;
    use crate::protocol::config::TransportConfig;
    use crate::protocol::tcp::TcpTransport;
    use crate::protocol::types::RegisterPayload;
    use std::collections::HashMap;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// Send a Tier 1 (no-auth) Register message from a simulated worker.
    async fn send_register<W: AsyncWriteExt + Unpin>(stream: &mut W) {
        let register = Message::Register(RegisterPayload {
            protocol_version: PROTOCOL_VERSION,
            auth_token: None,
        });
        send_frame(stream, &register).await.unwrap();
    }

    /// Read and assert RegisterAck from coordinator.
    async fn expect_register_ack<R: AsyncReadExt + Unpin>(stream: &mut R) -> u32 {
        let (msg, _) = recv_frame(stream, NodeConfig::default().max_payload_size)
            .await
            .unwrap();
        match msg {
            Message::RegisterAck(ack) => ack.worker_id,
            other => panic!("expected RegisterAck, got {:?}", other),
        }
    }

    // T1: accept_workers with N workers registering (Tier 1, no auth)
    // CW2: uses ChannelTransport instead of raw TCP (SPEC-17 R38)
    #[tokio::test]
    async fn test_accept_workers_success() {
        let (mut server, mut client) = ChannelTransport::pair(2, 65536);
        let config = NodeConfig {
            num_workers: 2,
            worker_connect_timeout: Duration::from_secs(5),
            ..NodeConfig::default()
        };

        let accept_handle = tokio::spawn({
            let config = config.clone();
            async move { accept_workers(&config, None, &mut server).await }
        });

        // Connect 2 workers and send Register
        let mut c1 = client.connect().await.unwrap();
        send_register(&mut c1).await;
        let id1 = expect_register_ack(&mut c1).await;

        let mut c2 = client.connect().await.unwrap();
        send_register(&mut c2).await;
        let id2 = expect_register_ack(&mut c2).await;

        assert_eq!(id1, 0);
        assert_eq!(id2, 1);

        let result = accept_handle.await.unwrap();
        assert!(result.is_ok());
        let streams = result.unwrap();
        assert_eq!(streams.len(), 2);
    }

    // T2: accept_workers timeout with fewer workers (TCP — needs real blocking accept)
    #[tokio::test]
    async fn test_accept_workers_timeout() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);

        let config = NodeConfig {
            bind: addr,
            num_workers: 3,
            worker_connect_timeout: Duration::from_millis(200),
            ..NodeConfig::default()
        };

        let mut transport = TcpTransport::new(addr, TransportConfig::default());
        let result = accept_workers(&config, None, &mut transport).await;
        assert!(matches!(result, Err(ProtocolError::Timeout { .. })));
    }

    // T2b: accept_workers with token auth — valid token accepted (ChannelTransport)
    #[tokio::test]
    async fn test_accept_workers_token_auth_success() {
        let token = AuthToken::generate();
        let (mut server, mut client) = ChannelTransport::pair(1, 65536);
        let config = NodeConfig {
            num_workers: 1,
            worker_connect_timeout: Duration::from_secs(5),
            ..NodeConfig::default()
        };

        let token_clone = token.clone();
        let accept_handle = tokio::spawn({
            let config = config.clone();
            async move { accept_workers(&config, Some(&token_clone), &mut server).await }
        });

        let mut w = client.connect().await.unwrap();
        let register = Message::Register(RegisterPayload {
            protocol_version: PROTOCOL_VERSION,
            auth_token: Some(*token.as_bytes()),
        });
        send_frame(&mut w, &register).await.unwrap();
        let id = expect_register_ack(&mut w).await;
        assert_eq!(id, 0);

        let result = accept_handle.await.unwrap();
        assert!(result.is_ok());
    }

    // T2c: accept_workers with token auth — wrong token rejected (ChannelTransport)
    // Uses 2 channels: first is rejected, second blocks recv_frame → timeout
    #[tokio::test]
    async fn test_accept_workers_token_auth_rejected() {
        let token = AuthToken::generate();
        let wrong_token = AuthToken::generate();
        let (mut server, mut client) = ChannelTransport::pair(2, 65536);
        let config = NodeConfig {
            num_workers: 1,
            worker_connect_timeout: Duration::from_millis(500),
            ..NodeConfig::default()
        };

        let token_clone = token.clone();
        let accept_handle = tokio::spawn({
            let config = config.clone();
            async move { accept_workers(&config, Some(&token_clone), &mut server).await }
        });

        // Send wrong token — should get RegisterNack
        let mut w = client.connect().await.unwrap();
        let register = Message::Register(RegisterPayload {
            protocol_version: PROTOCOL_VERSION,
            auth_token: Some(*wrong_token.as_bytes()),
        });
        send_frame(&mut w, &register).await.unwrap();

        let (msg, _) = recv_frame(&mut w, NodeConfig::default().max_payload_size)
            .await
            .unwrap();
        assert!(matches!(msg, Message::RegisterNack(_)));

        // Coordinator accepts second channel stream, blocks on recv_frame → timeout
        let result = accept_handle.await.unwrap();
        assert!(matches!(result, Err(ProtocolError::Timeout { .. })));
    }

    // === TASK-0347: PROTOCOL_VERSION bump 1 → 2 ===

    // TASK-0347 R1: canary against accidental rollback during merge conflicts.
    #[test]
    fn protocol_version_is_two() {
        assert_eq!(
            PROTOCOL_VERSION, 2,
            "v2 wire format requires PROTOCOL_VERSION = 2"
        );
    }

    // TASK-0347 R2: coordinator rejects v1 worker with a RegisterNack whose
    // reason carries the canonical phrase the worker keys on (SPEC-18 R29).
    #[tokio::test]
    async fn coordinator_rejects_v1_worker_with_register_nack() {
        let (mut server, mut client) = ChannelTransport::pair(2, 65536);
        let config = NodeConfig {
            num_workers: 1,
            worker_connect_timeout: Duration::from_millis(500),
            ..NodeConfig::default()
        };

        let accept_handle = tokio::spawn({
            let config = config.clone();
            async move { accept_workers(&config, None, &mut server).await }
        });

        let mut w = client.connect().await.unwrap();
        let v1_register = Message::Register(RegisterPayload {
            protocol_version: 1, // <-- v1 client against v2 coordinator
            auth_token: None,
        });
        send_frame(&mut w, &v1_register).await.unwrap();

        let (response, _) = recv_frame(&mut w, NodeConfig::default().max_payload_size)
            .await
            .unwrap();
        let nack = match response {
            Message::RegisterNack(p) => p,
            other => panic!("expected RegisterNack, got {:?}", other),
        };
        assert!(
            nack.reason.contains("protocol version mismatch"),
            "reason missing version-mismatch phrase: {}",
            nack.reason
        );
        assert!(
            nack.reason.contains("expected 2"),
            "expected version absent: {}",
            nack.reason
        );
        assert!(
            nack.reason.contains("got 1"),
            "received version absent (R29 wire literal uses 'got'): {}",
            nack.reason
        );

        // Coordinator continues waiting on a second slot → times out.
        let result = accept_handle.await.unwrap();
        assert!(matches!(result, Err(ProtocolError::Timeout { .. })));
    }

    // === SPEC-18 §3.6 — QA stage adversarial probes ===

    /// QA probe #5: spec R28-R29 only call out v1-vs-v2; verify a
    /// `protocol_version = 0` payload is rejected with the same canonical
    /// nack so the worker-side parser still surfaces `VersionMismatch`.
    /// (Guards against an asymmetric branch that would only fire for `1`.)
    #[tokio::test]
    async fn qa_probe_5_v0_register_rejected_with_canonical_nack() {
        let (mut server, mut client) = ChannelTransport::pair(2, 65536);
        let config = NodeConfig {
            num_workers: 1,
            worker_connect_timeout: Duration::from_millis(500),
            ..NodeConfig::default()
        };

        let accept_handle = tokio::spawn({
            let config = config.clone();
            async move { accept_workers(&config, None, &mut server).await }
        });

        let mut w = client.connect().await.unwrap();
        let v0_register = Message::Register(RegisterPayload {
            protocol_version: 0,
            auth_token: None,
        });
        send_frame(&mut w, &v0_register).await.unwrap();

        let (response, _) = recv_frame(&mut w, NodeConfig::default().max_payload_size)
            .await
            .unwrap();
        let nack = match response {
            Message::RegisterNack(p) => p,
            other => panic!("expected RegisterNack for v0, got {:?}", other),
        };
        assert!(
            nack.reason.contains("protocol version mismatch"),
            "missing canonical phrase: {}",
            nack.reason,
        );
        assert!(
            nack.reason.contains("expected 2"),
            "expected version absent: {}",
            nack.reason,
        );
        assert!(
            nack.reason.contains("got 0"),
            "received version absent (R29 wire literal uses 'got'): {}",
            nack.reason,
        );

        // Coordinator continues waiting on the slot → times out.
        let result = accept_handle.await.unwrap();
        assert!(matches!(result, Err(ProtocolError::Timeout { .. })));
    }

    /// QA probe #9: when a v1 worker arrives first and a v2 worker arrives
    /// second, the v1 connection is nacked while the v2 one ACKs and fills
    /// the only slot. Exercises the rejection-then-accept loop in
    /// `accept_workers` without time-of-check race.
    #[tokio::test]
    async fn qa_probe_9_v1_then_v2_workers_v1_nacked_v2_acked() {
        let (mut server, mut client) = ChannelTransport::pair(2, 65536);
        let config = NodeConfig {
            num_workers: 1,
            worker_connect_timeout: Duration::from_secs(5),
            ..NodeConfig::default()
        };

        let accept_handle = tokio::spawn({
            let config = config.clone();
            async move { accept_workers(&config, None, &mut server).await }
        });

        // First connector: v1 → must get a nack.
        let mut v1 = client.connect().await.unwrap();
        let v1_register = Message::Register(RegisterPayload {
            protocol_version: 1,
            auth_token: None,
        });
        send_frame(&mut v1, &v1_register).await.unwrap();
        let (response, _) = recv_frame(&mut v1, NodeConfig::default().max_payload_size)
            .await
            .unwrap();
        assert!(
            matches!(response, Message::RegisterNack(_)),
            "v1 first must be nacked, got {:?}",
            response,
        );

        // Second connector: v2 → must get an ack and consume the only slot.
        let mut v2 = client.connect().await.unwrap();
        send_register(&mut v2).await; // sends PROTOCOL_VERSION (== 2)
        let id = expect_register_ack(&mut v2).await;
        assert_eq!(id, 0, "v2 must take slot 0 after v1 was nacked");

        let result = accept_handle.await.unwrap();
        match result {
            Ok(streams) => assert_eq!(streams.len(), 1, "v2 must occupy the only slot"),
            Err(e) => panic!(
                "coordinator must accept v2 after rejecting v1, got error {:?}",
                e
            ),
        }
    }

    // TASK-0347 R7 smoke: a v2 coordinator + v2 worker handshake still ACKs.
    // Guards against regressing the happy path while wiring the version check.
    #[tokio::test]
    async fn smoke_v2_coordinator_v2_worker_handshake_succeeds() {
        let (mut server, mut client) = ChannelTransport::pair(1, 65536);
        let config = NodeConfig {
            num_workers: 1,
            worker_connect_timeout: Duration::from_secs(5),
            ..NodeConfig::default()
        };

        let accept_handle = tokio::spawn({
            let config = config.clone();
            async move { accept_workers(&config, None, &mut server).await }
        });

        let mut w = client.connect().await.unwrap();
        send_register(&mut w).await; // sends PROTOCOL_VERSION (== 2)

        let (response, _) = recv_frame(&mut w, NodeConfig::default().max_payload_size)
            .await
            .unwrap();
        assert!(
            matches!(response, Message::RegisterAck(_)),
            "v2/v2 handshake should ACK, got {:?}",
            response
        );
        assert!(accept_handle.await.unwrap().is_ok());
    }

    // T2d: accept_workers rejects missing token when auth required (ChannelTransport)
    #[tokio::test]
    async fn test_accept_workers_token_missing_rejected() {
        let token = AuthToken::generate();
        let (mut server, mut client) = ChannelTransport::pair(2, 65536);
        let config = NodeConfig {
            num_workers: 1,
            worker_connect_timeout: Duration::from_millis(500),
            ..NodeConfig::default()
        };

        let token_clone = token.clone();
        let accept_handle = tokio::spawn({
            let config = config.clone();
            async move { accept_workers(&config, Some(&token_clone), &mut server).await }
        });

        // Send Register without token — should get RegisterNack
        let mut w = client.connect().await.unwrap();
        send_register(&mut w).await; // sends auth_token: None

        let (msg, _) = recv_frame(&mut w, NodeConfig::default().max_payload_size)
            .await
            .unwrap();
        assert!(matches!(msg, Message::RegisterNack(_)));

        let result = accept_handle.await.unwrap();
        assert!(matches!(result, Err(ProtocolError::Timeout { .. })));
    }

    // T3: shutdown_workers sends Shutdown to all (ChannelTransport)
    #[tokio::test]
    async fn test_shutdown_workers() {
        let (mut server, mut client) = ChannelTransport::pair(2, 65536);

        let s1 = server.accept().await.unwrap();
        let s2 = server.accept().await.unwrap();
        let mut c1 = client.connect().await.unwrap();
        let mut c2 = client.connect().await.unwrap();

        let mut streams = vec![s1, s2];
        shutdown_workers(&mut streams).await;

        // Workers should receive Shutdown
        let (msg1, _) = recv_frame(&mut c1, NodeConfig::default().max_payload_size)
            .await
            .unwrap();
        let (msg2, _) = recv_frame(&mut c2, NodeConfig::default().max_payload_size)
            .await
            .unwrap();
        assert!(matches!(msg1, Message::Shutdown));
        assert!(matches!(msg2, Message::Shutdown));
    }

    // T4: distribute_partitions sends to all workers concurrently (ChannelTransport)
    #[tokio::test]
    async fn test_distribute_partitions() {
        let (mut server, mut client) = ChannelTransport::pair(2, 65536);

        let s1 = server.accept().await.unwrap();
        let s2 = server.accept().await.unwrap();
        let mut c1 = client.connect().await.unwrap();
        let mut c2 = client.connect().await.unwrap();

        let mut coordinator_streams = vec![s1, s2];

        let partitions = vec![
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
            },
            Partition {
                subnet: Net::new(),
                worker_id: 1,
                free_port_index: HashMap::new(),
                id_range: IdRange {
                    start: 100_000,
                    end: 200_000,
                },
                border_id_start: 0,
                border_id_end: 0,
            },
        ];

        let bytes = distribute_partitions(
            &mut coordinator_streams,
            partitions,
            42,
            Duration::from_secs(5),
        )
        .await
        .unwrap();
        assert!(bytes > 0);

        // Workers receive
        let max = NodeConfig::default().max_payload_size;
        let (msg1, _) = recv_frame(&mut c1, max).await.unwrap();
        let (msg2, _) = recv_frame(&mut c2, max).await.unwrap();

        match msg1 {
            Message::AssignPartition { round, partition } => {
                assert_eq!(round, 42);
                assert_eq!(partition.worker_id, 0);
            }
            _ => panic!("expected AssignPartition"),
        }
        match msg2 {
            Message::AssignPartition { round, partition } => {
                assert_eq!(round, 42);
                assert_eq!(partition.worker_id, 1);
            }
            _ => panic!("expected AssignPartition"),
        }
    }

    // T5: collect_results receives PartitionResults from workers (ChannelTransport)
    #[tokio::test]
    async fn test_collect_results() {
        let (mut server, mut client) = ChannelTransport::pair(2, 65536);

        let s1 = server.accept().await.unwrap();
        let s2 = server.accept().await.unwrap();
        let mut c1 = client.connect().await.unwrap();
        let mut c2 = client.connect().await.unwrap();

        let mut streams = vec![s1, s2];

        // Workers send results
        for (i, w) in [&mut c1, &mut c2].iter_mut().enumerate() {
            let msg = Message::PartitionResult {
                round: 0,
                partition: Partition {
                    subnet: Net::new(),
                    worker_id: i as WorkerId,
                    free_port_index: HashMap::new(),
                    id_range: IdRange {
                        start: 0,
                        end: 100_000,
                    },
                    border_id_start: 0,
                    border_id_end: 0,
                },
                stats: WorkerRoundStats {
                    worker_id: i as WorkerId,
                    agents_before: 10,
                    agents_after: 5,
                    local_redexes: 5,
                    reduce_duration_secs: 0.001,
                    interactions_by_rule: [1, 1, 1, 1, 1, 0],
                    has_border_activity: false,
                },
            };
            send_frame(*w, &msg).await.unwrap();
        }

        let (results, bytes) = collect_results(
            &mut streams,
            0,
            NodeConfig::default().max_payload_size,
            Duration::from_secs(5),
        )
        .await
        .unwrap();

        assert_eq!(results.len(), 2);
        assert!(bytes > 0);
        assert_eq!(results[0].1.worker_id, 0);
        assert_eq!(results[1].1.worker_id, 1);
    }

    // T6: collect_results rejects wrong round number (ChannelTransport)
    #[tokio::test]
    async fn test_collect_results_wrong_round() {
        let (mut server, mut client) = ChannelTransport::pair(1, 65536);

        let s1 = server.accept().await.unwrap();
        let mut c1 = client.connect().await.unwrap();
        let mut streams = vec![s1];

        let msg = Message::PartitionResult {
            round: 99, // Wrong round
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
        send_frame(&mut c1, &msg).await.unwrap();

        let result = collect_results(
            &mut streams,
            0, // Expected round 0
            NodeConfig::default().max_payload_size,
            Duration::from_secs(5),
        )
        .await;

        assert!(matches!(
            result,
            Err(ProtocolError::UnexpectedMessage { .. })
        ));
    }

    // T7: Full distributed G1 test — ERA-ERA via network (TcpTransport)
    #[tokio::test]
    async fn test_g1_distributed_era_era() {
        // Build a net with ERA(0) <-> ERA(1) — should annihilate to empty
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        // Sequential baseline
        let mut seq_net = net.clone();
        let _seq_stats = reduce_all(&mut seq_net);
        let seq_agents = seq_net.count_live_agents();

        // Distributed: coordinator with 1 worker
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);

        let config = NodeConfig {
            bind: addr,
            num_workers: 1,
            worker_connect_timeout: Duration::from_secs(5),
            ..NodeConfig::default()
        };
        let grid_config = GridConfig {
            num_workers: 1,
            max_rounds: None,
            ..GridConfig::default()
        };
        let strategy = ContiguousIdStrategy;

        // Spawn worker (Tier 1: no token)
        let worker_config = config.clone();
        let mut worker_transport = TcpTransport::new(addr, TransportConfig::default());
        let worker_handle = tokio::spawn(async move {
            crate::protocol::worker::run_worker(&worker_config, None, &mut worker_transport).await
        });

        // Run coordinator with TcpTransport
        let mut transport = TcpTransport::new(addr, TransportConfig::default());
        let (result_net, metrics) =
            run_coordinator(net, &config, &grid_config, &strategy, None, &mut transport)
                .await
                .unwrap();

        // Worker should have exited cleanly
        worker_handle.await.unwrap().unwrap();

        // G1: distributed result == sequential result
        assert_eq!(result_net.count_live_agents(), seq_agents);
        assert!(metrics.converged);
    }

    // T8: Full distributed G1 test — CON-CON via network with 2 workers (TcpTransport)
    #[tokio::test]
    async fn test_g1_distributed_con_con_2_workers() {
        // CON(0) <-> CON(1) with free aux ports
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(3));

        // Sequential baseline
        let mut seq_net = net.clone();
        reduce_all(&mut seq_net);
        let seq_agents = seq_net.count_live_agents();

        // Distributed with 2 workers
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);

        let config = NodeConfig {
            bind: addr,
            num_workers: 2,
            worker_connect_timeout: Duration::from_secs(5),
            ..NodeConfig::default()
        };
        let grid_config = GridConfig {
            num_workers: 2,
            max_rounds: None,
            ..GridConfig::default()
        };
        let strategy = ContiguousIdStrategy;

        // Spawn 2 workers (Tier 1: no token)
        let w1_config = config.clone();
        let w2_config = config.clone();
        let mut w1_transport = TcpTransport::new(addr, TransportConfig::default());
        let mut w2_transport = TcpTransport::new(addr, TransportConfig::default());
        let w1 = tokio::spawn(async move {
            crate::protocol::worker::run_worker(&w1_config, None, &mut w1_transport).await
        });
        let w2 = tokio::spawn(async move {
            crate::protocol::worker::run_worker(&w2_config, None, &mut w2_transport).await
        });

        let mut transport = TcpTransport::new(addr, TransportConfig::default());
        let (result_net, metrics) =
            run_coordinator(net, &config, &grid_config, &strategy, None, &mut transport)
                .await
                .unwrap();

        w1.await.unwrap().unwrap();
        w2.await.unwrap().unwrap();

        // G1: distributed == sequential
        assert_eq!(result_net.count_live_agents(), seq_agents);
        assert!(metrics.converged);
        assert!(metrics.total_interactions > 0);
    }

    // T9: Net already in normal form — 0 rounds (TcpTransport)
    #[tokio::test]
    async fn test_distributed_already_normal_form() {
        let net = Net::new(); // Empty net = normal form

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);

        let config = NodeConfig {
            bind: addr,
            num_workers: 1,
            worker_connect_timeout: Duration::from_secs(5),
            ..NodeConfig::default()
        };
        let grid_config = GridConfig {
            num_workers: 1,
            max_rounds: None,
            ..GridConfig::default()
        };
        let strategy = ContiguousIdStrategy;

        let worker_config = config.clone();
        let mut worker_transport = TcpTransport::new(addr, TransportConfig::default());
        let worker = tokio::spawn(async move {
            crate::protocol::worker::run_worker(&worker_config, None, &mut worker_transport).await
        });

        let mut transport = TcpTransport::new(addr, TransportConfig::default());
        let (result_net, metrics) =
            run_coordinator(net, &config, &grid_config, &strategy, None, &mut transport)
                .await
                .unwrap();

        worker.await.unwrap().unwrap();

        assert_eq!(result_net.count_live_agents(), 0);
        assert!(metrics.converged);
        assert_eq!(metrics.rounds, 0);
    }

    // T10: Full distributed with token auth (Tier 2, TcpTransport)
    #[tokio::test]
    async fn test_g1_distributed_with_token_auth() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        let token = AuthToken::generate();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);

        let config = NodeConfig {
            bind: addr,
            num_workers: 1,
            worker_connect_timeout: Duration::from_secs(5),
            ..NodeConfig::default()
        };
        let grid_config = GridConfig {
            num_workers: 1,
            max_rounds: None,
            ..GridConfig::default()
        };
        let strategy = ContiguousIdStrategy;

        let worker_config = config.clone();
        let worker_token = token.clone();
        let mut worker_transport = TcpTransport::new(addr, TransportConfig::default());
        let worker_handle = tokio::spawn(async move {
            crate::protocol::worker::run_worker(
                &worker_config,
                Some(&worker_token),
                &mut worker_transport,
            )
            .await
        });

        let mut transport = TcpTransport::new(addr, TransportConfig::default());
        let (result_net, metrics) = run_coordinator(
            net,
            &config,
            &grid_config,
            &strategy,
            Some(&token),
            &mut transport,
        )
        .await
        .unwrap();

        worker_handle.await.unwrap().unwrap();

        assert_eq!(result_net.count_live_agents(), 0);
        assert!(metrics.converged);
    }

    // UT-0351-09 (SPEC-19 §3.1 R7): wire-FSM untouched by the §3.1
    // bundle. R7 forbids new `Message::*` variants in this file for
    // bundle 2.34 — those belong to bundle 2.26 (Delta-Only Protocol).
    // Verified by source-grep against a frozen forbidden-variant list.
    //
    // Limitation: brittle to refactors but cheap and load-bearing —
    // it enforces a SCOPE DISCIPLINE, not a runtime property. If a
    // future bundle legitimately introduces any of these variants the
    // test moves with the spec change.
    //
    // The forbidden variant names are reconstructed from a prefix +
    // suffix split at runtime so the literal `Message::<Variant>`
    // substrings do NOT appear in this file (otherwise the
    // `include_str!`-based grep would match itself).
    #[test]
    fn ut_0351_09_coordinator_wire_fsm_untouched_by_3_1_bundle() {
        let src = include_str!("coordinator.rs");
        let prefix = ["Mes", "sage::"].concat(); // "Message::"
        let suffixes = [
            "RoundStart",
            "RoundResult",
            "FinalStateRequest",
            "FinalStateResult",
            "InitialPartition",
        ];
        for suffix in suffixes {
            let forbidden = format!("{prefix}{suffix}");
            assert!(
                !src.contains(&forbidden),
                "SPEC-19 §3.1 bundle MUST NOT introduce {forbidden}; \
                that belongs to §3.4 (item 2.26). R7 violation."
            );
        }
    }
}
