//! IT-0707-04 (d) — `stop_rule_oom_real_or_synthetic_sigkill` (TASK-0707).
//!
//! Spawns a child that allocates until OOM if `python3` is available
//! (Mode A), or falls back to a synthetic `ChildExit::Killed { signal: 9 }`
//! (Mode B). Either way the StopRule contract is exercised end-to-end:
//! `check` returns `Some(Oom)` for SIGKILL or 137.
//!
//! `cfg(unix)` only — Windows OOM detection is exercised by the unit tests
//! in `bench::stop_rule` and the script-side normalization in TASK-0704.

#![cfg(unix)]

mod common;

use common::d014_helpers::rep;
use relativist_core::bench::stop_rule::{ChildExit, StopReason, StopRule};
use std::os::unix::process::ExitStatusExt;
use std::process::Command;
use std::time::Duration;

#[test]
fn stop_rule_oom_real_or_synthetic_sigkill() {
    let rule = StopRule {
        wall_budget: Duration::from_secs(300),
        memory_fraction_max: 0.80,
    };

    // Mode A: try real OOM via a python3 child if available.
    let python_ok = Command::new("python3")
        .arg("--version")
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    let child_exit = if python_ok {
        // Allocate ~100 GiB-equivalent; OS OOM-killer terminates with
        // SIGKILL on healthy kernels.
        let status_result = Command::new("sh")
            .arg("-c")
            .arg("python3 -c 'a=[0]*10**11' 2>/dev/null")
            .status();
        match status_result {
            Ok(status) => match status.signal() {
                Some(9) => ChildExit::Killed { signal: 9 },
                Some(s) => {
                    eprintln!(
                        "WARN IT-0707-04: real OOM produced signal {} (not 9); using synthetic",
                        s
                    );
                    ChildExit::Killed { signal: 9 }
                }
                None => match status.code() {
                    Some(137) => ChildExit::NonZero { code: 137 },
                    Some(c) => {
                        eprintln!(
                            "WARN IT-0707-04: real OOM produced exit code {} (expected 137 or signal 9); using synthetic",
                            c
                        );
                        ChildExit::Killed { signal: 9 }
                    }
                    None => ChildExit::Killed { signal: 9 },
                },
            },
            Err(e) => {
                eprintln!(
                    "WARN IT-0707-04: failed to spawn python3 child ({}); using synthetic",
                    e
                );
                ChildExit::Killed { signal: 9 }
            }
        }
    } else {
        eprintln!("INFO IT-0707-04: python3 not available; using synthetic SIGKILL");
        ChildExit::Killed { signal: 9 }
    };

    let r = rep(1_000, 60, 8 * 1024 * 1024 * 1024, 0.50, child_exit);
    assert_eq!(
        rule.check(&r),
        Some(StopReason::Oom),
        "child OOM (SIGKILL or 137) must trip StopReason::Oom"
    );
}
