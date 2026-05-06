//! IT-0707-02 (b) — `stop_rule_wall_trips_at_6min_with_5min_budget`
//! (TASK-0707).
//!
//! Fakes a `RepResult` with `wall = 6 min`, `wall_budget = 5 min`; verifies
//! `check` returns `Some(WallTimeExceeded)`. Cross-platform.

mod common;

use common::d014_helpers::rep;
use relativist_core::bench::stop_rule::{ChildExit, StopReason, StopRule};
use std::time::Duration;

#[test]
fn stop_rule_wall_trips_at_6min_with_5min_budget() {
    let rule = StopRule {
        wall_budget: Duration::from_secs(300),
        memory_fraction_max: 0.80,
    };
    let r = rep(1_000, 360, 1024, 0.05, ChildExit::Ok); // 6 min wall

    assert_eq!(
        rule.check(&r),
        Some(StopReason::WallTimeExceeded),
        "6 min wall must trip WallTimeExceeded with 5 min budget"
    );
}
