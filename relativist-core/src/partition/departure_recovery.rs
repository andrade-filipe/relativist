//! Helpers for departure recovery and partition materialization (SPEC-20 §3.3.4).

use crate::error::PartitionError;
use crate::partition::remap::remap_partition_ids;
use crate::partition::{IdRange, Partition, WorkerId};
use crate::protocol::retained::RetainedStateRegistry;
use std::collections::HashMap;

/// Materializes the partitions of departed workers from retained state (TASK-0443).
///
/// Under the conservative path (R24a), we use the initial round-0 partition.
/// Each reclaimed partition MUST be remapped to a fresh, globally unique ID range
/// before reconstruction to prevent collisions with surviving partitions (SPEC-20 R30).
pub fn materialize_reclaimed_partitions(
    departed_worker_ids: &[WorkerId],
    registry: &RetainedStateRegistry,
    reclaimed_id_ranges: &HashMap<WorkerId, IdRange>,
) -> Result<Vec<Partition>, PartitionError> {
    let mut reclaimed: Vec<Partition> = Vec::with_capacity(departed_worker_ids.len());

    for &wid in departed_worker_ids {
        // R24a: materialize from retained_initial (conservative)
        if let Some(initial) = registry.initial.get(&wid) {
            let p = initial.partition().clone();

            // R30: remap to a fresh disjoint range
            if let Some(&new_range) = reclaimed_id_ranges.get(&wid) {
                // --- Invariant Defense (TASK-0452, MF-006) ---
                // R24d: verify no overlap between reclaimed partitions.
                // Uses `debug_assert!` for style consistency with D3 / D4 / D5.
                #[cfg(debug_assertions)]
                {
                    for r in &reclaimed {
                        debug_assert!(
                            new_range.start >= r.id_range.end || new_range.end <= r.id_range.start,
                            "R24d violated: overlapping remapped ranges in reclaim: {:?} vs {:?}",
                            new_range,
                            r.id_range
                        );
                    }
                }

                let remapped = remap_partition_ids(p, new_range)?;
                reclaimed.push(remapped);
            } else {
                tracing::error!(
                    worker_id = wid,
                    "No remapped ID range available for departed worker; skipping reclaim."
                );
                return Err(PartitionError::InvariantViolation(format!(
                    "No ID range for worker {}",
                    wid
                )));
            }
        } else {
            tracing::error!(
                worker_id = wid,
                "No retained state available for departed worker; state loss occurred!"
            );
            return Err(PartitionError::InvariantViolation(format!(
                "State loss for worker {}",
                wid
            )));
        }
    }

    Ok(reclaimed)
}
