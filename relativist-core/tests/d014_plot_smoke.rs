//! IT-0705-01 — `plot_script_produces_pdfs_from_synthetic_csv` (TASK-0705).
//!
//! Synthesizes a small fake `aggregated.csv` (single workload, 2 worker
//! counts, 2 N values, 3 reps each) and invokes
//! `python3 scripts/plot_stress_curve.py`. Verifies ≥ 4 PDFs are produced
//! (3 metric × 1 workload + 1 summary). Each PDF is sanity-checked by the
//! `%PDF-` magic header.
//!
//! Self-skips when:
//! - `python3` not in PATH
//! - matplotlib + pandas + numpy not all installed
//! - the plot script not present at the workspace root.

use std::path::PathBuf;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn plot_script_produces_pdfs_from_synthetic_csv() {
    if Command::new("python3")
        .arg("--version")
        .status()
        .map(|s| !s.success())
        .unwrap_or(true)
    {
        eprintln!("SKIP IT-0705-01: python3 not in PATH");
        return;
    }
    let pkg_check = Command::new("python3")
        .arg("-c")
        .arg("import matplotlib, pandas, numpy")
        .status();
    if pkg_check.map(|s| !s.success()).unwrap_or(true) {
        eprintln!("SKIP IT-0705-01: matplotlib + pandas + numpy not all installed");
        return;
    }
    let script = workspace_root().join("scripts/plot_stress_curve.py");
    if !script.exists() {
        eprintln!(
            "SKIP IT-0705-01: scripts/plot_stress_curve.py not found at {:?}",
            script
        );
        return;
    }

    let tmp = tempdir().expect("tempdir");
    let csv_path = tmp.path().join("aggregated.csv");
    let out_dir = tmp.path().join("figures");
    std::fs::create_dir_all(&out_dir).expect("mkdir figures");

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

    let status = Command::new("python3")
        .arg(&script)
        .arg("--input")
        .arg(&csv_path)
        .arg("--output-dir")
        .arg(&out_dir)
        .status()
        .expect("python3 spawn must succeed");
    assert!(
        status.success(),
        "plot script must exit 0 on a valid synthetic CSV; got {:?}",
        status
    );

    let pdfs: Vec<_> = std::fs::read_dir(&out_dir)
        .expect("read out_dir")
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("pdf"))
        })
        .collect();
    assert!(
        pdfs.len() >= 4,
        "expected >= 4 PDFs (3 metrics x 1 workload + 1 summary); got {}: {:?}",
        pdfs.len(),
        pdfs.iter().map(|e| e.file_name()).collect::<Vec<_>>()
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
            "{:?} is suspiciously small ({} bytes)",
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
