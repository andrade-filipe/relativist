//! IT-0707-06 (f) — `end_to_end_smoke_in_process` (TASK-0707).
//!
//! Direct in-process invocation of `StressCurveDescriptor::run_one_sequence`,
//! NOT via the bash orchestrator. Verifies `SequenceOutcome.completed_reps.len()
//! == 2` for `n_seq = [1_000, 10_000]`, reps = 1, workers = 2 — exercises the
//! BSP multi-worker path that workers=1 (covered by IT-0702-01) does not.

#![cfg(not(target_os = "macos"))]

use relativist_core::bench::stop_rule::StopRule;
use relativist_core::bench::suite::{Env, StressCurveDescriptor, StressWorkload};
use std::time::Duration;

#[test]
fn end_to_end_smoke_in_process() {
    let n_override = [1_000usize, 10_000usize];
    let stop = StopRule {
        wall_budget: Duration::from_secs(60),
        memory_fraction_max: 0.95,
    };

    let outcome = StressCurveDescriptor::run_one_sequence(
        StressWorkload::EpAnnihilation,
        Env::InProcess,
        /* workers */ 2,
        /* reps */ 1,
        Some(&n_override),
        Some(stop),
    )
    .expect("run_one_sequence must succeed");

    assert_eq!(
        outcome.completed_reps.len(),
        2,
        "end-to-end smoke for ep_annihilation N=[1k, 10k] reps=1 workers=2 must produce 2 reps"
    );
    assert_eq!(outcome.stop_reason, None, "smoke must not trip any rule");
    assert_eq!(outcome.last_attempted_n, Some(10_000));
}
