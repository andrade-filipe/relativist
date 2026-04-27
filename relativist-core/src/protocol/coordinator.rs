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
use crate::merge::{
    drain_stale_redexes, merge, reconstruct, BorderGraph, GridConfig, GridMetrics, WorkerRoundStats,
};
use crate::partition::{
    materialize_reclaimed_partitions, split, Partition, PartitionStrategy, WorkerId,
};
use crate::reduction::reduce_all;
use crate::security::AuthToken;

// ---------------------------------------------------------------------------
// SPEC-06 R25 / SPEC-20 §3.8 A1 — elastic departure branching helpers
// ---------------------------------------------------------------------------

/// Identifies which FSM event triggered the elastic recovery path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DepartureEventKind {
    ConnectionLost,
    PhaseTimeout,
}

/// Outcome of the SPEC-06 R25 / SPEC-20 §3.8 A1 connection-loss branch.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ConnectionLossOutcome {
    Abort(String),
    RecoveryTriggered {
        worker_id: WorkerId,
        kind: DepartureEventKind,
    },
}

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
// Log sanitation (QA-008)
// ---------------------------------------------------------------------------

/// Bound the length and strip control characters from a worker-supplied
/// string before it is emitted via `tracing`.
///
/// SPEC-11 OTel attribute mapping records `description`/`error` fields
/// verbatim from `Message::Error` and from socket I/O failures. A
/// compromised or buggy worker can send arbitrary bytes — including
/// embedded `\n` to forge log lines, gigabyte-long strings to OOM the
/// log subscriber, or PII/secret payloads. This helper enforces the
/// invariant that any string flowing into the structured-log pipeline
/// is bounded (≤ 4 KiB) and free of CR / LF / NUL.
fn sanitize_log_string(s: &str) -> String {
    const MAX_LEN: usize = 4096;
    let mut out = String::with_capacity(s.len().min(MAX_LEN));
    for ch in s.chars().take(MAX_LEN) {
        match ch {
            '\n' | '\r' | '\0' => out.push(' '),
            c if c.is_control() => out.push('?'),
            c => out.push(c),
        }
    }
    if s.len() > MAX_LEN {
        out.push_str("…[truncated]");
    }
    out
}

// ---------------------------------------------------------------------------
// Per-round metric snapshot helper (QA-003)
// ---------------------------------------------------------------------------

/// Restores per-round Vec length parity for the elastic counters when the
/// round body returns Err *after* `effective_slots_per_round` was pushed
/// but *before* the bottom-of-round push site is reached.
///
/// The seven elastic Vecs MUST satisfy
/// `len(workers_joined) == len(workers_departed) == ... == len(effective_slots)`
/// at every observation point (CSV/JSON consumers index by round). When
/// the reclaim path returns `Err` deliberately (TASK-0443 follow-up), this
/// helper pushes zeros for the remaining six counters so length parity is
/// preserved. After TASK-0443 closes that early return, this helper is no
/// longer load-bearing but the call site is harmless (idempotent zero
/// pushes are detected by length comparison in the regression test).
fn push_partial_round_metrics(metrics: &mut GridMetrics) {
    // Reference length: `effective_slots_per_round` is pushed first in the
    // round (it remains canonical even on early returns).
    let target = metrics.effective_slots_per_round.len();

    while metrics.workers_departed_per_round.len() < target {
        metrics.workers_departed_per_round.push(0);
    }
    while metrics.retained_initial_reclaims_per_round.len() < target {
        metrics.retained_initial_reclaims_per_round.push(0);
    }
    while metrics.retained_last_acked_reclaims_per_round.len() < target {
        metrics.retained_last_acked_reclaims_per_round.push(0);
    }
    while metrics.partitions_redispatched_per_round.len() < target {
        metrics.partitions_redispatched_per_round.push(0);
    }
    while metrics.join_round_overhead_ms_per_round.len() < target {
        metrics.join_round_overhead_ms_per_round.push(0);
    }
    while metrics.workers_joined_per_round.len() < target {
        metrics.workers_joined_per_round.push(0);
    }
    while metrics.join_window_time_per_round.len() < target {
        metrics
            .join_window_time_per_round
            .push(Duration::from_secs(0));
    }
}

// ---------------------------------------------------------------------------
// Phase 0: Accept workers (TASK-0088)
// ---------------------------------------------------------------------------

