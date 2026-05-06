//! IT-0703-01 + IT-0703-02 — D-014 CSV schema extension (TASK-0703;
//! updated by TASK-0720 BUG-006: cv_above_gate dropped from the schema —
//! CV is now owned by the bash orchestrator).
//!
//! Verifies the 3 new columns appended at the end of the existing D-012
//! `detail.csv` row:
//!   `vmrss_peak_mb`, `vmrss_current_end_mb`, `stop_reason`.
//!
//! Production code uses `bench::csv::write_csv_detail` (manual `writeln!`).
//! These tests roundtrip a representative `BenchmarkResult` through that
//! writer and confirm:
//! 1. The 3 new columns are at the END of the header in the correct order.
//! 2. The values render as expected (`stop_reason = ""` for `None`).
//! 3. A "legacy" struct that only knows the original 29 columns can still
//!    consume a row produced by post-D-014 code (forward-compat).

use relativist_core::bench::{
    csv::write_csv_detail, BenchmarkId, BenchmarkResult, InteractionsByRule, Mode,
    NetRepresentation, RecyclePolicy,
};
use std::collections::HashMap;
use std::io::Cursor;

/// Build a minimal D-014 stress-curve sample row. Values chosen so the
/// CSV cells are unambiguous (e.g., `123.4` is non-trivial to confuse).
fn sample_row(stop_reason: Option<&str>) -> BenchmarkResult {
    BenchmarkResult {
        benchmark: BenchmarkId::EPAnnihilation,
        input_size: 1000,
        mode: Mode::Sequential,
        workers: 1,
        repetition: 0,
        correct: true,
        wall_clock_secs: 0.5,
        total_interactions: 1000,
        mips: 2.0,
        interactions_by_rule: InteractionsByRule::default(),
        rounds: 0,
        border_redexes_per_round: vec![],
        border_ratio_per_round: vec![],
        peak_memory_bytes: 0,
        peak_memory_during_construction: 0,
        peak_memory_during_reduction: 0,
        agent_count_at_construction_complete: 100,
        live_agent_count_watermark: 50,
        representation: NetRepresentation::Dense,
        chunk_size: None,
        recycle_policy: RecyclePolicy::DisableUnderDelta,
        agents_per_round: vec![],
        bytes_sent: 0,
        bytes_received: 0,
        bytes_sent_per_round: vec![],
        bytes_received_per_round: vec![],
        partition_time_per_round: vec![],
        compute_time_per_round: vec![],
        merge_time_per_round: vec![],
        network_time_per_round: vec![],
        worker_stats: vec![],
        speedup: 1.0,
        efficiency: 1.0,
        overhead_ratio: 0.0,
        // D-014 columns under test (cv_above_gate dropped per TASK-0720 BUG-006).
        vmrss_peak_mb: 123.4,
        vmrss_current_end_mb: 100.0,
        stop_reason: stop_reason.map(|s| s.to_string()),
    }
}

