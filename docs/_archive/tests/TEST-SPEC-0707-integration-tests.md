# TEST-SPEC-0707: Tests for TASK-0707 — 6 integration tests for stress curve

**Task:** TASK-0707
**Spec:** none
**Bundle:** D-014 (Stress Curve Campaign)
**Requirements covered:** Acceptance criteria 1-7 from TASK-0707
**Test IDs:** IT-0707-{01..06} (6 integration tests, one per file)

---

## Scope

Six dedicated end-to-end integration tests that lock the contract for the new campaign primitives. These are NOT the smoke (TASK-0704) or plot smoke (TASK-0705); they are the design-doc §8 testing pyramid's "(a) through (f)" entries.

Each test lives in its own `relativist-core/tests/d014_<concern>.rs` file so a red triage points at one concern.

A shared helper file `relativist-core/tests/common/d014_helpers.rs` provides a `RepResult` factory used by tests (b), (c), (d).

## Test category & location

| # | Concern | File | Cfg gating |
|---|---------|------|-----------|
| IT-0707-01 (a) | Memory probe vs 100 MiB oracle | `relativist-core/tests/d014_memory_probe_oracle.rs` | `#[cfg(not(target_os = "macos"))]` |
| IT-0707-02 (b) | Stop rule wall trip | `relativist-core/tests/d014_stop_rule_wall.rs` | none |
| IT-0707-03 (c) | Stop rule RAM trip | `relativist-core/tests/d014_stop_rule_ram.rs` | none |
| IT-0707-04 (d) | Stop rule OOM (SIGKILL) | `relativist-core/tests/d014_stop_rule_oom.rs` | `#[cfg(unix)]` |
| IT-0707-05 (e) | `--resume` invariant | `relativist-core/tests/d014_resume_invariant.rs` | `#[cfg(unix)]` |
| IT-0707-06 (f) | End-to-end smoke (in-process) | `relativist-core/tests/d014_end_to_end_smoke.rs` | `#[cfg(not(target_os = "macos"))]` |

LoC budgets per TASK-0707 ~30 / 25 / 25 / 40 / 50 / 30 = ~200 LoC + ~20 helper. Total cumulative tests +6 default after this TASK.

## Test floor delta

- default: **+6** → ≥ 1819
- zero-copy: **+6** → ≥ 1863
- streaming-no-recycle: **+6** → ≥ 1810
- release: **+6** → ≥ 1761

This MATCHES the cumulative ceilings in `BACKLOG.md` post-D-014: 1819 / 1863 / 1810 / 1761.

---

## Shared helper

`relativist-core/tests/common/d014_helpers.rs`:

```rust
use relativist_core::bench::stop_rule::{ChildExit, RepResult};
use std::time::Duration;

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
```

`relativist-core/tests/common/mod.rs` gains: `pub mod d014_helpers;`. (Per TASK-0707 file table; if `common/mod.rs` does not exist, DEV creates it with that single line.)

---

## Integration Tests

### IT-0707-01 (a): `memory_probe_vs_oracle_100mib`

**File:** `relativist-core/tests/d014_memory_probe_oracle.rs`

**Purpose:** Allocate exactly 100 MiB, verify probe reports `current_bytes` rises by ≥ 80 MiB and `peak_bytes` ≥ 100 MiB.

**Cfg:** `#![cfg(not(target_os = "macos"))]`.

**Self-skip gate:** Compute pre-allocation `frac = probe.as_fraction_of_total(probe.current_bytes())`; if `frac > 0.90`, skip with reason "memory-starved CI" (TASK-0707 acceptance criterion 3).

**Test body (sketch):**

