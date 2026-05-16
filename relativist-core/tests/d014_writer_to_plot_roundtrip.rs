//! TASK-0720 D-014 Stage 6 REFACTOR — writer-to-plot end-to-end roundtrip.
//!
//! Covers AC-1 + AC-2 of the bundle:
//!   - AC-1 (BUG-001): `cargo run --release -- bench --campaign stress-curve`
//!     produces a valid CSV with the canonical writer schema.
//!   - AC-2 (BUG-002): the plot script consumes that CSV without errors and
//!     produces ≥ 1 PDF.
//!
//! Self-skips when:
//!   - `target/release/relativist[.exe]` is not built (CI may skip);
//!   - python3 + matplotlib + pandas + numpy are not all installed;
//!   - the plot script is not present at the workspace root.
//!
//! Scope contract: this test does NOT exercise the bash orchestrator
//! (`scripts/stress_curve.sh`); the orchestrator's smoke is covered by
//! `d014_stress_curve_smoke.rs`. This test pins the Rust binary's CSV
//! output ↔ plot script's REQUIRED_COLUMNS contract that BUG-001 + BUG-002
//! broke.

use std::path::PathBuf;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn writer_to_plot_roundtrip_produces_pdf() {
    // 1. Locate the release binary; skip if not built.
    let bin_unix = workspace_root().join("target/release/relativist");
    let bin_windows = workspace_root().join("target/release/relativist.exe");
    let bin = if bin_unix.exists() {
        bin_unix
    } else if bin_windows.exists() {
        bin_windows
    } else {
        eprintln!(
            "SKIP IT-0720-01 (writer→plot roundtrip): target/release/relativist[.exe] \
             not built; run `cargo build --release` before this test"
        );
        return;
    };

    // 2. Locate the plot script; skip if missing.
    let script = workspace_root().join("scripts/plot_stress_curve.py");
    if !script.exists() {
        eprintln!(
            "SKIP IT-0720-01: scripts/plot_stress_curve.py not found at {:?}",
            script
        );
        return;
    }

    // 3. Verify the plotter stack — skip if absent.
    if Command::new("python3")
        .arg("--version")
        .status()
        .map(|s| !s.success())
        .unwrap_or(true)
    {
        eprintln!("SKIP IT-0720-01: python3 not in PATH");
        return;
    }
    let pkg_check = Command::new("python3")
        .arg("-c")
        .arg("import matplotlib, pandas, numpy")
        .status();
    if pkg_check.map(|s| !s.success()).unwrap_or(true) {
        eprintln!(
            "SKIP IT-0720-01: matplotlib + pandas + numpy not all installed; \
             see scripts/requirements-stress-curve.txt"
        );
        return;
    }

    // 4. Invoke the binary; capture stdout into a CSV file.
    let tmp = tempdir().expect("tempdir");
    let csv_path = tmp.path().join("in_process.csv");
    let output = Command::new(&bin)
        .args([
            "bench",
            "--campaign",
            "stress-curve",
            "--workload",
            "ep_annihilation",
            "--env",
            "in-process",
            "--workers",
            "1",
            "--n-seq",
            "1000",
            "--reps",
            "1",
        ])
        .output()
        .expect("relativist binary spawn must succeed");
    assert!(
        output.status.success(),
        "relativist binary must exit 0; stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::write(&csv_path, &output.stdout).expect("write CSV file");

    // 5. Sanity: header line is the writer's canonical schema.
    let csv_text = String::from_utf8_lossy(&output.stdout);
    let header_line = csv_text.lines().next().expect("CSV must have a header");
    assert!(
        header_line.starts_with("benchmark,input_size,mode,workers,repetition"),
        "CSV header must start with the canonical writer schema; got: {header_line}"
    );
    assert!(
        header_line.contains("vmrss_peak_mb")
            && header_line.contains("vmrss_current_end_mb")
            && header_line.contains("stop_reason"),
        "CSV header must include the D-014 stress-curve columns; got: {header_line}"
    );
    // BUG-006 — `cv_above_gate` MUST NOT be in the schema any more.
    assert!(
        !header_line.contains("cv_above_gate"),
        "TASK-0720 BUG-006: cv_above_gate MUST be absent from the schema"
    );
    let row_count = csv_text.lines().count() - 1; // minus header
    assert!(
        row_count >= 1,
        "CSV must contain at least 1 data row; got {row_count}"
    );

    // 6. Run the plot script against the CSV.
    let figures_dir = tmp.path().join("figures");
    std::fs::create_dir_all(&figures_dir).expect("mkdir figures");
    let status = Command::new("python3")
        .arg(&script)
        .arg("--input")
        .arg(&csv_path)
        .arg("--output-dir")
        .arg(&figures_dir)
        .status()
        .expect("python3 spawn must succeed");
    assert!(
        status.success(),
        "plot script must exit 0 on the writer's canonical schema; got {status:?}"
    );

    // 7. Assert at least one real PDF was produced.
    let pdfs: Vec<_> = std::fs::read_dir(&figures_dir)
        .expect("read figures/")
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("pdf"))
        })
        .collect();
    assert!(
        !pdfs.is_empty(),
        "plot script must produce at least 1 PDF on a valid CSV"
    );
    for entry in &pdfs {
        let data = std::fs::read(entry.path()).expect("read pdf");
        assert!(
            data.starts_with(b"%PDF-"),
            "{:?} does not start with %PDF- magic",
            entry.file_name()
        );
        assert!(
            data.len() > 100,
            "{:?} is suspiciously small ({} bytes) — placeholder leak?",
            entry.file_name(),
            data.len()
        );
    }
}

fn workspace_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    while !p.join("Cargo.lock").exists() && p.pop() {}
    p
}
