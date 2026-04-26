//! Coordinator logic (SPEC-06, Sections 4.6, 4.12).
//!
//! Implements the coordinator side of the distributed grid loop:
//! accept workers, distribute partitions, collect results, shutdown.
//!
//! ## SPEC-06 R25 amendment (SPEC-20 §3.8 A1)
//!
//! SPEC-06 R25 mandates that the coordinator abort the grid loop when a
//! worker connection is lost. SPEC-20 §3.8 A1 narrows this rule with a
//! conditional clause: *unless `GridConfig.elastic_departure = true`*, in
//! which case the `WaitingForResults × ConnectionLost(id)` and the
//! `WaitingForResults × PhaseTimeout(id)` transitions route to the elastic
//! recovery path (reclaim state, remove worker from `W_active`). The default
//! `elastic_departure = false` preserves v1 byte-identical behavior (TASK-0413).

use std::time::{Duration, Instant};

use super::transport::{Transport, TransportStream};
use futures::future::join_all;

use super::config::NodeConfig;
use super::error::ProtocolError;
use super::frame::{recv_frame, send_frame};
use super::types::{Message, RegisterAckPayload, RegisterNackPayload};
use crate::merge::{drain_stale_redexes, merge, GridConfig, GridMetrics, WorkerRoundStats};
use crate::partition::{split, Partition, PartitionStrategy, WorkerId};
use crate::reduction::reduce_all;
use crate::security::AuthToken;

// ---------------------------------------------------------------------------
// SPEC-06 R25 / SPEC-20 §3.8 A1 — elastic departure branching helpers
// ---------------------------------------------------------------------------

/// Identifies which FSM event triggered the elastic recovery path.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DepartureEventKind {
    ConnectionLost,
    PhaseTimeout,
}

/// Outcome of the SPEC-06 R25 / SPEC-20 §3.8 A1 connection-loss branch.
#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ConnectionLossOutcome {
    Abort(String),
    RecoveryTriggered {
        worker_id: WorkerId,
        kind: DepartureEventKind,
    },
}

#[allow(dead_code)]
pub(crate) fn handle_connection_loss(
    worker_id: WorkerId,
    error_description: &str,
    elastic_departure: bool,
) -> ConnectionLossOutcome {
    if elastic_departure {
        tracing::warn!(
            worker_id,
            error = error_description,
            "Worker connection lost during execution — elastic departure enabled; routing to recovery path."
        );
        ConnectionLossOutcome::RecoveryTriggered {
            worker_id,
            kind: DepartureEventKind::ConnectionLost,
        }
    } else {
        ConnectionLossOutcome::Abort(error_description.to_owned())
    }
}

#[allow(dead_code)]
pub(crate) fn handle_phase_timeout(
    worker_id: WorkerId,
    elapsed: Duration,
    elastic_departure: bool,
) -> ConnectionLossOutcome {
    if elastic_departure {
        tracing::warn!(
            worker_id,
            elapsed_secs = elapsed.as_secs_f64(),
            "Worker phase timeout — elastic departure enabled; routing to recovery path."
        );
        ConnectionLossOutcome::RecoveryTriggered {
            worker_id,
            kind: DepartureEventKind::PhaseTimeout,
        }
    } else {
        ConnectionLossOutcome::Abort(format!("phase timeout after {:.2}s", elapsed.as_secs_f64()))
    }
}

// ---------------------------------------------------------------------------
// Phase 0: Accept workers (TASK-0088)
// ---------------------------------------------------------------------------

/// Current wire protocol version (SPEC-20 R37).
pub const PROTOCOL_VERSION: u8 = 4;

