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

/// IT-0605-05 — CSV emits `peak_memory_during_construction` as a tail column.
///
/// Acceptance criterion #6: the new column is appended at the RIGHT of the
/// existing v1 22-column schema, NOT inserted in the middle. Defends
/// `v1_local_baseline` cross-comparison (existing pandas joins on the v1
/// column order MUST continue to work).
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

    // Column-position assertions: header ends with the new column name.
    assert!(
        header.ends_with("peak_memory_during_construction"),
        "IT-0605-05: header MUST end with the new column name (rightmost-appended); \
         got header: {header}"
    );
    let v1_tail = "dup_era,peak_memory_during_construction";
    assert!(
        header.ends_with(v1_tail),
        "IT-0605-05: new column MUST be appended immediately after the v1 schema's \
         final column (`dup_era`); got header: {header}"
    );

    // Column-count assertions: exactly +1 over the v1 22-column schema = 23 columns.
    let column_count = header.split(',').count();
    assert_eq!(
        column_count, 23,
        "IT-0605-05: header MUST be 22 (v1) + 1 (TASK-0605) = 23 columns; got {column_count}"
    );

    // Per-row check: the value parses as `u64`.
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let last = trimmed
            .rsplit(',')
            .next()
            .expect("row must have at least one column");
        let _v: u64 = last.parse().unwrap_or_else(|e| {
            panic!(
                "IT-0605-05: rightmost column value must parse as u64; \
                    last={last}, error={e}, line={line}"
            )
        });
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
