# TEST-SPEC-0705: Tests for TASK-0705 — `scripts/plot_stress_curve.py`

**Task:** TASK-0705
**Spec:** none
**Bundle:** D-014 (Stress Curve Campaign)
**Requirements covered:** Acceptance criteria 1-6 from TASK-0705
**Test IDs:** IT-0705-01 (single integration test)

---

## Scope

A single Rust integration test that synthesizes a small fake `aggregated.csv`, invokes `python3 scripts/plot_stress_curve.py --input <csv> --output-dir <tmp>`, and verifies that ≥ 4 PDF files are produced (3 metric figures for the synthetic single-workload data + 1 summary).

The test SKIPS gracefully on systems without `python3` (≥ 3.10) in PATH or without the required Python packages installed (matplotlib, pandas, numpy). Skipping is preferred over failing — the campaign workflow assumes the user has Python set up before running the orchestrator (TASK-0704), and CI may not.

The +1 test floor delta is for **a single integration test file with a single `#[test]` function**.

## Test category & location

| # | Name | Category | File | Cfg gating |
|---|------|----------|------|-----------|
| IT-0705-01 | `plot_script_produces_pdfs_from_synthetic_csv` | integration | `relativist-core/tests/d014_plot_smoke.rs` | none (cross-platform, but skips if python3 missing) |

LoC budget ~30 LoC matching TASK-0705.

## Test floor delta

- default: **+1** → ≥ 1813
- zero-copy: **+1** → ≥ 1857
- streaming-no-recycle: **+1** → ≥ 1804
- release: **+1** → ≥ 1755

---

## Integration Tests

### IT-0705-01: `plot_script_produces_pdfs_from_synthetic_csv`

**Purpose:** Acceptance criteria 1, 5, 6 — script produces PDFs and they pass `pdfinfo` (or at minimum exist with non-zero size and the `%PDF-` magic header).

**Preconditions checked at runtime (skip with reason; do NOT panic):**
1. `which python3` returns success.
2. `python3 -c "import matplotlib, pandas, numpy"` exit 0 (the test runs this as a sub-process; if it fails, skip).
3. `scripts/plot_stress_curve.py` exists at the workspace root.

**Test body (sketch):**

```rust
use std::process::Command;
use std::path::PathBuf;
use tempfile::tempdir;

#[test]
fn plot_script_produces_pdfs_from_synthetic_csv() {
    // 1. python3 availability gate.
    if Command::new("python3").arg("--version").status().map(|s| !s.success()).unwrap_or(true) {
        eprintln!("SKIP: python3 not in PATH");
        return;
    }
    // 2. Required Python packages.
    let pkg_check = Command::new("python3")
        .arg("-c")
        .arg("import matplotlib, pandas, numpy")
        .status();
    if pkg_check.map(|s| !s.success()).unwrap_or(true) {
        eprintln!("SKIP: matplotlib + pandas + numpy not all installed");
        return;
    }

    // 3. Locate the plot script.
    let script = workspace_root().join("scripts/plot_stress_curve.py");
    if !script.exists() {
        eprintln!("SKIP: scripts/plot_stress_curve.py not found");
        return;
    }

    // 4. Synthesize a tiny aggregated.csv with the post-D-014 schema.
    let tmp = tempdir().expect("tempdir");
    let csv_path = tmp.path().join("aggregated.csv");
    let out_dir  = tmp.path().join("figures");
    std::fs::create_dir_all(&out_dir).unwrap();

    // Header + 12 rows: 1 workload (ep_annihilation) × 1 env (in_process)
    // × 2 worker counts (1, 2) × 2 N values (1000, 10000) × 3 reps each.
    // Provide ALL columns the plot script reads by name:
    //   workload, env, workers, n, rep, wall_seconds, mips,
    //   vmrss_peak_mb, vmrss_current_end_mb, stop_reason, cv_above_gate
    // Plus enough placeholder columns to satisfy the CSV schema if the
    // script schema-validates beyond the names it reads (it doesn't, per
    // TASK-0705 acceptance criterion 4 — only `vmrss_peak_mb` is required).
    let csv_text = "\
workload,env,workers,n,rep,wall_seconds,mips,vmrss_peak_mb,vmrss_current_end_mb,stop_reason,cv_above_gate
ep_annihilation,in_process,1,1000,1,0.10,9.5,12.3,10.5,,false
ep_annihilation,in_process,1,1000,2,0.11,9.0,12.4,10.6,,false
ep_annihilation,in_process,1,1000,3,0.10,9.5,12.5,10.5,,false
ep_annihilation,in_process,1,10000,1,0.50,18.0,45.6,40.2,,false
ep_annihilation,in_process,1,10000,2,0.52,17.5,45.7,40.3,,false
ep_annihilation,in_process,1,10000,3,0.51,17.7,45.8,40.4,,false
ep_annihilation,in_process,2,1000,1,0.08,11.5,15.0,12.8,,false
ep_annihilation,in_process,2,1000,2,0.08,11.6,15.1,12.9,,false
ep_annihilation,in_process,2,1000,3,0.08,11.5,15.2,13.0,,false
ep_annihilation,in_process,2,10000,1,0.30,28.0,52.0,45.0,,false
ep_annihilation,in_process,2,10000,2,0.31,27.7,52.1,45.1,,false
ep_annihilation,in_process,2,10000,3,0.30,28.0,52.2,45.2,,false
";
    std::fs::write(&csv_path, csv_text).expect("write synthetic csv");

    // 5. Invoke the script.
    let status = Command::new("python3")
        .arg(&script)
        .arg("--input").arg(&csv_path)
        .arg("--output-dir").arg(&out_dir)
        .status()
        .expect("python3 spawn must succeed");
    assert!(status.success(),
        "plot script must exit 0 on a valid synthetic CSV; got {:?}", status);

    // 6. Verify ≥ 4 PDFs were produced (3 metrics for ep_annihilation + summary).
    let pdfs: Vec<_> = std::fs::read_dir(&out_dir).unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |x| x == "pdf"))
        .collect();
    assert!(pdfs.len() >= 4,
        "expected ≥ 4 PDFs (3 metrics × 1 workload + 1 summary); got {} files: {:?}",
        pdfs.len(),
        pdfs.iter().map(|e| e.file_name()).collect::<Vec<_>>());

    // 7. Each PDF has the magic %PDF- header (lightweight pdfinfo replacement).
    for entry in &pdfs {
        let data = std::fs::read(entry.path()).expect("read pdf");
        assert!(data.starts_with(b"%PDF-"),
            "{:?} does not start with %PDF- magic", entry.file_name());
        assert!(data.len() > 100,
            "{:?} is suspiciously small ({} bytes)", entry.file_name(), data.len());
    }
}

fn workspace_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    while !p.join("Cargo.lock").exists() && p.pop() {}
    p
}
```

