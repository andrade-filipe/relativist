//! IT-0704-01 — `script_smoke_runs_and_produces_artifacts` (TASK-0704).
//!
//! Drives `scripts/stress_curve.sh --smoke --no-docker --output-dir <tmp>`
//! from `Command::new("bash")` and verifies the output structure
//! (MANIFEST.md, raw/in_process.csv, aggregated.csv, figures/*.pdf,
//! checksums.sha256). `cfg(unix)` only — Windows hosts can run the
//! script through WSL manually.

#![cfg(unix)]

use std::path::PathBuf;
use std::process::Command;
use tempfile::tempdir;

#[test]
#[ignore = "D-014 stress-curve smoke: runs the bash/python stress-curve script; runner-speed + env dependent. Run manually: cargo test -- --ignored"]
fn script_smoke_runs_and_produces_artifacts() {
    // 1. Locate the script.
    let script = workspace_root().join("reproduce_article/scripts/stress_curve.sh");
    if !script.exists() {
        eprintln!(
            "SKIP IT-0704-01: scripts/stress_curve.sh not found at {:?}",
            script
        );
        return;
    }

    // 2. Locate the release binary; skip if missing (CI may not have built).
    let bin_unix = workspace_root().join("target/release/relativist");
    let bin_windows = workspace_root().join("target/release/relativist.exe");
    if !bin_unix.exists() && !bin_windows.exists() {
        eprintln!(
            "SKIP IT-0704-01: target/release/relativist not built; \
             run `cargo build --release` before this test"
        );
        return;
    }

    // 3. Free-RAM gate — skip on memory-starved CI.
    if let Ok(probe) = relativist_core::bench::memory_probe::MemoryProbe::new() {
        let cur = probe.current_bytes().unwrap_or(0);
        let frac = probe.as_fraction_of_total(cur);
        if frac > 0.85 {
            eprintln!(
                "SKIP IT-0704-01: host RAM > 85% used (frac={}); smoke would be flaky",
                frac
            );
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
    assert!(
        status.success(),
        "smoke script must exit 0; got {:?}",
        status
    );

    // 5. Verify output structure.
    let manifest = outdir_path.join("MANIFEST.md");
    assert!(
        manifest.exists(),
        "MANIFEST.md must exist at {:?}",
        manifest
    );
    let manifest_text = std::fs::read_to_string(&manifest).expect("read MANIFEST");
    assert!(
        manifest_text.contains("git rev"),
        "MANIFEST.md must include git rev section; got:\n{}",
        manifest_text
    );
    assert!(
        manifest_text.contains("rustc"),
        "MANIFEST.md must include rustc version; got:\n{}",
        manifest_text
    );

    let raw_csv = outdir_path.join("raw").join("in_process.csv");
    assert!(
        raw_csv.exists(),
        "raw/in_process.csv must exist at {:?}",
        raw_csv
    );

    let aggregated = outdir_path.join("aggregated.csv");
    assert!(aggregated.exists(), "aggregated.csv must exist");

    let figures_dir = outdir_path.join("figures");
    assert!(figures_dir.is_dir(), "figures/ must be a directory");
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
        "figures/ must contain at least 1 PDF (smoke placeholder is acceptable when matplotlib is unavailable)"
    );

    let checksums = outdir_path.join("checksums.sha256");
    let checksums_meta = std::fs::metadata(&checksums).expect("checksums.sha256 must exist");
    assert!(
        checksums_meta.len() > 0,
        "checksums.sha256 must be non-empty"
    );
}

fn workspace_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    while !p.join("Cargo.lock").exists() && p.pop() {}
    p
}