/// Accepts and authenticates workers (SPEC-06 R17, R24; SPEC-10 R14-R17).
pub async fn accept_workers(
    config: &NodeConfig,
    token: Option<&AuthToken>,
    transport: &mut dyn Transport,
    hybrid_mode: bool,
) -> Result<Vec<TransportStream>, ProtocolError> {
    transport.listen().await?;

    tracing::info!("Coordinator listening on {}", config.bind);

    let mut streams = Vec::with_capacity(config.num_workers as usize);
    let mut next_worker_id: u32 = if hybrid_mode { 1 } else { 0 };

    let accept_future = async {
        while streams.len() < config.num_workers as usize {
            let mut stream = transport.accept().await?;

            // Read Register message from worker (SPEC-10 R14)
            let (msg, _) = recv_frame(&mut stream, config.max_payload_size).await?;

            match msg {
                Message::Register(payload) => {
                    // R11: WorkerId space exhaustion
                    if next_worker_id == u32::MAX {
                        let nack = Message::RegisterNack(RegisterNackPayload {
                            reason: "WorkerId space exhausted (R11)".into(),
                        });
                        let _ = send_frame(&mut stream, &nack).await;
                        return Err(ProtocolError::Coordinator(Box::new(
                            crate::error::CoordinatorError::WorkerIdSpaceExhausted,
                        )));
                    }

                    // Validate protocol version
                    if payload.protocol_version != PROTOCOL_VERSION {
                        let nack = Message::RegisterNack(RegisterNackPayload {
                            reason: format!(
                                "protocol version mismatch: expected {}, got {}",
                                PROTOCOL_VERSION, payload.protocol_version
                            ),
                        });
                        let _ = send_frame(&mut stream, &nack).await;
                        continue;
                    }

                    // Validate token
                    if let Some(expected_token) = token {
                        match payload.auth_token {
                            Some(provided_bytes) => {
                                let provided = AuthToken::from_bytes(provided_bytes);
                                if !expected_token.verify(&provided) {
                                    let nack = Message::RegisterNack(RegisterNackPayload {
                                        reason: "authentication failed".into(),
                                    });
                                    let _ = send_frame(&mut stream, &nack).await;
                                    continue;
                                }
                            }
                            None => {
                                let nack = Message::RegisterNack(RegisterNackPayload {
                                    reason: "authentication failed".into(),
                                });
                                let _ = send_frame(&mut stream, &nack).await;
                                continue;
                            }
                        }
                    }

                    // Assign worker ID and send Ack
                    let worker_id = next_worker_id;
                    next_worker_id += 1;

                    let ack = Message::RegisterAck(RegisterAckPayload { worker_id });
                    send_frame(&mut stream, &ack).await?;

                    tracing::info!("Worker {} registered (id={})", streams.len() + 1, worker_id);
                    streams.push(stream);
                }
                Message::JoinRequest {
                    protocol_version, ..
                } => {
                    // SPEC-20 R37a: JoinRequest is for mid-session joins.
                    // During the initial accept_workers window, we strictly enforce Register.
                    tracing::warn!(
                        "Rejected: JoinRequest received during initial WaitingForWorkers window"
                    );

                    // R0d + NF-009: handle version mismatch even on rejected path
                    if protocol_version != PROTOCOL_VERSION {
                        let nack = Message::JoinNack {
                            reason:
                                crate::protocol::types::JoinNackReason::ProtocolVersionMismatch {
                                    expected: PROTOCOL_VERSION,
                                    got: protocol_version,
                                },
                        };
                        let _ = send_frame(&mut stream, &nack).await;
                    } else {
                        let nack = Message::JoinNack {
                            reason: crate::protocol::types::JoinNackReason::ElasticJoinDisabled,
                        };
                        let _ = send_frame(&mut stream, &nack).await;
                    }
                    continue;
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

pub async fn distribute_partitions(
    worker_streams: &mut [TransportStream],
    partitions: Vec<Partition>,
    round: u32,
    distribute_timeout: Duration,
) -> Result<usize, ProtocolError> {
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

pub async fn shutdown_workers(worker_streams: &mut [TransportStream]) {
    for (i, stream) in worker_streams.iter_mut().enumerate() {
        if let Err(e) = send_frame(stream, &Message::Shutdown).await {
            tracing::warn!("Failed to send Shutdown to worker {}: {}", i, e);
        }
    }
}

// ---------------------------------------------------------------------------
// run_coordinator: distributed grid loop (TASK-0092)
// ---------------------------------------------------------------------------

pub async fn run_coordinator(
    net: crate::net::Net,
    config: &NodeConfig,
    grid_config: &GridConfig,
    strategy: &dyn PartitionStrategy,
    token: Option<&AuthToken>,
    transport: &mut dyn Transport,
) -> Result<(crate::net::Net, GridMetrics), ProtocolError> {
    let mut worker_streams =
        accept_workers(config, token, transport, grid_config.hybrid_coordinator).await?;

    let mut current_net = net;
    let mut metrics = GridMetrics::default();
    let start_time = Instant::now();

    loop {
        drain_stale_redexes(&mut current_net);
        if current_net.redex_queue.is_empty() {
            metrics.converged = true;
            break;
        }

        if let Some(max) = grid_config.max_rounds {
            if metrics.rounds >= max {
                metrics.converged = false;
                break;
            }
        }

        // === SPEC-20 R5: SoloReducing state ===
        if worker_streams.is_empty() {
            if grid_config.hybrid_coordinator {
                tracing::info!("Entering SoloReducing (budget={})", grid_config.solo_budget);

                // Foundational tokio::select! loop for SoloReducing.
                // Arms (a, b, c, d) from TASK-0422 are prepared.
                while !current_net.redex_queue.is_empty() {
                    tokio::select! {
                        // Arm (c): placeholder for accepting joins during solo-reduction
                        // _ = transport.accept() => { /* queue for handshake */ }

                        // Arm (a, b, d): pure batch reduction for now
                        else => {
                            let stats = crate::reduction::reduce_n(&mut current_net, grid_config.solo_budget as usize);
                            metrics.total_interactions += stats.total_interactions;
                            for (i, &count) in stats.interactions_by_rule.iter().enumerate() {
                                metrics.total_interactions_by_rule[i] += count;
                            }
                            if stats.total_interactions == 0 { break; }
                        }
                    }
                    tokio::task::yield_now().await;
                }

                if current_net.redex_queue.is_empty() {
                    metrics.converged = true;
                    break;
                }
            } else {
                return Err(ProtocolError::Fatal(
                    "No workers connected and hybrid_coordinator = false".into(),
                ));
            }
        }

        metrics
            .agents_per_round
            .push(current_net.count_live_agents());

        // Foundational loop remains sequential BSP for this wave,
        // preparing for full asynchronous event routing in Wave 4.

        // PHASE 1: PARTITION
        let t_partition = Instant::now();
        let plan = split(current_net, config.num_workers, strategy);
        metrics.partition_time_per_round.push(t_partition.elapsed());

        // PHASE 2a: DISTRIBUTE
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

        // PHASE 2b: COLLECT
        let t_recv = Instant::now();
        let (results, bytes_received): (Vec<(Partition, WorkerRoundStats)>, usize) =
            collect_results(
                &mut worker_streams,
                metrics.rounds,
                config.max_payload_size,
                config.collect_timeout,
            )
            .await?;
        metrics.network_recv_time_per_round.push(t_recv.elapsed());
        metrics.bytes_received_per_round.push(bytes_received);

        let mut reduced_partitions = Vec::with_capacity(results.len());
        let mut worker_stats = Vec::with_capacity(results.len());
        for (partition, stats) in results {
            reduced_partitions.push(partition);
            worker_stats.push(stats);
        }
        metrics.worker_stats_per_round.push(worker_stats.clone());

        // PHASE 3: MERGE + RESOLVE BORDERS
        let t_merge = Instant::now();
        let merge_plan = crate::partition::PartitionPlan {
            partitions: reduced_partitions,
            borders: plan.borders,
            next_border_id: plan.next_border_id,
        };
        let (mut merged_net, border_redex_count) = merge(merge_plan);
        metrics.border_redexes_per_round.push(border_redex_count);

        let local_interactions: u64 = worker_stats.iter().map(|s| s.local_redexes as u64).sum();
        metrics
            .local_interactions_per_round
            .push(local_interactions);

        for s in &worker_stats {
            for (i, &count) in s.interactions_by_rule.iter().enumerate() {
                metrics.total_interactions_by_rule[i] += count;
            }
        }

        let t_border = Instant::now();
        let border_stats = reduce_all(&mut merged_net);
        metrics
            .border_reduce_time_per_round
            .push(t_border.elapsed());
        metrics
            .border_interactions_per_round
            .push(border_stats.total_interactions);

        for (i, &count) in border_stats.interactions_by_rule.iter().enumerate() {
            metrics.total_interactions_by_rule[i] += count;
        }

        metrics.merge_time_per_round.push(t_merge.elapsed());
        metrics.total_interactions += local_interactions + border_stats.total_interactions;

        current_net = merged_net;
        metrics.rounds += 1;
    }

    shutdown_workers(&mut worker_streams).await;
    metrics.total_time = start_time.elapsed();
    Ok((current_net, metrics))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::channel::ChannelTransport;
    use crate::protocol::config::TransportConfig;
    use crate::protocol::tcp::TcpTransport;
    use crate::protocol::types::RegisterPayload;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;


    async fn send_register<W: AsyncWriteExt + Unpin>(stream: &mut W) {
        let register = Message::Register(RegisterPayload {
            protocol_version: PROTOCOL_VERSION,
            auth_token: None,
        });
        send_frame(stream, &register).await.unwrap();
    }

    async fn expect_register_ack<R: AsyncReadExt + Unpin>(stream: &mut R) -> u32 {
        let (msg, _) = recv_frame(stream, NodeConfig::default().max_payload_size)
            .await
            .unwrap();
        match msg {
            Message::RegisterAck(ack) => ack.worker_id,
            other => panic!("expected RegisterAck, got {:?}", other),
        }
    }

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
            async move { accept_workers(&config, None, &mut server, false).await }
        });
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
    }

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
        let result = accept_workers(&config, None, &mut transport, false).await;
        assert!(matches!(result, Err(ProtocolError::Timeout { .. })));
    }

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
            async move { accept_workers(&config, Some(&token_clone), &mut server, false).await }
        });
        let mut w = client.connect().await.unwrap();
        let register = Message::Register(RegisterPayload {
            protocol_version: PROTOCOL_VERSION,
            auth_token: Some(*token.as_bytes()),
        });
        send_frame(&mut w, &register).await.unwrap();
        let id = expect_register_ack(&mut w).await;
        assert_eq!(id, 0);
        assert!(accept_handle.await.unwrap().is_ok());
    }

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
            async move { accept_workers(&config, Some(&token_clone), &mut server, false).await }
        });
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
        let result = accept_handle.await.unwrap();
        assert!(matches!(result, Err(ProtocolError::Timeout { .. })));
    }

    #[test]
    fn protocol_version_is_four() {
        assert_eq!(PROTOCOL_VERSION, 4);
    }

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
            async move { accept_workers(&config, None, &mut server, false).await }
        });
        let mut w = client.connect().await.unwrap();
        let v1_register = Message::Register(RegisterPayload {
            protocol_version: 1,
            auth_token: None,
        });
        send_frame(&mut w, &v1_register).await.unwrap();
        let (response, _) = recv_frame(&mut w, NodeConfig::default().max_payload_size)
            .await
            .unwrap();
        let nack = match response {
            Message::RegisterNack(p) => p,
            other => panic!("got {:?}", other),
        };
        assert!(nack.reason.contains("protocol version mismatch"));
        assert!(nack.reason.contains("expected 4"));
        assert!(nack.reason.contains("got 1"));
        let result = accept_handle.await.unwrap();
        assert!(matches!(result, Err(ProtocolError::Timeout { .. })));
    }

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
            async move { accept_workers(&config, None, &mut server, false).await }
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
            other => panic!("got {:?}", other),
        };
        assert!(nack.reason.contains("protocol version mismatch"));
        assert!(nack.reason.contains("expected 4"));
        assert!(nack.reason.contains("got 0"));
        let result = accept_handle.await.unwrap();
        assert!(matches!(result, Err(ProtocolError::Timeout { .. })));
    }

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
            async move { accept_workers(&config, None, &mut server, false).await }
        });
        let mut v1 = client.connect().await.unwrap();
        let v1_register = Message::Register(RegisterPayload {
            protocol_version: 1,
            auth_token: None,
        });
        send_frame(&mut v1, &v1_register).await.unwrap();
        let (response, _) = recv_frame(&mut v1, NodeConfig::default().max_payload_size)
            .await
            .unwrap();
        assert!(matches!(response, Message::RegisterNack(_)));
        let mut v2 = client.connect().await.unwrap();
        send_register(&mut v2).await;
        let id = expect_register_ack(&mut v2).await;
        assert_eq!(id, 0);
        let result = accept_handle.await.unwrap();
        assert!(result.is_ok());
    }

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
            async move { accept_workers(&config, None, &mut server, false).await }
        });
        let mut w = client.connect().await.unwrap();
        send_register(&mut w).await;
        let (response, _) = recv_frame(&mut w, NodeConfig::default().max_payload_size)
            .await
            .unwrap();
        assert!(matches!(response, Message::RegisterAck(_)));
        assert!(accept_handle.await.unwrap().is_ok());
    }
}
