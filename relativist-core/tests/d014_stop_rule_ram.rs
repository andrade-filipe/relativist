//! IT-0707-03 (c) — `stop_rule_ram_trips_at_85pct_with_80pct_max`
//! (TASK-0707).
//!
//! Fakes a `RepResult` with `vmrss_peak_fraction_of_total = 0.85`,
//! threshold `0.80`; verifies `check` returns `Some(MemoryExceeded)`.
//! Cross-platform.

mod common;

use common::d014_helpers::rep;
use relativist_core::bench::stop_rule::{ChildExit, StopReason, StopRule};
use std::time::Duration;

#[test]
fn stop_rule_ram_trips_at_85pct_with_80pct_max() {
    let rule = StopRule {
        wall_budget: Duration::from_secs(300),
        memory_fraction_max: 0.80,
    };
    let r = rep(1_000, 60, 8 * 1024 * 1024 * 1024, 0.85, ChildExit::Ok);

    assert_eq!(
        rule.check(&r),
        Some(StopReason::MemoryExceeded),
        "0.85 fraction must trip MemoryExceeded with 0.80 max"
    );
}
