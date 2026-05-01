//! Integration tests for TASK-0607 — `sparse_construction_memory.csv`
//! sub-writer (D-011 Phase D-3, P1).
//!
//! These tests exercise the new sub-CSV writer through the full bench-suite
//! path:
//!
//! - **IT-0607-02** (Linux-gated): full sparse + dense pair at
//!   `dual_tree(depth=12)` produces 2 rows with a populated `ratio_to_dense`,
//!   and the ratio satisfies the relaxed 80%-gate (consistency with
//!   IT-0606-04).
//! - **IT-0607-03**: a sparse-only run via direct row construction emits a
//!   blank `ratio_to_dense` (NOT `NaN`, NOT `0`) per the spec-locked
//!   convention.
//! - **IT-0607-04**: setting `sparse_construction_memory_csv_path = Some(_)`
//!   does NOT contaminate the existing detail / rounds / summary CSV
//!   outputs (additive change).
//!
//! Spec authority:
//! - SPEC-09 R18a–R18g + §3.4.5 (committed `82b2d27`).
//!
//! See `docs/tests/TASK-0607-tests.md` for the full test specification.

use relativist_core::bench::csv::{
    write_csv_detail, write_csv_rounds, write_csv_sparse_construction, write_csv_summary,
    SparseConstructionRow,
};
use relativist_core::bench::suite::run_benchmark_suite;
use relativist_core::bench::{
    BenchmarkId, BenchmarkSuiteConfig, Mode, NetRepresentation,
    RecyclePolicy as BenchRecyclePolicy, DEFAULT_BENCH_MAX_PENDING_LIFETIME,
};

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

fn make_suite_config(
    bench_id: BenchmarkId,
    size: u32,
    representation: NetRepresentation,
    sparse_csv_path: Option<String>,
) -> BenchmarkSuiteConfig {
    BenchmarkSuiteConfig {
        benchmarks: vec![bench_id],
        sizes: Some(vec![size]),
        workers: vec![],
        mode: Mode::Sequential,
        warmup_runs: 0,
        repetitions: 1,
        csv_detail_path: None,
        csv_rounds_path: None,
        csv_summary_path: None,
        max_rounds: None,
        strict_bsp: false,
        skip_g1: false,
        chunk_size: None,
        max_pending_lifetime: DEFAULT_BENCH_MAX_PENDING_LIFETIME,
        recycle_policy: BenchRecyclePolicy::DisableUnderDelta,
        representation,
        sparse_construction_memory_csv_path: sparse_csv_path,
    }
}

// ---------------------------------------------------------------------------
// IT-0607-02 — full run emits correct rows + ratio (Linux only)
// ---------------------------------------------------------------------------

