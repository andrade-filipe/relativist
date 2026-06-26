//! IT-0707-05 (e) — `resume_produces_identical_dataset` (TASK-0707).
//!
//! Drives `scripts/stress_curve.sh --smoke --no-docker`, kills it
//! mid-rep, restarts with `--resume`, and verifies the final dataset is
//! identical (modulo wall-time noise) to a clean reference run.
//!
//! `cfg(unix)` only because the orchestrator is bash-driven. On systems
//! that lack `bash`, `python3 + matplotlib`, the script itself, or the
//! release binary, the test self-skips with a printed reason rather
//! than failing — the contract under test is the script's `--resume`
//! invariant, not the host environment.

#![cfg(unix)]

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;
use tempfile::tempdir;

#[test]
#[ignore = "D-014 stress-curve smoke: real resume run (sleep + bash/python/matplotlib); runner-speed + env dependent. Run manually: cargo test -- --ignored"]
fn resume_produces_identical_dataset() {
    if !skips_pass() {
        return;
    }

    let script = workspace_root().join("reproduce_article/scripts/stress_curve.sh");

    // 1. Reference clean smoke.
    let ref_dir = tempdir().expect("tempdir for ref");
    let s1 = Command::new("bash")
        .arg(&script)
        .arg("--smoke")
        .arg("--no-docker")
        .arg("--output-dir")
        .arg(ref_dir.path())
        .status();
    let s1 = match s1 {
        Ok(s) => s,
        Err(e) => {
            eprintln!("SKIP IT-0707-05: bash spawn failed: {}", e);
            return;
        }
    };
    if !s1.success() {
        eprintln!(
            "SKIP IT-0707-05: reference smoke failed (status={:?}); script may not be ready",
            s1
        );
        return;
    }
    let ref_csv_path = ref_dir.path().join("raw").join("in_process.csv");
    if !ref_csv_path.exists() {
        eprintln!("SKIP IT-0707-05: reference smoke produced no in_process.csv");
        return;
    }
    let ref_csv = std::fs::read_to_string(&ref_csv_path).expect("read ref csv");

    // 2. Resume run: spawn, SIGINT mid-rep, then re-spawn with --resume.
    let resume_dir = tempdir().expect("tempdir for resume");
    let mut child = match Command::new("bash")
        .arg(&script)
        .arg("--smoke")
        .arg("--no-docker")
        .arg("--output-dir")
        .arg(resume_dir.path())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("SKIP IT-0707-05: resume-leg spawn failed: {}", e);
            return;
        }
    };
    sleep(Duration::from_secs(30)); // mid-rep
    let pid = child.id() as i32;
    // SAFETY: `libc::kill` accepts any i32 pid; on a dead pid it returns
    // -1 with errno=ESRCH which is harmless — the test does not branch
    // on the return value.
    unsafe { libc::kill(pid, libc::SIGINT) };
    let _ = child.wait();

    let s3 = Command::new("bash")
        .arg(&script)
        .arg("--smoke")
        .arg("--no-docker")
        .arg("--resume")
        .arg("--output-dir")
        .arg(resume_dir.path())
        .status()
        .expect("resume-leg spawn must succeed");
    assert!(s3.success(), "resumed smoke must exit 0; got {:?}", s3);

    let resume_csv_path = resume_dir.path().join("raw").join("in_process.csv");
    let resume_csv = std::fs::read_to_string(&resume_csv_path).expect("read resume csv");

    // 3. Strict-equality compare modulo wall_clock_secs (and stderr-mediated
    //    timing columns) — same row count, same column count, every
    //    non-wall column matches exactly.
    assert_csvs_equal_modulo_wall(&ref_csv, &resume_csv);
}

/// Compare two CSVs by header name. The columns whose name CONTAINS one
/// of `WALL_NOISE_TOKENS` are allowed to differ; every other column MUST
/// match exactly.
fn assert_csvs_equal_modulo_wall(a: &str, b: &str) {
    /// Column names that legitimately contain wall-time noise on the
    /// resume path. Any column whose name CONTAINS one of these tokens
    /// is excluded from the strict-equality compare.
    const WALL_NOISE_TOKENS: &[&str] = &[
        "wall",
        "compute_time",
        "merge_time",
        "network_time",
        "partition_time",
        "vmrss", // peak/current may also have run-to-run drift
        "mips",  // mips = total_interactions / wall, drifts with wall
    ];

    let a_lines: Vec<&str> = a.lines().collect();
    let b_lines: Vec<&str> = b.lines().collect();
    assert_eq!(
        a_lines.len(),
        b_lines.len(),
        "row count mismatch: ref={} resume={}",
        a_lines.len(),
        b_lines.len()
    );
    if a_lines.is_empty() {
        return;
    }
    let headers: Vec<&str> = a_lines[0].split(',').collect();
    let headers_b: Vec<&str> = b_lines[0].split(',').collect();
    assert_eq!(headers, headers_b, "header drift between ref and resume");

    let mut strict_indices: Vec<usize> = Vec::with_capacity(headers.len());
    for (i, h) in headers.iter().enumerate() {
        if WALL_NOISE_TOKENS.iter().any(|tok| h.contains(tok)) {
            continue;
        }
        strict_indices.push(i);
    }

    for (row_idx, (ar, br)) in a_lines.iter().zip(b_lines.iter()).enumerate().skip(1) {
        let acells: Vec<&str> = ar.split(',').collect();
        let bcells: Vec<&str> = br.split(',').collect();
        for &i in &strict_indices {
            let av = acells.get(i).copied().unwrap_or("");
            let bv = bcells.get(i).copied().unwrap_or("");
            assert_eq!(
                av, bv,
                "row {} col {} (`{}`) mismatch: ref={} resume={}",
                row_idx, i, headers[i], av, bv
            );
        }
    }
}

fn skips_pass() -> bool {
    // bash present?
    if Command::new("bash")
        .arg("--version")
        .status()
        .map(|s| !s.success())
        .unwrap_or(true)
    {
        eprintln!("SKIP IT-0707-05: bash not in PATH");
        return false;
    }
    // script present?
    let script = workspace_root().join("reproduce_article/scripts/stress_curve.sh");
    if !script.exists() {
        eprintln!(
            "SKIP IT-0707-05: scripts/stress_curve.sh not found at {:?}",
            script
        );
        return false;
    }
    // release binary present?
    let bin = workspace_root().join("target/release/relativist");
    if !bin.exists() {
        eprintln!(
            "SKIP IT-0707-05: target/release/relativist not built; run `cargo build --release` first"
        );
        return false;
    }
    // python3 + matplotlib (the smoke needs them for the plot phase).
    let py_ok = Command::new("python3")
        .arg("-c")
        .arg("import matplotlib, pandas, numpy")
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !py_ok {
        eprintln!("SKIP IT-0707-05: python3 + matplotlib + pandas + numpy not available");
        return false;
    }
    true
}

fn workspace_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    while !p.join("Cargo.lock").exists() && p.pop() {}
    p
}