**Expected output:** All assertions pass; runs in well under 60 seconds.

**Edge cases:**
- (EC-1) Empty CSV (header only) — acceptance criterion 3 says exit 2. NOT tested directly here (would require a second test or sub-test; deferred — DEV may add as a second `#[test]` if budget allows but TASK-0705 says +1 only).
- (EC-2) Missing required column — acceptance criterion 4 says exit 1. Same deferral as EC-1.
- (EC-3) `pdfinfo` is not actually invoked here; the lighter `%PDF-` magic + ≥ 100 bytes check is sufficient for the smoke. If DEV wants the full `pdfinfo` validation, add a `which pdfinfo` skip-gate, but that adds another tool dependency. Recommendation: keep the magic+size check.
- (EC-4) Synthetic CSV uses 3 reps per (W, N) — enough for non-degenerate stddev / errorbar computation in the plot script.
- (EC-5) The script may produce extra PDFs (e.g., if multiple workers are detected, more figures); the assertion is `≥ 4`, not `== 4`, to allow for that.

---

## Acceptance criteria mapping

| TASK-0705 AC | Test coverage |
|---|---|
| AC-1 (full matrix → 10 PDFs) | NOT directly tested (cost-prohibitive in a smoke). Implicitly covered by `≥ 4` on the synthetic single-workload data. |
| AC-2 (partial CSV → only that workload's PDFs) | covered by IT-0705-01 (synthetic CSV is single-workload) |
| AC-3 (empty CSV → exit 2) | DEFERRED to DEV manual check |
| AC-4 (missing column → exit 1) | DEFERRED to DEV manual check |
| AC-5 (pdfinfo clean) | LIGHTWEIGHT replacement (magic + size) in IT-0705-01 |
| AC-6 (test skips if python3 absent) | IT-0705-01 preconditions |

## Edge Cases Catalog

| # | Scenario | Expected | Test |
|---|----------|----------|------|
| EC-0705-01 | python3 missing | SKIP with message | IT-0705-01 precondition |
| EC-0705-02 | matplotlib/pandas/numpy missing | SKIP with message | IT-0705-01 precondition |
| EC-0705-03 | Synthetic single-workload CSV | ≥ 4 PDFs produced | IT-0705-01 |
| EC-0705-04 | PDF magic header absent | assert fails | IT-0705-01 (7) |
| EC-0705-05 | Empty CSV | script exits 2 | DEFERRED |
| EC-0705-06 | Missing column | script exits 1 | DEFERRED |

## Out of scope

- Property tests.
- Visual regression on PDF contents (would require image diff tooling).
- IEEE-style validation (column width, font, error bars) — visual review by REDATOR before TCC submission.
- Full 10-PDF matrix.

## Open questions for DEV

1. Should empty-CSV / missing-column be Rust-tested or left as DEV manual check? Recommendation: leave as manual + document in PR description, to keep the +1 test floor delta intact.
2. Is `pdfinfo` available in the standard Linux/Windows dev images? On Linux it usually is (`poppler-utils`); on Windows + WSL it is. If DEV wants the full validation, add a skip-gate.