/// IT-0607-02 — End-to-end: a sparse run at `dual_tree(depth=12)` triggers
/// the auto-paired dense build and produces 2 sub-CSV rows: one Sparse, one
/// Dense, with a correctly computed `ratio_to_dense`.
///
/// **Linux-gated.** The VmHWM probe returns 0 on non-Linux platforms; the
/// auto-paired dense run would still emit a row but with a meaningless
/// `0`-derived ratio. So this test only runs on Linux where the probe is
/// real.
///
/// Per IT-0606-04, depth 12 is the at-target witness for the 80% memory
/// gate. We emit to a temp file and parse back via the `csv` crate.
#[cfg(target_os = "linux")]
#[test]
fn full_run_emits_correct_rows_and_ratio() {
    use std::io::Read;

    let depth = 12u32;
    let tmp =
        std::env::temp_dir().join(format!("relativist_it_0607_02_{}.csv", std::process::id()));
    let path = tmp.to_string_lossy().to_string();

    let cfg = make_suite_config(
        BenchmarkId::DualTree,
        depth,
        NetRepresentation::Sparse,
        Some(path.clone()),
    );

    // Run the suite directly (run_benchmark_suite owns CSV emission via
    // `commands.rs`; for the test we drive the writer ourselves to avoid
    // depending on the CLI binary).
    let result = run_benchmark_suite(&cfg).expect("IT-0607-02: suite run must succeed");

    // The suite collected 2 rows: the main sparse run + the auto-paired
    // dense build.
    assert_eq!(
        result.sparse_construction_rows.len(),
        2,
        "IT-0607-02: sparse main run + auto-paired dense build = 2 rows; got {:?}",
        result.sparse_construction_rows
    );

    // Write to disk and parse back.
    let mut buf = Vec::new();
    write_csv_sparse_construction(&mut buf, &result.sparse_construction_rows)
        .expect("IT-0607-02: writer must succeed");
    std::fs::write(&tmp, &buf).expect("IT-0607-02: write to temp file must succeed");

    let mut content = String::new();
    std::fs::File::open(&tmp)
        .and_then(|mut f| f.read_to_string(&mut content))
        .expect("IT-0607-02: read-back must succeed");

    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(
        lines.len(),
        3,
        "IT-0607-02: header + 2 data rows = 3 lines; got {content:?}"
    );

    // Locate the dense and sparse rows by representation column (order is
    // sparse-first then dense per the auto-pair flow).
    let sparse_line = lines
        .iter()
        .find(|l| l.starts_with("dual_tree,") && l.contains(",sparse,"))
        .expect("IT-0607-02: sparse row must be present");
    let dense_line = lines
        .iter()
        .find(|l| l.starts_with("dual_tree,") && l.contains(",dense,"))
        .expect("IT-0607-02: dense row must be present");

    // Both rows are well-formed.
    assert!(
        sparse_line.starts_with(&format!("dual_tree,{},sparse,", depth)),
        "IT-0607-02: sparse row prefix; got {sparse_line:?}"
    );
    assert!(
        dense_line.starts_with(&format!("dual_tree,{},dense,", depth)),
        "IT-0607-02: dense row prefix; got {dense_line:?}"
    );

    // Parse the dense and sparse peaks + ratio.
    let dense_fields: Vec<&str> = dense_line.split(',').collect();
    let sparse_fields: Vec<&str> = sparse_line.split(',').collect();
    assert_eq!(dense_fields.len(), 5, "IT-0607-02: 5 columns expected");
    assert_eq!(sparse_fields.len(), 5, "IT-0607-02: 5 columns expected");

    let dense_peak: u64 = dense_fields[3].parse().expect("dense peak parses");
    let sparse_peak: u64 = sparse_fields[3].parse().expect("sparse peak parses");
    let dense_ratio: f64 = dense_fields[4].parse().expect("dense ratio parses");
    let sparse_ratio: f64 = sparse_fields[4].parse().expect("sparse ratio parses");

    assert!(
        dense_peak > 0 && sparse_peak > 0,
        "IT-0607-02: both peaks must be > 0 on Linux"
    );
    assert!(
        (dense_ratio - 1.0).abs() < 1e-9,
        "IT-0607-02: dense row ratio_to_dense must be 1.0; got {dense_ratio}"
    );

    let expected_ratio = sparse_peak as f64 / dense_peak as f64;
    assert!(
        (sparse_ratio - expected_ratio).abs() < 1e-6,
        "IT-0607-02: sparse ratio_to_dense ({sparse_ratio}) must equal sparse_peak/dense_peak ({expected_ratio})"
    );

    eprintln!(
        "IT-0607-02: sparse_peak={} dense_peak={} ratio={:.4}",
        sparse_peak, dense_peak, sparse_ratio
    );

    // Headline 80%-gate consistency with IT-0606-04.
    assert!(
        sparse_ratio < 0.80,
        "IT-0607-02: sparse/dense ratio {sparse_ratio} must be < 0.80 (consistent with IT-0606-04)"
    );

    let _ = std::fs::remove_file(&tmp);
}

// ---------------------------------------------------------------------------
// IT-0607-03 — sparse-only row leaves ratio_to_dense blank
// ---------------------------------------------------------------------------

