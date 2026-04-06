//! Merge and grid cycle (SPEC-05).
//!
//! Reunites reduced partitions, resolves border redexes,
//! and orchestrates the BSP grid loop.

pub mod core;
pub mod grid;
pub mod helpers;
pub mod types;

// Re-exports: convenience access via `crate::merge::*`
pub use core::merge;
pub use grid::run_grid;
pub use helpers::{drain_stale_redexes, rebuild_free_port_index};
pub use types::{GridConfig, GridMetrics, WorkerRoundStats};
