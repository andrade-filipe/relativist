//! D-014 stress-curve test helpers (TASK-0707 / TEST-SPEC-0707).
//!
//! Provides a `RepResult` factory used by the dedicated stop-rule
//! integration tests (b), (c), (d) to keep their bodies focused on the
//! contract under test.

use relativist_core::bench::stop_rule::{ChildExit, RepResult};
use std::time::Duration;

/// Build a `RepResult` from explicit per-axis values. Mirrors the
/// in-module `rep` helper from `bench/stop_rule.rs::tests`; surfaced as a
/// shared helper for the integration tests in `tests/d014_*.rs`.
pub fn rep(
    n: usize,
    wall_secs: u64,
    vmrss_peak_bytes: u64,
    vmrss_peak_fraction: f64,
    child_exit: ChildExit,
) -> RepResult {
    RepResult {
        n,
        wall: Duration::from_secs(wall_secs),
        vmrss_peak_bytes,
        vmrss_peak_fraction_of_total: vmrss_peak_fraction,
        child_exit,
    }
}