/// IT-0607-03 — When a sparse row exists with no paired dense row in the
/// same suite invocation, `ratio_to_dense` is emitted as the empty string
/// (blank), NOT `NaN`, NOT `0`. Locks the convention from TEST-SPEC-0607.
///
/// This test bypasses the auto-pair flow (which would always produce a
/// dense pair when representation=Sparse) and constructs a single sparse
/// row directly to exercise the writer's blank-string emission for missing
/// values. This corresponds to a future scenario where the suite is
/// extended to allow opting out of auto-pairing, OR where rows from
/// multiple runs are combined externally.
#[test]
fn sparse_only_run_leaves_ratio_blank() {
    let rows = vec![SparseConstructionRow {
        benchmark: BenchmarkId::DualTree,
        size: 8,
        representation: NetRepresentation::Sparse,
        peak_memory_during_construction: Some(12345),
        ratio_to_dense: None, // no paired dense → blank
    }];

    let mut buf = Vec::new();
    write_csv_sparse_construction(&mut buf, &rows).expect("IT-0607-03: writer must succeed");
    let csv = String::from_utf8(buf).expect("UTF-8");
    let lines: Vec<&str> = csv.lines().collect();

    assert_eq!(lines.len(), 2, "IT-0607-03: header + 1 row = 2 lines");

    // Final field must be empty (between the trailing comma and the EOL).
    let row = lines[1];
    assert!(
        row.ends_with(','),
        "IT-0607-03: ratio_to_dense None must emit blank (line ends with comma); got {row:?}"
    );

    let fields: Vec<&str> = row.split(',').collect();
    assert_eq!(fields.len(), 5, "IT-0607-03: 5 columns expected");
    assert_eq!(
        fields[4], "",
        "IT-0607-03: ratio_to_dense field must be blank (not NaN, not 0); got {:?}",
        fields[4]
    );

    // Sanity: peak field IS populated (Some(12345)).
    assert_eq!(
        fields[3], "12345",
        "IT-0607-03: peak field must be populated when Some(_); got {:?}",
        fields[3]
    );
}

/// IT-0607-03b — `None` `peak_memory_during_construction` (non-Linux probe)
/// also emits as blank, NOT `0`. Locks the convention.
#[test]
fn sparse_only_row_with_none_peak_emits_blank_peak() {
    let rows = vec![SparseConstructionRow {
        benchmark: BenchmarkId::DualTree,
        size: 8,
        representation: NetRepresentation::Sparse,
        peak_memory_during_construction: None,
        ratio_to_dense: None,
    }];

    let mut buf = Vec::new();
    write_csv_sparse_construction(&mut buf, &rows).expect("IT-0607-03b: writer must succeed");
    let csv = String::from_utf8(buf).expect("UTF-8");
    let lines: Vec<&str> = csv.lines().collect();

    assert_eq!(lines.len(), 2);
    let fields: Vec<&str> = lines[1].split(',').collect();
    assert_eq!(fields.len(), 5);
    assert_eq!(
        fields[3], "",
        "IT-0607-03b: peak None must emit blank, NOT '0' (would be indistinguishable from a real-zero measurement)"
    );
    assert_eq!(fields[4], "", "IT-0607-03b: ratio None also blank");
}

// ---------------------------------------------------------------------------
// IT-0607-04 — existing CSVs unchanged when sparse path is set
// ---------------------------------------------------------------------------

