# TEST-SPEC-0704: Tests for TASK-0704 — `scripts/stress_curve.sh` orchestrator

**Task:** TASK-0704
**Spec:** none
**Bundle:** D-014 (Stress Curve Campaign)
**Requirements covered:** Acceptance criteria 1-6 from TASK-0704
**Test IDs:** IT-0704-01 (single integration test, `cfg(unix)` only)

---

## Scope

A single Rust integration test that drives `scripts/stress_curve.sh --smoke --no-docker --output-dir <tmp>` from `Command::new("bash")` and asserts the output structure. The script's internal correctness (precondition gate, resume logic, MANIFEST synthesis) is exercised end-to-end by this smoke; finer-grained unit testing of bash subroutines is OUT of scope (bash unit testing infra not present in the repo; not worth adding for this campaign).

The test is **`#[cfg(unix)]`-gated** — Windows hosts run via WSL but the harness invokes `bash` directly, so the test simply skips on `target_os = "windows"` (the dev still runs WSL manually for full validation).

## Test category & location

| # | Name | Category | File | Cfg gating |
|---|------|----------|------|-----------|
| IT-0704-01 | `script_smoke_runs_and_produces_artifacts` | integration | `relativist-core/tests/d014_stress_curve_smoke.rs` | `#[cfg(unix)]` |

LoC budget ~80 LoC matching TASK-0704.

## Test floor delta

- default: **+1** → ≥ 1812
- zero-copy: **+1** → ≥ 1856
- streaming-no-recycle: **+1** → ≥ 1803
- release: **+1** → ≥ 1754

