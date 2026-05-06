//! CSV output for benchmark results (SPEC-09 R39-R42).

use super::{AggregatedStats, BenchmarkId, BenchmarkResult, NetRepresentation};
use std::io::{self, Write};

/// SF-005 (D-011 review): convert the raw VmHWM probe value to the
/// `Option<u64>` representation used by
/// [`SparseConstructionRow::peak_memory_during_construction`]. Pre-fix this
/// helper lived in `suite.rs` but was only consumed here in `csv.rs`;
/// co-located here for cohesion. `pub(super)` so `suite.rs` can still call
/// it on the construction-time probe value.
///
/// The probe returns `0` on non-Linux targets where `/proc/self/status` is
/// unavailable. Per TEST-SPEC-0607, the CSV column should be **blank** in
/// that case (not literal `0`, which would be indistinguishable from
/// "sparse used zero memory" — a false success signal). Linux captures
/// (`>0`) round-trip as `Some(_)`.
pub(super) fn peak_for_sparse_row(raw: u64) -> Option<u64> {
    if raw == 0 {
        None
    } else {
        Some(raw)
    }
}

/// Render `peak_memory_during_construction` (a `u64`) as a CSV cell, using
/// the §4.9 non-Linux convention (blank string instead of literal `0`) when
/// the probe is unavailable.
///
/// QA-D011-009 / SF-002: ensures the main `detail.csv` writer renders blank
/// for non-Linux runs, matching the sub-CSV writer's convention. Linux runs
/// (where VmHWM is non-zero) emit the literal byte count.
fn render_peak_memory_cell(raw: u64) -> String {
    if raw == 0 {
        String::new()
    } else {
        raw.to_string()
    }
}

/// Render `chunk_size: Option<u32>` as a CSV cell. `None` (eager path)
/// renders as the empty string per SPEC-09 §4.9 (R18f); `Some(N)` renders
/// as the literal integer.
fn render_chunk_size_cell(value: Option<u32>) -> String {
    match value {
        None => String::new(),
        Some(n) => n.to_string(),
    }
}

/// Render `recycle_policy` as the kebab-case form ("disable-under-delta"
/// / "border-clean") per SPEC-09 R18g, matching the `clap::ValueEnum`
/// rename on the CLI side. Locally implemented here so a future drift in
/// `RecyclePolicy::Display` (which doesn't exist today) doesn't silently
/// change the CSV column.
fn render_recycle_policy_cell(policy: super::RecyclePolicy) -> &'static str {
    match policy {
        super::RecyclePolicy::DisableUnderDelta => "disable-under-delta",
        super::RecyclePolicy::BorderClean => "border-clean",
    }
}

