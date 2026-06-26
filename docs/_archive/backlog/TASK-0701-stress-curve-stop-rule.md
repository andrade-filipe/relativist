# TASK-0701 — D-014-STOPRULE: `StopRule` (wall-time / RAM / OOM sequence aborter)

**Phase:** D-014 (Stress Curve Campaign) — Stage 3 DEV scope
**Bundle:** D-014 — Stress Curve Campaign
**Status:** TODO
**Priority:** P0 (sequence-level safety net; blocks campaign run TASK-0708)
**Spec:** none.
**Depends on:** TASK-0700 (consumes `MemoryProbe` to evaluate the RAM gate).
**Estimated complexity:** S–M (~90 LoC production + ~80 LoC unit tests).

---

## Context

The stress-curve campaign sweeps `n_seq = [10_000, 31_623, ..., 1_000_000_000]` (×√10, 11 points). For any given `(workload, env, W)` the sequence MUST stop early — the design doc §4.4 explicitly documents that the `10⁹` ceiling is aspirational; the real ceiling is set by the StopRule. Without it, a single `(condup_expansion, W=1, env=docker)` arm will consume 7+ hours alone or OOM-kill the host.

`StopRule` evaluates **after** each rep finishes (or after a child rep process exits abnormally) and returns `Some(StopReason)` if the next-larger N would breach one of:

1. **Wall budget** — the previous rep took longer than the configured budget (5 min in-process, 7m30s Docker).
2. **Memory fraction** — the previous rep's `vmrss_peak_mb` exceeded `memory_fraction_max` (default 0.80) of total host RAM.
3. **OOM** — the previous rep's child process exited with `SIGKILL` (Linux) or non-zero exit code consistent with OOM-killer (Windows: `0xC0000017` STATUS_NO_MEMORY or job-object termination).

A sentinel CSV row with `stop_reason = $variant` is emitted by the harness so the dataset retains evidence of where the wall is.

## Files in scope (file:line pointers)

| File | Change |
|------|--------|
| `relativist-core/src/bench/stop_rule.rs` | **CREATE.** New module implementing `StopReason` enum + `StopRule` struct + `SequenceOutcome` aggregate. ~90 LoC. |
| `relativist-core/src/bench/mod.rs` | **MODIFY.** Add `pub mod stop_rule;` (1 line). |
| `relativist-core/src/bench/memory_probe.rs` | **READ-ONLY reference.** Consumed via `MemoryProbe::as_fraction_of_total`. Created in TASK-0700. |
| `relativist-core/src/error.rs` | **MODIFY (if needed).** Add `Sequence(StopReason)` variant if integrating into `BenchError`. ~3 LoC. |

## Files explicitly OUT of scope

- `bench/suite.rs` integration — wired in TASK-0702 (campaign descriptor).
- The actual `Command::spawn`-based rep runner — TASK-0704's territory (the script orchestrates child processes; `StopRule` is reusable from the in-process path too if needed).
- Worker-side enforcement — StopRule lives in the orchestrator only.

## Required public API

```rust
// relativist-core/src/bench/stop_rule.rs
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StopReason {
    WallTimeExceeded,
    MemoryExceeded,
    Oom,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepResult {
    pub n: usize,
    pub wall: Duration,
    pub vmrss_peak_bytes: u64,
    pub vmrss_peak_fraction_of_total: f64,
    pub child_exit: ChildExit,        // Ok | Killed { signal } | NonZero { code }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChildExit {
    Ok,
    Killed { signal: i32 },
    NonZero { code: i32 },
}

pub struct StopRule {
    pub wall_budget: Duration,
    pub memory_fraction_max: f64,    // 0.0..1.0; campaign default 0.80
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequenceOutcome {
    pub completed_reps: Vec<RepResult>,
    pub stop_reason: Option<StopReason>,
    pub last_attempted_n: Option<usize>,
}

impl StopRule {
    /// Evaluate the most recent rep. Returns `Some(reason)` if the sequence
    /// MUST stop; `None` if it is safe to attempt the next N.
    pub fn check(&self, last_rep: &RepResult) -> Option<StopReason>;

    /// Iterate `n_seq`, calling `runner(n)` per N, evaluating `check` after
    /// each. Stops on first `Some(reason)`, recording it in the outcome.
    pub fn run_sequence<F>(&self, n_seq: &[usize], runner: F) -> SequenceOutcome
        where F: FnMut(usize) -> RepResult;
}
```