(On Windows the test compiles to a no-op stub; the +1 still counts because cargo-test reports it as a passed unit. If DEV wants to be conservative, mark with `#[ignore]` on Windows and skip the count there — adjust the floor for the Windows row of the cumulative table per TASK-0704's discretion.)

---

## Integration Tests

### IT-0704-01: `script_smoke_runs_and_produces_artifacts`

**Purpose:** Acceptance criteria 1, 2, 5 — invoking the script in `--smoke --no-docker` mode produces the expected output tree.

**Cfg gating:**
```rust
#![cfg(unix)]   // file-level gate, skip the entire file on Windows
```

**Preconditions checked at runtime (skip with `eprintln!` + early return; do NOT panic):**
1. `which bash` returns success.
2. The crate's workspace root can be determined (e.g., `env!("CARGO_MANIFEST_DIR")` then walk up to find `scripts/stress_curve.sh`).
3. ≥ 1 GiB free RAM available (use `MemoryProbe` if convenient; otherwise `/proc/meminfo` parse). If less, skip with a printed reason — smoke must not run on a truly memory-starved CI.
4. `cargo build --release` for the bench binary has succeeded earlier in the test run (the smoke invokes the release binary). Detection: existence of `target/release/relativist` (Unix) — if absent, skip with reason "release binary not built; run `cargo build --release` first". DEV may instead invoke `cargo build --release` from within the test; recommend skip-with-reason for fast CI iteration.

**Test body (sketch):**

```rust
use std::process::Command;
use std::path::PathBuf;
use tempfile::tempdir;

#[test]
fn script_smoke_runs_and_produces_artifacts() {
    // 1. Locate the script.
    let script = workspace_root().join("scripts/stress_curve.sh");
    if !script.exists() {
        eprintln!("SKIP: scripts/stress_curve.sh not found at {:?}", script);
        return;
    }

    // 2. Locate the release binary; skip if missing.
    let bin = workspace_root().join("target/release/relativist");
    if !bin.exists() {
        eprintln!("SKIP: target/release/relativist not built; \
                   run `cargo build --release` before this test");
        return;
    }

    // 3. Free-RAM gate.
    if let Ok(probe) = relativist_core::bench::memory_probe::MemoryProbe::new() {
        let cur = probe.current_bytes().unwrap_or(0);
        let frac = probe.as_fraction_of_total(cur);
        if frac > 0.85 {
            eprintln!("SKIP: host RAM > 85% used (frac={}); smoke would be flaky", frac);
            return;
        }
    }

    // 4. Run smoke.
    let outdir = tempdir().expect("tempdir");
    let outdir_path = outdir.path();
    let status = Command::new("bash")
        .arg(&script)
        .arg("--smoke")
        .arg("--no-docker")
        .arg("--output-dir")
        .arg(outdir_path)
        .status()
        .expect("bash spawn must succeed");

    assert!(status.success(),
        "smoke script must exit 0; got {:?}", status);

    // 5. Verify output structure.
    let manifest = outdir_path.join("MANIFEST.md");
    assert!(manifest.exists(), "MANIFEST.md must exist at {:?}", manifest);
    let manifest_text = std::fs::read_to_string(&manifest).expect("read MANIFEST");
    assert!(manifest_text.contains("git rev"),
        "MANIFEST.md must include git SHA section");
    assert!(manifest_text.contains("rustc"),
        "MANIFEST.md must include rustc version");

    let raw_csv = outdir_path.join("raw").join("in_process.csv");
    assert!(raw_csv.exists(),
        "raw/in_process.csv must exist at {:?}", raw_csv);
    let csv_text = std::fs::read_to_string(&raw_csv).expect("read csv");
    let line_count = csv_text.lines().count();
    assert!(line_count >= 3,   // header + ≥ 2 data rows for N=[1k, 10k]
        "raw CSV must have header + ≥ 2 rows; got {} lines", line_count);
    assert!(csv_text.lines().next().unwrap().contains("vmrss_peak_mb"),
        "CSV header must include the new column `vmrss_peak_mb`");

    let aggregated = outdir_path.join("aggregated.csv");
    assert!(aggregated.exists(), "aggregated.csv must exist");

    let figures_dir = outdir_path.join("figures");
    assert!(figures_dir.is_dir(), "figures/ must be a directory");
    let pdfs: Vec<_> = std::fs::read_dir(&figures_dir).unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |x| x == "pdf"))
        .collect();
    assert!(!pdfs.is_empty(), "figures/ must contain ≥ 1 PDF");

    let checksums = outdir_path.join("checksums.sha256");
    assert!(checksums.exists() && std::fs::metadata(&checksums).unwrap().len() > 0,
        "checksums.sha256 must exist and be non-empty");
}

fn workspace_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // Walk up until we find Cargo.lock at the workspace level.
    while !p.join("Cargo.lock").exists() && p.pop() {}
    p
}
```

**Expected output:** All assertions pass within the 20-minute budget.

**Edge cases:**
- (EC-1) Release binary missing: test SKIPS (no panic) with a printed message. Acceptable on iterative-DEV runs that haven't built release.
- (EC-2) Memory-starved host: SKIPS with a printed reason rather than producing a flaky failure.
- (EC-3) `python3` missing on the test host: the script will exit non-zero in Phase 3 (plot invocation). Acceptance criterion 1 says "exits 0 within 20 minutes on a workstation with ≥ 8 GiB RAM" — the developer documents the python3 dependency in TASK-0706 docs; the test itself fails with a clear `status.success()` panic. DEV may also pre-check `which python3` and skip-if-absent.
- (EC-4) `--resume` logic NOT exercised in this test (acceptance criterion 3 is covered separately by TEST-SPEC-0707 IT-0707-05).
- (EC-5) Pre-condition gate (acceptance criterion 4 — dirty tree triggers exit 1) NOT exercised here; out of test scope due to the cost of intentionally polluting the working tree from a parallel test. DEV verifies manually + documents the verification in the PR description.
- (EC-6) `shellcheck` (acceptance criterion 6) is NOT exercised by Rust test — that's a CI lint step. Document in DEV's PR that `shellcheck --severity=warning scripts/stress_curve.sh` was run locally.

---

## Acceptance criteria mapping

| TASK-0704 AC | Test coverage |
|---|---|
| AC-1 (smoke exits 0 within 20 min) | IT-0704-01 |
| AC-2 (output structure: MANIFEST + raw CSV + aggregated CSV + figures + checksums) | IT-0704-01 |
| AC-3 (`--resume` invariant) | TEST-SPEC-0707 IT-0707-05 (separate task) |
| AC-4 (pre-condition gate fires on dirty tree) | manual verification by DEV; out of automated scope |
| AC-5 (test passes on Linux, skipped on Windows-without-WSL) | `#[cfg(unix)]` file gate |
| AC-6 (`shellcheck` clean) | manual / CI lint step; not Rust-test |

## Out of scope

- Property tests.
- `--resume` (covered by TEST-SPEC-0707).
- Pre-condition gate manual fault injection.
- shellcheck integration.
- Phase 2 Docker leg — needs Docker running, out of fast-test scope.
- Cross-platform PowerShell port — explicit non-goal per TASK-0704.

## Open questions for DEV

1. **Release-binary dependency**: should the test invoke `cargo build --release` itself (slow first run; correct behavior) or skip-if-missing (fast iteration; risk of false-green)? Recommendation: skip-if-missing, document the requirement in TASK-0706 docs; CI workflows can `cargo build --release` before invoking `cargo test`.
2. **Python dependency**: should the test pre-check `which python3` and skip-if-missing? Recommendation: yes — keeps the failure mode clean and matches TASK-0705's plot-script availability assumption.
3. **bash 4+ requirement**: TASK-0704 implementation hint #2 mandates bash 4+. Modern Linux shipped images comply; if older bash is present the script panics early. Test does not pre-check; the failure surfaces through `status.success()`.

## Cross-references

- TASK-0704 (this TEST-SPEC's contract).
- TEST-SPEC-0707 IT-0707-05 (`--resume` invariant).
- `scripts/bench_docker_v2.sh` (reference template for Docker compose plumbing).
