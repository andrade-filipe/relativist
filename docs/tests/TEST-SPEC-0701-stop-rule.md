# TEST-SPEC-0701: Tests for TASK-0701 — `StopRule` (wall-time / RAM / OOM aborter)

**Task:** TASK-0701
**Spec:** none
**Bundle:** D-014 (Stress Curve Campaign)
**Requirements covered:** Acceptance criteria 1-7 from TASK-0701
**Test IDs:** UT-0701-{01..06} (6 unit tests in-module)

---

## Scope

Verify the public API of `relativist-core/src/bench/stop_rule.rs`:
- `StopReason` enum (`WallTimeExceeded`, `MemoryExceeded`, `Oom`)
- `RepResult` / `ChildExit` data types (serde-roundtrippable)
- `StopRule { wall_budget, memory_fraction_max }` struct
- `StopRule::check(&self, last_rep: &RepResult) -> Option<StopReason>`
- `StopRule::run_sequence<F>(&self, n_seq, runner) -> SequenceOutcome` where `F: FnMut(usize) -> RepResult`
- Priority order on simultaneous trips: **`Oom > MemoryExceeded > WallTimeExceeded`**

All 6 unit tests live **in-module** in `bench/stop_rule.rs` under `#[cfg(test)] mod tests`. No integration test target. Cross-platform (no cfg gating; `ChildExit` carries unix-style `signal: i32` even on Windows per TASK-0701 implementation hint #1).

## Test category & location

| # | Name | Category | File | LoC |
|---|------|----------|------|-----|
| UT-0701-01 | `check_wall_time_exceeded` | unit | `relativist-core/src/bench/stop_rule.rs` | ~12 |
| UT-0701-02 | `check_memory_exceeded` | unit | same | ~12 |
| UT-0701-03 | `check_oom_sigkill_and_known_exit_codes` | unit | same | ~18 |
| UT-0701-04 | `check_priority_order_oom_over_memory_over_wall` | unit | same | ~18 |
| UT-0701-05 | `run_sequence_empty_input` | unit | same | ~10 |
| UT-0701-06 | `run_sequence_completes_or_stops_at_first_violation` | unit | same | ~22 |

## Test floor delta

- default: **+6** → ≥ 1808 (cumulative 1798 + 4 + 6)
- zero-copy: **+6** → ≥ 1852
- streaming-no-recycle: **+6** → ≥ 1799
- release: **+6** → ≥ 1750

---

## Shared test fixture

```rust
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
    }
}

fn default_rule() -> StopRule {
    StopRule {
        wall_budget: Duration::from_secs(300),
        memory_fraction_max: 0.80,
    }
}
```

Place this helper inside the same `#[cfg(test)] mod tests` block (private to the module).

---

## Unit Tests

### UT-0701-01: `check_wall_time_exceeded`

**Purpose:** `check` returns `Some(WallTimeExceeded)` iff `last_rep.wall > wall_budget`, AND nothing else trips.

**Inputs (3 sub-cases — all in one `#[test]` body):**
```rust
let rule = default_rule();

// (a) wall == budget: NOT exceeded
let r_eq = rep(1000, 300, 1024, 0.05, ChildExit::Ok);

// (b) wall > budget by 1 second: EXCEEDED
let r_over = rep(1000, 301, 1024, 0.05, ChildExit::Ok);

// (c) wall < budget: NOT exceeded
let r_under = rep(1000, 100, 1024, 0.05, ChildExit::Ok);
```

**Expected output:**
```rust
assert_eq!(rule.check(&r_eq),    None);
assert_eq!(rule.check(&r_over),  Some(StopReason::WallTimeExceeded));
assert_eq!(rule.check(&r_under), None);
```

**Edge cases:**
- (EC-1) Boundary equality (`wall == wall_budget`): MUST be `None` per the strict-inequality contract `wall > budget`.
- (EC-2) Wall = `Duration::ZERO` (e.g., zero-cost rep): trivially `None`.

---

### UT-0701-02: `check_memory_exceeded`

**Purpose:** `check` returns `Some(MemoryExceeded)` iff `vmrss_peak_fraction_of_total > memory_fraction_max`, with no OOM and wall under budget.

**Inputs:**
```rust
let rule = default_rule();

// (a) fraction == 0.80 exactly: NOT exceeded (strict >)
let r_eq = rep(1000, 60, 1024, 0.80, ChildExit::Ok);

// (b) fraction = 0.85 (>): EXCEEDED
let r_over = rep(1000, 60, 1024, 0.85, ChildExit::Ok);

// (c) fraction = 0.50 (<): NOT exceeded
let r_under = rep(1000, 60, 1024, 0.50, ChildExit::Ok);
```

**Expected output:**
```rust
assert_eq!(rule.check(&r_eq),    None);
assert_eq!(rule.check(&r_over),  Some(StopReason::MemoryExceeded));
assert_eq!(rule.check(&r_under), None);
```

**Edge cases:**
- (EC-1) Fraction = `1.0` (host completely full): trips MemoryExceeded.
- (EC-2) Fraction = `f64::NAN`: undefined behavior at the spec level. The test does NOT cover NaN; the rep runner upstream is responsible for producing finite fractions. If desired, DEV may add `assert!(rep.vmrss_peak_fraction_of_total.is_finite())` as a debug_assert at the top of `check`.

---

### UT-0701-03: `check_oom_sigkill_and_known_exit_codes`

**Purpose:** `check` returns `Some(Oom)` when the child exit indicates OOM. Three signatures recognized:

1. `ChildExit::Killed { signal: 9 }` (SIGKILL on Linux; also synthesized for Windows job-object termination per TASK-0701 hint #1).
2. `ChildExit::NonZero { code: 137 }` (= 128 + SIGKILL on bash-mediated paths).
3. `ChildExit::NonZero { code: -1073741801 }` (= `0xC0000017` Windows STATUS_NO_MEMORY interpreted as i32).

Other non-zero exit codes that are NOT in the OOM signature MUST NOT trigger `Oom`. With wall and memory under their limits, these return `None`.

**Inputs:**
```rust
let rule = default_rule();

// (a) SIGKILL
let r_sigkill = rep(1000, 60, 1024, 0.05, ChildExit::Killed { signal: 9 });

// (b) bash-mediated 137
let r_137 = rep(1000, 60, 1024, 0.05, ChildExit::NonZero { code: 137 });

// (c) Windows STATUS_NO_MEMORY
let r_winoom = rep(1000, 60, 1024, 0.05, ChildExit::NonZero { code: -1073741801 });

// (d) generic non-zero (NOT OOM): must NOT trip Oom (returns None — no other rule trips)
let r_generic = rep(1000, 60, 1024, 0.05, ChildExit::NonZero { code: 1 });

// (e) other signal (SIGSEGV = 11): NOT Oom
let r_segv = rep(1000, 60, 1024, 0.05, ChildExit::Killed { signal: 11 });
```

**Expected output:**
```rust
assert_eq!(rule.check(&r_sigkill), Some(StopReason::Oom));
assert_eq!(rule.check(&r_137),     Some(StopReason::Oom));
assert_eq!(rule.check(&r_winoom),  Some(StopReason::Oom));
assert_eq!(rule.check(&r_generic), None);
assert_eq!(rule.check(&r_segv),    None);
```

**Edge cases:**
- (EC-1) Generic non-zero (code=1) MUST NOT trip Oom — important guard against false positives. The campaign relies on this to distinguish "rep crashed" from "host ran out of memory".
- (EC-2) Other signals (e.g., SIGSEGV = 11) MUST NOT trip Oom.
- (EC-3) `ChildExit::Ok` MUST trivially return `None` (covered as a baseline in UT-0701-01).

**Open question for DEV:** Whether the OOM exit-code list should be a const slice `OOM_EXIT_CODES: &[i32] = &[137, -1073741801]` for clarity. Recommendation: yes.

---

### UT-0701-04: `check_priority_order_oom_over_memory_over_wall`

**Purpose:** When multiple conditions trip simultaneously, the priority is **`Oom > MemoryExceeded > WallTimeExceeded`** (TASK-0701 acceptance criterion 4).

**Inputs (4 sub-cases):**
```rust
let rule = default_rule();

// (a) Oom + Memory + Wall all true — Oom wins
let r_all = rep(1000, 301, 1024, 0.85, ChildExit::Killed { signal: 9 });

// (b) Memory + Wall true, Oom false — Memory wins
let r_mem_wall = rep(1000, 301, 1024, 0.85, ChildExit::Ok);

// (c) Wall only — Wall wins (only one rule trips)
let r_wall = rep(1000, 301, 1024, 0.05, ChildExit::Ok);

// (d) Oom + Wall (Memory under) — Oom wins
let r_oom_wall = rep(1000, 301, 1024, 0.05, ChildExit::Killed { signal: 9 });
```

**Expected output:**
```rust
assert_eq!(rule.check(&r_all),      Some(StopReason::Oom));
assert_eq!(rule.check(&r_mem_wall), Some(StopReason::MemoryExceeded));
assert_eq!(rule.check(&r_wall),     Some(StopReason::WallTimeExceeded));
assert_eq!(rule.check(&r_oom_wall), Some(StopReason::Oom));
```

**Edge cases:**
- (EC-1) The priority is total — there is no `Some(WallTimeExceeded)` outcome when memory or OOM is also tripped.
- (EC-2) Documented invariant: `check` is total (always returns `Option`, never panics, even for absurd inputs).

---

### UT-0701-05: `run_sequence_empty_input`

**Purpose:** Empty `n_seq` yields `SequenceOutcome { completed_reps: vec![], stop_reason: None, last_attempted_n: None }`.

**Input:**
```rust
let rule = default_rule();
let n_seq: &[usize] = &[];

let outcome = rule.run_sequence(n_seq, |_n| {
    panic!("runner must NOT be invoked for empty n_seq")
});
```

**Expected output:**
```rust
assert!(outcome.completed_reps.is_empty(), "expected zero reps");
assert_eq!(outcome.stop_reason, None);
assert_eq!(outcome.last_attempted_n, None);
```

**Edge cases:**
- (EC-1) The runner closure MUST NOT be invoked even once. The `panic!` in the closure body verifies this (if it executes, the test fails with the panic message).

---

### UT-0701-06: `run_sequence_completes_or_stops_at_first_violation`

**Purpose:** End-to-end sequence iteration: stops on first `Some(reason)`, captures the offending rep + reason; runs to completion when no rule trips.

**Inputs (2 sub-cases in one `#[test]`):**

**Sub-case A — completes successfully:**
```rust
let rule = default_rule();
let n_seq: &[usize] = &[1_000, 10_000, 100_000];
let mut call_count = 0usize;

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
```

**Sub-case B — stops at second N due to wall:**
```rust
let rule = default_rule();
let n_seq: &[usize] = &[1_000, 10_000, 100_000];
let mut call_count = 0usize;

let outcome = rule.run_sequence(n_seq, |n| {
    call_count += 1;
    if n == 10_000 {
        rep(n, 301, 1024, 0.05, ChildExit::Ok) // wall exceeded at second N
    } else {
        rep(n, 1, 1024, 0.05, ChildExit::Ok)
    }
});

assert_eq!(call_count, 2, "must NOT call runner for 100_000 after stop");
assert_eq!(outcome.completed_reps.len(), 2,
    "completed_reps must include the offending rep that triggered the stop");
assert_eq!(outcome.stop_reason, Some(StopReason::WallTimeExceeded));
assert_eq!(outcome.last_attempted_n, Some(10_000));
```

**Expected output:** see assertions inline.

**Edge cases:**
- (EC-1) After a stop, the runner closure is NOT invoked again — `call_count` confirms.
- (EC-2) The offending rep IS included in `completed_reps` (TASK-0701 acceptance criterion 5: "all reps that succeeded plus the offending rep"). This is a non-obvious contract; UT-0701-06 sub-case B locks it.
- (EC-3) `last_attempted_n` is `Some(10_000)` (not `Some(100_000)`); useful for the script + plot to know where the wall sat.
- (EC-4) Sub-case A confirms the "no stop, sequence completes" path explicitly per TASK-0701 acceptance criterion 7.

---

## Edge Cases Catalog (consolidated)

| # | Scenario | Expected Behavior | Test |
|---|----------|-------------------|------|
| EC-0701-01 | Wall == budget exactly | `None` (strict `>`) | UT-0701-01 (a) |
| EC-0701-02 | Wall = 0 | `None` | UT-0701-01 (c) |
| EC-0701-03 | Memory fraction == max exactly | `None` (strict `>`) | UT-0701-02 (a) |
| EC-0701-04 | Memory fraction = 1.0 | `Some(MemoryExceeded)` | implicitly via UT-0701-02 (b) at 0.85; explicit boundary not asserted |
| EC-0701-05 | Generic non-zero exit (code=1) | `None` (NOT Oom) | UT-0701-03 (d) |
| EC-0701-06 | SIGSEGV (signal=11) | `None` (NOT Oom) | UT-0701-03 (e) |
| EC-0701-07 | Oom + Memory + Wall together | `Some(Oom)` | UT-0701-04 (a) |
| EC-0701-08 | Memory + Wall together | `Some(MemoryExceeded)` | UT-0701-04 (b) |
| EC-0701-09 | Wall only | `Some(WallTimeExceeded)` | UT-0701-04 (c) |
| EC-0701-10 | Empty `n_seq` | runner not invoked; outcome zeroed | UT-0701-05 |
| EC-0701-11 | Stop mid-sequence | offending rep included; no further calls | UT-0701-06 (B) |
| EC-0701-12 | Sequence runs to completion | `stop_reason = None`, `last_attempted_n = Some(last)` | UT-0701-06 (A) |

## Out of scope

- Property tests — deferred per stage-2 directive.
- `MemoryProbe` integration — covered separately in TASK-0707 integration tests.
- Real child-process spawn / OOM-kill — that's TASK-0707 (d) territory.
- Async / concurrent `run_sequence` — out of scope (sequence is inherently serial).

## Open questions to surface to DEV

1. Should the OOM exit-code list be a public const so the script (TASK-0704) can reference it via FFI / parse? Recommendation: yes, name it `OOM_EXIT_CODES: &[i32]`.
2. Should `RepResult` carry a `ts_started: SystemTime` for forensic logs? Out of scope here; surface to TASK-UPDATER if needed.
