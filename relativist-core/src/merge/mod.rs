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
    AddBorderEntry, BorderDelta, BorderGraph, BorderState, LocalReconnection, MintedAgent,
    PendingCommutation,
};
pub use core::merge;
pub use grid::run_grid;
pub(crate) use grid::run_grid_entry;
pub use helpers::{drain_stale_redexes, rebuild_free_port_index};
pub use types::{GridConfig, GridMetrics, WorkerRoundStats};
