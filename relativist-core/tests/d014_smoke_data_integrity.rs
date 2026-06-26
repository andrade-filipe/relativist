//! TASK-0722 D-014 follow-up #2 — stress-curve smoke data integrity.
//!
//! Covers AC-3 of TASK-0722:
//!   - Spawns `relativist bench --campaign stress-curve --workload
//!     ep_annihilation --env in-process --workers 2 --n-seq 1000`,
//!     captures stdout into a temp CSV, parses it with the `csv` crate,
//!     and asserts the four data-integrity invariants:
//!       * header row is on line 1 (no banner contamination — BUG-A);
//!       * at least one data row;
//!       * `total_interactions > 0`, `mips > 0`,
//!         `agent_count_at_construction_complete > 0` for the
//!         `ep_annihilation N=1000 W=2` row (BUG-B);
//!       * `correct == true` for every emitted row.
//!
//! Self-skips when `target/release/relativist[.exe]` is not built —
//! mirrors `tests/d014_writer_to_plot_roundtrip.rs`.
//!
//! Scope contract: this test does NOT exercise the bash orchestrator;
//! `d014_writer_to_plot_roundtrip.rs` covers the writer-to-plot schema
//! match. This test pins the CSV's per-row data integrity.

use std::path::PathBuf;
use std::process::Command;
use tempfile::tempdir;

#[test]
#[ignore = "D-014 stress-curve smoke: emits real benchmark data; runner-speed + env dependent. Run manually: cargo test -- --ignored"]
fn stress_curve_smoke_emits_real_benchmark_data() {
    // 1. Locate the release binary; skip if not built.
    let bin_unix = workspace_root().join("target/release/relativist");
    let bin_windows = workspace_root().join("target/release/relativist.exe");
    let bin = if bin_unix.exists() {
        bin_unix
    } else if bin_windows.exists() {
        bin_windows
    } else {
        eprintln!(
            "SKIP IT-0722-01 (stress-curve smoke data integrity): \
             target/release/relativist[.exe] not built; run \
             `cargo build --release` before this test"
        );
        return;
    };

    // 2. Invoke the binary; capture stdout into a CSV file.
    let tmp = tempdir().expect("tempdir");
    let csv_path = tmp.path().join("smoke.csv");
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
            "2",
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

    // 3. BUG-A — header line MUST be on line 1 (no banner above it).
    let csv_text = String::from_utf8_lossy(&output.stdout);
    let header_line = csv_text.lines().next().expect("CSV must have a header");
    assert!(
        header_line.starts_with("benchmark,input_size,mode,workers,repetition"),
        "BUG-A: line 1 must be the canonical CSV header, not a banner; got: {header_line}"
    );

    // 4. BUG-B — parse with the csv crate and assert per-column
    // invariants on the ep_annihilation N=1000 W=2 row.
    let mut rdr = csv::Reader::from_path(&csv_path).expect("csv reader");
    let headers = rdr.headers().expect("csv headers").clone();
    let col = |name: &str| {
        headers
            .iter()
            .position(|h| h == name)
            .unwrap_or_else(|| panic!("missing column '{name}' in header: {headers:?}"))
    };
    let idx_benchmark = col("benchmark");
    let idx_input_size = col("input_size");
    let idx_workers = col("workers");
    let idx_correct = col("correct");
    let idx_total_interactions = col("total_interactions");
    let idx_mips = col("mips");
    let idx_agents_construction = col("agent_count_at_construction_complete");

    let mut data_rows = 0usize;
    let mut found_target_row = false;
    for record in rdr.records() {
        let record = record.expect("csv record");
        data_rows += 1;

        // Every emitted row must report `correct == true` for a smoke
        // run — N=1000 is well within both wall and memory budgets.
        let correct = &record[idx_correct];
        assert_eq!(
            correct, "true",
            "every smoke row must be `correct=true`; got `{correct}` in row {record:?}"
        );

        let benchmark = &record[idx_benchmark];
        let input_size: u64 = record[idx_input_size]
            .parse()
            .expect("input_size must parse as u64");
        let workers: u32 = record[idx_workers]
            .parse()
            .expect("workers must parse as u32");

        if benchmark == "ep_annihilation" && input_size == 1000 && workers == 2 {
            found_target_row = true;
            let total_interactions: u64 = record[idx_total_interactions]
                .parse()
                .expect("total_interactions must parse as u64");
            let mips: f64 = record[idx_mips].parse().expect("mips must parse as f64");
            let agents_construction: u64 = record[idx_agents_construction]
                .parse()
                .expect("agent_count_at_construction_complete must parse as u64");

            assert!(
                total_interactions > 0,
                "BUG-B: total_interactions must be > 0 for ep_annihilation N=1000 W=2; \
                 got {total_interactions}"
            );
            assert!(
                mips > 0.0,
                "BUG-B: mips must be > 0 for ep_annihilation N=1000 W=2; got {mips}"
            );
            assert!(
                agents_construction > 0,
                "BUG-B: agent_count_at_construction_complete must be > 0 for \
                 ep_annihilation N=1000 W=2 (expect ~2000 agents from 1000 ERA-ERA pairs); \
                 got {agents_construction}"
            );
        }
    }

    assert!(
        data_rows >= 1,
        "CSV must contain at least 1 data row; got {data_rows}"
    );
    assert!(
        found_target_row,
        "CSV must contain a row for benchmark=ep_annihilation, input_size=1000, \
         workers=2 — none found among {data_rows} rows"
    );
}

fn workspace_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    while !p.join("Cargo.lock").exists() && p.pop() {}
    p
}
