//! In-process self-worker task for hybrid coordinator (SPEC-20 R3, R3a, R3b).

use std::time::Instant;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use super::channel::ChannelTransport;
use super::frame::{recv_frame, send_frame};
use super::transport::{Transport, TransportStream};
use super::types::Message;
use crate::merge::WorkerRoundStats;
use crate::reduction::reduce_all;

/// Handle to a spawned in-process self-worker.
pub struct SelfWorkerHandle {
    /// Coordinator-side stream for communicating with the self-worker.
    pub stream: TransportStream,
    /// Join handle for the blocking reduction task.
    pub join_handle: JoinHandle<()>,
    /// Receiver for panic/error signals from the self-worker.
    pub panic_rx: oneshot::Receiver<String>,
}

/// Spawns a blocking task that acts as an in-process worker (SPEC-20 R3).
///
/// Communicates with the coordinator via `ChannelTransport`.
pub async fn spawn_self_partition(max_payload_size: u32) -> SelfWorkerHandle {
    let (mut server_transport, mut client_transport) = ChannelTransport::pair(1, 1024 * 1024);

    let (panic_tx, panic_rx) = oneshot::channel();

    // Accept on server side (coordinator)
    let server_stream = server_transport
        .accept()
        .await
        .expect("ChannelTransport accept must succeed");

    // Spawn the worker logic in a blocking task
    let join_handle = tokio::task::spawn_blocking(move || {
        // Run a mini worker loop for one round
        let runtime = tokio::runtime::Handle::current();

        let result = runtime.block_on(async {
            let mut stream = client_transport
                .connect()
                .await
                .map_err(|e| e.to_string())?;

            // Phase 1: Wait for partition assignment
            let (msg, _) = recv_frame(&mut stream, max_payload_size)
                .await
                .map_err(|e| e.to_string())?;
            let (round, mut partition) = match msg {
                Message::AssignPartition { round, partition } => (round, partition),
                other => return Err(format!("expected AssignPartition, got {:?}", other)),
            };

            let start_time = Instant::now();
            let agents_before = partition.subnet.count_live_agents();

            // Phase 2: Perform reduction
            let reduction_stats = reduce_all(&mut partition.subnet);

            let agents_after = partition.subnet.count_live_agents();
            let reduce_duration = start_time.elapsed();

            let stats = WorkerRoundStats {
                worker_id: partition.worker_id,
                agents_before,
                agents_after,
                local_redexes: reduction_stats.total_interactions as usize,
                reduce_duration_secs: reduce_duration.as_secs_f64(),
                interactions_by_rule: reduction_stats.interactions_by_rule,
                has_border_activity: false, // self-partition has no local borders
                is_coordinator_self: true,
            };

            // Phase 3: Return result
            let result_msg = Message::PartitionResult {
                round,
                partition,
                stats,
            };
            send_frame(&mut stream, &result_msg)
                .await
                .map_err(|e| e.to_string())?;

            Ok::<(), String>(())
        });

        if let Err(e) = result {
            let _ = panic_tx.send(e);
        }
    });

    SelfWorkerHandle {
        stream: server_stream,
        join_handle,
        panic_rx,
    }
}

/// Helper for SoloReducing state: performs a single batch of reduction.
pub fn reduce_solo_batch(
    net: &mut crate::net::Net,
    budget: u32,
) -> crate::reduction::ReductionStats {
    crate::reduction::reduce_n(net, budget as usize)
}