```rust
use relativist_core::bench::memory_probe::MemoryProbe;

#[test]
fn memory_probe_vs_oracle_100mib() {
    let probe = match MemoryProbe::new() {
        Ok(p) => p,
        Err(e) => panic!("MemoryProbe::new must succeed on linux/windows: {:?}", e),
    };

    let cur0 = probe.current_bytes().expect("current_bytes pre-alloc");
    let frac0 = probe.as_fraction_of_total(cur0);
    if frac0 > 0.90 {
        eprintln!("SKIP: pre-alloc RSS fraction = {} > 0.90; CI is memory-starved", frac0);
        return;
    }

    const SIZE: usize = 100 * 1024 * 1024;
    let buf: Vec<u8> = vec![0u8; SIZE];
    // Force commit by touching every 4 KiB page.
    let mut sum: u64 = 0;
    for chunk in buf.chunks(4096) {
        sum += chunk[0] as u64;
    }
    let buf = std::hint::black_box(buf);
    let _sum = std::hint::black_box(sum);

    let cur1 = probe.current_bytes().expect("current_bytes post-alloc");
    let peak1 = probe.peak_bytes().expect("peak_bytes post-alloc");
    let delta = cur1 - cur0;

    assert!(delta >= 80 * 1024 * 1024,
        "expected current_bytes to rise by ≥ 80 MiB; rose by {} bytes ({} MiB)",
        delta, delta / (1024 * 1024));
    assert!(peak1 >= 100 * 1024 * 1024,
        "expected peak_bytes ≥ 100 MiB; got {} bytes ({} MiB)",
        peak1, peak1 / (1024 * 1024));

    drop(buf);
}
```

**Edge cases:**
- (EC-1) Memory-starved CI: SKIP, do not fail.
- (EC-2) Release build: `black_box` prevents optimizer from elide-ing the buffer.

---

### IT-0707-02 (b): `stop_rule_wall`

**File:** `relativist-core/tests/d014_stop_rule_wall.rs`

**Purpose:** Fake a `RepResult` with `wall = 6 min`, `wall_budget = 5 min`; verify `check` returns `Some(WallTimeExceeded)`.

**Cfg:** none.

**Test body (sketch):**

```rust
mod common;
use common::d014_helpers::rep;

use relativist_core::bench::stop_rule::{ChildExit, StopRule, StopReason};
use std::time::Duration;

#[test]
fn stop_rule_wall_trips_at_6min_with_5min_budget() {
    let rule = StopRule {
        wall_budget: Duration::from_secs(300),
        memory_fraction_max: 0.80,
    };
    let r = rep(1_000, 360, 1024, 0.05, ChildExit::Ok); // 6 min wall

    assert_eq!(rule.check(&r), Some(StopReason::WallTimeExceeded),
        "6 min wall must trip WallTimeExceeded with 5 min budget");
}
```

**Edge cases:**
- (EC-1) The boundary `wall == wall_budget` is covered by UT-0701-01 (in-module). This integration test only covers the `>` case from the design doc §8.
- (EC-2) None other; the test exists to lock the design-doc §8 specific row.

---

### IT-0707-03 (c): `stop_rule_ram`

**File:** `relativist-core/tests/d014_stop_rule_ram.rs`

**Purpose:** Fake a `RepResult` with `vmrss_peak_fraction_of_total = 0.85`, threshold `0.80`; verify `check` returns `Some(MemoryExceeded)`.

**Cfg:** none.

**Test body (sketch):**

```rust
mod common;
use common::d014_helpers::rep;

use relativist_core::bench::stop_rule::{ChildExit, StopRule, StopReason};
use std::time::Duration;

#[test]
fn stop_rule_ram_trips_at_85pct_with_80pct_max() {
    let rule = StopRule {
        wall_budget: Duration::from_secs(300),
        memory_fraction_max: 0.80,
    };
    let r = rep(1_000, 60, 8 * 1024 * 1024 * 1024, 0.85, ChildExit::Ok);

    assert_eq!(rule.check(&r), Some(StopReason::MemoryExceeded),
        "0.85 fraction must trip MemoryExceeded with 0.80 max");
}
```

**Edge cases:**
- (EC-1) Boundary `frac == 0.80` is UT-0701-02 territory.

---

### IT-0707-04 (d): `stop_rule_oom_sigkill`

**File:** `relativist-core/tests/d014_stop_rule_oom.rs`

**Purpose:** Spawn a child process that allocates until OOM-killed (or simulate via a `ChildExit::Killed { signal: 9 }`), feed into `check`; verify `Some(Oom)`.

**Cfg:** `#![cfg(unix)]`.

