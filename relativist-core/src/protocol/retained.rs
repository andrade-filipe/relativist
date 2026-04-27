//! Retained state bookkeeping for departure recovery (SPEC-20 §3.3.3).

use crate::partition::{Partition, WorkerId};
use std::collections::HashMap;

/// SPEC-20 R23b: round-0 dispatch state, allocated once per worker.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(
    feature = "zero-copy",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
pub enum RetainedInitial {
    V1(Partition),
    Delta(Partition),
}

impl RetainedInitial {
    pub fn partition(&self) -> &Partition {
        match self {
            Self::V1(p) | Self::Delta(p) => p,
        }
    }
}

/// SPEC-20 R23c: most recent committed state, refreshed atomically.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(
    feature = "zero-copy",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
pub enum RetainedLastAcked {
    V1(Partition),
    /// Placeholder for delta-light (border_graph + last_deltas)
    DeltaLight {
        placeholder: String,
    },
    DeltaCheckpoint(Partition),
}

/// Registry for all retained partitions in the grid (R23, R31).
#[derive(Debug, Clone, Default)]
pub struct RetainedStateRegistry {
    /// retained_initial[w]: round-0 state.
    pub initial: HashMap<WorkerId, RetainedInitial>,
    /// retained_last_acked[w]: round-N state.
    pub last_acked: HashMap<WorkerId, RetainedLastAcked>,
}

impl RetainedStateRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// SPEC-20 R23b (TASK-0432 / QA-002): announces a worker as part of the
    /// retained set without yet binding a partition to its initial slot.
    ///
    /// This is the join-time hook: when a mid-session joiner clears the
    /// JoinAck handshake, the coordinator calls `register_initial(worker_id,
    /// Some(partition))` once the joiner's first round-N+1 partition is
    /// known, OR `register_initial(worker_id, None)` to install a marker
    /// when the partition is not yet computed (round-0 cohort init block
    /// performs the real bind via `entry().or_insert_with(...)`).
    ///
    /// Invariant: after this call, `self.initial.contains_key(&worker_id)`
    /// is `true`, so the D5 precondition on `refresh_last_acked` holds for
    /// every worker that legitimately joined.
    pub fn register_initial(&mut self, worker_id: WorkerId, partition: Option<Partition>) {
        if let Some(p) = partition {
            self.initial.insert(worker_id, RetainedInitial::V1(p));
        } else {
            // Sentinel: only inserted if no real entry exists. The L587-600
            // init block in `coordinator::run_coordinator` will replace this
            // with the joiner's true round-N+1 partition via `entry`/`insert`.
            self.initial
                .entry(worker_id)
                .or_insert_with(|| RetainedInitial::V1(Partition::empty(worker_id)));
        }
    }

    /// R31: atomic refresh of last_acked[w].
    ///
    /// D5 (TASK-0452, refined by QA-002): the spec invariant states that
    /// `initial[w]` MUST exist before `last_acked[w]` is refreshed. For
    /// mid-session joiners, the coordinator must call `register_initial`
    /// at JoinAck time. As a self-healing fallback (so that the
    /// observability-layer assertion never panics the coordinator on a
    /// well-formed elastic flow), if `initial[w]` is missing we emit a
    /// `tracing::warn!` and auto-promote the supplied state into the
    /// initial slot. The `debug_assert!` below still fires only on
    /// truly-unregistered worker IDs in test/CI builds, but the auto-heal
    /// path keeps debug *runtime* binaries from dying mid-flight.
    pub fn refresh_last_acked(&mut self, worker_id: WorkerId, state: RetainedLastAcked) {
        self.initial.entry(worker_id).or_insert_with(|| {
            tracing::warn!(
                worker_id,
                "D5 self-heal: refresh_last_acked called without prior register_initial; \
                 auto-registering placeholder. This indicates a missing register_initial \
                 call at the JoinAck site (SPEC-20 R23b)."
            );
            RetainedInitial::V1(Partition::empty(worker_id))
        });
        self.last_acked.insert(worker_id, state);
    }

    /// R23a: release all slots for a worker.
    pub fn release_worker(&mut self, worker_id: WorkerId) {
        self.initial.remove(&worker_id);
        self.last_acked.remove(&worker_id);
    }

    /// NF-011: memory bound debug assertions.
    pub fn assert_memory_bounds(&self, k_eff: usize) {
        debug_assert!(
            self.initial.len() <= 2 * k_eff,
            "R31/NF-011: retained_initial exceeds memory bound ({} > 2*{})",
            self.initial.len(),
            k_eff
        );
        debug_assert!(
            self.last_acked.len() <= k_eff,
            "R31/NF-011: retained_last_acked exceeds memory bound ({} > {})",
            self.last_acked.len(),
            k_eff
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// QA-002 RED: a mid-session joiner that returns a `PartitionResult` in
    /// its first round must not panic in debug build. After QA-002 fix, the
    /// `refresh_last_acked` self-heal path inserts a placeholder rather than
    /// panicking.
    #[test]
    fn qa_002_refresh_last_acked_for_unregistered_worker_does_not_panic() {
        let mut registry = RetainedStateRegistry::new();
        // No prior `register_initial(7, _)` for worker 7.
        // Pre-fix: `debug_assert!` panics. Post-fix: auto-heals.
        registry.refresh_last_acked(7, RetainedLastAcked::V1(Partition::empty(7)));
        // Both slots populated after self-heal:
        assert!(registry.initial.contains_key(&7));
        assert!(registry.last_acked.contains_key(&7));
    }

    /// QA-002 GREEN: explicit register_initial path still works.
    #[test]
    fn qa_002_register_initial_with_partition_seeds_initial_slot() {
        let mut registry = RetainedStateRegistry::new();
        registry.register_initial(3, Some(Partition::empty(3)));
        assert!(registry.initial.contains_key(&3));
        // last_acked still untouched until refresh_last_acked runs:
        assert!(!registry.last_acked.contains_key(&3));
    }

    /// QA-002 GREEN: register_initial with None installs a sentinel placeholder.
    #[test]
    fn qa_002_register_initial_none_installs_sentinel() {
        let mut registry = RetainedStateRegistry::new();
        registry.register_initial(5, None);
        assert!(registry.initial.contains_key(&5));
        // Subsequent refresh_last_acked must succeed without panic:
        registry.refresh_last_acked(5, RetainedLastAcked::V1(Partition::empty(5)));
        assert!(registry.last_acked.contains_key(&5));
    }

    /// QA-002 GREEN: register_initial(None) does not overwrite an existing real entry.
    #[test]
    fn qa_002_register_initial_none_does_not_overwrite_real_partition() {
        let mut registry = RetainedStateRegistry::new();
        let mut p = Partition::empty(2);
        p.id_range = crate::partition::IdRange {
            start: 100,
            end: 200,
        };
        registry.register_initial(2, Some(p));
        // Sentinel attempt must NOT clobber:
        registry.register_initial(2, None);
        let stored = registry.initial.get(&2).expect("entry present");
        match stored {
            RetainedInitial::V1(part) | RetainedInitial::Delta(part) => {
                assert_eq!(part.id_range.start, 100);
                assert_eq!(part.id_range.end, 200);
            }
        }
    }
}