/// IT-0703-01 — Roundtrip of the 3 new columns (cv_above_gate dropped
/// per TASK-0720 BUG-006).
#[test]
fn roundtrip_writes_and_reads_new_columns() {
    // Sub-step 1+2: write a row with stop_reason=Some.
    let row = sample_row(Some("MemoryExceeded"));
    let mut buf: Vec<u8> = Vec::new();
    write_csv_detail(&mut buf, std::slice::from_ref(&row)).expect("write_csv_detail");
    let csv_text = String::from_utf8(buf.clone()).expect("utf-8");

    // Sub-step 3: header sanity — last 3 column names in order.
    let header_line = csv_text.lines().next().expect("at least header line");
    assert!(
        header_line.ends_with("vmrss_peak_mb,vmrss_current_end_mb,stop_reason"),
        "header MUST end with the 3 new columns in order (cv_above_gate dropped); got: {}",
        header_line
    );

    // Sub-step 4: roundtrip via csv crate.
    let mut rdr = csv::Reader::from_reader(Cursor::new(buf));
    let row_back: HashMap<String, String> = rdr
        .deserialize()
        .next()
        .expect("at least one data row")
        .expect("deserialize ok");
    let vmrss_peak: f64 = row_back
        .get("vmrss_peak_mb")
        .expect("column present")
        .parse()
        .expect("parse f64");
    let vmrss_cur: f64 = row_back
        .get("vmrss_current_end_mb")
        .expect("column present")
        .parse()
        .expect("parse f64");
    let stop_reason_str = row_back.get("stop_reason").expect("column present").clone();

    assert!(
        (vmrss_peak - 123.4).abs() < f64::EPSILON * 16.0,
        "vmrss_peak_mb roundtrip mismatch: got {}",
        vmrss_peak
    );
    assert!(
        (vmrss_cur - 100.0).abs() < f64::EPSILON * 16.0,
        "vmrss_current_end_mb roundtrip mismatch: got {}",
        vmrss_cur
    );
    assert_eq!(stop_reason_str, "MemoryExceeded");
    assert!(
        !row_back.contains_key("cv_above_gate"),
        "cv_above_gate MUST NOT be present in the schema (TASK-0720 BUG-006)"
    );

    // Sub-step 5: empty stop_reason renders as blank.
    let row2 = sample_row(None);
    let mut buf2: Vec<u8> = Vec::new();
    write_csv_detail(&mut buf2, std::slice::from_ref(&row2)).expect("write_csv_detail");
    let txt2 = String::from_utf8(buf2.clone()).expect("utf-8");
    let data_line = txt2.lines().nth(1).expect("data row exists");
    assert!(
        data_line.ends_with(","),
        "row's last 3 fields must end with `,` (the blank stop_reason cell after vmrss); got: {}",
        data_line
    );

    let mut rdr2 = csv::Reader::from_reader(Cursor::new(buf2));
    let row_back2: HashMap<String, String> = rdr2
        .deserialize()
        .next()
        .expect("data row")
        .expect("deserialize ok");
    assert_eq!(row_back2.get("stop_reason").map(String::as_str), Some(""));
}

/// IT-0703-02 — A header→value map produced by the csv crate's
/// `StringRecord` API can still consume a post-D-014 row losslessly.
/// Demonstrates the forward-compat invariant that downstream readers
/// joining on column NAMES (not positional indices) are unaffected by
/// the additive 4-column extension; trailing columns are simply
/// available alongside the legacy 29.
#[test]
fn legacy_struct_reads_post_d014_row() {
    // Write a post-D-014 row (3 new columns populated; cv_above_gate dropped
    // per TASK-0720 BUG-006).
    let row = sample_row(None);
    let mut buf: Vec<u8> = Vec::new();
    write_csv_detail(&mut buf, std::slice::from_ref(&row)).expect("write_csv_detail");

    // Use StringRecord — the canonical "legacy" reader idiom: read header
    // names, read row, build a name→value map. This is what the existing
    // Python tooling does (`pandas.read_csv` works the same way). The csv
    // crate does NOT error on extra columns under this API.
    let mut rdr = csv::Reader::from_reader(Cursor::new(buf));
    let headers: csv::StringRecord = rdr.headers().expect("read header").clone();
    let mut records = rdr.records();
    let record: csv::StringRecord = records.next().expect("must have a row").expect("read row");

    let map: HashMap<String, String> = headers
        .iter()
        .zip(record.iter())
        .map(|(h, v)| (h.to_string(), v.to_string()))
        .collect();

    // Legacy columns survive alongside the new ones — protects against a
    // regression that would silently re-order columns.
    assert!(
        map.contains_key("benchmark"),
        "legacy column 'benchmark' missing"
    );
    assert!(
        map.contains_key("input_size"),
        "legacy column 'input_size' missing"
    );
    assert!(
        map.contains_key("recycle_policy"),
        "legacy column 'recycle_policy' missing"
    );
    // New D-014 columns alongside (cv_above_gate dropped).
    assert!(
        map.contains_key("vmrss_peak_mb"),
        "new column 'vmrss_peak_mb' missing"
    );
    assert!(
        map.contains_key("stop_reason"),
        "new column 'stop_reason' missing"
    );
    assert!(
        !map.contains_key("cv_above_gate"),
        "TASK-0720 BUG-006: cv_above_gate MUST be absent from the schema"
    );
    assert_eq!(
        map.len(),
        32,
        "post-TASK-0720 row must expose 32 columns; got {}",
        map.len()
    );
}
