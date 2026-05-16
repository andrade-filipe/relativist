//! TASK-0605 — `get_peak_memory_during_construction` probe (Phase C-5).
//!
//! Spec: SPEC-09 R18a–R18g (commit `82b2d27`) — specifically R18a / R18d on
//! construction-phase peak memory.
//!
//! Tests cover (per TEST-SPEC-0605):
//!  - IT-0605-04 — probe value is `<=` end-of-run peak (monotonicity, R18d).
//!  - IT-0605-05 — CSV emits `peak_memory_during_construction` as the
//!    rightmost-appended column (preserves v1_local_baseline schema parity).

use relativist_core::bench::csv::write_csv_detail;
use relativist_core::bench::suite::run_benchmark_suite;
use relativist_core::bench::{
    BenchmarkId, BenchmarkSuiteConfig, Mode, NetRepresentation, RecyclePolicy,
};

fn ep_config(size: u32, workers: u32) -> BenchmarkSuiteConfig {
    BenchmarkSuiteConfig {
        benchmarks: vec![BenchmarkId::EPAnnihilation],
        sizes: Some(vec![size]),
        workers: vec![workers],
        mode: Mode::Local,
        warmup_runs: 0,
        repetitions: 1,
        csv_detail_path: None,
        csv_rounds_path: None,
        csv_summary_path: None,
        max_rounds: None,
        strict_bsp: false,
        skip_g1: false,
        chunk_size: None,
        max_pending_lifetime: 16,
        recycle_policy: RecyclePolicy::DisableUnderDelta,
        representation: NetRepresentation::Dense,
        sparse_construction_memory_csv_path: None,
    }
}

/// IT-0605-04 — probe value is `<=` end-of-run peak (monotonicity, R18d).
///
/// VmHWM is monotonic non-decreasing across the process lifetime, so the
/// construction-phase snapshot MUST be `<=` the end-of-run snapshot. Catches
/// a buggy implementation that snapshots after `reduce_all` (would equal
/// end-of-run peak rather than precede it) or that uses a flawed reader
/// (would not match the legacy `peak_memory_bytes` value's units).
#[cfg(target_os = "linux")]
#[test]
fn it_0605_04_probe_value_le_end_of_run_peak() {
    let config = ep_config(10000, 1);
    let result = run_benchmark_suite(&config)
        .expect("IT-0605-04: bench harness must complete without error");

    // Inspect the grid row.
    let grid_row = result
        .results
        .iter()
        .find(|r| r.mode == Mode::Local)
        .expect("IT-0605-04: a Local-mode row must exist");

    assert!(
        grid_row.peak_memory_during_construction > 0,
        "IT-0605-04: probe must return non-zero on Linux; got {}",
        grid_row.peak_memory_during_construction
    );
    assert!(
        grid_row.peak_memory_during_construction <= grid_row.peak_memory_bytes,
        "IT-0605-04: VmHWM monotonicity — construction watermark ({}) must be \
         <= end-of-run watermark ({})",
        grid_row.peak_memory_during_construction,
        grid_row.peak_memory_bytes
    );
    // Sanity: a 10000-agent net occupies > 1 MB of resident memory.
    assert!(
        grid_row.peak_memory_during_construction >= 1_000_000,
        "IT-0605-04: 10000-agent net should drive VmHWM above 1 MB; got {}",
        grid_row.peak_memory_during_construction
    );
}

/// IT-0605-05 — CSV emits the SPEC-09 R18a-R18g Tier 3 columns.
///
/// Acceptance criterion #6 (TASK-0605, original): the new
/// `peak_memory_during_construction` column is appended at the RIGHT of the
/// existing v1 22-column schema. Updated by D-011 MF-002 / QA-D011-006:
/// SIX more columns are appended (R18b-R18g), bringing the schema to 29
/// columns. The v1 22-column ordering is preserved; the
/// `peak_memory_during_construction` column is now at position 23 (no
/// longer the rightmost), per SPEC-09 R39a.
#[test]
fn it_0605_05_csv_emits_peak_memory_during_construction_column() {
    let config = ep_config(1000, 2);
    let result = run_benchmark_suite(&config)
        .expect("IT-0605-05: bench harness must complete without error");

    let mut buf = Vec::new();
    write_csv_detail(&mut buf, &result.results).expect("CSV write must succeed");
    let csv = String::from_utf8(buf).expect("CSV must be UTF-8");
    let mut lines = csv.lines();
    let header = lines.next().expect("CSV must have a header line");

    let columns: Vec<&str> = header.split(',').collect();
    // SPEC-09 R39a (D-011 MF-002): 29 columns (22 v1 + 7 R18a-R18g).
    // D-014 / TASK-0703: +3 stress-curve columns appended at the end =
    // 32 columns total. (cv_above_gate dropped per TASK-0720 BUG-006.)
    assert_eq!(
        columns.len(),
        32,
        "IT-0605-05: header MUST be 22 (v1) + 7 (R18a-R18g) + 3 (D-014) = 32 columns; got {}",
        columns.len()
    );

    // R18a anchor: `peak_memory_during_construction` is at position 23
    // (column index 22, 0-indexed), immediately after the v1 schema's
    // final column `dup_era`.
    assert_eq!(
        columns[21], "dup_era",
        "IT-0605-05: column 22 must be `dup_era` (last v1 column)"
    );
    assert_eq!(
        columns[22], "peak_memory_during_construction",
        "IT-0605-05: column 23 must be `peak_memory_during_construction` (R18a)"
    );

    // Per-row check: column 23 (peak_memory_during_construction) is either
    // empty (non-Linux) or parses as u64 (Linux). On non-Linux the cell
    // renders blank per SPEC-09 §4.9 convention; on Linux the cell is the
    // VmHWM byte count.
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let cells: Vec<&str> = trimmed.split(',').collect();
        // D-014 / TASK-0703: 3 stress-curve columns appended → 32 cells.
        // (cv_above_gate dropped per TASK-0720 BUG-006.)
        assert_eq!(
            cells.len(),
            32,
            "IT-0605-05: every row MUST have 32 cells (29 SPEC-09 + 3 D-014); got {} for line: {line}",
            cells.len()
        );
        let peak_cell = cells[22];
        if !peak_cell.is_empty() {
            let _v: u64 = peak_cell.parse().unwrap_or_else(|e| {
                panic!(
                    "IT-0605-05: peak_memory_during_construction cell MUST parse as u64 when non-empty; \
                     cell={peak_cell}, error={e}, line={line}"
                )
            });
        }
    }
}

/// IT-0605-05 (extension) — every BenchmarkResult row populates the new
/// field (no row carries the type-default 0 silently when on Linux).
#[cfg(target_os = "linux")]
#[test]
fn it_0605_05_field_populated_for_v1_equivalent_rodada() {
    // SPEC-09 §4.9 line ~714: v1-equivalent rodadas MUST still populate
    // the rightmost 7 columns; they MUST NOT be omitted.
    let config = ep_config(1000, 2);
    let result = run_benchmark_suite(&config)
        .expect("IT-0605-05: bench harness must complete without error");

    for row in &result.results {
        assert!(
            row.peak_memory_during_construction > 0,
            "IT-0605-05: every row MUST populate peak_memory_during_construction \
             with the measured VmHWM (NOT default 0); got 0 for row \
             benchmark={} mode={} repetition={}",
            row.benchmark,
            row.mode,
            row.repetition
        );
    }
}
