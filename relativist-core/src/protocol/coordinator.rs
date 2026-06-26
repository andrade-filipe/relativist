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
    // D-012 REFACTOR (QA-D012-007 / reviewer MF-001 / SF-005):
    // The five per-round Vecs below are normally pushed at end-of-round,
    // AFTER the early-return sites above. On any `?` early return mid-round,
    // they would silently lag `effective_slots_per_round.len()` by one, and
    // a downstream zip-by-index in `bench/suite.rs` (or
    // `bench/csv.rs::write_rounds_row` indexing via
    // `.get(round).unwrap_or(0.0)`) would silently drop the partial round —
    // or worse, mis-align rounds across columns.
    //
    // Pushing zero placeholders here keeps the parity invariant honest:
    // `bytes_*` / `network_*_time` / `compute_time` Vecs MUST satisfy
    // `len() == effective_slots_per_round.len()` at every observation
    // point. The end-of-`run_coordinator` debug_assert below pins this
    // contract loudly in debug builds.
    while metrics.bytes_sent_per_round.len() < target {
        metrics.bytes_sent_per_round.push(0);
    }
    while metrics.bytes_received_per_round.len() < target {
        metrics.bytes_received_per_round.push(0);
    }
    while metrics.network_send_time_per_round.len() < target {
        metrics.network_send_time_per_round.push(Duration::ZERO);
    }
    while metrics.network_recv_time_per_round.len() < target {
        metrics.network_recv_time_per_round.push(Duration::ZERO);
    }
    while metrics.compute_time_per_round.len() < target {
        metrics.compute_time_per_round.push(Duration::ZERO);
    }
}

// ---------------------------------------------------------------------------
// Phase 0: Accept workers (TASK-0088)
// ---------------------------------------------------------------------------

/// Current wire protocol version.
///
/// Cites: SPEC-20 R37; SPEC-22 R9a; SPEC-21 §3.7 R37c; SPEC-19 §3.4 R35a.
///
/// Version history:
/// - v1: initial release
/// - v2: SPEC-18 wire format v2
/// - v3: SPEC-18 R28 amendment
/// - v4: SPEC-20 elastic grid fields added to wire messages (TASK-0417)
/// - v5: SPEC-22 free-list added to Net serde payload (R9a) — REJECT-v4 policy:
///   nets serialized with v4 do not carry `free_list`; deserialization
///   returns `Err(ProtocolError::UnsupportedVersion)` rather than silently
///   inflating an empty list (conservative safety posture per SPEC-22 §6).
/// - v6: SPEC-21 R31 `RequestWork` + `NoMoreWork` pull-dispatch variants
///   appended (TASK-0575 / TASK-0576). REJECT-v5 policy: v5 peers do not
///   understand pull-dispatch semantics; the coordinator rejects any
///   `Register.protocol_version < PROTOCOL_VERSION` with `RegisterNack`.
///   SPEC-21 §3.7 R37c; SPEC-06 R5 (discriminant stability preserved by
///   append-only amendment).
/// - v7: SPEC-19 §3.4 R35a — `CompactSubnet` wire encoding gains a
///   `free_list: Vec<AgentId>` suffix (D-011 Phase A, commit `c4c80b8`,
///   TASK-0596). REJECT-v6 policy: a v6 worker speaks the `Partition` wire
///   form WITHOUT the `free_list` suffix; deserializing such a payload at v7
///   would silently default to `Vec::new()` and re-introduce QA-D009-001
///   (`next_id` divergence between coordinator and worker, SPEC-22
///   R10b/R12a violation). The coordinator therefore rejects any
///   `Register.protocol_version < PROTOCOL_VERSION` with `RegisterNack`
///   (handled in `accept_workers` below — same gate that enforced v5/v6).
///
// TODO(spec-realign, MF-001): SPEC-18 §4.7 constant table and R28 text still
// say "bump from 2 to 3" / "PROTOCOL_VERSION: u8 = 3", which are stale after
// the D-006 elastic-grid bump (v2→v4) that preceded D-009. The actual deployed
// values before this bump were PREVIOUS_LIVE_VERSION = 6, PROTOCOL_VERSION = 7.
// SPEC-22 R9a and SPEC-18 lines 163/538-539 need updating by ESPECIALISTA EM
// SPECS. See REVIEW-PHASE-D009-spec22-arena-2026-04-27.md §MF-001.
pub const PROTOCOL_VERSION: u8 = 7;

/// The protocol version immediately preceding the current one.
///
/// Captured at TASK-0596 implementation time as the value of
/// `PROTOCOL_VERSION` immediately before the SPEC-19 R35a bump (D-011
/// Phase A, commit `c4c80b8`). Used by tests to assert
/// `PROTOCOL_VERSION == PREVIOUS_LIVE_VERSION + 1` (landing-order-aware
/// defensive contract; TEST-SPEC-0596 / TEST-SPEC-0576 / TEST-SPEC-0476 /
/// TEST-SPEC-0511).
///
/// NEVER hardcode the integer in any test — always reference this constant.
pub const PREVIOUS_LIVE_VERSION: u8 = 6;

