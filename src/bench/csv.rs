//! CSV output for benchmark results (SPEC-09 R39-R42).

use super::{AggregatedStats, BenchmarkResult};
use std::io::{self, Write};

/// Write detail CSV: one row per datapoint (SPEC-09 R39a).
pub fn write_csv_detail<W: Write>(writer: &mut W, results: &[BenchmarkResult]) -> io::Result<()> {
    writeln!(
        writer,
        "benchmark,input_size,mode,workers,repetition,correct,wall_clock_secs,\
         total_interactions,mips,rounds,speedup,efficiency,overhead_ratio,\
         peak_memory_bytes,bytes_sent,bytes_received,\
         con_con,dup_dup,era_era,con_dup,con_era,dup_era"
    )?;

    for r in results {
        writeln!(
            writer,
            "{},{},{},{},{},{},{:.6},{},{:.3},{},{:.4},{:.4},{:.4},{},{},{},{},{},{},{},{},{}",
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
        )?;
    }
    Ok(())
}

/// Write summary CSV: one row per configuration (SPEC-09 R39c).
pub fn write_csv_summary<W: Write>(
    writer: &mut W,
    stats: &[AggregatedStats],
) -> io::Result<()> {
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
        }
    }

    #[test]
    fn test_csv_detail_header() {
        let mut buf = Vec::new();
        write_csv_detail(&mut buf, &[]).unwrap();
        let csv = String::from_utf8(buf).unwrap();
        assert!(csv.starts_with("benchmark,input_size,mode,workers"));
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
}