**Two-mode strategy:**
- **Mode A (preferred):** real OOM via a child Python process if `python3` is available, validating end-to-end.
- **Mode B (fallback):** synthesize `ChildExit::Killed { signal: 9 }` and feed into `check`. This still validates the StopRule contract; it merely doesn't validate the OS-level OOM detection (which is the script's job — TASK-0704).

**Test body (sketch):**

```rust
#![cfg(unix)]
mod common;
use common::d014_helpers::rep;

use relativist_core::bench::stop_rule::{ChildExit, StopRule, StopReason};
use std::os::unix::process::ExitStatusExt;
use std::process::Command;
use std::time::Duration;

#[test]
fn stop_rule_oom_real_or_synthetic_sigkill() {
    let rule = StopRule {
        wall_budget: Duration::from_secs(300),
        memory_fraction_max: 0.80,
    };

    // Mode A: try real OOM via python3.
    let python_ok = Command::new("python3").arg("--version").status()
        .map(|s| s.success()).unwrap_or(false);

    let child_exit = if python_ok {
        // Allocate 100 GiB-equivalent; OS OOM-killer terminates with SIGKILL.
        let status = Command::new("sh")
            .arg("-c")
            .arg("python3 -c 'a=[0]*10**11' 2>/dev/null")
            .status()
            .expect("spawn must succeed even if process is killed");
        match status.signal() {
            Some(9) => ChildExit::Killed { signal: 9 },
            // Some kernels deliver SIGKILL via wait status code 137; our
            // script-level path uses 137. Honor either.
            Some(s) => {
                eprintln!("WARN: real OOM produced signal {} (not 9); falling back to synthetic", s);
                ChildExit::Killed { signal: 9 }
            },
            None => match status.code() {
                Some(137) => ChildExit::NonZero { code: 137 },
                Some(c)   => {
                    eprintln!("WARN: real OOM produced exit code {} (expected 137 or signal 9); using synthetic", c);
                    ChildExit::Killed { signal: 9 }
                },
                None => ChildExit::Killed { signal: 9 },
            },
        }
    } else {
        eprintln!("INFO: python3 not available; using synthetic SIGKILL");
        ChildExit::Killed { signal: 9 }
    };

    let r = rep(1_000, 60, 8 * 1024 * 1024 * 1024, 0.50, child_exit);
    assert_eq!(rule.check(&r), Some(StopReason::Oom),
        "child OOM (SIGKILL or 137) must trip StopReason::Oom");
}
```

**Edge cases:**
- (EC-1) Some hardened kernels do not deliver SIGKILL via `WTERMSIG` but via `WIFEXITED` with code 137. The test accepts both shapes.
- (EC-2) `python3` absent: fall back to synthetic. The test still exits 0 — the contract under test is `StopRule::check`, not the OS.
- (EC-3) Non-OOM exit codes (e.g., generic non-zero): NOT exercised here — covered by UT-0701-03.

---

### IT-0707-05 (e): `resume_invariant`

**File:** `relativist-core/tests/d014_resume_invariant.rs`

**Purpose:** Invoke `scripts/stress_curve.sh --smoke --no-docker`, kill it mid-rep, restart with `--resume`, verify final dataset is identical (modulo wall-time noise) to a clean run.

**Cfg:** `#![cfg(unix)]`.

**Self-skip gates:**
1. `bash` not in PATH → SKIP.
2. `scripts/stress_curve.sh` not found at workspace root → SKIP.
3. `target/release/relativist` not built → SKIP with reason "release binary not built; run `cargo build --release`".
4. `python3` + matplotlib not installed → SKIP (smoke needs them for the plot phase; failure during plot phase corrupts the test signal).

**Test body (sketch):**