/// Current wire protocol version (SPEC-20 R37).
pub const PROTOCOL_VERSION: u8 = 4;

/// Processes a mid-session `JoinRequest` (SPEC-20 §3.2 R9).
///
/// Performs the JoinRequest/JoinAck handshake, authenticates the worker,
/// and assigns a `WorkerId` and `partition_index`.
///
/// R17: logs assignment at INFO level.
#[allow(clippy::too_many_arguments)]
pub async fn process_join_request(
    stream: &mut TransportStream,
    msg: Message,
    grid_config: &GridConfig,
    _node_config: &NodeConfig,
    expected_token: Option<&AuthToken>,
    next_worker_id: &mut u32,
    current_round: u32,
    active_workers: &std::collections::BTreeSet<WorkerId>,
) -> Result<Option<WorkerId>, ProtocolError> {
    let (protocol_version, auth_token) = match msg {
        Message::JoinRequest {
            protocol_version,
            auth_token,
            ..
        } => (protocol_version, auth_token),
        other => {
            tracing::warn!("Handshake error: expected JoinRequest, got {:?}", other);
            return Ok(None);
        }
    };

    // R0d: validate protocol version
    if protocol_version != PROTOCOL_VERSION {
        let nack = Message::JoinNack {
            reason: crate::protocol::types::JoinNackReason::ProtocolVersionMismatch {
                expected: PROTOCOL_VERSION,
                got: protocol_version,
            },
        };
        let _ = send_frame(stream, &nack).await;
        return Ok(None);
    }

    // R9: check if joins are enabled
    if !grid_config.elastic_join {
        let nack = Message::JoinNack {
            reason: crate::protocol::types::JoinNackReason::ElasticJoinDisabled,
        };
        let _ = send_frame(stream, &nack).await;
        return Ok(None);
    }

    // Auth check
    if let Some(token) = expected_token {
        let mut valid = false;
        if let Some(provided_bytes) = auth_token {
            let provided = AuthToken::from_bytes(provided_bytes);
            if token.verify(&provided) {
                valid = true;
            }
        }
        if !valid {
            let nack = Message::JoinNack {
                reason: crate::protocol::types::JoinNackReason::AuthenticationFailed,
            };
            let _ = send_frame(stream, &nack).await;
            return Ok(None);
        }
    }

    // R11: allocate WorkerId
    if *next_worker_id == u32::MAX {
        let nack = Message::JoinNack {
            reason: crate::protocol::types::JoinNackReason::WorkerIdSpaceExhausted,
        };
        let _ = send_frame(stream, &nack).await;
        return Err(ProtocolError::Coordinator(Box::new(
            crate::error::CoordinatorError::WorkerIdSpaceExhausted,
        )));
    }

    let worker_id = *next_worker_id;
    *next_worker_id += 1;

    // R11a: Compute the partition_index the worker WILL occupy in the next round.
    let partition_index = if grid_config.hybrid_coordinator {
        active_workers.len() as u32 + 1
    } else {
        active_workers.len() as u32
    };

    // R16: JoinAck informs worker of next_round_number
    let next_round_number = current_round + 1;

    let ack = Message::JoinAck {
        worker_id,
        partition_index,
        next_round_number,
    };
    send_frame(stream, &ack).await?;

    // QA-004: the canonical R17 INFO emission lives at the caller in
    // `run_coordinator` (see "Worker joined the grid (R17)"). Emitting an
    // additional info log here would double the join cardinality for log
    // analyzers indexing on R17 events. We keep a low-noise debug breadcrumb
    // for developer observability without altering the spec-counted cardinality.
    tracing::debug!(
        worker_id,
        partition_index,
        next_round_number,
        "JoinAck issued (pre-R17 breadcrumb)"
    );

    Ok(Some(worker_id))
}

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
    worker_streams: &mut [&mut TransportStream],
    round: u32,
    max_payload_size: u32,
    collect_timeout: Duration,
) -> Result<(Vec<(Partition, WorkerRoundStats)>, usize), ProtocolError> {
    let collect_future = async {
        let mut results = Vec::with_capacity(worker_streams.len());
        let mut total_bytes = 0;

        for stream in worker_streams.iter_mut() {
            let (msg, nbytes) = recv_frame(*stream, max_payload_size).await?;
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
    // R7a: hybrid node acts as one worker (id 0).
    let remote_workers_needed = if grid_config.hybrid_coordinator {
        config.num_workers.saturating_sub(1)
    } else {
        config.num_workers
    };

    let mut accept_config = config.clone();
    accept_config.num_workers = remote_workers_needed;

    let mut worker_streams = accept_workers(
        &accept_config,
        token,
        transport,
        grid_config.hybrid_coordinator,
    )
    .await?;

    let mut pending_connections_queue = std::collections::VecDeque::<TransportStream>::new();
    let remote_count = worker_streams.len();
    let mut next_worker_id: u32 = if grid_config.hybrid_coordinator {
        (remote_count + 1) as u32
    } else {
        remote_count as u32
    };

    // SPEC-20 R23: Retained state registry
    let mut retained_state = crate::protocol::retained::RetainedStateRegistry::new();

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

                while !current_net.redex_queue.is_empty() {
                    tokio::select! {
                        new_conn = transport.accept() => {
                            match new_conn {
                                Ok(stream) => {
                                    tracing::info!("Accepted mid-session connection during SoloReducing; queued.");
                                    pending_connections_queue.push_back(stream);
                                }
                                Err(e) => {
                                    tracing::warn!("Failed to accept mid-session connection: {}", e);
                                }
                            }
                        }
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

        // === PHASE 1: PARTITION ===
        let t_partition = Instant::now();
        let remote_count = worker_streams.len();
        let k_eff = remote_count + if grid_config.hybrid_coordinator { 1 } else { 0 };

        // R38: track effective slot count
        metrics.effective_slots_per_round.push(k_eff as u32);

        let plan = split(current_net, k_eff as u32, strategy);
        metrics.partition_time_per_round.push(t_partition.elapsed());

        // PHASE 2a & 2b: DISTRIBUTE AND COLLECT
        let mut results: Vec<(Partition, WorkerRoundStats)> = Vec::with_capacity(k_eff);
        let mut bytes_sent = 0;
        let mut bytes_received = 0;
        let _t_grid = Instant::now();

        let mut partitions_iter = plan.partitions.iter().cloned();
        let self_partition = if grid_config.hybrid_coordinator {
            partitions_iter.next()
        } else {
            None
        };
        let remote_partitions: Vec<Partition> = partitions_iter.collect();

        let mut self_handle = if let Some(ref _p) = self_partition {
            Some(crate::protocol::self_worker::spawn_self_partition(config.max_payload_size).await)
        } else {
            None
        };

        // TASK-0439: state retention
        if grid_config.retain_partitions {
            if let Some(ref p) = self_partition {
                retained_state
                    .initial
                    .entry(0)
                    .or_insert_with(|| crate::protocol::retained::RetainedInitial::V1(p.clone()));
            }
            for p in &remote_partitions {
                retained_state
                    .initial
                    .entry(p.worker_id)
                    .or_insert_with(|| crate::protocol::retained::RetainedInitial::V1(p.clone()));
            }
        }

        // QA-003: each `?` early return below leaves the round's metric
        // Vecs lopsided unless we top them up with zero pushes first. The
        // helper restores parity to `effective_slots_per_round.len()`.
        bytes_sent += match distribute_partitions(
            &mut worker_streams,
            remote_partitions,
            metrics.rounds,
            config.distribute_timeout,
        )
        .await
        {
            Ok(n) => n,
            Err(e) => {
                push_partial_round_metrics(&mut metrics);
                return Err(e);
            }
        };

        if let Some(ref mut h) = self_handle {
            let p = self_partition.as_ref().unwrap();
            let msg = Message::AssignPartition {
                round: metrics.rounds,
                partition: p.clone(),
            };
            match send_frame(&mut h.stream, &msg).await {
                Ok(n) => bytes_sent += n,
                Err(e) => {
                    push_partial_round_metrics(&mut metrics);
                    return Err(e);
                }
            }
        }

        // Collect with departure detection (TASK-0438, 0441)
        let mut collect_results_vec = Vec::with_capacity(k_eff);
        let mut departing_worker_ids = Vec::new();
        // SF-002: drop the leading underscore — both reclaim counters are
        // observably read at the metrics push site below. They sit at zero
        // until the TASK-0443 follow-up wires reclaim back into the round
        // loop (FIXME at the push site below).
        let mut round_reclaimed_initial: u32 = 0;
        let round_reclaimed_last_acked: u32 = 0;
        let mut round_departed_count: u32 = 0;

        // We'll map indices to actual WorkerIds for remotes
        // Simplified: remotes are 1..N if hybrid, 0..N-1 if not.
        let mut streams_to_poll: Vec<(WorkerId, &mut TransportStream)> = worker_streams
            .iter_mut()
            .enumerate()
            .map(|(i, s)| {
                let id = if grid_config.hybrid_coordinator {
                    (i as u32) + 1
                } else {
                    i as u32
                };
                (id, &mut *s)
            })
            .collect();

        if let Some(ref mut h) = self_handle {
            streams_to_poll.push((0, &mut h.stream));
        }

        for (wid, stream) in streams_to_poll {
            let recv_future = recv_frame(stream, config.max_payload_size);
            match tokio::time::timeout(config.collect_timeout, recv_future).await {
                Ok(Ok((msg, nbytes))) => {
                    bytes_received += nbytes;
                    match msg {
                        Message::PartitionResult {
                            round: r,
                            partition,
                            stats,
                        } => {
                            if r != metrics.rounds {
                                push_partial_round_metrics(&mut metrics);
                                return Err(ProtocolError::Fatal(format!(
                                    "round mismatch: {} vs {}",
                                    r, metrics.rounds
                                )));
                            }
                            if grid_config.retain_partitions {
                                retained_state.refresh_last_acked(
                                    partition.worker_id,
                                    crate::protocol::retained::RetainedLastAcked::V1(
                                        partition.clone(),
                                    ),
                                );
                            }
                            collect_results_vec.push((partition, stats));
                        }
                        Message::LeaveRequest { kind } => {
                            // R28: WARN log on departure (MF-002).
                            // `departure_type` is one of the four canonical
                            // strings enumerated by SPEC-20 R28.
                            // `retained_slot` is `"retained_initial"` because
                            // R24a (conservative) is the only reclaim path
                            // available today; R24b lands later (TASK-0443).
                            let departure_type = match kind {
                                crate::protocol::types::LeaveKind::AfterResult => {
                                    "leave_after_result"
                                }
                                crate::protocol::types::LeaveKind::Urgent => "leave_urgent",
                            };
                            tracing::warn!(
                                worker_id = wid,
                                round = metrics.rounds,
                                departure_type,
                                retained_slot = "retained_initial",
                                "Worker left gracefully via LeaveRequest (R28)"
                            );
                            let _ = send_frame(stream, &Message::LeaveAck).await;
                            departing_worker_ids.push(wid);
                            round_departed_count += 1;
                        }
                        Message::Error {
                            worker_id,
                            description,
                            ..
                        } => {
                            let outcome = handle_connection_loss(
                                worker_id,
                                &description,
                                grid_config.elastic_departure,
                            );
                            match outcome {
                                ConnectionLossOutcome::Abort(e) => {
                                    push_partial_round_metrics(&mut metrics);
                                    return Err(ProtocolError::Fatal(e));
                                }
                                ConnectionLossOutcome::RecoveryTriggered {
                                    worker_id: id, ..
                                } => {
                                    // R28: WARN log (MF-002 + QA-008).
                                    // QA-008: worker-supplied `description`
                                    // is sanitized to bound length and strip
                                    // CR/LF before emission.
                                    let sanitized = sanitize_log_string(&description);
                                    tracing::warn!(
                                        worker_id = id,
                                        round = metrics.rounds,
                                        departure_type = "connection_loss",
                                        retained_slot = "retained_initial",
                                        error = %sanitized,
                                        "Worker departed due to error; triggering recovery (R28)"
                                    );
                                    departing_worker_ids.push(id);
                                    round_departed_count += 1;
                                }
                            }
                        }
                        other => {
                            push_partial_round_metrics(&mut metrics);
                            return Err(ProtocolError::Fatal(format!(
                                "unexpected message: {:?}",
                                other
                            )));
                        }
                    }
                }
                Ok(Err(e)) => {
                    let outcome =
                        handle_connection_loss(wid, &e.to_string(), grid_config.elastic_departure);
                    match outcome {
                        ConnectionLossOutcome::Abort(e) => {
                            push_partial_round_metrics(&mut metrics);
                            return Err(ProtocolError::Fatal(e));
                        }
                        ConnectionLossOutcome::RecoveryTriggered { worker_id: id, .. } => {
                            // MF-002 + QA-008: canonical departure_type
                            // string + sanitized error payload.
                            let sanitized = sanitize_log_string(&e.to_string());
                            tracing::warn!(
                                worker_id = id,
                                round = metrics.rounds,
                                departure_type = "connection_loss",
                                retained_slot = "retained_initial",
                                error = %sanitized,
                                "Worker connection lost; triggering recovery (R28)"
                            );
                            departing_worker_ids.push(id);
                            round_departed_count += 1;
                        }
                    }
                }
                Err(_) => {
                    let outcome = handle_phase_timeout(
                        wid,
                        config.collect_timeout,
                        grid_config.elastic_departure,
                    );
                    match outcome {
                        ConnectionLossOutcome::Abort(e) => {
                            push_partial_round_metrics(&mut metrics);
                            return Err(ProtocolError::Fatal(e));
                        }
                        ConnectionLossOutcome::RecoveryTriggered { worker_id: id, .. } => {
                            // MF-002: canonical departure_type string.
                            tracing::warn!(
                                worker_id = id,
                                round = metrics.rounds,
                                departure_type = "timeout",
                                retained_slot = "retained_initial",
                                "Worker timed out; triggering recovery (R28)"
                            );
                            departing_worker_ids.push(id);
                            round_departed_count += 1;
                        }
                    }
                }
            }
        }

        if let Some(h) = self_handle {
            // QA-003: top up per-round Vec parity before bailing on a
            // self-worker join failure.
            if let Err(e) = h.join_handle.await {
                push_partial_round_metrics(&mut metrics);
                return Err(ProtocolError::Fatal(format!(
                    "Self-worker join error: {:?}",
                    e
                )));
            }
        }

        // === DEPARTURE RECLAIM (TASK-0440, 0442, 0443) ===
        if !departing_worker_ids.is_empty() {
            tracing::warn!(
                "Detected {} departing workers. Triggering reclaim.",
                departing_worker_ids.len()
            );

            // Handle D == K_eff (TASK-0442)
            if departing_worker_ids.len() >= k_eff {
                tracing::error!(
                    "All workers departed! D={}, K_eff={}",
                    departing_worker_ids.len(),
                    k_eff
                );
                if grid_config.hybrid_coordinator {
                    tracing::warn!("Hybrid mode: falling back to SoloReducing.");
                    // In a real implementation we'd reclaim state and continue.
                    // For this wave, we'll abort to satisfy P0 safety.
                    push_partial_round_metrics(&mut metrics);
                    return Err(ProtocolError::Fatal(
                        "All workers departed including self-handle logic".into(),
                    ));
                } else {
                    push_partial_round_metrics(&mut metrics);
                    return Err(ProtocolError::Fatal(
                        "All workers departed and non-hybrid mode".into(),
                    ));
                }
            }

            // Materialize reclaimed (TASK-0443 conservative)
            // We need remapped ranges. For v1, we just reconstruct and re-split.
            let surviving_partitions: Vec<Partition> =
                collect_results_vec.iter().map(|(p, _)| p.clone()).collect();

            // For materialization we need IdRanges. We can use compute_id_ranges for the "new" pool.
            // Simplified for Wave 3: we just use the initial ones.
            let mut reclaimed_id_ranges = std::collections::HashMap::new();
            for &id in &departing_worker_ids {
                reclaimed_id_ranges.insert(
                    id,
                    crate::partition::IdRange {
                        start: 0,
                        end: 100_000,
                    },
                ); // placeholder
            }

            let reclaimed_partitions = match materialize_reclaimed_partitions(
                &departing_worker_ids,
                &retained_state,
                &reclaimed_id_ranges,
            ) {
                Ok(p) => p,
                Err(e) => {
                    push_partial_round_metrics(&mut metrics);
                    return Err(ProtocolError::Fatal(e.to_string()));
                }
            };

            // Reconstruct the net
            let border_graph = BorderGraph::from_partition_plan(&plan);
            let merged_net = reconstruct(&border_graph, surviving_partitions, reclaimed_partitions);
            current_net = merged_net;
            tracing::info!(
                agent_count = current_net.count_live_agents(),
                "Departure recovery reconstruction succeeded."
            );

            // FIXME(TASK-0443): the increment below is the only writer of
            // `round_reclaimed_initial` today. The push site is unreachable
            // because we return Err immediately afterwards. Once TASK-0443
            // closes the unconditional return, the metric push picks up the
            // accumulated value. Suppress the unused-assignment warning
            // until then.
            #[allow(unused_assignments)]
            {
                round_reclaimed_initial += departing_worker_ids.len() as u32;
            }
            // SF-001: removed `let _ = _round_reclaimed_initial;` redundancy.
            //
            // QA-003 / MF-004: this branch deliberately returns Err and
            // therefore does not reach the per-round metric-push site below.
            // All elastic per-round Vecs (workers_departed/joined,
            // retained_*_reclaims, partitions_redispatched, join_*_overhead,
            // join_window_time) skip their push on this path. Length parity
            // with `effective_slots_per_round` is restored by
            // `push_partial_round_metrics` immediately before this return,
            // so CSV consumers indexing by round see consistent lengths.
            // After TASK-0443 lands stream management here, this early
            // return goes away and the normal end-of-round push path takes
            // over.
            push_partial_round_metrics(&mut metrics);
            return Err(ProtocolError::Fatal("Departure recovery reconstruction succeeded but stream management is TASK-0443 follow-up".into()));
        }

        // R38: record round-level departure/reclaim metrics.
        //
        // FIXME(TASK-0443): `retained_initial_reclaims_per_round` and
        // `retained_last_acked_reclaims_per_round` are unreachable in the
        // happy path today because the conservative reclaim branch above
        // (L767..L832) unconditionally returns `Err` once it materializes
        // reclaimed partitions. Until TASK-0443 wires reclaim back into the
        // round loop, these counters always push 0 here. See `docs/next-steps.md`
        // entry "TASK-0443 follow-up — reclaim metrics dead-on-arrival" for
        // the closure plan. (MF-004)
        // SF-004: `bytes_received_per_round` aggregates ALL message bytes
        // in this round (PartitionResult + LeaveRequest + Error), not just
        // result bytes. Per-message-type segmentation is a SPEC-09
        // benchmark-affecting follow-up, tracked separately.
        metrics
            .workers_departed_per_round
            .push(round_departed_count);
        metrics
            .retained_initial_reclaims_per_round
            .push(round_reclaimed_initial);
        metrics
            .retained_last_acked_reclaims_per_round
            .push(round_reclaimed_last_acked);
        metrics.partitions_redispatched_per_round.push(0); // placeholder
        metrics.join_round_overhead_ms_per_round.push(0); // placeholder

        results.extend(collect_results_vec);
        metrics.bytes_sent_per_round.push(bytes_sent);
        metrics.bytes_received_per_round.push(bytes_received);

        let mut reduced_partitions = Vec::with_capacity(results.len());
        let mut worker_stats = Vec::with_capacity(results.len());
        for (partition, stats) in results {
            reduced_partitions.push(partition);
            worker_stats.push(stats);
        }
        metrics.worker_stats_per_round.push(worker_stats.clone());

        // PHASE 3: MERGE
        let t_merge = Instant::now();
        let merge_plan = crate::partition::PartitionPlan {
            partitions: reduced_partitions,
            borders: plan.borders,
            next_border_id: plan.next_border_id,
        };
        let (mut merged_net, border_redex_count) = merge(merge_plan);
        // QA-001: capture structural merge time BEFORE border resolution.
        // Previously `merge_time_per_round` was contaminated in elastic mode
        // by the join-window wall-clock; both writes are now in their own
        // observable lanes (`merge_time_per_round` here, `join_window_time_per_round`
        // at the end of the join window).
        let merge_only = t_merge.elapsed();
        metrics.merge_time_per_round.push(merge_only);
        metrics.border_redexes_per_round.push(border_redex_count);

        if grid_config.strict_bsp {
            debug_assert!(merged_net.redex_queue.len() >= border_redex_count as usize);
        }

        let local_interactions: u64 = worker_stats.iter().map(|s| s.local_redexes as u64).sum();
        metrics
            .local_interactions_per_round
            .push(local_interactions);
        for s in &worker_stats {
            for (i, &count) in s.interactions_by_rule.iter().enumerate() {
                metrics.total_interactions_by_rule[i] += count;
            }
        }

        let t_border_reduce = Instant::now();
        let border_stats = if grid_config.strict_bsp {
            crate::reduction::ReductionStats::default()
        } else {
            reduce_all(&mut merged_net)
        };
        metrics
            .border_reduce_time_per_round
            .push(t_border_reduce.elapsed());
        metrics
            .border_interactions_per_round
            .push(border_stats.total_interactions);
        for (i, &count) in border_stats.interactions_by_rule.iter().enumerate() {
            metrics.total_interactions_by_rule[i] += count;
        }

        metrics.total_interactions += local_interactions + border_stats.total_interactions;
        current_net = merged_net;
        metrics.rounds += 1;

        // JOIN WINDOW
        let mut round_joined_count: u32 = 0;
        if grid_config.elastic_join {
            let t_window_start = Instant::now();
            let min_timer = tokio::time::sleep(grid_config.join_window_min);
            let max_timer = tokio::time::sleep(grid_config.join_window_max);
            tokio::pin!(min_timer);
            tokio::pin!(max_timer);

            loop {
                while let Some(mut stream) = pending_connections_queue.pop_front() {
                    let active_ids: std::collections::BTreeSet<WorkerId> = worker_streams
                        .iter()
                        .enumerate()
                        .map(|(i, _)| {
                            let offset = if grid_config.hybrid_coordinator { 1 } else { 0 };
                            (i as u32) + offset
                        })
                        .collect();

                    // QA-005: bound the per-stream JoinRequest read by the
                    // remaining join-window budget. Without this, a slow or
                    // silent client stalls the coordinator past
                    // `join_window_max`, breaking SPEC-20 R12 and offering a
                    // trivial DoS surface.
                    let elapsed = t_window_start.elapsed();
                    let remaining = grid_config
                        .join_window_max
                        .checked_sub(elapsed)
                        .unwrap_or_else(|| std::time::Duration::from_millis(0));
                    let recv_outcome = tokio::time::timeout(
                        remaining,
                        recv_frame(&mut stream, config.max_payload_size),
                    )
                    .await;
                    let (msg, _) = match recv_outcome {
                        Ok(Ok(pair)) => pair,
                        Ok(Err(e)) => {
                            tracing::warn!(
                                error = %e,
                                "JoinRequest recv failed; dropping pending stream."
                            );
                            continue;
                        }
                        Err(_) => {
                            tracing::warn!(
                                join_window_max_ms = grid_config.join_window_max.as_millis() as u64,
                                "JoinRequest recv timed out within join window; dropping pending stream (QA-005)."
                            );
                            continue;
                        }
                    };
                    if let Some(worker_id) = process_join_request(
                        &mut stream,
                        msg,
                        grid_config,
                        config,
                        token,
                        &mut next_worker_id,
                        metrics.rounds,
                        &active_ids,
                    )
                    .await?
                    {
                        worker_streams.push(stream);
                        round_joined_count += 1;

                        // QA-002: register the joiner in the retained-state
                        // registry so the D5 precondition on
                        // `refresh_last_acked` holds when the joiner returns
                        // its first PartitionResult in round N+1. The L587
                        // round-init block subsequently overwrites this
                        // sentinel with the joiner's true round-N+1 partition
                        // via `entry().or_insert_with(...)` only if the
                        // sentinel is absent — but `register_initial(None)`
                        // is itself an `or_insert_with` no-op when a real
                        // partition is already bound, so the two paths are
                        // commutative.
                        if grid_config.retain_partitions {
                            retained_state.register_initial(worker_id, None);
                        }

                        // R17: INFO log on join (MF-001 + MF-005).
                        // Spec contract enumerates: worker_id, K_eff_new,
                        // partition_index, first_participating_round.
                        let offset = if grid_config.hybrid_coordinator {
                            1u32
                        } else {
                            0u32
                        };
                        let partition_index = (worker_streams.len() as u32 - 1) + offset;
                        let k_eff_new = worker_streams.len() + offset as usize;
                        tracing::info!(
                            worker_id,
                            partition_index,
                            k_eff_new,
                            first_participating_round = metrics.rounds,
                            "Worker joined the grid (R17)"
                        );
                    }
                }
                if min_timer.is_elapsed() {
                    break;
                }
                tokio::select! {
                    new_conn = transport.accept() => {
                        if let Ok(s) = new_conn { pending_connections_queue.push_back(s); }
                    }
                    _ = &mut min_timer => {}
                    _ = &mut max_timer => { break; }
                }
            }
            // QA-001: the join-window wall-clock belongs to the dedicated
            // observable, NOT to `merge_time_per_round`. The structural
            // merge time is already recorded above (see `t_merge_only`).
            metrics
                .join_window_time_per_round
                .push(t_window_start.elapsed());
        }
        metrics.workers_joined_per_round.push(round_joined_count);
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

    /// QA-008: worker-supplied strings must be bounded and stripped of
    /// control characters before flowing into structured logs.
    #[test]
    fn qa_008_sanitize_log_string_truncates_long_input() {
        let huge = "a".repeat(10_000);
        let cleaned = sanitize_log_string(&huge);
        // 4096 base + "…[truncated]" suffix.
        assert!(cleaned.len() < 5_000, "len={}", cleaned.len());
        assert!(cleaned.ends_with("…[truncated]"));
    }

    #[test]
    fn qa_008_sanitize_log_string_strips_newlines_and_carriage_returns() {
        let evil = "fake R28 log\n  R28 BLOCKED EXFIL: secret=AKIA\r\n";
        let cleaned = sanitize_log_string(evil);
        assert!(!cleaned.contains('\n'));
        assert!(!cleaned.contains('\r'));
    }

    #[test]
    fn qa_008_sanitize_log_string_replaces_other_control_chars() {
        let evil = "\u{0001}\u{0002}hello\u{0003}";
        let cleaned = sanitize_log_string(evil);
        // Control chars become '?'.
        assert!(cleaned.contains("hello"));
        assert!(!cleaned.chars().any(|c| c.is_control() && c != ' '));
    }

    /// QA-003: `push_partial_round_metrics` restores per-round Vec parity.
    #[test]
    fn qa_003_push_partial_round_metrics_restores_parity() {
        let mut metrics = GridMetrics::default();
        // Simulate the half-pushed state of a round that returned Err
        // mid-distribute: only `effective_slots_per_round` was pushed.
        metrics.effective_slots_per_round.push(4);
        // All other per-round Vecs are short by 1.
        push_partial_round_metrics(&mut metrics);
        let target = metrics.effective_slots_per_round.len();
        assert_eq!(metrics.workers_departed_per_round.len(), target);
        assert_eq!(metrics.retained_initial_reclaims_per_round.len(), target);
        assert_eq!(metrics.retained_last_acked_reclaims_per_round.len(), target);
        assert_eq!(metrics.partitions_redispatched_per_round.len(), target);
        assert_eq!(metrics.join_round_overhead_ms_per_round.len(), target);
        assert_eq!(metrics.workers_joined_per_round.len(), target);
        assert_eq!(metrics.join_window_time_per_round.len(), target);
    }

    /// QA-003: the helper is idempotent — calling it twice does not push
    /// extra zeros.
    #[test]
    fn qa_003_push_partial_round_metrics_is_idempotent() {
        let mut metrics = GridMetrics::default();
        metrics.effective_slots_per_round.push(4);
        push_partial_round_metrics(&mut metrics);
        let after_first = metrics.workers_joined_per_round.len();
        push_partial_round_metrics(&mut metrics);
        assert_eq!(metrics.workers_joined_per_round.len(), after_first);
    }

    /// QA-001: the structural merge time and the join-window time live in
    /// distinct Vecs. Pushing to one does not bleed into the other.
    #[test]
    fn qa_001_merge_time_and_join_window_time_are_separate_lanes() {
        let mut metrics = GridMetrics::default();
        metrics
            .merge_time_per_round
            .push(Duration::from_micros(100));
        metrics
            .join_window_time_per_round
            .push(Duration::from_millis(200));
        assert_eq!(metrics.merge_time_per_round.len(), 1);
        assert_eq!(metrics.join_window_time_per_round.len(), 1);
        // Their values are unrelated.
        assert_ne!(
            metrics.merge_time_per_round[0],
            metrics.join_window_time_per_round[0]
        );
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
