# TEST-SPEC-0702: Tests for TASK-0702 — `stress-curve` Campaign Descriptor

**Task:** TASK-0702
**Spec:** none
**Bundle:** D-014 (Stress Curve Campaign)
**Requirements covered:** Acceptance criteria 1-6 from TASK-0702
**Test IDs:** IT-0702-01 (single integration test)

---

## Scope

Verify the public surface added to `relativist-core/src/bench/suite.rs`:
- `Env` enum (`InProcess`, `DockerTcp`)
- `StressWorkload` enum (`EpAnnihilation`, `DualTree`, `CondupExpansion`)
- `StressCurveDescriptor` namespace struct
- `StressCurveDescriptor::n_seq() -> &'static [usize]`
- `StressCurveDescriptor::default_stop_rule(env: Env) -> StopRule`
- `StressCurveDescriptor::run_one_sequence(workload, env, workers, reps, n_seq_override, stop_rule_override) -> Result<SequenceOutcome, BenchError>`

Plus the CLI dispatch in `relativist-core/src/commands.rs::run_bench_command` (line 288) consuming the new `BenchArgs` fields (`relativist-core/src/config.rs:571`).

The +1 test floor delta is for **a single integration test file** with **a single `#[test]` function** that exercises multiple sub-assertions (the pure data getters + the smoke-execution of `run_one_sequence`). This matches TASK-0702's "Test floor delta: +1 default" wording. Splitting into multiple `#[test]` would inflate the count beyond the contract.

## Test category & location

| # | Name | Category | File | Cfg gating |
|---|------|----------|------|-----------|
| IT-0702-01 | `descriptor_data_and_smoke_run` | integration | `relativist-core/tests/d014_stress_curve_descriptor.rs` | none (cross-platform) |

LoC budget: ~60 LoC matching TASK-0702.

## Test floor delta

- default: **+1** → ≥ 1809
- zero-copy: **+1** → ≥ 1853
- streaming-no-recycle: **+1** → ≥ 1800
- release: **+1** → ≥ 1751

---

## Integration Tests

### IT-0702-01: `descriptor_data_and_smoke_run`

**Purpose:** Cover acceptance criteria 1-3 + 6 in a single test function. Each sub-assertion has a clear failure message so triage on red is immediate.

**Preconditions:** Cargo build of `relativist-core` succeeds; `MemoryProbe` constructable on the host (Linux + Windows; on macOS the test self-skips).

**Imports (sketch):**
```rust
use relativist_core::bench::stop_rule::{StopReason, ChildExit};
use relativist_core::bench::suite::{
    Env, StressCurveDescriptor, StressWorkload,
};
use std::time::Duration;
```

**Test body — Part A: canonical N sweep (acceptance criterion 1):**

```rust
const EXPECTED_N: &[usize] = &[
    10_000, 31_623, 100_000, 316_228,
    1_000_000, 3_162_278, 10_000_000, 31_622_776,
    100_000_000, 316_227_766, 1_000_000_000,
];

let n_seq = StressCurveDescriptor::n_seq();
assert_eq!(
    n_seq, EXPECTED_N,
    "n_seq must match design doc §4.4 verbatim (×√10 from 10⁴ to 10⁹)"
);
assert_eq!(n_seq.len(), 11, "n_seq must have exactly 11 entries");
```

**Part B: per-env stop rule defaults (acceptance criterion 2):**

```rust
let r_inp = StressCurveDescriptor::default_stop_rule(Env::InProcess);
assert_eq!(r_inp.wall_budget, Duration::from_secs(300),
    "InProcess wall budget must be 5 min (300s)");
assert!((r_inp.memory_fraction_max - 0.80).abs() < f64::EPSILON,
    "InProcess memory_fraction_max must be 0.80; got {}", r_inp.memory_fraction_max);

let r_doc = StressCurveDescriptor::default_stop_rule(Env::DockerTcp);
assert_eq!(r_doc.wall_budget, Duration::from_secs(450),
    "DockerTcp wall budget must be 7m30s (450s)");
assert!((r_doc.memory_fraction_max - 0.80).abs() < f64::EPSILON,
    "DockerTcp memory_fraction_max must be 0.80; got {}", r_doc.memory_fraction_max);
```

**Part C: smoke run (acceptance criteria 3 + 6):**

