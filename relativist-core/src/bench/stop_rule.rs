//! Sequence-level stop rule for the D-014 stress-curve campaign.
//!
//! Evaluates AFTER each rep finishes (or after a child rep process exits
//! abnormally). Returns `Some(StopReason)` when the next-larger N would
//! breach a wall-time, RAM-fraction, or OOM gate.
//!
//! Priority on simultaneous trips: **`Oom > MemoryExceeded > WallTimeExceeded`**.
//!
//! `StopRule` deliberately does NOT call [`MemoryProbe`] — the rep runner is
//! responsible for filling [`RepResult::vmrss_peak_fraction_of_total`]
//! BEFORE invoking [`StopRule::check`]. This keeps the rule platform-free
//! and trivially testable.
//!
//! [`MemoryProbe`]: crate::bench::memory_probe::MemoryProbe

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Why the stress-curve sequence stopped early. Serialized as the variant
/// name into the CSV `stop_reason` column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StopReason {
    WallTimeExceeded,
    MemoryExceeded,
    Oom,
}

/// Outcome of a single rep child process. `signal: i32` uses Unix-style
/// signal numbers even on Windows; the campaign script (TASK-0704)
/// normalizes Windows job-object termination to `signal: 9` for
/// cross-platform comparability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChildExit {
    Ok,
    Killed { signal: i32 },
    NonZero { code: i32 },
}

/// Empirical observations from a finished rep, fed into [`StopRule::check`].
///
/// `bench_results` carries the per-repetition `BenchmarkResult` rows
/// produced by [`run_benchmark_suite`] for this `n`. It is populated only
/// by the in-process `StressCurveDescriptor::run_one_sequence` path
/// (TASK-0722 BUG-B): all other call sites — including the unit tests in
/// this module — leave the vector empty. The field is `#[serde(skip)]`
/// because [`BenchmarkResult`] is `Serialize`-only (no `Deserialize`,
/// no `PartialEq`); skipping it preserves the field-level cmp/round-trip
/// behaviour expected by existing fixtures while letting the runtime
/// dispatch path carry full per-rep telemetry in-memory.
///
/// `RepResult` no longer derives `Deserialize` / `PartialEq` after the
/// new field landed (no external consumer uses either — confirmed by a
/// repo-wide audit). Field-level comparisons in the unit tests below
/// continue to work unchanged.
///
/// [`run_benchmark_suite`]: crate::bench::suite::run_benchmark_suite
/// [`BenchmarkResult`]: crate::bench::BenchmarkResult
#[derive(Debug, Clone, Serialize)]
pub struct RepResult {
    pub n: usize,
    pub wall: Duration,
    pub vmrss_peak_bytes: u64,
    /// Fraction of total host RAM the peak represents. Must be in `[0.0, 1.0]`
    /// in production; the rule itself does not enforce finiteness — the
    /// caller is responsible.
    pub vmrss_peak_fraction_of_total: f64,
    pub child_exit: ChildExit,
    /// Real per-rep `BenchmarkResult` rows captured from
    /// `run_benchmark_suite`. Empty for non-stress-curve callers.
    /// (TASK-0722 BUG-B fix.)
    #[serde(skip)]
    pub bench_results: Vec<super::BenchmarkResult>,
}

/// Aggregated outcome of [`StopRule::run_sequence`]. `stop_reason == None`
/// means the sequence ran to completion; otherwise it carries the reason
/// the sequence aborted, and `last_attempted_n` points at the offending N.
///
/// `Deserialize` / `PartialEq` were dropped together with [`RepResult`]'s
/// (TASK-0722 BUG-B); the only consumers compare individual fields.
#[derive(Debug, Clone, Serialize)]
pub struct SequenceOutcome {
    pub completed_reps: Vec<RepResult>,
    pub stop_reason: Option<StopReason>,
    pub last_attempted_n: Option<usize>,
}

/// Sequence-level safety net. Cheap to clone; intended to live in the
/// orchestrator's main loop.
#[derive(Debug, Clone)]
pub struct StopRule {
    /// Wall budget per rep. The rule trips on strict `>` (a rep at exactly
    /// the budget is allowed).
    pub wall_budget: Duration,
    /// Maximum allowed `vmrss_peak_fraction_of_total`. Strict `>` again —
    /// a rep at exactly the threshold is allowed. Campaign default: 0.80.
    pub memory_fraction_max: f64,
}

/// Public list of exit codes recognised as OOM signatures by `check`.
///
/// - `137` — bash-mediated `128 + SIGKILL` shape (Linux + macOS).
/// - `-1073741801` — `0xC0000017` (`STATUS_NO_MEMORY`) interpreted as i32 on
///   Windows. The campaign script (TASK-0704) reflects either form back as
///   `ChildExit::NonZero { code }`.
///
/// Exposed publicly so the bash orchestrator (TASK-0704) can pull the same
/// list and avoid drift.
pub const OOM_EXIT_CODES: &[i32] = &[137, -1073741801];