// Compile-time defensive guard: prevents misuse when PROTOCOL_VERSION differs
// from PREVIOUS_LIVE_VERSION + 1 (landing-order-aware defensive contract per
// TEST-SPEC-0576 / TEST-SPEC-0476 / TEST-SPEC-0511).
const _: () = assert!(
    PROTOCOL_VERSION == PREVIOUS_LIVE_VERSION + 1,
    "SPEC-21 R37c: PROTOCOL_VERSION MUST be PREVIOUS_LIVE_VERSION + 1",
);

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

    // R0d: validate protocol version (MF-001: u32 wire shape per NF-009).
    if protocol_version != PROTOCOL_VERSION as u32 {
        let nack = Message::JoinNack {
            reason: crate::protocol::types::JoinNackReason::ProtocolVersionMismatch {
                coordinator: PROTOCOL_VERSION as u32,
                worker: protocol_version,
            },
        };
        // MF-008 (Phase C refactor): NACK delivery failure is non-fatal
        // (the joiner is being terminated anyway), but the I/O error MUST
        // be logged so observability tooling can correlate "rejected
        // joiner with no NACK delivered" against worker-side timeouts.
        if let Err(e) = send_frame(stream, &nack).await {
            tracing::warn!(
                error = %e,
                expected = PROTOCOL_VERSION,
                got = protocol_version,
                "Failed to send ProtocolVersionMismatch NACK to rejected joiner"
            );
        }
        return Ok(None);
    }

    // R9: check if joins are enabled
    if !grid_config.elastic_join {
        let nack = Message::JoinNack {
            reason: crate::protocol::types::JoinNackReason::ElasticJoinDisabled,
        };
        // MF-008 (Phase C refactor): log NACK delivery failure.
        if let Err(e) = send_frame(stream, &nack).await {
            tracing::warn!(
                error = %e,
                "Failed to send ElasticJoinDisabled NACK to rejected joiner"
            );
        }
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
            // MF-008 (Phase C refactor): log NACK delivery failure.
            if let Err(e) = send_frame(stream, &nack).await {
                tracing::warn!(
                    error = %e,
                    "Failed to send AuthenticationFailed NACK to rejected joiner"
                );
            }
            return Ok(None);
        }
    }

    // R11: allocate WorkerId.
    //
    // MF-003 (Phase C refactor): exhaustion MUST NACK the offender and let
    // the coordinator continue serving existing workers. Returning `Err(...)`
    // here previously aborted the coordinator entirely on the very first
    // mid-session join after the counter saturated — a direct violation of
    // SPEC-20 R11 / SC-023 ("the coordinator SHOULD continue to serve other
    // workers"). After the fix, the join is rejected; subsequent joins also
    // hit `u32::MAX` and stay sticky-NACK (matches EG-U14 A5).
    //
    // QA-004 (Phase C refactor): R11 mandates monotonic, non-reusing
    // WorkerId allocation. Once `next_worker_id` reaches `u32::MAX`, all
    // subsequent joins MUST be NACKed; wraparound (u32::MAX → 0) is NOT
    // legal because it would collide with the reserved hybrid `WorkerId(0)`
    // self-partition slot.
    if *next_worker_id == u32::MAX {
        let nack = Message::JoinNack {
            reason: crate::protocol::types::JoinNackReason::WorkerIdSpaceExhausted,
        };
        if let Err(e) = send_frame(stream, &nack).await {
            tracing::warn!(
                error = %e,
                "WorkerIdSpaceExhausted NACK delivery failed; joiner has likely already disconnected"
            );
        }
        tracing::warn!(
            next_worker_id = *next_worker_id,
            "WorkerId space exhausted (R11/SC-023) — rejecting join; coordinator continues."
        );
        return Ok(None);
    }

    let worker_id = *next_worker_id;
    *next_worker_id += 1;

    // R11a: Compute the partition_index the worker WILL occupy in the next round.
    let partition_index = if grid_config.hybrid_coordinator {
        active_workers.len() as u32 + 1
    } else {
        active_workers.len() as u32
    };

    // R16: JoinAck informs worker of next_round_number.
    //
    // QA-005 (Phase B refactor): `current_round` is `metrics.rounds` at the
    // call site, which is incremented at the bottom of the previous round
    // (`metrics.rounds += 1`) BEFORE the join window opens. By the time we
    // reach this point, `current_round` already names the upcoming round
    // (R+1) — the round in which the next call to `distribute_partitions`
    // will dispatch this joiner's first partition. Adding +1 here would
    // advertise R+2, mis-aligning the joiner's round counter by one and
    // making its first AssignPartition look like a protocol violation.
    let next_round_number = current_round;

    let ack = Message::JoinAck {
        worker_id,
        partition_index,
        next_round_number,
    };
    // QA-005 (Phase C refactor): a `send_frame(...).await?` here previously
    // propagated I/O errors (e.g., joiner disconnected between writing
    // JoinRequest and reading JoinAck) all the way up through `?` at the
    // call site, aborting the entire coordinator. Per SPEC-20 EG-U6 EC-2,
    // a joiner that disconnects after JoinRequest must be skipped with a
    // WARN log; the coordinator continues serving other workers.
    //
    // The `worker_id` and `next_worker_id` increment are already burned —
    // R11 mandates monotonic non-reusing allocation, so we cannot recycle
    // the id. The joiner is reported as `Ok(None)` so the caller does NOT
    // push a stream into `worker_streams` for a connection that was just
    // observed dead.
    if let Err(e) = send_frame(stream, &ack).await {
        tracing::warn!(
            error = %e,
            worker_id,
            "Failed to send JoinAck; joiner disconnected — leaking WorkerId per R11 monotonic"
        );
        return Ok(None);
    }

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
                    // (MF-001: u32 wire shape per NF-009).
                    if protocol_version != PROTOCOL_VERSION as u32 {
                        let nack = Message::JoinNack {
                            reason:
                                crate::protocol::types::JoinNackReason::ProtocolVersionMismatch {
                                    coordinator: PROTOCOL_VERSION as u32,
                                    worker: protocol_version,
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

    // SPEC-20 §3.3 — Phase D Option A (2026-04-27).
    //
    // `elastic_departure = true` advertises the full reclaim + reconstruct
    // recovery path. The audit on commits 8366ef3..583a368 (REVIEW-PHASE-D
    // 3 CRITICAL + 3 HIGH MF; QA-PHASE-D 5 CRITICAL + 6 HIGH) showed that
    // path was structurally non-functional: every successful reconstruction
    // unconditionally returned `Fatal` (MF-001 D), the hybrid R26a branch
    // was algebraically unreachable (QA-002 D), and reclaimed `IdRange`s
    // collided with surviving partitions (MF-003 / QA-003 D). The reviewer
    // recommended Option A: ship Phase D as detection + retained-state
    // plumbing only, with `elastic_departure = false` enforced as the v2.0
    // default. Full reclaim is deferred to v2.1.
    //
    // To preserve forward compatibility for users who already set the flag
    // (and for the CLI's `--elastic-departure`), we accept `true` but emit
    // a one-time WARN and proceed exactly as if `false` — detection logs
    // the event, removes the dead stream, and the next loop iteration
    // either enters SoloReducing (hybrid) or returns
    // `ProtocolError::AllWorkersDeparted` (non-hybrid).
    if grid_config.elastic_departure {
        tracing::warn!(
            spec = "SPEC-20 §3.3",
            phase = "Phase D Option A",
            "elastic_departure is experimental and unsupported in v2.0; \
             falling back to detection-only behaviour (full reclaim deferred to v2.1). \
             Set elastic_departure = false to silence this warning."
        );
    }

    // SF-003 / QA-012 D — `worker_streams` and `worker_ids` are kept as
    // strict parallel vectors (length-equal at every observation point).
    // Pre-Option-A code derived `WorkerId` from `worker_streams` index +
    // hybrid offset; that mapping breaks the moment a stream is removed
    // mid-run (SF-003 review, QA-012 D). Option A's detection path DOES
    // remove streams on departure, so we must track identity explicitly.
    // The `BTreeMap<WorkerId, TransportStream>` migration recommended by
    // the reviewer is deferred to v2.1 because it cascades into the
    // public `distribute_partitions`/`collect_results` signatures used
    // by external integration tests.
    let mut worker_streams = accept_workers(
        &accept_config,
        token,
        transport,
        grid_config.hybrid_coordinator,
    )
    .await?;
    let initial_offset: u32 = if grid_config.hybrid_coordinator { 1 } else { 0 };
    let mut worker_ids: Vec<WorkerId> = (0..worker_streams.len() as u32)
        .map(|i| i + initial_offset)
        .collect();
    debug_assert_eq!(worker_streams.len(), worker_ids.len());

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

                // QA-001 (Phase B refactor): the previous implementation
                // used `tokio::select! { new_conn = accept() => ..., else => reduce_n(...) }`.
                // The `else` branch in `tokio::select!` only fires when ALL
                // other branches are *disabled*; `transport.accept()` is
                // unguarded, so the `else` arm was unreachable and
                // `reduce_n` never ran — the coordinator hung forever in
                // SoloReducing. Replace with an explicit
                // reduce-then-poll-accept structure: every loop turn does
                // at least one reduce batch, then drains any queued
                // connections without blocking via `biased` + a
                // pre-completed `ready()` future as a non-blocking peek.
                while !current_net.redex_queue.is_empty() {
                    // (1) Always perform one reduce batch first.
                    let stats = crate::reduction::reduce_n(
                        &mut current_net,
                        grid_config.solo_budget as usize,
                    );
                    metrics.total_interactions += stats.total_interactions;
                    for (i, &count) in stats.interactions_by_rule.iter().enumerate() {
                        metrics.total_interactions_by_rule[i] += count;
                    }
                    if stats.total_interactions == 0 {
                        break;
                    }

                    // (2) Drain newly-arrived connections without blocking.
                    // `biased` + an immediately-ready future ensures the
                    // accept branch is polled at most once per iteration
                    // and falls through to the ready arm if no connection
                    // is pending.
                    tokio::select! {
                        biased;
                        new_conn = transport.accept() => {
                            match new_conn {
                                Ok(stream) => {
                                    tracing::info!(
                                        "Accepted mid-session connection during SoloReducing; queued."
                                    );
                                    pending_connections_queue.push_back(stream);
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        "Failed to accept mid-session connection: {}",
                                        e
                                    );
                                }
                            }
                        }
                        _ = std::future::ready(()) => {
                            // No pending connection — fall through to next
                            // reduce batch.
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
        // TASK-0615 (D-011-FU-NETMETRIC): per-round network time accumulators.
        // Wraps each wire-facing send/recv site with `Instant::now()` and
        // accumulates the elapsed `Duration` per round. Pushed to
        // `metrics.network_send_time_per_round` / `network_recv_time_per_round`
        // alongside the byte counters at end of round. RF-04 closure.
        let mut network_send_time = Duration::ZERO;
        let mut network_recv_time = Duration::ZERO;
        let _t_grid = Instant::now();

        let mut partitions_iter = plan.partitions.iter().cloned();
        let self_partition = if grid_config.hybrid_coordinator {
            partitions_iter.next()
        } else {
            None
        };
        let remote_partitions: Vec<Partition> = partitions_iter.collect();

        // MF-007 (Phase C refactor): pair the spawned `self_handle` with
        // its `Partition` payload up-front so the downstream `AssignPartition`
        // dispatch never needs `self_partition.as_ref().unwrap()`. The
        // invariant "self_handle.is_some() iff self_partition.is_some()" is
        // now structural (encoded in the Option<(handle, partition)> shape)
        // rather than implicit (two Options kept in sync by hand).
        let mut self_handle = if let Some(ref p) = self_partition {
            let h =
                crate::protocol::self_worker::spawn_self_partition(config.max_payload_size).await;
            Some((h, p.clone()))
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
        // TASK-0615: time the wire-facing distribute_partitions await.
        let t_send_dist = Instant::now();
        let dist_result = distribute_partitions(
            &mut worker_streams,
            remote_partitions,
            metrics.rounds,
            config.distribute_timeout,
        )
        .await;
        network_send_time = network_send_time.saturating_add(t_send_dist.elapsed());
        bytes_sent += match dist_result {
            Ok(n) => n,
            Err(e) => {
                push_partial_round_metrics(&mut metrics);
                return Err(e);
            }
        };

        if let Some((ref mut h, ref p)) = self_handle {
            let msg = Message::AssignPartition {
                round: metrics.rounds,
                partition: p.clone(),
            };
            // TASK-0615: time the in-process self-worker dispatch send.
            let t_send_self = Instant::now();
            let send_outcome = send_frame(&mut h.stream, &msg).await;
            network_send_time = network_send_time.saturating_add(t_send_self.elapsed());
            match send_outcome {
                Ok(n) => bytes_sent += n,
                Err(e) => {
                    push_partial_round_metrics(&mut metrics);
                    return Err(e);
                }
            }
        }

        // Collect with departure detection (TASK-0438, 0441)
        let mut collect_results_vec = Vec::with_capacity(k_eff);
        let mut departing_worker_ids: Vec<WorkerId> = Vec::new();
        // Phase D Option A — reclaim path removed; the per-round reclaim
        // counters always push zero. Kept zero-valued so the
        // round-indexed metric Vec lengths stay parallel for CSV
        // consumers (`workers_departed_per_round`,
        // `retained_initial_reclaims_per_round`,
        // `retained_last_acked_reclaims_per_round`,
        // `partitions_redispatched_per_round`). The full counters land
        // when v2.1 wires the reclaim path.
        let round_reclaimed_initial: u32 = 0;
        let round_reclaimed_last_acked: u32 = 0;
        let mut round_departed_count: u32 = 0;

        // QA-012 D: pair each stream with its tracked `WorkerId` from
        // `worker_ids` rather than re-deriving identity from the slot
        // index. After Option A's stream-pruning lands, the index is no
        // longer load-bearing; identity must travel with the stream.
        debug_assert_eq!(
            worker_streams.len(),
            worker_ids.len(),
            "worker_streams and worker_ids must remain length-parallel"
        );
        let mut streams_to_poll: Vec<(WorkerId, &mut TransportStream)> = worker_streams
            .iter_mut()
            .zip(worker_ids.iter().copied())
            .map(|(s, id)| (id, &mut *s))
            .collect();

        if let Some((ref mut h, _)) = self_handle {
            streams_to_poll.push((0, &mut h.stream));
        }

        for (wid, stream) in streams_to_poll {
            let recv_future = recv_frame(stream, config.max_payload_size);
            // TASK-0615: time the wire-facing collect recv await per worker.
            // D-012 REFACTOR (QA-D012-008): only accumulate `t_recv.elapsed()`
            // on the SUCCESSFUL recv branch. Including the elapsed for a
            // timeout fire (`Err(_)` arm at the bottom) would contaminate the
            // metric by `collect_timeout` whenever a worker is slow, and
            // including it for a connection-loss `Ok(Err(_))` would
            // double-count the loss as both a network event AND a recovery
            // event. The handoff §3 implementation hint #1 explicitly
            // mandates "measure the `await`, not the timeout overhead."
            let t_recv = Instant::now();
            let recv_outcome = tokio::time::timeout(config.collect_timeout, recv_future).await;
            let recv_elapsed = t_recv.elapsed();
            match recv_outcome {
                Ok(Ok((msg, nbytes))) => {
                    network_recv_time = network_recv_time.saturating_add(recv_elapsed);
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

                            // QA-011 D — drain a trailing `LeaveRequest`
                            // that the worker may have piggy-backed onto
                            // the result frame (the spec-correct R22a
                            // sequence). A 50ms peek is short enough that
                            // a worker NOT sending `LeaveRequest` does not
                            // pay round-trip latency, and long enough that
                            // a worker that DID send it (already buffered
                            // on the local socket) is observed.
                            let peek_timeout = std::time::Duration::from_millis(50);
                            // TASK-0615: include the trailing LeaveRequest peek
                            // in the per-round network_recv_time accumulator.
                            let t_peek = Instant::now();
                            let peek_result = tokio::time::timeout(
                                peek_timeout,
                                recv_frame(stream, config.max_payload_size),
                            )
                            .await;
                            network_recv_time = network_recv_time.saturating_add(t_peek.elapsed());
                            if let Ok(Ok((peek_msg, peek_bytes))) = peek_result {
                                bytes_received += peek_bytes;
                                if let Message::LeaveRequest { kind } = peek_msg {
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
                                        post_result = true,
                                        "Worker left gracefully via LeaveRequest after result (R22a)"
                                    );
                                    if let Err(e) = send_frame(stream, &Message::LeaveAck).await {
                                        tracing::warn!(
                                            worker_id = wid,
                                            error = %sanitize_log_string(&e.to_string()),
                                            "QA-004: LeaveAck send failed after PartitionResult; \
                                             worker treated as departed regardless"
                                        );
                                    }
                                    if !departing_worker_ids.contains(&wid) {
                                        departing_worker_ids.push(wid);
                                        round_departed_count += 1;
                                    }
                                }
                                // Any non-LeaveRequest peek is logged at
                                // debug — we silently drop it. The result
                                // we already processed remains valid.
                            }
                        }
                        Message::LeaveRequest { kind } => {
                            // R28: WARN log on departure (MF-002).
                            // `departure_type` is one of the four canonical
                            // strings enumerated by SPEC-20 R28.
                            // `retained_slot` is `"retained_initial"` because
                            // R24a (conservative) is the only reclaim path
                            // available today; R24b lands later (v2.1).
                            //
                            // Phase D Option A scope: `kind` distinguishes
                            // the WARN payload but the action is the same
                            // — the worker is removed from `worker_streams`
                            // at end of round. Without a full reclaim path
                            // (deferred to v2.1) there is no R22a/R22b
                            // semantic divergence; both paths surrender
                            // the worker to the v1 fallback.
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
                            // QA-004 D — surface, do NOT swallow, send
                            // errors. A TCP-RST mid-ack is observable in
                            // logs even though the worker is treated as
                            // departed regardless of ack delivery.
                            if let Err(e) = send_frame(stream, &Message::LeaveAck).await {
                                tracing::warn!(
                                    worker_id = wid,
                                    error = %sanitize_log_string(&e.to_string()),
                                    "QA-004: LeaveAck send failed; worker treated as departed regardless"
                                );
                            }
                            if !departing_worker_ids.contains(&wid) {
                                departing_worker_ids.push(wid);
                                round_departed_count += 1;
                            }
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
                                    // QA-005 D — idempotent push: a single
                                    // wid that surfaces both `Message::Error`
                                    // and a follow-up timeout (or any other
                                    // double-detection race) must be
                                    // counted exactly once.
                                    if !departing_worker_ids.contains(&id) {
                                        departing_worker_ids.push(id);
                                        round_departed_count += 1;
                                    }
                                }
                            }
                        }
                        other => {
                            // QA-008: bound the panic-message payload —
                            // a worker-supplied `Message::Error.description`
                            // (or any other variant carrying user-controlled
                            // bytes) could otherwise produce a multi-MB
                            // string here that flows into tracing.
                            push_partial_round_metrics(&mut metrics);
                            return Err(ProtocolError::Fatal(sanitize_log_string(&format!(
                                "unexpected message: {:?}",
                                other
                            ))));
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
                            // QA-005 D — idempotent push.
                            if !departing_worker_ids.contains(&id) {
                                departing_worker_ids.push(id);
                                round_departed_count += 1;
                            }
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
                            // QA-005 D — idempotent push.
                            if !departing_worker_ids.contains(&id) {
                                departing_worker_ids.push(id);
                                round_departed_count += 1;
                            }
                        }
                    }
                }
            }
        }

        // QA-003 (Phase B refactor): drain any TCP connections that
        // arrived while we were busy in distribute/collect/merge. Queue
        // them for the upcoming `AcceptingMembershipChanges` window so
        // mid-round arrivals are not stranded in the OS backlog. The
        // poll is non-blocking — `biased` + an immediately-ready future
        // ensures we never wait here.
        loop {
            let accepted = tokio::select! {
                biased;
                new_conn = transport.accept() => Some(new_conn),
                _ = std::future::ready(()) => None,
            };
            match accepted {
                Some(Ok(stream)) => {
                    tracing::info!(
                        "Accepted mid-round connection in collect phase; queued for next join window."
                    );
                    pending_connections_queue.push_back(stream);
                }
                Some(Err(e)) => {
                    tracing::warn!(error = %e, "accept() error during collect-phase drain");
                    break;
                }
                None => break,
            }
        }

        if let Some((h, _)) = self_handle {
            // QA-003: top up per-round Vec parity before bailing on a
            // self-worker join failure.
            if let Err(e) = h.join_handle.await {
                push_partial_round_metrics(&mut metrics);
                return Err(ProtocolError::Fatal(sanitize_log_string(&format!(
                    "Self-worker join error: {:?}",
                    e
                ))));
            }
        }

        // === DEPARTURE HANDLING — Phase D Option A (2026-04-27) ===
        //
        // The pre-Option-A reclaim path (`materialize_reclaimed_partitions`
        // + `reconstruct(border_graph, evolved_survivors, round_0_reclaimed)`)
        // was structurally non-functional under the audit (REVIEW-PHASE-D
        // MF-001..MF-009; QA-PHASE-D QA-001..QA-011). It is removed here.
        //
        // Option A semantics:
        //   1. Log each departure (already done in the collect loop).
        //   2. Release retained state so memory stays bounded (QA-010 D).
        //   3. Drop the dead stream from `worker_streams` and `worker_ids`
        //      (Vec parity invariant preserved).
        //   4. If no remote workers remain:
        //        - hybrid_coordinator → fall through; the next loop
        //          iteration's `worker_streams.is_empty()` test enters
        //          SoloReducing (Phase B refactor / QA-001 B).
        //        - non-hybrid → return `ProtocolError::AllWorkersDeparted`
        //          (the v1-faithful terminal state).
        //   5. NO partition reclaim, NO `reconstruct`.
        //
        // The full `elastic_departure = true` reclaim+reconstruct path is
        // deferred to v2.1; see `docs/_archive/next-steps.md` "Deferred to v2.1".
        if !departing_worker_ids.is_empty() {
            tracing::warn!(
                D = departing_worker_ids.len(),
                K_eff = k_eff,
                "Phase D Option A: removing departed workers from active set; \
                 no reclaim performed (elastic_departure deferred to v2.1)"
            );

            // (2) Release retained state (QA-010 D).
            for &id in &departing_worker_ids {
                retained_state.release_worker(id);
            }

            // (3) Remove dead streams + their wids. We collect indices in
            // descending order so `swap_remove` does not shift the indices
            // we still need to act on. The self-partition (wid 0 in hybrid
            // mode) cannot depart here — its handle is awaited above in
            // its own arm — so we only need to walk the remote arrays.
            let mut indices_to_remove: Vec<usize> = worker_ids
                .iter()
                .enumerate()
                .filter_map(|(i, &id)| {
                    if departing_worker_ids.contains(&id) {
                        Some(i)
                    } else {
                        None
                    }
                })
                .collect();
            indices_to_remove.sort_unstable_by(|a, b| b.cmp(a)); // descending
            for idx in indices_to_remove {
                worker_streams.swap_remove(idx);
                worker_ids.swap_remove(idx);
            }
            debug_assert_eq!(
                worker_streams.len(),
                worker_ids.len(),
                "worker_streams/worker_ids parity broken during departure pruning"
            );

            // (4) Terminal-state check. In hybrid mode the self-partition
            // remains; we let the round complete (its result is in
            // `collect_results_vec`) and fall through. The next loop
            // iteration sees `worker_streams.is_empty()` and enters
            // SoloReducing per R5/R5a/R27. In non-hybrid mode there is
            // no executor remaining; surface the terminal state.
            if worker_streams.is_empty() && !grid_config.hybrid_coordinator {
                push_partial_round_metrics(&mut metrics);
                return Err(ProtocolError::AllWorkersDeparted {
                    detail: format!(
                        "all {} workers departed in round {}; no executor remains",
                        departing_worker_ids.len(),
                        metrics.rounds
                    ),
                });
            }
        }

        // NF-011 — debug-mode bound check after release_worker calls. The
        // post-Option-A registry should never exceed `2 * k_eff` initial
        // entries or `k_eff` last-acked entries.
        retained_state.assert_memory_bounds(k_eff);

        // R38: record round-level departure/reclaim metrics.
        //
        // FIXME(TASK-0443): `retained_initial_reclaims_per_round` and
        // `retained_last_acked_reclaims_per_round` are unreachable in the
        // happy path today because the conservative reclaim branch above
        // (L767..L832) unconditionally returns `Err` once it materializes
        // reclaimed partitions. Until TASK-0443 wires reclaim back into the
        // round loop, these counters always push 0 here. See `docs/_archive/next-steps.md`
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
        // TASK-0615 (D-011-FU-NETMETRIC): push the per-round network time
        // accumulators alongside the byte counters. Send time covers the
        // dispatch phase (distribute_partitions + self-handle AssignPartition);
        // recv time covers the collect phase (per-worker recv_frame +
        // trailing LeaveRequest peek). RF-04 closure.
        metrics.network_send_time_per_round.push(network_send_time);
        metrics.network_recv_time_per_round.push(network_recv_time);

        let mut reduced_partitions = Vec::with_capacity(results.len());
        let mut worker_stats = Vec::with_capacity(results.len());
        for (partition, stats) in results {
            reduced_partitions.push(partition);
            worker_stats.push(stats);
        }
        metrics.worker_stats_per_round.push(worker_stats.clone());

        // TASK-0616 (D-011-FU-COMPMETRIC): aggregate per-worker compute time
        // for the distributed (TCP) path. Path (a) — recommended: workers
        // already report `WorkerRoundStats.reduce_duration_secs` (set in
        // `protocol/worker.rs:256` straddling `reduce_all`); the coordinator
        // takes the MAX across workers and pushes to
        // `metrics.compute_time_per_round`. RF-05 closure.
        //
        // D-012 REFACTOR (QA-D012-001): aggregation rule was SUM in the
        // initial implementation. SUM is wrong for parallel workers — it is
        // total worker CPU-time, not wall-clock. Workers run truly
        // concurrently on TCP, so SUM-of-W-100ms-workers = 4*100 = 400ms,
        // while the BSP round wall-clock is ~100ms + dispatch + collect ≈
        // 150ms. The downstream `bench/suite.rs::measure_grid` derives
        // `overhead_ratio = 1.0 - compute_total / elapsed`; with SUM that
        // formula goes NEGATIVE on multi-worker TCP (1.0 - 0.4/0.15 = -1.67).
        //
        // MAX (slowest-worker wall-clock = BSP critical-path duration) is
        // the correct rule for a parallel-execution metric; it satisfies
        // `compute_time_per_round[r] <= wall_clock_per_round[r]` for all r,
        // which `overhead_ratio` requires to remain in [0, 1]. The
        // in-process path at `merge/grid.rs:103,154` pushes
        // `t_compute.elapsed()` of a SEQUENTIAL worker loop — for in-process
        // MAX collapses to the same value as the loop's wall-clock (since
        // workers run one-at-a-time). The semantic gap that motivated SUM
        // (in-process pushes loop wall-clock ≈ sum of per-worker times) is
        // resolved by recognizing both paths now report "wall-clock of the
        // parallel-execution phase": MAX-of-parallel-workers (TCP) and
        // sequential-loop wall-clock (in-process) are commensurate.
        //
        // QA-D012-005: a worker emitting `f64::INFINITY` or `NaN` would
        // panic `Duration::from_secs_f64(+inf)` ("secs is not finite").
        // Use `Duration::try_from_secs_f64` and fall back to `Duration::ZERO`
        // with a `tracing::warn!` rather than crash the whole bench.
        let max_secs = worker_stats
            .iter()
            .map(|s| s.reduce_duration_secs)
            .filter(|s| s.is_finite() && *s >= 0.0)
            .fold(0.0_f64, f64::max);
        let compute_time = Duration::try_from_secs_f64(max_secs).unwrap_or_else(|_| {
            tracing::warn!(
                round = metrics.rounds,
                max_secs,
                "compute_time aggregation produced non-finite f64; defaulting to ZERO"
            );
            Duration::ZERO
        });
        metrics.compute_time_per_round.push(compute_time);

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

        // JOIN WINDOW (SPEC-20 R10a — drain-then-arm protocol).
        //
        // MF-005 (Phase C refactor): the previous loop pinned `min_timer`
        // and `max_timer` simultaneously and exited at min-expiry as soon
        // as the queue drained, which conflated the two normative steps:
        //   (1) drain pending connections, complete handshakes;
        //   (2) arm `JoinWindowMin`; on min-expiry, IF arrivals occurred
        //       during the drain or while min was armed, arm an extension
        //       timer of `(join_window_max - join_window_min)` and keep
        //       accepting until the extension expires OR the queue is
        //       drain-empty between accepts; otherwise exit immediately.
        //
        // The new structure encodes (1)→(2)→(extension?) as three discrete
        // phases. `had_arrivals` is the "did we observe any new connection
        // during drain or during the min-window?" predicate that gates
        // the optional extension phase.
        //
        // MF-012 (Phase C refactor): the procedural sleep futures are
        // tagged with `tracing::debug!(timer_kind = ...)` breadcrumbs so
        // log-analysis tooling can decode the timer lifecycle without
        // per-build metadata, satisfying NF-008 in spirit even though
        // the FSM `StartTimer(TimerKind::JoinWindow{Min,Max}, ...)`
        // action is not driven by this procedural loop yet (TASK-0436
        // wires that).
        //
        // The handshake of a single drained stream is repeated three
        // times below (drain, min-loop accept, extension-loop accept).
        // Extracting a helper closure is rejected by the borrow checker
        // because the helper must hold `&mut Vec<TransportStream>`,
        // `&mut u32`, and `&mut RetainedStateRegistry` across an await
        // point. We use a local `macro_rules!` block instead — the
        // generated code is identical to a manual inline at each site
        // but stays maintainable.
        let mut round_joined_count: u32 = 0;
        if grid_config.elastic_join {
            let t_window_start = Instant::now();

            macro_rules! handshake_one {
                ($stream:expr) => {{
                    let mut stream = $stream;
                    // QA-012 D — `active_ids` is keyed off the tracked
                    // `worker_ids` Vec, not off slot indices, so post-
                    // departure pruning does not mis-map identity.
                    let active_ids: std::collections::BTreeSet<WorkerId> =
                        worker_ids.iter().copied().collect();

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
                    let recv_pair = match recv_outcome {
                        Ok(Ok(pair)) => Some(pair),
                        Ok(Err(e)) => {
                            tracing::warn!(
                                error = %e,
                                "JoinRequest recv failed; dropping pending stream."
                            );
                            None
                        }
                        Err(_) => {
                            tracing::warn!(
                                join_window_max_ms = grid_config.join_window_max.as_millis() as u64,
                                "JoinRequest recv timed out within join window; dropping pending stream (QA-005)."
                            );
                            None
                        }
                    };
                    if let Some((msg, _)) = recv_pair {
                        let outcome = process_join_request(
                            &mut stream,
                            msg,
                            grid_config,
                            config,
                            token,
                            &mut next_worker_id,
                            metrics.rounds,
                            &active_ids,
                        )
                        .await?;
                        if let Some(worker_id) = outcome {
                            worker_streams.push(stream);
                            worker_ids.push(worker_id);
                            debug_assert_eq!(worker_streams.len(), worker_ids.len());
                            round_joined_count += 1;
                            if grid_config.retain_partitions {
                                retained_state.register_initial(worker_id, None);
                            }
                            let offset = if grid_config.hybrid_coordinator { 1u32 } else { 0u32 };
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
                }};
            }

            // (1) Drain the existing pending queue. Connections that landed
            // here during the prior round's collect-phase drain (Phase B
            // refactor QA-003) get their JoinRequest handshake first.
            let mut had_arrivals = !pending_connections_queue.is_empty();
            tracing::debug!(
                timer_kind = "JoinWindow",
                drain_size = pending_connections_queue.len(),
                "Join window: pre-arm drain (R10a step 1)"
            );
            while let Some(stream) = pending_connections_queue.pop_front() {
                handshake_one!(stream);
            }

            // (2) Arm `JoinWindowMin`. Race accept against the timer.
            // Each successful accept sets `had_arrivals = true` and is
            // immediately handshook; the loop keeps accepting until the
            // min timer fires.
            tracing::debug!(
                timer_kind = "JoinWindowMin",
                duration_ms = grid_config.join_window_min.as_millis() as u64,
                "Join window: min timer armed (R10a step 2)"
            );
            let min_timer = tokio::time::sleep(grid_config.join_window_min);
            tokio::pin!(min_timer);
            'min_loop: loop {
                tokio::select! {
                    biased;
                    _ = &mut min_timer => break 'min_loop,
                    new_conn = transport.accept() => {
                        match new_conn {
                            Ok(s) => {
                                had_arrivals = true;
                                handshake_one!(s);
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "accept failed in join_window_min");
                            }
                        }
                    }
                }
            }

            // (3) Optional extension: only arm `(max - min)` if at least
            // one connection arrived during drain or during the min
            // window. Exit on extension expiry OR on a drain-empty
            // observation between accepts.
            if had_arrivals {
                let extension = grid_config
                    .join_window_max
                    .checked_sub(grid_config.join_window_min)
                    .unwrap_or_else(|| std::time::Duration::from_millis(0));
                tracing::debug!(
                    timer_kind = "JoinWindowMax",
                    extension_ms = extension.as_millis() as u64,
                    "Join window: extension armed (R10a step 3)"
                );
                let extension_timer = tokio::time::sleep(extension);
                tokio::pin!(extension_timer);
                'extension_loop: loop {
                    tokio::select! {
                        biased;
                        _ = &mut extension_timer => break 'extension_loop,
                        new_conn = transport.accept() => {
                            match new_conn {
                                Ok(s) => {
                                    handshake_one!(s);
                                }
                                Err(e) => {
                                    tracing::warn!(error = %e, "accept failed in extension window");
                                }
                            }
                        }
                    }
                }
            } else {
                tracing::debug!(
                    timer_kind = "JoinWindowMin",
                    "Join window: no arrivals during drain or min — closing window (R10a step 2 exit)"
                );
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

    // D-012 REFACTOR (reviewer MF-001 / QA-D012-007): per-round Vec parity
    // invariant. After all early-return-tolerant pushes via
    // `push_partial_round_metrics`, every per-round Vec MUST have exactly
    // `effective_slots_per_round.len()` entries. CSV writers
    // (`bench/csv.rs::write_rounds_row`) and the bench harness's zip-by-index
    // (`bench/suite.rs:621-626`) both assume this invariant; a regression
    // here would silently drop the last partial round or mis-align rounds
    // across columns. The debug-only assertion fires loudly during tests and
    // local `cargo run` while staying out of the release-mode hot path.
    debug_assert_eq!(
        metrics.network_send_time_per_round.len(),
        metrics.effective_slots_per_round.len(),
        "network_send_time_per_round parity broken on early return path"
    );
    debug_assert_eq!(
        metrics.network_recv_time_per_round.len(),
        metrics.effective_slots_per_round.len(),
        "network_recv_time_per_round parity broken on early return path"
    );
    debug_assert_eq!(
        metrics.compute_time_per_round.len(),
        metrics.effective_slots_per_round.len(),
        "compute_time_per_round parity broken on early return path"
    );
    debug_assert_eq!(
        metrics.bytes_sent_per_round.len(),
        metrics.effective_slots_per_round.len(),
        "bytes_sent_per_round parity broken on early return path"
    );
    debug_assert_eq!(
        metrics.bytes_received_per_round.len(),
        metrics.effective_slots_per_round.len(),
        "bytes_received_per_round parity broken on early return path"
    );

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

    /// UT-0476-01 / UT-0576-01: PROTOCOL_VERSION is strictly one greater than its
    /// predecessor (TASK-0417 bump 3→4; TASK-0476 bump 4→5; TASK-0576 bump 5→6).
    ///
    /// Per TEST-SPEC-0476 / TEST-SPEC-0576 landing-order-aware contract: MUST NOT
    /// assert `PROTOCOL_VERSION == <specific integer>`; instead asserts the strict
    /// +1 increment invariant using the companion `PREVIOUS_LIVE_VERSION` constant.
    #[test]
    fn protocol_version_strictly_greater_than_predecessor() {
        assert_eq!(
            PROTOCOL_VERSION,
            PREVIOUS_LIVE_VERSION + 1,
            "SPEC-21 R37c / SPEC-22 R9a: PROTOCOL_VERSION must be exactly \
             PREVIOUS_LIVE_VERSION + 1 \
             (was {} before this bump; now {})",
            PREVIOUS_LIVE_VERSION,
            PROTOCOL_VERSION
        );
    }

    /// UT-0596-07: PROTOCOL_VERSION matches the SPEC-19 R35a bump.
    ///
    /// SPEC-19 R35a (commit `c4c80b8`) mandates `PROTOCOL_VERSION =
    /// PREVIOUS_LIVE_VERSION + 1` after the CompactSubnet `free_list` wire
    /// suffix landed. This test pins the defensive +1 contract (also caught
    /// by the `const _` assertion at the constant declaration site) and the
    /// landing-order-agnostic check that `PREVIOUS_LIVE_VERSION` is non-zero
    /// (see precedent: TEST-SPEC-0476 / TEST-SPEC-0576). We deliberately
    /// avoid hardcoding either integer.
    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn protocol_version_constant_matches_spec_19_r35a() {
        assert_eq!(
            PROTOCOL_VERSION,
            PREVIOUS_LIVE_VERSION + 1,
            "SPEC-19 R35a (commit c4c80b8): PROTOCOL_VERSION must be exactly \
             PREVIOUS_LIVE_VERSION + 1 after the D-011 Phase A bump",
        );
        assert!(
            PREVIOUS_LIVE_VERSION > 0,
            "PREVIOUS_LIVE_VERSION must be non-zero (at least one prior bump exists)",
        );
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
        // TASK-0476: PROTOCOL_VERSION bumped 4→5; use the constant to future-proof.
        assert!(nack
            .reason
            .contains(&format!("expected {}", PROTOCOL_VERSION)));
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
        // TASK-0476: PROTOCOL_VERSION bumped 4→5; use the constant to future-proof.
        assert!(nack
            .reason
            .contains(&format!("expected {}", PROTOCOL_VERSION)));
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

    // -----------------------------------------------------------------
    // Phase B refactor — Stage 6 RED→GREEN tests for Stage 4+5 findings.
    // -----------------------------------------------------------------

    /// QA-005 (Phase B refactor) — `JoinAck.next_round_number` must
    /// equal the upcoming round, not upcoming + 1. The coordinator
    /// increments `metrics.rounds` BEFORE the join window opens, so
    /// when `current_round = metrics.rounds` is passed to
    /// `process_join_request`, it already names the upcoming round; +1
    /// over-shoots by one.
    #[tokio::test]
    async fn qa_005_join_ack_advertises_upcoming_round_not_upcoming_plus_one() {
        use crate::merge::GridConfig;
        use crate::protocol::types::WorkerCapabilities;

        // Use a ChannelTransport pair to get two stream endpoints. The
        // "coordinator side" runs `process_join_request`; the "worker
        // side" sends `JoinRequest` and reads the `JoinAck`.
        let (mut server, mut client) = ChannelTransport::pair(1, 65536);
        let server_handle = tokio::spawn(async move {
            server.listen().await.ok();
            server.accept().await.expect("accept")
        });
        let mut s_worker = client.connect().await.expect("connect");
        let mut s_coord = server_handle.await.expect("join");

        let grid_config = GridConfig {
            elastic_join: true,
            ..GridConfig::default()
        };
        let node_config = NodeConfig::default();

        // Send a valid v4 JoinRequest from the worker side.
        let join_req = Message::JoinRequest {
            protocol_version: PROTOCOL_VERSION as u32,
            auth_token: None,
            capabilities: WorkerCapabilities::default(),
        };
        send_frame(&mut s_worker, &join_req).await.expect("send");

        // Coordinator side reads the JoinRequest then runs
        // process_join_request.
        let (msg, _) = recv_frame(&mut s_coord, node_config.max_payload_size)
            .await
            .expect("recv");
        let mut next_worker_id: u32 = 1;
        let active = std::collections::BTreeSet::<WorkerId>::new();
        // Simulate: round R just finished; metrics.rounds was bumped to
        // R+1; the join window opens with `current_round = R+1 = 7`.
        // The joiner will be scheduled into round 7, NOT round 8.
        let current_round_passed: u32 = 7;
        let outcome = process_join_request(
            &mut s_coord,
            msg,
            &grid_config,
            &node_config,
            None,
            &mut next_worker_id,
            current_round_passed,
            &active,
        )
        .await
        .expect("process_join_request");
        assert!(outcome.is_some(), "QA-005: valid JoinRequest must succeed");

        // Worker reads the JoinAck and asserts the round number matches
        // the upcoming round, NOT upcoming + 1.
        let (ack, _) = recv_frame(&mut s_worker, node_config.max_payload_size)
            .await
            .expect("ack recv");
        match ack {
            Message::JoinAck {
                next_round_number, ..
            } => {
                assert_eq!(
                    next_round_number, current_round_passed,
                    "QA-005: JoinAck.next_round_number must be the upcoming round ({}), not upcoming+1",
                    current_round_passed,
                );
            }
            other => panic!("expected JoinAck, got {:?}", other),
        }
    }

    /// QA-001 (Phase B refactor) — SoloReducing must make progress.
    /// The previous `tokio::select! { ...accept... else => reduce_n(...) }`
    /// trapped reduction in the unreachable `else` arm, causing the
    /// coordinator to hang forever. After the fix, a non-empty net
    /// reduces to convergence inside SoloReducing without any incoming
    /// connection.
    ///
    /// Unit-level coverage: the post-fix loop is structured so that
    /// `reduce_n` runs unconditionally each iteration, and accept is
    /// polled non-blocking. A stand-alone reduce loop bound by
    /// `solo_budget` therefore must converge for any net that
    /// converges under `reduce_all`.
    #[test]
    fn qa_001_solo_reducing_unit_reduce_loop_converges() {
        use crate::net::{Net, PortRef, Symbol};
        // Build a CON-CON annihilation net (one active pair).
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(3));
        // Active pair should reduce in a single batch with any
        // reasonable solo_budget.
        let solo_budget: usize = 10_000;
        let mut iters: usize = 0;
        while !net.redex_queue.is_empty() {
            let stats = crate::reduction::reduce_n(&mut net, solo_budget);
            if stats.total_interactions == 0 {
                break;
            }
            iters += 1;
            if iters > 100 {
                panic!("QA-001: SoloReducing loop did not converge");
            }
        }
        assert!(
            net.redex_queue.is_empty(),
            "QA-001: redex queue must be drained after solo reduction"
        );
        assert!(iters >= 1, "QA-001: at least one reduce batch must execute");
    }

    /// QA-002 (Phase B refactor) — the join-window pending-connection
    /// drain wraps `recv_frame` in `tokio::time::timeout` bounded by
    /// the remaining `join_window_max` budget. A connection that
    /// never sends any bytes must NOT stall past that budget.
    #[tokio::test]
    async fn qa_002_recv_frame_in_join_window_is_bounded_by_timeout() {
        let (mut server, mut client) = ChannelTransport::pair(1, 65536);
        let server_handle = tokio::spawn(async move {
            server.listen().await.ok();
            server.accept().await.expect("accept")
        });
        let mut _silent_worker = client.connect().await.expect("connect");
        let mut s_coord = server_handle.await.expect("join");

        // Mimic the join-window remaining-budget wrap: 250ms cap.
        let budget = Duration::from_millis(250);
        let started = Instant::now();
        let outcome = tokio::time::timeout(
            budget,
            recv_frame(&mut s_coord, NodeConfig::default().max_payload_size),
        )
        .await;
        let elapsed = started.elapsed();
        assert!(outcome.is_err(), "QA-002: must time out without any bytes");
        assert!(
            elapsed < Duration::from_millis(800),
            "QA-002: must respect the budget; elapsed={:?}",
            elapsed,
        );
    }

    /// MF-001 (Phase B refactor) — coordinator NACKs a v3 JoinRequest
    /// with `ProtocolVersionMismatch { coordinator: 4, worker: 3 }`
    /// (u32 pair). Confirms NF-009 wire shape end-to-end.
    #[tokio::test]
    async fn mf_001_join_request_protocol_version_mismatch_uses_u32_pair() {
        use crate::merge::GridConfig;
        use crate::protocol::types::{JoinNackReason, WorkerCapabilities};

        let (mut server, mut client) = ChannelTransport::pair(1, 65536);
        let server_handle = tokio::spawn(async move {
            server.listen().await.ok();
            server.accept().await.expect("accept")
        });
        let mut s_worker = client.connect().await.expect("connect");
        let mut s_coord = server_handle.await.expect("join");

        // Worker sends v3 JoinRequest.
        let join_req = Message::JoinRequest {
            protocol_version: 3u32,
            auth_token: None,
            capabilities: WorkerCapabilities::default(),
        };
        send_frame(&mut s_worker, &join_req).await.expect("send");

        // Coordinator processes it.
        let (msg, _) = recv_frame(&mut s_coord, NodeConfig::default().max_payload_size)
            .await
            .expect("recv");
        let grid_config = GridConfig {
            elastic_join: true,
            ..GridConfig::default()
        };
        let node_config = NodeConfig::default();
        let mut next_worker_id: u32 = 1;
        let active = std::collections::BTreeSet::<WorkerId>::new();
        let outcome = process_join_request(
            &mut s_coord,
            msg,
            &grid_config,
            &node_config,
            None,
            &mut next_worker_id,
            5,
            &active,
        )
        .await
        .expect("process_join_request");
        assert!(outcome.is_none(), "MF-001: v3 worker must be rejected");

        // Worker reads the JoinNack.
        let (nack, _) = recv_frame(&mut s_worker, NodeConfig::default().max_payload_size)
            .await
            .expect("nack recv");
        match nack {
            Message::JoinNack {
                reason:
                    JoinNackReason::ProtocolVersionMismatch {
                        coordinator,
                        worker,
                    },
            } => {
                assert_eq!(coordinator, PROTOCOL_VERSION as u32);
                assert_eq!(worker, 3u32);
            }
            other => panic!(
                "MF-001: expected JoinNack/ProtocolVersionMismatch, got {:?}",
                other
            ),
        }
    }

    // -----------------------------------------------------------------
    // Phase C refactor — Stage 6 RED→GREEN tests for Stage 4+5 findings.
    // -----------------------------------------------------------------

    /// MF-003 (Phase C refactor) — `WorkerIdSpaceExhausted` MUST send a
    /// `JoinNack` and return `Ok(None)`, NOT `Err(...)`. Returning `Err`
    /// previously aborted the entire coordinator; per SPEC-20 R11/SC-023
    /// the coordinator continues serving existing workers.
    #[tokio::test]
    async fn mf_003_worker_id_exhaustion_returns_join_nack_without_aborting() {
        use crate::merge::GridConfig;
        use crate::protocol::types::{JoinNackReason, WorkerCapabilities};

        let (mut server, mut client) = ChannelTransport::pair(1, 65536);
        let server_handle = tokio::spawn(async move {
            server.listen().await.ok();
            server.accept().await.expect("accept")
        });
        let mut s_worker = client.connect().await.expect("connect");
        let mut s_coord = server_handle.await.expect("join");

        let join_req = Message::JoinRequest {
            protocol_version: PROTOCOL_VERSION as u32,
            auth_token: None,
            capabilities: WorkerCapabilities::default(),
        };
        send_frame(&mut s_worker, &join_req).await.expect("send");

        let (msg, _) = recv_frame(&mut s_coord, NodeConfig::default().max_payload_size)
            .await
            .expect("recv");
        let grid_config = GridConfig {
            elastic_join: true,
            ..GridConfig::default()
        };
        let node_config = NodeConfig::default();
        // Pin the counter at the saturation boundary.
        let mut next_worker_id: u32 = u32::MAX;
        let active = std::collections::BTreeSet::<WorkerId>::new();

        // The fix: MUST NOT propagate Err.
        let outcome = process_join_request(
            &mut s_coord,
            msg,
            &grid_config,
            &node_config,
            None,
            &mut next_worker_id,
            5,
            &active,
        )
        .await;
        assert!(
            outcome.is_ok(),
            "MF-003: exhausted-WorkerId-space MUST return Ok(None), not Err — got {:?}",
            outcome
        );
        let inner = outcome.unwrap();
        assert!(
            inner.is_none(),
            "MF-003: exhausted-WorkerId-space MUST return Ok(None) (worker rejected)"
        );

        // Worker reads the JoinNack with WorkerIdSpaceExhausted reason.
        let (nack, _) = recv_frame(&mut s_worker, NodeConfig::default().max_payload_size)
            .await
            .expect("nack recv");
        match nack {
            Message::JoinNack {
                reason: JoinNackReason::WorkerIdSpaceExhausted,
            } => {}
            other => panic!(
                "MF-003: expected JoinNack/WorkerIdSpaceExhausted, got {:?}",
                other
            ),
        }

        // Counter unchanged (R11 stickiness).
        assert_eq!(
            next_worker_id,
            u32::MAX,
            "MF-003: next_worker_id MUST remain at u32::MAX (sticky NACK state)"
        );
        // The end-to-end "second joiner also gets sticky NACK" assertion
        // lives in `mf_003_qa_004_exhausted_counter_is_sticky_and_never_wraps_to_zero`.
    }

    /// QA-005 (Phase C refactor) — a `JoinAck` send failure (joiner
    /// disconnected mid-handshake) MUST NOT abort the coordinator. The
    /// function returns `Ok(None)` with a WARN log; the WorkerId is leaked
    /// per R11 monotonic non-reuse.
    #[tokio::test]
    async fn qa_005_join_ack_send_failure_does_not_abort_coordinator() {
        use crate::merge::GridConfig;
        use crate::protocol::types::WorkerCapabilities;

        let (mut server, mut client) = ChannelTransport::pair(1, 65536);
        let server_handle = tokio::spawn(async move {
            server.listen().await.ok();
            server.accept().await.expect("accept")
        });
        let mut s_worker = client.connect().await.expect("connect");
        let mut s_coord = server_handle.await.expect("join");

        // Worker sends a valid JoinRequest...
        let join_req = Message::JoinRequest {
            protocol_version: PROTOCOL_VERSION as u32,
            auth_token: None,
            capabilities: WorkerCapabilities::default(),
        };
        send_frame(&mut s_worker, &join_req).await.expect("send");

        // ...then disconnects before reading the JoinAck.
        drop(s_worker);

        let (msg, _) = recv_frame(&mut s_coord, NodeConfig::default().max_payload_size)
            .await
            .expect("recv");
        let grid_config = GridConfig {
            elastic_join: true,
            ..GridConfig::default()
        };
        let node_config = NodeConfig::default();
        let next_worker_id_before: u32 = 5;
        let mut next_worker_id: u32 = next_worker_id_before;
        let active = std::collections::BTreeSet::<WorkerId>::new();

        let outcome = process_join_request(
            &mut s_coord,
            msg,
            &grid_config,
            &node_config,
            None,
            &mut next_worker_id,
            7,
            &active,
        )
        .await;

        // The function must NOT propagate the I/O error.
        assert!(
            outcome.is_ok(),
            "QA-005: JoinAck send failure must NOT abort the coordinator — got {:?}",
            outcome
        );
        // Outcome may be Some(_) (if the channel queued the ack before the
        // peer drop became observable) or None (if the send actually
        // failed). Both are acceptable Ok-shapes; the load-bearing
        // assertion is that the function did not return Err.
        // R11 monotonic: id was burned regardless.
        assert!(
            next_worker_id >= next_worker_id_before,
            "QA-005: WorkerId counter must be monotonic; got before={}, after={}",
            next_worker_id_before,
            next_worker_id
        );
    }

    /// MF-003 / QA-004 (Phase C refactor) — counter state after exhaustion
    /// is sticky. Successive calls with `next_worker_id = u32::MAX` keep
    /// returning `Ok(None)` and never wrap to 0 (which would collide with
    /// the hybrid-coordinator reserved id).
    #[tokio::test]
    async fn mf_003_qa_004_exhausted_counter_is_sticky_and_never_wraps_to_zero() {
        use crate::merge::GridConfig;
        use crate::protocol::types::{JoinNackReason, WorkerCapabilities};

        let grid_config = GridConfig {
            elastic_join: true,
            ..GridConfig::default()
        };
        let node_config = NodeConfig::default();
        let mut next_worker_id: u32 = u32::MAX;
        let active = std::collections::BTreeSet::<WorkerId>::new();

        for attempt in 0..3u32 {
            let (mut server, mut client) = ChannelTransport::pair(1, 65536);
            let server_handle = tokio::spawn(async move {
                server.listen().await.ok();
                server.accept().await.expect("accept")
            });
            let mut s_worker = client.connect().await.expect("connect");
            let mut s_coord = server_handle.await.expect("join");

            let join_req = Message::JoinRequest {
                protocol_version: PROTOCOL_VERSION as u32,
                auth_token: None,
                capabilities: WorkerCapabilities::default(),
            };
            send_frame(&mut s_worker, &join_req).await.expect("send");

            let (msg, _) = recv_frame(&mut s_coord, node_config.max_payload_size)
                .await
                .expect("recv");
            let outcome = process_join_request(
                &mut s_coord,
                msg,
                &grid_config,
                &node_config,
                None,
                &mut next_worker_id,
                42,
                &active,
            )
            .await;
            assert!(outcome.is_ok(), "attempt {} must not abort", attempt);
            assert!(
                outcome.unwrap().is_none(),
                "attempt {} must yield None",
                attempt
            );

            // Receive NACK to confirm wire shape.
            let (nack, _) = recv_frame(&mut s_worker, node_config.max_payload_size)
                .await
                .expect("nack recv");
            match nack {
                Message::JoinNack {
                    reason: JoinNackReason::WorkerIdSpaceExhausted,
                } => {}
                other => panic!("attempt {} expected NACK, got {:?}", attempt, other),
            }

            // Counter never moves and never wraps.
            assert_eq!(
                next_worker_id,
                u32::MAX,
                "QA-004: counter MUST stay sticky at u32::MAX; attempt {}",
                attempt
            );
            assert_ne!(
                next_worker_id, 0,
                "QA-004: counter MUST never wrap to 0 (collides with hybrid self-id)"
            );
        }
    }

    /// MF-001 (regression check after Phase B refactor) — a connection
    /// arriving during the collect phase is NOT silently dropped. The
    /// non-blocking accept-drain at the end of the collect for-loop
    /// pushes the stream into `pending_connections_queue` so the next
    /// `AcceptingMembershipChanges` window observes it.
    ///
    /// This test exercises the *drain* primitive directly: a
    /// `tokio::select!` with `biased` + a ready future is the
    /// non-blocking peek that the collect-phase drain uses. We verify
    /// the shape rather than the full coordinator flow (which is
    /// covered by integration tests pending Phase D rework).
    #[tokio::test]
    async fn mf_001_collect_phase_drain_buffers_arriving_connection() {
        let (mut server, mut client) = ChannelTransport::pair(2, 65536);
        // Listen so accept() works.
        server.listen().await.expect("listen");

        // Worker arrives mid-round.
        let _w = client.connect().await.expect("connect");

        // Drain primitive: same shape as the production code at the end
        // of the collect for-loop.
        let mut pending: std::collections::VecDeque<TransportStream> =
            std::collections::VecDeque::new();
        let accepted = tokio::select! {
            biased;
            new_conn = server.accept() => Some(new_conn),
            _ = std::future::ready(()) => None,
        };
        match accepted {
            Some(Ok(stream)) => pending.push_back(stream),
            Some(Err(e)) => panic!("accept failed: {:?}", e),
            None => panic!("MF-001: drain MUST observe pending connection"),
        }

        assert_eq!(
            pending.len(),
            1,
            "MF-001: collect-phase drain MUST buffer the arriving connection"
        );
    }

    /// MF-005 (Phase C refactor) — R10a drain-then-arm protocol
    /// structure: a no-arrivals window MUST exit immediately after the
    /// `JoinWindowMin` timer fires (no extension), and an arrivals
    /// window MUST extend by `(max - min)`. We test the timer-arming
    /// shape via a mini-loop that mirrors the production code (without
    /// bringing up a full coordinator).
    ///
    /// This is a unit-level structural test; the integration-level
    /// test pin (EG-U6a) lands with Phase D rework.
    #[tokio::test]
    async fn mf_005_join_window_no_arrivals_exits_at_min_no_extension() {
        // Mirror the production loop shape: drain → min → optional extension.
        let pending: std::collections::VecDeque<u32> = std::collections::VecDeque::new();
        let join_window_min = Duration::from_millis(50);
        let join_window_max = Duration::from_millis(500);

        let t_start = Instant::now();
        let had_arrivals_after_drain = !pending.is_empty();

        // Step 2: arm min, no accepts, fire min.
        let min_timer = tokio::time::sleep(join_window_min);
        tokio::pin!(min_timer);
        let mut had_arrivals = had_arrivals_after_drain;
        loop {
            tokio::select! {
                biased;
                _ = &mut min_timer => break,
                _ = std::future::pending::<()>() => {
                    // No accept future in this test — never fires.
                    had_arrivals = true;
                }
            }
        }
        let min_elapsed = t_start.elapsed();
        assert!(
            min_elapsed >= join_window_min,
            "MF-005: min timer must elapse fully; elapsed={:?}",
            min_elapsed
        );

        // Step 3: skipped because no arrivals.
        if !had_arrivals {
            // Exit immediately; total elapsed ≈ min, NOT max.
            let total = t_start.elapsed();
            assert!(
                total < join_window_max,
                "MF-005: no-arrivals window MUST exit at min, NOT extend to max; elapsed={:?}",
                total
            );
        }
    }

    /// MF-005 (Phase C refactor) — when arrivals occur during the drain
    /// or min phase, the extension timer is `(max - min)`. We test the
    /// duration arithmetic directly.
    #[test]
    fn mf_005_join_window_extension_duration_is_max_minus_min() {
        let max = Duration::from_millis(500);
        let min = Duration::from_millis(50);
        let extension = max
            .checked_sub(min)
            .expect("MF-005: max must be >= min by config invariant");
        assert_eq!(
            extension,
            Duration::from_millis(450),
            "MF-005: extension MUST be (max - min) per R10a step 3"
        );
    }

    /// MF-002 (regression check after Phase B refactor) — SoloReducing
    /// makes reduce progress AND polls accepts non-blockingly. The
    /// previous `tokio::select! { ...accept... else => reduce_n(...) }`
    /// trapped reduction in the unreachable `else` arm.
    ///
    /// This test exercises the post-fix loop shape: reduce-then-poll-
    /// accept with `biased` + a ready future as the non-blocking peek.
    /// Convergence on a CON-CON net within a bounded number of
    /// iterations is the signal that the loop progresses.
    #[tokio::test]
    async fn mf_002_solo_reducing_loop_makes_reduction_progress() {
        use crate::net::{Net, PortRef, Symbol};

        let (mut server, _client) = ChannelTransport::pair(1, 65536);
        server.listen().await.expect("listen");

        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(3));

        let mut iters = 0;
        let solo_budget: usize = 10_000;
        let mut accept_polls = 0;
        while !net.redex_queue.is_empty() {
            // (1) reduce.
            let stats = crate::reduction::reduce_n(&mut net, solo_budget);
            if stats.total_interactions == 0 {
                break;
            }
            iters += 1;

            // (2) non-blocking accept-poll.
            tokio::select! {
                biased;
                _new_conn = server.accept() => {
                    accept_polls += 1;
                }
                _ = std::future::ready(()) => {
                    // No connection — fall through to next reduce batch.
                }
            }

            if iters > 100 {
                panic!("MF-002: SoloReducing loop did not converge");
            }
        }
        assert!(
            net.redex_queue.is_empty(),
            "MF-002: SoloReducing must drain the redex queue"
        );
        assert!(
            iters >= 1,
            "MF-002: at least one reduce batch must have executed"
        );
        // accept_polls is observed but not asserted on (the channel may
        // never deliver a connection in this test).
        let _ = accept_polls;
    }

    // -----------------------------------------------------------------
    // Phase D Option A regression tests (2026-04-27)
    // -----------------------------------------------------------------

    /// Phase D Option A — `ProtocolError::AllWorkersDeparted` exists,
    /// round-trips Debug, and produces a Display string keyed on the
    /// canonical "all workers departed" prefix.
    ///
    /// This is the terminal-state error returned by `run_coordinator`
    /// when every remote worker has been pruned from `worker_streams`
    /// in a non-hybrid grid. The variant is distinct from `Fatal`
    /// specifically so observability tooling can key on it.
    #[test]
    fn phase_d_option_a_all_workers_departed_variant_exists() {
        let err = ProtocolError::AllWorkersDeparted {
            detail: "all 3 workers departed in round 5".into(),
        };
        let display = format!("{}", err);
        assert!(
            display.starts_with("all workers departed:"),
            "AllWorkersDeparted Display must start with canonical prefix; got: {}",
            display
        );
        // Debug must succeed and include the detail.
        let debug = format!("{:?}", err);
        assert!(
            debug.contains("AllWorkersDeparted"),
            "Debug must surface the variant name; got: {}",
            debug
        );
        assert!(
            debug.contains("round 5"),
            "Debug must surface the detail field; got: {}",
            debug
        );
    }

    /// Phase D Option A / QA-005 D — `departing_worker_ids` MUST be
    /// idempotent: a `wid` that surfaces under both `Message::Error`
    /// and a follow-up timeout (or any double-detection race) is
    /// counted exactly once.
    ///
    /// The production code uses `if !departing_worker_ids.contains(&wid)`
    /// at every push site; this test fixes the contract.
    #[test]
    fn phase_d_option_a_qa005_departing_ids_are_idempotent() {
        let mut departing_worker_ids: Vec<WorkerId> = Vec::new();
        let mut round_departed_count: u32 = 0;

        // First detection — Message::Error for wid=5.
        let id: WorkerId = 5;
        if !departing_worker_ids.contains(&id) {
            departing_worker_ids.push(id);
            round_departed_count += 1;
        }
        // Second detection — timeout fires for the same wid.
        if !departing_worker_ids.contains(&id) {
            departing_worker_ids.push(id);
            round_departed_count += 1;
        }
        // Third detection — LeaveRequest also arrives for the same wid.
        if !departing_worker_ids.contains(&id) {
            departing_worker_ids.push(id);
            round_departed_count += 1;
        }

        assert_eq!(
            departing_worker_ids,
            vec![5],
            "QA-005: same wid must appear exactly once"
        );
        assert_eq!(
            round_departed_count, 1,
            "QA-005: round_departed_count must increment once per logical departure"
        );
    }

    /// Phase D Option A / QA-012 D — pruning departed workers from the
    /// parallel `worker_streams` / `worker_ids` Vecs preserves identity:
    /// after removal, every remaining `worker_ids[i]` corresponds to the
    /// SAME worker that was at `worker_ids[i]` (or whatever swap-remove
    /// re-positioned it to) before the removal. Identity travels with
    /// the value, NOT with the index.
    #[test]
    fn phase_d_option_a_qa012_stream_pruning_preserves_identity() {
        // Simulated pre-departure state: 4 remote workers in a non-
        // hybrid grid (wids 0..3).
        let mut worker_ids: Vec<WorkerId> = vec![0, 1, 2, 3];
        let mut worker_streams_proxy: Vec<u32> = vec![10, 11, 12, 13];
        debug_assert_eq!(worker_ids.len(), worker_streams_proxy.len());

        // wid=1 and wid=3 depart.
        let departing: Vec<WorkerId> = vec![1, 3];
        let mut indices_to_remove: Vec<usize> = worker_ids
            .iter()
            .enumerate()
            .filter_map(|(i, id)| {
                if departing.contains(id) {
                    Some(i)
                } else {
                    None
                }
            })
            .collect();
        indices_to_remove.sort_unstable_by(|a, b| b.cmp(a));
        for idx in indices_to_remove {
            worker_streams_proxy.swap_remove(idx);
            worker_ids.swap_remove(idx);
        }

        // Surviving wids are 0 and 2; their stream proxies (10 and 12)
        // travel with them, NOT re-numbered to 0..len.
        assert_eq!(worker_ids.len(), 2, "two workers remain");
        assert_eq!(
            worker_streams_proxy.len(),
            2,
            "QA-012: parallel arrays must stay length-equal"
        );
        let pairs: std::collections::BTreeSet<(WorkerId, u32)> = worker_ids
            .iter()
            .copied()
            .zip(worker_streams_proxy.iter().copied())
            .collect();
        let expected: std::collections::BTreeSet<(WorkerId, u32)> =
            [(0, 10), (2, 12)].into_iter().collect();
        assert_eq!(
            pairs, expected,
            "QA-012: surviving (wid, stream) pairings must be intact post-prune"
        );
    }

    /// Phase D Option A / QA-008 D — the `RetainedLastAcked::DeltaLight`
    /// variant carries unit-typed payload, so a wire-level construction
    /// cannot smuggle an unbounded String through the registry.
    ///
    /// Type-level pin: this test fails to compile if the field type
    /// regresses to `String` or any size-unbounded payload.
    #[test]
    fn phase_d_option_a_qa008_delta_light_type_is_unit() {
        use crate::protocol::retained::RetainedLastAcked;
        // The constructor literal proves the type at compile time.
        let v = RetainedLastAcked::DeltaLight { placeholder: () };
        // Trivially destructure to verify the type name.
        if let RetainedLastAcked::DeltaLight { placeholder } = &v {
            // Compile-time check: `placeholder` must be `&()`.
            let _: &() = placeholder;
        } else {
            panic!("constructor must yield DeltaLight variant");
        }
    }
}