/// Write detail CSV: one row per datapoint (SPEC-09 R39a).
///
/// SPEC-09 §4.9 R18a-R18g (D-011 Phase F-1, commit `82b2d27`; D-011 MF-002
/// extension): the v1 22-column schema is preserved at the LEFT; the Tier 3
/// measurement columns are appended to the RIGHT. The 7 appended columns
/// per R39a (line 711):
///
/// 23: `peak_memory_during_construction`  (R18a)
/// 24: `peak_memory_during_reduction`     (R18b)
/// 25: `agent_count_at_construction_complete` (R18c)
/// 26: `live_agent_count_watermark`       (R18d)
/// 27: `representation`                   (R18e)  "dense" / "sparse"
/// 28: `chunk_size`                       (R18f)  N / blank
/// 29: `recycle_policy`                   (R18g)  "disable-under-delta" / "border-clean"
///
/// v1-equivalent rodadas (eager path, dense, default recycle) MUST still
/// populate every column — none of the rightmost 7 may be omitted (§4.9
/// line ~714 / line 538). The `peak_memory_during_construction` and
/// `peak_memory_during_reduction` cells render BLANK on non-Linux targets
/// (where the VmHWM probe returns 0) per the §4.9 convention; literal
/// `0` would be indistinguishable from "construction used 0 bytes".
pub fn write_csv_detail<W: Write>(writer: &mut W, results: &[BenchmarkResult]) -> io::Result<()> {
    writeln!(
        writer,
        "benchmark,input_size,mode,workers,repetition,correct,wall_clock_secs,\
         total_interactions,mips,rounds,speedup,efficiency,overhead_ratio,\
         peak_memory_bytes,bytes_sent,bytes_received,\
         con_con,dup_dup,era_era,con_dup,con_era,dup_era,\
         peak_memory_during_construction,peak_memory_during_reduction,\
         agent_count_at_construction_complete,live_agent_count_watermark,\
         representation,chunk_size,recycle_policy,\
         vmrss_peak_mb,vmrss_current_end_mb,stop_reason,cv_above_gate"
    )?;

    for r in results {
        writeln!(
            writer,
            "{},{},{},{},{},{},{:.6},{},{:.3},{},{:.4},{:.4},{:.4},{},{},{},\
             {},{},{},{},{},{},\
             {},{},{},{},{},{},{},\
             {:.6},{:.6},{},{}",
            r.benchmark,
            r.input_size,
            r.mode,
            r.workers,
            r.repetition,
            r.correct,
            r.wall_clock_secs,
            r.total_interactions,
            r.mips,
            r.rounds,
            r.speedup,
            r.efficiency,
            r.overhead_ratio,
            r.peak_memory_bytes,
            r.bytes_sent,
            r.bytes_received,
            r.interactions_by_rule.con_con,
            r.interactions_by_rule.dup_dup,
            r.interactions_by_rule.era_era,
            r.interactions_by_rule.con_dup,
            r.interactions_by_rule.con_era,
            r.interactions_by_rule.dup_era,
            // R18a-R18g (D-011 MF-002):
            render_peak_memory_cell(r.peak_memory_during_construction),
            render_peak_memory_cell(r.peak_memory_during_reduction),
            r.agent_count_at_construction_complete,
            r.live_agent_count_watermark,
            r.representation,
            render_chunk_size_cell(r.chunk_size),
            render_recycle_policy_cell(r.recycle_policy),
            // D-014 stress-curve columns (TASK-0703).
            r.vmrss_peak_mb,
            r.vmrss_current_end_mb,
            r.stop_reason.as_deref().unwrap_or(""),
            r.cv_above_gate,
        )?;
    }
    Ok(())
}

/// Write rounds CSV: one row per round per execution (SPEC-09 R39b).
/// Only populated for distributed modes (rounds > 0).
pub fn write_csv_rounds<W: Write>(writer: &mut W, results: &[BenchmarkResult]) -> io::Result<()> {
    writeln!(
        writer,
        "benchmark,input_size,workers,mode,repetition,round,\
         partition_time_secs,compute_time_secs,merge_time_secs,network_time_secs,\
         border_redexes,border_ratio,agents_at_start,bytes_sent,bytes_received"
    )?;

    for r in results {
        if r.rounds == 0 {
            continue;
        }
        for round in 0..r.rounds as usize {
            let partition_t = r
                .partition_time_per_round
                .get(round)
                .copied()
                .unwrap_or(0.0);
            let compute_t = r.compute_time_per_round.get(round).copied().unwrap_or(0.0);
            let merge_t = r.merge_time_per_round.get(round).copied().unwrap_or(0.0);
            let network_t = r.network_time_per_round.get(round).copied().unwrap_or(0.0);
            let border_redexes = r.border_redexes_per_round.get(round).copied().unwrap_or(0);
            let border_ratio = r.border_ratio_per_round.get(round).copied().unwrap_or(0.0);
            let agents = r.agents_per_round.get(round).copied().unwrap_or(0);
            let sent = r.bytes_sent_per_round.get(round).copied().unwrap_or(0);
            let recv = r.bytes_received_per_round.get(round).copied().unwrap_or(0);

            writeln!(
                writer,
                "{},{},{},{},{},{},{:.6},{:.6},{:.6},{:.6},{},{:.6},{},{},{}",
                r.benchmark,
                r.input_size,
                r.workers,
                r.mode,
                r.repetition,
                round,
                partition_t,
                compute_t,
                merge_t,
                network_t,
                border_redexes,
                border_ratio,
                agents,
                sent,
                recv,
            )?;
        }
    }
    Ok(())
}