/// SIGKILL signal number on POSIX; Windows job-object terminations are
/// normalized to this value at the script boundary (TASK-0704).
pub const SIGKILL_SIGNUM: i32 = 9;

impl StopRule {
    /// Evaluate the most recent rep. Returns `Some(reason)` if the sequence
    /// MUST stop; `None` if it is safe to attempt the next N.
    ///
    /// Priority on simultaneous trips: **`Oom > MemoryExceeded > WallTimeExceeded`**.
    pub fn check(&self, last_rep: &RepResult) -> Option<StopReason> {
        // Priority 1: OOM detection.
        match last_rep.child_exit {
            ChildExit::Killed { signal } if signal == SIGKILL_SIGNUM => {
                return Some(StopReason::Oom);
            }
            ChildExit::NonZero { code } if OOM_EXIT_CODES.contains(&code) => {
                return Some(StopReason::Oom);
            }
            _ => {}
        }

        // Priority 2: memory fraction.
        if last_rep.vmrss_peak_fraction_of_total > self.memory_fraction_max {
            return Some(StopReason::MemoryExceeded);
        }

        // Priority 3: wall time.
        if last_rep.wall > self.wall_budget {
            return Some(StopReason::WallTimeExceeded);
        }

        None
    }

    /// Iterate over `n_seq`, calling `runner(n)` per N, evaluating
    /// [`Self::check`] after each. Stops on first `Some(reason)`, recording
    /// it in the outcome together with the offending rep.
    ///
    /// `n_seq.is_empty()` returns an outcome with empty `completed_reps`,
    /// `stop_reason: None`, `last_attempted_n: None` — the runner is NOT
    /// invoked.
    pub fn run_sequence<F>(&self, n_seq: &[usize], mut runner: F) -> SequenceOutcome
    where
        F: FnMut(usize) -> RepResult,
    {
        let mut completed_reps: Vec<RepResult> = Vec::with_capacity(n_seq.len());
        let mut last_attempted_n: Option<usize> = None;
        let mut stop_reason: Option<StopReason> = None;

        for &n in n_seq {
            last_attempted_n = Some(n);
            let rep = runner(n);
            // Per acceptance criterion 5 (TASK-0701): the offending rep IS
            // included in `completed_reps`.
            let outcome = self.check(&rep);
            completed_reps.push(rep);
            if let Some(reason) = outcome {
                stop_reason = Some(reason);
                break;
            }
        }

        SequenceOutcome {
            completed_reps,
            stop_reason,
            last_attempted_n,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn rep(
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
            bench_results: Vec::new(),
        }
    }

    fn default_rule() -> StopRule {
        StopRule {
            wall_budget: Duration::from_secs(300),
            memory_fraction_max: 0.80,
        }
    }

    /// UT-0701-01 — `check` returns `Some(WallTimeExceeded)` iff
    /// `wall > wall_budget`; boundary `wall == wall_budget` is `None`.
    #[test]
    fn check_wall_time_exceeded() {
        let rule = default_rule();

        let r_eq = rep(1000, 300, 1024, 0.05, ChildExit::Ok);
        let r_over = rep(1000, 301, 1024, 0.05, ChildExit::Ok);
        let r_under = rep(1000, 100, 1024, 0.05, ChildExit::Ok);

        assert_eq!(
            rule.check(&r_eq),
            None,
            "wall == budget must NOT trip (strict >)"
        );
        assert_eq!(
            rule.check(&r_over),
            Some(StopReason::WallTimeExceeded),
            "wall > budget by 1s must trip WallTimeExceeded"
        );
        assert_eq!(rule.check(&r_under), None, "wall < budget must NOT trip");
    }

    /// UT-0701-02 — `check` returns `Some(MemoryExceeded)` iff
    /// `vmrss_peak_fraction > memory_fraction_max` (strict).
    #[test]
    fn check_memory_exceeded() {
        let rule = default_rule();

        let r_eq = rep(1000, 60, 1024, 0.80, ChildExit::Ok);
        let r_over = rep(1000, 60, 1024, 0.85, ChildExit::Ok);
        let r_under = rep(1000, 60, 1024, 0.50, ChildExit::Ok);

        assert_eq!(
            rule.check(&r_eq),
            None,
            "frac == max must NOT trip (strict >)"
        );
        assert_eq!(
            rule.check(&r_over),
            Some(StopReason::MemoryExceeded),
            "frac > max must trip MemoryExceeded"
        );
        assert_eq!(rule.check(&r_under), None, "frac < max must NOT trip");
    }

    /// UT-0701-03 — `check` returns `Some(Oom)` for SIGKILL,
    /// bash-mediated 137, Windows STATUS_NO_MEMORY; NOT for generic
    /// non-zero exit codes or other signals.
    #[test]
    fn check_oom_sigkill_and_known_exit_codes() {
        let rule = default_rule();

        let r_sigkill = rep(1000, 60, 1024, 0.05, ChildExit::Killed { signal: 9 });
        let r_137 = rep(1000, 60, 1024, 0.05, ChildExit::NonZero { code: 137 });
        let r_winoom = rep(
            1000,
            60,
            1024,
            0.05,
            ChildExit::NonZero { code: -1073741801 },
        );
        let r_generic = rep(1000, 60, 1024, 0.05, ChildExit::NonZero { code: 1 });
        let r_segv = rep(1000, 60, 1024, 0.05, ChildExit::Killed { signal: 11 });

        assert_eq!(rule.check(&r_sigkill), Some(StopReason::Oom));
        assert_eq!(rule.check(&r_137), Some(StopReason::Oom));
        assert_eq!(rule.check(&r_winoom), Some(StopReason::Oom));
        assert_eq!(
            rule.check(&r_generic),
            None,
            "generic non-zero must NOT trip Oom"
        );
        assert_eq!(rule.check(&r_segv), None, "SIGSEGV must NOT trip Oom");
    }

    /// UT-0701-04 — Priority order: `Oom > MemoryExceeded > WallTimeExceeded`.
    #[test]
    fn check_priority_order_oom_over_memory_over_wall() {
        let rule = default_rule();

        // (a) all three trip — Oom wins.
        let r_all = rep(1000, 301, 1024, 0.85, ChildExit::Killed { signal: 9 });
        // (b) memory + wall trip, no OOM — Memory wins.
        let r_mem_wall = rep(1000, 301, 1024, 0.85, ChildExit::Ok);
        // (c) wall only — Wall wins.
        let r_wall = rep(1000, 301, 1024, 0.05, ChildExit::Ok);
        // (d) OOM + wall, memory under — Oom wins.
        let r_oom_wall = rep(1000, 301, 1024, 0.05, ChildExit::Killed { signal: 9 });

        assert_eq!(rule.check(&r_all), Some(StopReason::Oom));
        assert_eq!(rule.check(&r_mem_wall), Some(StopReason::MemoryExceeded));
        assert_eq!(rule.check(&r_wall), Some(StopReason::WallTimeExceeded));
        assert_eq!(rule.check(&r_oom_wall), Some(StopReason::Oom));
    }

    /// UT-0701-05 — Empty `n_seq` produces a zero outcome and never
    /// invokes the runner closure.
    #[test]
    fn run_sequence_empty_input() {
        let rule = default_rule();
        let n_seq: &[usize] = &[];

        let outcome = rule.run_sequence(n_seq, |_n| {
            panic!("runner must NOT be invoked for empty n_seq")
        });

        assert!(outcome.completed_reps.is_empty(), "expected zero reps");
        assert_eq!(outcome.stop_reason, None);
        assert_eq!(outcome.last_attempted_n, None);
    }

    /// UT-0701-06 — End-to-end: completes when no rule trips, OR stops at
    /// the first `Some(reason)` keeping the offending rep in
    /// `completed_reps`.
    #[test]
    fn run_sequence_completes_or_stops_at_first_violation() {
        // Sub-case A: completes successfully.
        {
            let rule = default_rule();
            let n_seq: &[usize] = &[1_000, 10_000, 100_000];
            let mut call_count: usize = 0;

            let outcome = rule.run_sequence(n_seq, |n| {
                call_count += 1;
                rep(n, 1, 1024, 0.05, ChildExit::Ok)
            });

            assert_eq!(call_count, 3);
            assert_eq!(outcome.completed_reps.len(), 3);
            assert_eq!(outcome.stop_reason, None);
            assert_eq!(outcome.last_attempted_n, Some(100_000));
            assert_eq!(outcome.completed_reps[0].n, 1_000);
            assert_eq!(outcome.completed_reps[2].n, 100_000);
        }

        // Sub-case B: stops at second N due to wall.
        {
            let rule = default_rule();
            let n_seq: &[usize] = &[1_000, 10_000, 100_000];
            let mut call_count: usize = 0;

            let outcome = rule.run_sequence(n_seq, |n| {
                call_count += 1;
                if n == 10_000 {
                    rep(n, 301, 1024, 0.05, ChildExit::Ok)
                } else {
                    rep(n, 1, 1024, 0.05, ChildExit::Ok)
                }
            });

            assert_eq!(call_count, 2, "must NOT call runner for 100_000 after stop");
            assert_eq!(
                outcome.completed_reps.len(),
                2,
                "completed_reps must include the offending rep that triggered the stop"
            );
            assert_eq!(outcome.stop_reason, Some(StopReason::WallTimeExceeded));
            assert_eq!(outcome.last_attempted_n, Some(10_000));
        }
    }
}
