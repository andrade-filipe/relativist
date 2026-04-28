//! Merge and grid cycle (SPEC-05).
//!
//! Reunites reduced partitions, resolves border redexes,
//! and orchestrates the BSP grid loop.

pub mod border_graph;
pub mod border_resolver;
pub mod core;
pub mod grid;
#[cfg(test)]
mod grid_delta_integration_tests;
pub mod helpers;
#[cfg(test)]
pub(crate) mod internal;
pub mod types;

// Re-exports: convenience access via `crate::merge::*`
pub use border_graph::{
    AddBorderEntry, BorderDelta, BorderGraph, BorderState, LocalReconnection, LocalWiringHint,
    MintedAgent, PendingCommutation,
};
pub use core::merge;
// `grid::reconstruct` was re-exported here for the Phase D reclaim path in
// `protocol/coordinator.rs`. Phase D Option A (2026-04-27) removed the
// reclaim path; the function still exists in `grid.rs` (and is exercised by
// its own unit tests) and will be re-exported when the v2.1 reclaim path
// re-introduces a public caller.
pub use grid::run_grid;
pub(crate) use grid::run_grid_entry;
pub use helpers::{drain_stale_redexes, rebuild_free_port_index};
pub use types::{DispatchMode, GridConfig, GridMetrics, StreamingStrategyConfig, WorkerRoundStats};
