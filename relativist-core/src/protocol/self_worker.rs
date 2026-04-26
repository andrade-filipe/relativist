//! In-process self-worker task for hybrid coordinator (SPEC-20 R3, R3a, R3b).

use std::time::Instant;
use tokio::sync::oneshot;

use crate::merge::WorkerRoundStats;
use crate::partition::Partition;
use crate::reduction::reduce_all;

/// Spawns a blocking task to reduce a partition locally.
///
/// Returns a receiver for the `WorkerRoundStats` result.
/// If the task panics, the sender is dropped and the receiver returns `Err`.
pub fn spawn_self_worker(
    mut partition: Partition,
) -> oneshot::Receiver<Result<WorkerRoundStats, String>> {
    let (tx, rx) = oneshot::channel();

    // SPEC-20 R3: spawn_blocking to keep the async loop responsive for join/leave events.
    tokio::task::spawn_blocking(move || {
        let start_time = Instant::now();
        let agents_before = partition.subnet.count_live_agents();

        // Perform reduction
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
            // Self-partition by definition has no external border activity
            // during its local reduction; coordinator merge handles its
            // borders later.
            has_border_activity: false,
            is_coordinator_self: true,
        };

        let _ = tx.send(Ok(stats));
    });

    rx
}

/// Helper for SoloReducing state: performs a single batch of reduction.
pub fn reduce_solo_batch(
    net: &mut crate::net::Net,
    budget: u32,
) -> crate::reduction::ReductionStats {
    crate::reduction::reduce_n(net, budget as usize)
}