## Acceptance criteria

1. `check` returns `Some(WallTimeExceeded)` iff `last_rep.wall > wall_budget`.
2. `check` returns `Some(MemoryExceeded)` iff `last_rep.vmrss_peak_fraction_of_total > memory_fraction_max`.
3. `check` returns `Some(Oom)` iff `child_exit` is `Killed { signal: SIGKILL }` (= 9 on Linux) OR `NonZero { code }` where `code` matches the campaign-documented OOM signature (Windows `0xC0000017` = -1073741801 i32, or `137` = 128+SIGKILL on bash-mediated paths).
4. Wall and memory checks are evaluated in priority order: `Oom > MemoryExceeded > WallTimeExceeded` (so an OOM during a wall-budget overrun reports as OOM).
5. `run_sequence` stops at the first `Some(reason)` and returns the partial completion (all reps that succeeded plus the offending rep).
6. `run_sequence` on an empty `n_seq` returns `SequenceOutcome { completed_reps: vec![], stop_reason: None, last_attempted_n: None }`.
7. Unit tests cover all three reasons, the priority ordering, the empty-input case, and the "no stop, sequence completes" case.
8. `cargo test` floor: **+6 default = ≥ 1808** (cumulative with TASK-0700: 1798 + 4 + 6).
9. `cargo test --features zero-copy` floor: **+6 = ≥ 1852**.
10. `cargo test --features streaming-no-recycle` floor: **+6 = ≥ 1799**.
11. `cargo test --release` floor: **+6 = ≥ 1750**.
12. v1 floor (690) inviolable.
13. `cargo clippy --all-features -- -D warnings` clean.
14. `cargo fmt --check` clean.

## Test floor delta

**+6 default** (six new unit tests in `stop_rule.rs`). Cumulative after TASK-0700+0701:
- default ≥ 1808
- zero-copy ≥ 1852
- streaming-no-recycle ≥ 1799
- release ≥ 1750

## Implementation hints

1. The `ChildExit::Killed { signal: i32 }` arm uses the Unix-style signal number even when the harness later runs on Windows; the script TASK-0704 normalizes Windows job-object termination to `signal: 9` for cross-platform comparability. Document this in a doc-comment.
2. Keep `StopRule` `#[derive(Debug, Clone)]` — it must be cheap to clone for the script's child-process supervisor.
3. Do NOT call `MemoryProbe` from inside `check`; the rep runner is responsible for filling `RepResult.vmrss_peak_*` BEFORE calling `check`. This keeps `StopRule` testable without platform plumbing.
4. `run_sequence` is the convenience wrapper for in-process testing; the real campaign runs reps through the bash script (`scripts/stress_curve.sh`) which calls `Command::spawn` directly. `run_sequence` exists for the integration tests in TASK-0707.
5. Use `serde` derive on all serializable types — the `StopReason` and `ChildExit` enums end up as CSV string columns via `serde::Serialize`.

## Estimated LoC

- Production: ~90 LoC (struct + enums + 2 methods).
- Tests: ~80 LoC (6 unit tests).
- Total: ~170 LoC. Under the 200 LoC ceiling.

## Cross-references

- Design doc: `docs/superpowers/specs/2026-05-05-stress-test-large-nets-design.md` §4.2 row 3, §4.3 (the `StopRule` interface), §5 Phase 1 + Phase 2 (wall budgets and memory thresholds).
- Consumes: TASK-0700 (`MemoryProbe::as_fraction_of_total`).
- Consumed by: TASK-0702 (descriptor wires it into the bench matrix), TASK-0704 (script invokes it as the sequence guard), TASK-0707 (integration tests).