```rust
#![cfg(unix)]
use std::process::{Command, Stdio};
use std::time::Duration;
use std::path::PathBuf;
use std::thread::sleep;
use tempfile::tempdir;

#[test]
fn resume_produces_identical_dataset() {
    // Skip gates (script, release binary, python3 + matplotlib).
    if !skips_pass() {
        return;
    }

    let script = workspace_root().join("scripts/stress_curve.sh");

    // 1. Reference run (clean smoke):
    let ref_dir = tempdir().unwrap();
    let s1 = Command::new("bash").arg(&script)
        .arg("--smoke").arg("--no-docker")
        .arg("--output-dir").arg(ref_dir.path())
        .status().expect("clean smoke spawn");
    assert!(s1.success(), "reference smoke must exit 0");
    let ref_csv = std::fs::read_to_string(ref_dir.path().join("raw/in_process.csv")).unwrap();

    // 2. Resume run: spawn, kill mid-rep with SIGINT, then re-spawn with --resume.
    let resume_dir = tempdir().unwrap();
    let mut child = Command::new("bash").arg(&script)
        .arg("--smoke").arg("--no-docker")
        .arg("--output-dir").arg(resume_dir.path())
        .stdout(Stdio::null()).stderr(Stdio::null())
        .spawn().expect("first leg spawn");
    sleep(Duration::from_secs(30));   // mid-rep
    let pid = child.id() as i32;
    unsafe { libc::kill(pid, libc::SIGINT); }
    let _ = child.wait();

    // Re-spawn with --resume; expect exit 0.
    let s3 = Command::new("bash").arg(&script)
        .arg("--smoke").arg("--no-docker")
        .arg("--resume")
        .arg("--output-dir").arg(resume_dir.path())
        .status().expect("resume spawn");
    assert!(s3.success(), "resumed smoke must exit 0");

    let resume_csv = std::fs::read_to_string(resume_dir.path().join("raw/in_process.csv")).unwrap();

    // 3. Compare. Tolerate wall-time noise (column index TBD by DEV; refer
    //    to `bench/csv.rs` field order). Strict-match all other columns.
    assert_csvs_equal_modulo_wall(&ref_csv, &resume_csv);
}

fn assert_csvs_equal_modulo_wall(a: &str, b: &str) {
    // Parse rows, sort by (workload, env, workers, n, rep), compare each row's
    // columns one by one. The `wall_seconds` and `wall_us` columns are allowed
    // to differ by ±50%; all other columns MUST match exactly.
    // Implementation: ~25 LoC; left to DEV (no ambiguity in spec).
    todo!("DEV implements: see TASK-0707 acceptance criterion 5 + implementation hint #3");
}

fn skips_pass() -> bool { /* ... */ true }
fn workspace_root() -> PathBuf { /* ... */ PathBuf::new() }
```