/// IT-0607-04 — Setting `sparse_construction_memory_csv_path = Some(_)`
/// MUST NOT alter the bytes emitted by the existing `write_csv_detail`,
/// `write_csv_rounds`, or `write_csv_summary` writers. This is the core
/// "additive change" guarantee — frozen `v1_local_baseline` rodadas remain
/// bit-stable.
///
/// We exercise this by running two suite invocations with identical
/// configuration (small, fast workload — `ep_annihilation` at size 10),
/// one with `sparse_construction_memory_csv_path = Some(...)` and one with
/// `None`, and comparing the existing CSV outputs byte-for-byte.
#[test]
fn existing_csv_outputs_unchanged_when_sparse_csv_path_set() {
    let bench_id = BenchmarkId::EPAnnihilation;
    let size = 10u32;

    // Run 1: WITHOUT sparse CSV.
    let cfg_without = make_suite_config(bench_id, size, NetRepresentation::Dense, None);
    let result_without =
        run_benchmark_suite(&cfg_without).expect("IT-0607-04: WITHOUT-sparse suite must succeed");

    // Run 2: WITH sparse CSV path.
    let cfg_with = make_suite_config(
        bench_id,
        size,
        NetRepresentation::Dense,
        Some("/tmp/relativist_it_0607_04_unused.csv".to_string()),
    );
    let result_with =
        run_benchmark_suite(&cfg_with).expect("IT-0607-04: WITH-sparse suite must succeed");

    // --- Detail CSV byte-equivalence ---
    //
    // Time-dependent fields (wall_clock_secs, mips, speedup, peak_memory_bytes)
    // can vary between runs; we compare the SCHEMA + per-column structure,
    // not the per-row time values. The point of this test is NOT to assert
    // bench-time stability — that's covered elsewhere — but to assert that
    // setting the new field does not ADD/REMOVE columns or rows from the
    // existing CSVs.

    let mut buf_without = Vec::new();
    write_csv_detail(&mut buf_without, &result_without.results).unwrap();
    let mut buf_with = Vec::new();
    write_csv_detail(&mut buf_with, &result_with.results).unwrap();

    let csv_without = String::from_utf8(buf_without).unwrap();
    let csv_with = String::from_utf8(buf_with).unwrap();
    let lines_without: Vec<&str> = csv_without.lines().collect();
    let lines_with: Vec<&str> = csv_with.lines().collect();

    // Header must be identical.
    assert_eq!(
        lines_without[0], lines_with[0],
        "IT-0607-04: detail CSV header MUST be identical regardless of sparse path"
    );
    // Row count must be identical.
    assert_eq!(
        lines_without.len(),
        lines_with.len(),
        "IT-0607-04: detail CSV row count MUST be identical"
    );
    // Per row, column count must be identical (so we know no column was
    // added/removed for the sparse-aware run).
    for (i, (a, b)) in lines_without.iter().zip(lines_with.iter()).enumerate() {
        assert_eq!(
            a.matches(',').count(),
            b.matches(',').count(),
            "IT-0607-04: detail CSV row {i} column count must be identical"
        );
    }

    // --- Rounds CSV ---
    let mut buf_without = Vec::new();
    write_csv_rounds(&mut buf_without, &result_without.results).unwrap();
    let mut buf_with = Vec::new();
    write_csv_rounds(&mut buf_with, &result_with.results).unwrap();
    assert_eq!(
        String::from_utf8(buf_without).unwrap().lines().next(),
        String::from_utf8(buf_with).unwrap().lines().next(),
        "IT-0607-04: rounds CSV header MUST be identical regardless of sparse path"
    );

    // --- Summary CSV ---
    let mut buf_without = Vec::new();
    write_csv_summary(&mut buf_without, &result_without.summaries).unwrap();
    let mut buf_with = Vec::new();
    write_csv_summary(&mut buf_with, &result_with.summaries).unwrap();
    assert_eq!(
        String::from_utf8(buf_without).unwrap().lines().next(),
        String::from_utf8(buf_with).unwrap().lines().next(),
        "IT-0607-04: summary CSV header MUST be identical regardless of sparse path"
    );

    // --- Sparse rows: empty without, populated with ---
    assert!(
        result_without.sparse_construction_rows.is_empty(),
        "IT-0607-04: sparse_construction_rows MUST be empty when path is None"
    );
    assert!(
        !result_with.sparse_construction_rows.is_empty(),
        "IT-0607-04: sparse_construction_rows MUST be populated when path is Some(_) (Dense → 1 row)"
    );
    assert_eq!(
        result_with.sparse_construction_rows.len(),
        1,
        "IT-0607-04: representation=Dense + sparse path = exactly 1 row (no auto-pair)"
    );
    let only_row = &result_with.sparse_construction_rows[0];
    assert_eq!(only_row.representation, NetRepresentation::Dense);
    assert_eq!(only_row.ratio_to_dense, Some(1.0));
}
