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

    /// R31: atomic refresh of last_acked[w].
    pub fn refresh_last_acked(&mut self, worker_id: WorkerId, state: RetainedLastAcked) {
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