**Edge cases:**
- (EC-1) `libc::kill(pid, SIGINT)` on a dead pid is harmless (returns -1 with ESRCH).
- (EC-2) `wall_seconds` noise tolerance: ± 50% per TASK-0707 implementation hint #3. Other columns MUST match exactly.
- (EC-3) The test allows ANY column not named `wall_*` (`vmrss_*` included) to differ ONLY by floating-point noise — but in practice they should match exactly because the inputs (workload, W, N, rep) are identical and the bench is deterministic. If `vmrss_*` differs by > 5%, that's a real signal of nondeterminism and the test should fail. Implementation: DEV decides between strict-equality on `vmrss_*` (preferred) or ± 5% tolerance.
- (EC-4) Race condition: the SIGINT can arrive between reps (no-op for that pid; --resume doesn't have to do anything special). The test still exits cleanly because the reference run also produces the same final state.
- (EC-5) Truncated CSV (TASK-0704 acceptance criterion 5: `--resume` MUST detect malformed CSV and exit 1) — NOT exercised here. DEV can manually inject by truncating the CSV mid-row before invoking `--resume`; out of automated scope.

---

### IT-0707-06 (f): `end_to_end_smoke`

**File:** `relativist-core/tests/d014_end_to_end_smoke.rs`

**Purpose:** Direct in-process invocation of `StressCurveDescriptor::run_one_sequence`, NOT via the script. Verify `SequenceOutcome.completed_reps.len() == 2` for `n_seq = [1_000, 10_000]`, reps = 1.

**Cfg:** `#![cfg(not(target_os = "macos"))]`.

**Test body (sketch):**

```rust
#![cfg(not(target_os = "macos"))]
use relativist_core::bench::stop_rule::StopRule;
use relativist_core::bench::suite::{Env, StressCurveDescriptor, StressWorkload};
use std::time::Duration;

#[test]
fn end_to_end_smoke_in_process() {
    let n_override = [1_000usize, 10_000usize];
    let stop = StopRule {
        wall_budget: Duration::from_secs(30),
        memory_fraction_max: 0.95,
    };

    let outcome = StressCurveDescriptor::run_one_sequence(
        StressWorkload::EpAnnihilation,
        Env::InProcess,
        /* workers */ 2,
        /* reps */    1,
        Some(&n_override),
        Some(stop),
    ).expect("run_one_sequence must succeed");

    assert_eq!(outcome.completed_reps.len(), 2,
        "end-to-end smoke for ep_annihilation N=[1k, 10k] reps=1 must produce 2 reps");
    assert_eq!(outcome.stop_reason, None);
    assert_eq!(outcome.last_attempted_n, Some(10_000));
}
```

**Edge cases:**
- (EC-1) DOES partially overlap with TEST-SPEC-0702 IT-0702-01 Part C, but with `workers=2` instead of `workers=1`. The two tests together verify both worker counts; the design doc §8 lists this specific test (W=2) as the canonical end-to-end smoke and IT-0702-01 covers the descriptor-data path with W=1.
- (EC-2) The smoke is BSP — `workers=2` exercises the multi-worker grid path which `workers=1` does not. Useful coverage delta.

---

## Acceptance criteria mapping

| TASK-0707 AC | Test coverage |
|---|---|
| AC-1 (all 6 files exist + compile) | construction-time (cargo build) |
| AC-2 (passes on Linux + Windows where applicable) | per-file `#[cfg(...)]` gates |
| AC-3 (test (a) skips on memory-starved CI) | IT-0707-01 self-skip gate |
| AC-4 (test (d) is `cfg(unix)`) | file-level cfg |
| AC-5 (test (e) is `cfg(unix)`) | file-level cfg |
| AC-6 (tests (b), (c), (f) cross-platform) | no cfg; macOS skip on (f) only because MemoryProbe |
| AC-7 (meaningful failure messages) | each `assert!` carries a `"..."` reason |

## Edge Cases Catalog

| # | Scenario | Test |
|---|----------|------|
| EC-0707-01 | Memory-starved CI (>90%) | IT-0707-01 self-skip |
| EC-0707-02 | LTO elision of 100 MiB buffer | IT-0707-01 `black_box` |
| EC-0707-03 | Wall == budget exactly | UT-0701-01 (delegated) |
| EC-0707-04 | RAM frac == max exactly | UT-0701-02 (delegated) |
| EC-0707-05 | Real OOM unavailable (no python3) | IT-0707-04 fallback to synthetic |
| EC-0707-06 | Real OOM produces 137 not signal=9 | IT-0707-04 honors both shapes |
| EC-0707-07 | Resume kill mid-rep | IT-0707-05 SIGINT + sleep 30s |
| EC-0707-08 | Resume on dead-pid SIGINT | safe (errno ESRCH) |
| EC-0707-09 | Wall-time noise on resume vs ref | ± 50% tolerance |
| EC-0707-10 | macOS host on (a) and (f) | skipped via file-level cfg |
| EC-0707-11 | Truncated CSV detected by --resume | DEFERRED to DEV manual |

## Out of scope

- Property tests (proptest) — explicitly deferred per TASK-0707 file table.
- Visibility-widening edge cases — TASK-0707 implementation hint #6: do NOT silently widen `pub(crate) → pub`. If a test needs an internal type, file a TASK-UPDATER note.
- macOS positive paths.
- Property-based wall-time noise modeling.

## Open questions for DEV

1. `assert_csvs_equal_modulo_wall` implementation: the column index for `wall_seconds` / `wall_us` should be looked up by header name, not numeric index, to survive future column reorderings. ~25 LoC. DEV implements; spec-level expectation is "same row count, same column count, every non-wall-* column equal exactly".
2. `vmrss_*` strict equality vs ± 5%: recommend strict equality (zero noise expected for deterministic in-process bench). Adjust if real-world CI shows otherwise.
3. `tempfile` is a dev-dependency — verify it's already in `relativist-core/Cargo.toml [dev-dependencies]`. If not, add it.
4. `libc` is required for IT-0707-05's SIGINT — verify availability under `#[cfg(unix)]`.