```rust
// Skip on macOS — MemoryProbe is unsupported there per TASK-0700.
#[cfg(target_os = "macos")]
{
    return;
}

#[cfg(not(target_os = "macos"))]
{
    let n_override = [1_000usize, 10_000usize];
    let stop = relativist_core::bench::stop_rule::StopRule {
        wall_budget: Duration::from_secs(30),
        memory_fraction_max: 0.95,  // very loose — smoke must not trip
    };

    let outcome = StressCurveDescriptor::run_one_sequence(
        StressWorkload::EpAnnihilation,
        Env::InProcess,
        /* workers */ 1,
        /* reps */    1,
        Some(&n_override),
        Some(stop),
    ).expect("smoke run must succeed in-process for ep_annihilation N=[1k, 10k]");

    assert_eq!(
        outcome.completed_reps.len(),
        2,
        "expected exactly 2 completed reps for n_seq=[1k, 10k] reps=1; got {}",
        outcome.completed_reps.len()
    );
    assert_eq!(outcome.stop_reason, None,
        "smoke must complete without tripping any rule");
    assert_eq!(outcome.last_attempted_n, Some(10_000));

    // Per-rep sanity:
    for (i, r) in outcome.completed_reps.iter().enumerate() {
        let expected_n = n_override[i];
        assert_eq!(r.n, expected_n, "rep[{}] N mismatch", i);
        assert!(r.wall > Duration::ZERO, "rep[{}] wall must be > 0", i);
        assert!(r.vmrss_peak_bytes > 0, "rep[{}] vmrss_peak_bytes must be > 0", i);
        assert!(r.vmrss_peak_fraction_of_total > 0.0
                && r.vmrss_peak_fraction_of_total <= 1.0,
            "rep[{}] vmrss_peak_fraction_of_total must be in (0, 1]; got {}",
            i, r.vmrss_peak_fraction_of_total);
        match r.child_exit {
            ChildExit::Ok => {},
            other => panic!("rep[{}] expected ChildExit::Ok; got {:?}", i, other),
        }
    }
}
```

**Expected output:** All assertions pass. The test runs in well under 30 seconds for `N <= 10_000` of `ep_annihilation`.

**Edge cases (in-test):**
- (EC-1) macOS host: test early-returns. Documented at the top of the body.
- (EC-2) DockerTcp env: NOT exercised here — that path panics with `BenchError::Unsupported` per TASK-0702 implementation hint #5. Covered indirectly by the script in TASK-0704.
- (EC-3) `n_seq_override = None` (canonical sweep): out of scope for this smoke — the canonical sweep would take 7+ hours; the override path is the one we test.
- (EC-4) `reps=0`: out of scope; the script always passes `reps >= 1`. No assertion needed.

---

## Acceptance criteria mapping

| TASK-0702 AC | Test coverage |
|---|---|
| AC-1 (canonical n_seq) | IT-0702-01 Part A |
| AC-2 (per-env stop_rule defaults) | IT-0702-01 Part B |
| AC-3 (`run_one_sequence` returns SequenceOutcome) | IT-0702-01 Part C |
| AC-4 (macOS BenchError propagation) | IT-0702-01 EC-1 (test self-skips; DEV verifies the propagation manually) |
| AC-5 (CLI command works) | NOT covered by Rust test — manual / script-level (TASK-0704). Document this gap in DEV's PR description. |
| AC-6 (test compiles + passes) | IT-0702-01 by construction |

**Open question for DEV:** AC-5 ("relativist-bench --campaign stress-curve --workload ep_annihilation --env in_process --workers 1 --reps 1 --n-seq 1000,10000 runs a 2-row sequence and emits stdout-friendly CSV") could be covered with `assert_cmd` crate. If `assert_cmd` is already a dev-dependency in `relativist-core/Cargo.toml`, the test can drive the binary directly and validate the CSV row count. Otherwise leave as a script-level check (TASK-0704). Recommendation: keep this TEST-SPEC at +1 test (don't add a CLI smoke); cover the CLI path through TASK-0704's smoke instead. The TASK ownership is clean that way.

## Edge Cases Catalog

| # | Scenario | Expected | Test |
|---|----------|----------|------|
| EC-0702-01 | macOS host | early return; no assertions | IT-0702-01 Part C |
| EC-0702-02 | `n_seq_override=Some(&[])` | `SequenceOutcome { completed_reps:[], stop_reason:None, last_attempted_n:None }` | NOT TESTED — covered by UT-0701-05 in TEST-SPEC-0701 (delegated; same `run_sequence` underlies it) |
| EC-0702-03 | Stop rule with very tight wall budget | should trip on first rep | NOT TESTED — covered by UT-0701-01; descriptor wires through `StopRule` unchanged |
| EC-0702-04 | DockerTcp env | descriptor returns `BenchError::Unsupported` for in-Rust calls | NOT TESTED in this TEST-SPEC; manually verified by DEV during implementation |

## Out of scope

- Property tests — deferred.
- Long sequences (canonical 11-point sweep) — that's the campaign run, TASK-0708.
- DockerTcp in-Rust path — by design panics with `BenchError::Unsupported`.
- CLI assert_cmd — see "Open question for DEV" above.

## Cross-references

- TASK-0702 (this TEST-SPEC's contract).
- TEST-SPEC-0700 (MemoryProbe — `run_one_sequence` consumes it).
- TEST-SPEC-0701 (StopRule — `default_stop_rule` returns it; `run_one_sequence` honors it).
- TEST-SPEC-0707 (end-to-end smoke `(f)` partially overlaps; that test focuses on the smoke contract whereas IT-0702-01 also asserts the canonical-data getters).