/// Write summary CSV: one row per configuration (SPEC-09 R39c).
pub fn write_csv_summary<W: Write>(writer: &mut W, stats: &[AggregatedStats]) -> io::Result<()> {
    writeln!(
        writer,
        "benchmark,input_size,mode,workers,repetitions,all_correct,\
         wall_clock_mean,wall_clock_std,wall_clock_median,wall_clock_min,wall_clock_max,\
         mips_mean,speedup_mean,efficiency_mean,overhead_ratio_mean,cv"
    )?;

    for s in stats {
        writeln!(
            writer,
            "{},{},{},{},{},{},{:.6},{:.6},{:.6},{:.6},{:.6},{:.3},{:.4},{:.4},{:.4},{:.4}",
            s.benchmark,
            s.input_size,
            s.mode,
            s.workers,
            s.repetitions,
            s.all_correct,
            s.wall_clock_mean,
            s.wall_clock_std,
            s.wall_clock_median,
            s.wall_clock_min,
            s.wall_clock_max,
            s.mips_mean,
            s.speedup_mean,
            s.efficiency_mean,
            s.overhead_ratio_mean,
            s.cv,
        )?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// TASK-0607 — sparse_construction_memory.csv sub-writer (SPEC-09 §3.4.5)
// ---------------------------------------------------------------------------

/// One row of the `sparse_construction_memory.csv` sub-CSV
/// (SPEC-09 §3.4.5, D-011 Phase D-3).
///
/// The five columns map 1:1 to `peak_memory_during_construction` (SPEC-09
/// R18a, line 482, commit `82b2d27`) per (benchmark, size, representation)
/// tuple, plus the derived `ratio_to_dense` for sparse/dense comparison.
///
/// Both numeric fields are `Option<_>` so that:
/// - `peak_memory_during_construction = None` → emitted as the empty string
///   on non-Linux platforms where the `/proc/self/status` probe is
///   unavailable. (On Linux the harness normally sets `Some(0..)`.)
/// - `ratio_to_dense = None` → emitted as the empty string when no paired
///   dense row exists in the same suite invocation. The blank-string
///   convention (vs `NaN`) is locked by TEST-SPEC-0607 §IT-0607-03 — blanks
///   are forward-compatible with downstream CSV consumers without requiring
///   NaN-aware parsing.
#[derive(Debug, Clone, PartialEq)]
pub struct SparseConstructionRow {
    pub benchmark: BenchmarkId,
    pub size: u32,
    pub representation: NetRepresentation,
    pub peak_memory_during_construction: Option<u64>,
    pub ratio_to_dense: Option<f64>,
}

/// Write the sparse-construction-memory sub-CSV (SPEC-09 §3.4.5).
///
/// Header (locked by UT-0607-01, must match SPEC-09 R18a verbatim):
///
/// ```csv
/// benchmark,size,representation,peak_memory_during_construction,ratio_to_dense
/// ```
///
/// One row is emitted per (benchmark, size, representation) tuple. `None`
/// values for `peak_memory_during_construction` and `ratio_to_dense` are
/// emitted as the empty string (per the test-spec's locked convention —
/// NOT `NaN`, NOT `0`).
///
/// The existing detail / rounds / summary CSV writers are unaffected by
/// this writer; this is an additive emission channel.
pub fn write_csv_sparse_construction<W: Write>(
    writer: &mut W,
    rows: &[SparseConstructionRow],
) -> io::Result<()> {
    writeln!(
        writer,
        "benchmark,size,representation,peak_memory_during_construction,ratio_to_dense"
    )?;

    for row in rows {
        let peak_field = row
            .peak_memory_during_construction
            .map(|v| v.to_string())
            .unwrap_or_default();
        let ratio_field = row
            .ratio_to_dense
            .map(|r| format!("{:.6}", r))
            .unwrap_or_default();

        writeln!(
            writer,
            "{},{},{},{},{}",
            row.benchmark, row.size, row.representation, peak_field, ratio_field,
        )?;
    }
    Ok(())
}

/// Build the sub-CSV rows from a slice of `BenchmarkResult` plus a
/// per-(benchmark, size, representation) index of `peak_memory_during_construction`.
///
/// The bench harness collects construction-phase peaks during the suite
/// run (one per `(BenchmarkId, size, NetRepresentation)` triple — see
/// `SuiteResult.sparse_construction_rows`); this helper computes
/// `ratio_to_dense` at emit time:
///
/// - Dense rows: `ratio_to_dense = Some(1.0)`.
/// - Sparse rows: if a Dense row exists for the same `(benchmark, size)`
///   AND both peaks are `Some(_)` AND the dense peak is non-zero, then
///   `ratio_to_dense = Some(sparse_peak / dense_peak)`. Otherwise `None`
///   (emitted blank).
///
/// Defined here so the writer test (UT-0607-01) and the full suite path
/// (IT-0607-02 / IT-0607-04) share the same row-construction logic.
pub fn compute_ratios_for_sparse_rows(rows: &mut [SparseConstructionRow]) {
    use std::collections::HashMap;

    // Index dense peaks by (benchmark, size) so sparse rows can look them up.
    let mut dense_peaks: HashMap<(BenchmarkId, u32), Option<u64>> = HashMap::new();
    for row in rows.iter() {
        if row.representation == NetRepresentation::Dense {
            dense_peaks.insert(
                (row.benchmark, row.size),
                row.peak_memory_during_construction,
            );
        }
    }

    for row in rows.iter_mut() {
        match row.representation {
            NetRepresentation::Dense => {
                row.ratio_to_dense = Some(1.0);
            }
            NetRepresentation::Sparse => {
                let key = (row.benchmark, row.size);
                row.ratio_to_dense = match (
                    row.peak_memory_during_construction,
                    dense_peaks.get(&key).and_then(|v| *v),
                ) {
                    (Some(s), Some(d)) if d > 0 => Some(s as f64 / d as f64),
                    _ => None,
                };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bench::*;

    fn sample_result() -> BenchmarkResult {
        BenchmarkResult {
            benchmark: BenchmarkId::EPAnnihilation,
            input_size: 100,
            mode: Mode::Sequential,
            workers: 1,
            repetition: 0,
            correct: true,
            wall_clock_secs: 0.001234,
            total_interactions: 100,
            mips: 81.037,
            interactions_by_rule: InteractionsByRule {
                con_con: 0,
                dup_dup: 0,
                era_era: 100,
                con_dup: 0,
                con_era: 0,
                dup_era: 0,
            },
            rounds: 0,
            border_redexes_per_round: vec![],
            border_ratio_per_round: vec![],
            peak_memory_bytes: 0,
            peak_memory_during_construction: 0,
            peak_memory_during_reduction: 0,
            agent_count_at_construction_complete: 0,
            live_agent_count_watermark: 0,
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
            vmrss_peak_mb: 0.0,
            vmrss_current_end_mb: 0.0,
            stop_reason: None,
            cv_above_gate: false,
        }
    }

    #[test]
    fn test_csv_detail_header() {
        let mut buf = Vec::new();
        write_csv_detail(&mut buf, &[]).unwrap();
        let csv = String::from_utf8(buf).unwrap();
        assert!(csv.starts_with("benchmark,input_size,mode,workers"));
    }

    /// MF-002 / QA-D011-006 (extended D-014 / TASK-0703) — `detail.csv`
    /// schema MUST be exactly 33 columns in the SPEC-09 R39a-mandated order
    /// **plus** the 4 D-014 stress-curve columns appended at the end.
    /// Any drift here breaks the downstream Python tooling that joins on
    /// (benchmark, size, representation, chunk_size, workers, mode).
    #[test]
    fn detail_csv_header_locks_29_columns_spec_09_r39a() {
        let mut buf = Vec::new();
        write_csv_detail(&mut buf, &[]).unwrap();
        let csv = String::from_utf8(buf).unwrap();

        let header = csv.lines().next().expect("header line must exist");
        let columns: Vec<&str> = header.split(',').collect();
        assert_eq!(
            columns.len(),
            33,
            "TASK-0703: SPEC-09 R39a 29 columns + 4 D-014 stress-curve columns; got {}",
            columns.len()
        );

        // Verbatim column-name order — the v1 22 + 7 Tier 3 (R18a-R18g)
        // + 4 D-014 stress-curve columns appended at the end (TASK-0703).
        let expected: Vec<&str> = vec![
            "benchmark",
            "input_size",
            "mode",
            "workers",
            "repetition",
            "correct",
            "wall_clock_secs",
            "total_interactions",
            "mips",
            "rounds",
            "speedup",
            "efficiency",
            "overhead_ratio",
            "peak_memory_bytes",
            "bytes_sent",
            "bytes_received",
            "con_con",
            "dup_dup",
            "era_era",
            "con_dup",
            "con_era",
            "dup_era",
            "peak_memory_during_construction",
            "peak_memory_during_reduction",
            "agent_count_at_construction_complete",
            "live_agent_count_watermark",
            "representation",
            "chunk_size",
            "recycle_policy",
            // D-014 stress-curve columns (TASK-0703).
            "vmrss_peak_mb",
            "vmrss_current_end_mb",
            "stop_reason",
            "cv_above_gate",
        ];
        assert_eq!(
            columns, expected,
            "TASK-0703: detail.csv columns MUST match SPEC-09 R39a + D-014 appendix verbatim"
        );
    }

    /// MF-002 / QA-D011-006 — populated row contains all 7 Tier 3 columns
    /// at the rightmost positions, with the documented value formatting:
    /// `peak_memory_*` blank for `0`, `chunk_size` blank for `None`,
    /// `representation` lowercase, `recycle_policy` kebab-case.
    #[test]
    fn detail_csv_row_renders_all_7_tier3_columns_with_correct_formatting() {
        let mut r = sample_result();
        r.peak_memory_during_construction = 0; // → blank
        r.peak_memory_during_reduction = 12345;
        r.agent_count_at_construction_complete = 200;
        r.live_agent_count_watermark = 150;
        r.representation = NetRepresentation::Sparse;
        r.chunk_size = Some(100);
        r.recycle_policy = RecyclePolicy::BorderClean;

        let mut buf = Vec::new();
        write_csv_detail(&mut buf, &[r]).unwrap();
        let csv = String::from_utf8(buf).unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 2, "header + 1 row");
        let cells: Vec<&str> = lines[1].split(',').collect();
        assert_eq!(
            cells.len(),
            33,
            "row must have exactly 33 cells (29 SPEC-09 + 4 D-014 stress-curve)"
        );

        // Cells 22..28 — Tier 3 (R18a..R18g):
        assert_eq!(
            cells[22], "",
            "peak_memory_during_construction = 0 must render blank, not '0'"
        );
        assert_eq!(cells[23], "12345", "peak_memory_during_reduction");
        assert_eq!(cells[24], "200", "agent_count_at_construction_complete");
        assert_eq!(cells[25], "150", "live_agent_count_watermark");
        assert_eq!(cells[26], "sparse", "representation (lowercase)");
        assert_eq!(cells[27], "100", "chunk_size = Some(100)");
        assert_eq!(cells[28], "border-clean", "recycle_policy (kebab-case)");

        // Cells 29..32 — D-014 stress-curve (TASK-0703); zero defaults on
        // a sample result that did not opt into the campaign.
        assert_eq!(cells[29], "0.000000", "vmrss_peak_mb default = 0.0");
        assert_eq!(cells[30], "0.000000", "vmrss_current_end_mb default = 0.0");
        assert_eq!(cells[31], "", "stop_reason = None must render blank");
        assert_eq!(cells[32], "false", "cv_above_gate default = false");
    }

    /// MF-002 — `chunk_size = None` renders as the empty string in the
    /// eager rodada.
    #[test]
    fn detail_csv_eager_path_chunk_size_renders_blank() {
        let mut r = sample_result();
        r.chunk_size = None;
        let mut buf = Vec::new();
        write_csv_detail(&mut buf, &[r]).unwrap();
        let csv = String::from_utf8(buf).unwrap();
        let cells: Vec<&str> = csv.lines().nth(1).unwrap().split(',').collect();
        assert_eq!(
            cells[27], "",
            "MF-002: eager path (chunk_size=None) must render blank chunk_size cell"
        );
    }

    #[test]
    fn test_csv_detail_row() {
        let mut buf = Vec::new();
        write_csv_detail(&mut buf, &[sample_result()]).unwrap();
        let csv = String::from_utf8(buf).unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 2); // header + 1 row
        assert!(lines[1].starts_with("ep_annihilation,100,sequential,1"));
    }

    #[test]
    fn test_csv_rounds_header() {
        let mut buf = Vec::new();
        write_csv_rounds(&mut buf, &[]).unwrap();
        let csv = String::from_utf8(buf).unwrap();
        assert!(csv.starts_with("benchmark,input_size,workers,mode"));
        assert!(csv.contains("partition_time_secs"));
    }

    #[test]
    fn test_csv_rounds_skips_sequential() {
        let mut buf = Vec::new();
        // sample_result has rounds=0, so should produce no data rows
        write_csv_rounds(&mut buf, &[sample_result()]).unwrap();
        let csv = String::from_utf8(buf).unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 1); // header only
    }

    #[test]
    fn test_csv_rounds_with_data() {
        let mut r = sample_result();
        r.rounds = 2;
        r.mode = Mode::Local;
        r.workers = 2;
        r.partition_time_per_round = vec![0.001, 0.002];
        r.compute_time_per_round = vec![0.01, 0.02];
        r.merge_time_per_round = vec![0.003, 0.004];
        r.network_time_per_round = vec![0.0, 0.0];
        r.border_redexes_per_round = vec![5, 3];
        r.border_ratio_per_round = vec![0.1, 0.05];
        r.agents_per_round = vec![100, 50];
        r.bytes_sent_per_round = vec![1024, 512];
        r.bytes_received_per_round = vec![2048, 1024];

        let mut buf = Vec::new();
        write_csv_rounds(&mut buf, &[r]).unwrap();
        let csv = String::from_utf8(buf).unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 3); // header + 2 rounds
    }

    #[test]
    fn test_csv_summary_header() {
        let mut buf = Vec::new();
        write_csv_summary(&mut buf, &[]).unwrap();
        let csv = String::from_utf8(buf).unwrap();
        assert!(csv.starts_with("benchmark,input_size,mode,workers"));
    }

    #[test]
    fn test_csv_summary_row() {
        let stats = AggregatedStats {
            benchmark: BenchmarkId::EPAnnihilation,
            input_size: 100,
            mode: Mode::Sequential,
            workers: 1,
            repetitions: 5,
            all_correct: true,
            wall_clock_mean: 0.001,
            wall_clock_std: 0.0001,
            wall_clock_median: 0.001,
            wall_clock_min: 0.0009,
            wall_clock_max: 0.0012,
            mips_mean: 100.0,
            speedup_mean: 1.0,
            efficiency_mean: 1.0,
            overhead_ratio_mean: 0.0,
            cv: 0.1,
        };
        let mut buf = Vec::new();
        write_csv_summary(&mut buf, &[stats]).unwrap();
        let csv = String::from_utf8(buf).unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[1].starts_with("ep_annihilation,100,sequential,1"));
    }

    // -----------------------------------------------------------------------
    // TASK-0607 — sparse_construction_memory.csv writer (Phase D-3)
    // -----------------------------------------------------------------------

    /// UT-0607-01 — Header lock vs SPEC-09 §3.4.5.
    ///
    /// Pin the header line to the EXACT column order mandated by SPEC-09
    /// §3.4.5 (committed `82b2d27`). Catches header drift, including the
    /// legacy `peak_construction_bytes` name (the SPEC-09 R18a canonical
    /// name `peak_memory_during_construction` is the SUPERSEDING column
    /// header).
    #[test]
    fn sparse_csv_header_matches_spec_09_section_3_4_5() {
        let mut buf = Vec::new();
        write_csv_sparse_construction(&mut buf, &[]).unwrap();
        let csv = String::from_utf8(buf).unwrap();

        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(
            lines.len(),
            1,
            "UT-0607-01: header-only output must be exactly 1 line; got {csv:?}"
        );

        let expected_header =
            "benchmark,size,representation,peak_memory_during_construction,ratio_to_dense";
        assert_eq!(
            lines[0], expected_header,
            "UT-0607-01: header must match SPEC-09 §3.4.5 verbatim. \
             The canonical metric name is `peak_memory_during_construction` per \
             SPEC-09 R18a (line 482, commit `82b2d27`); legacy `peak_construction_bytes` \
             is REJECTED."
        );

        assert_eq!(
            lines[0].matches(',').count() + 1,
            5,
            "UT-0607-01: header must have exactly 5 columns"
        );
    }

    /// UT-0607-01b — Row formatting: well-formed values + blank-string
    /// convention for `None`.
    ///
    /// Two rows: one Dense (peak=Some, ratio=Some(1.0)) and one Sparse
    /// (peak=None, ratio=None) → the empty fields land as `""` (blank),
    /// not `"NaN"`, not `"0"`. Locks the convention from TEST-SPEC-0607
    /// §IT-0607-03.
    #[test]
    fn sparse_csv_row_emits_blank_for_none_values() {
        let rows = vec![
            SparseConstructionRow {
                benchmark: BenchmarkId::DualTree,
                size: 12,
                representation: NetRepresentation::Dense,
                peak_memory_during_construction: Some(123_456_789),
                ratio_to_dense: Some(1.0),
            },
            SparseConstructionRow {
                benchmark: BenchmarkId::DualTree,
                size: 12,
                representation: NetRepresentation::Sparse,
                peak_memory_during_construction: None,
                ratio_to_dense: None,
            },
        ];

        let mut buf = Vec::new();
        write_csv_sparse_construction(&mut buf, &rows).unwrap();
        let csv = String::from_utf8(buf).unwrap();
        let lines: Vec<&str> = csv.lines().collect();

        assert_eq!(lines.len(), 3, "UT-0607-01b: header + 2 rows = 3 lines");
        assert_eq!(
            lines[1], "dual_tree,12,dense,123456789,1.000000",
            "UT-0607-01b: dense row formatting"
        );
        assert_eq!(
            lines[2], "dual_tree,12,sparse,,",
            "UT-0607-01b: sparse row with None values must emit blank fields, not NaN or 0"
        );
    }

    /// UT-0607-01c — `compute_ratios_for_sparse_rows` covers all 4 cases:
    /// dense→1.0, sparse with paired dense, sparse without paired dense,
    /// sparse with dense_peak=0 (div-by-zero guard).
    #[test]
    fn sparse_csv_ratio_computation_covers_all_cases() {
        let mut rows = vec![
            // Pair 1: full sparse + dense pair, ratio = 0.4.
            SparseConstructionRow {
                benchmark: BenchmarkId::DualTree,
                size: 12,
                representation: NetRepresentation::Dense,
                peak_memory_during_construction: Some(1000),
                ratio_to_dense: None,
            },
            SparseConstructionRow {
                benchmark: BenchmarkId::DualTree,
                size: 12,
                representation: NetRepresentation::Sparse,
                peak_memory_during_construction: Some(400),
                ratio_to_dense: None,
            },
            // Pair 2: sparse without dense pair → ratio stays None.
            SparseConstructionRow {
                benchmark: BenchmarkId::DualTree,
                size: 8,
                representation: NetRepresentation::Sparse,
                peak_memory_during_construction: Some(200),
                ratio_to_dense: None,
            },
            // Pair 3: dense_peak = 0 → ratio None (div-by-zero guard).
            SparseConstructionRow {
                benchmark: BenchmarkId::DualTree,
                size: 4,
                representation: NetRepresentation::Dense,
                peak_memory_during_construction: Some(0),
                ratio_to_dense: None,
            },
            SparseConstructionRow {
                benchmark: BenchmarkId::DualTree,
                size: 4,
                representation: NetRepresentation::Sparse,
                peak_memory_during_construction: Some(50),
                ratio_to_dense: None,
            },
        ];

        compute_ratios_for_sparse_rows(&mut rows);

        assert_eq!(rows[0].ratio_to_dense, Some(1.0), "dense row → 1.0");
        assert_eq!(
            rows[1].ratio_to_dense,
            Some(0.4),
            "sparse with paired dense → sparse/dense"
        );
        assert_eq!(rows[2].ratio_to_dense, None, "sparse w/o dense → None");
        assert_eq!(
            rows[3].ratio_to_dense,
            Some(1.0),
            "dense row → 1.0 even when peak is 0"
        );
        assert_eq!(
            rows[4].ratio_to_dense, None,
            "sparse with dense_peak=0 → None (div-by-zero guard)"
        );
    }
}
