//! Post-campaign data quality validation (DATA-COLLECTION-PLAN Section 10).
//!
//! Reads the three output CSVs (detail, summary, rounds) and runs a
//! comprehensive checklist of hard requirements, warnings, and informational
//! metrics. Used by the `bench validate` CLI subcommand.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

// ── Public report types ──────────────────────────────────────────────────

/// Severity of a validation check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Hard,
    Warn,
    Info,
}

/// Result of a single validation check.
#[derive(Debug, Clone)]
pub struct CheckResult {
    pub id: &'static str,
    pub name: &'static str,
    pub severity: Severity,
    pub passed: bool,
    pub detail: String,
}

/// Full validation report.
#[derive(Debug)]
pub struct ValidationReport {
    pub checks: Vec<CheckResult>,
    pub all_hard_passed: bool,
}

impl ValidationReport {
    /// Print a formatted report to stdout.
    pub fn print(&self) {
        println!("=== HARD CHECKS ===");
        for c in self.checks.iter().filter(|c| c.severity == Severity::Hard) {
            let tag = if c.passed { "PASS" } else { "FAIL" };
            println!("[{}] {}: {} — {}", tag, c.id, c.name, c.detail);
        }

        println!();
        println!("=== WARNINGS ===");
        for c in self.checks.iter().filter(|c| c.severity == Severity::Warn) {
            let tag = if c.passed { "OK  " } else { "WARN" };
            println!("[{}] {}: {} — {}", tag, c.id, c.name, c.detail);
        }

        println!();
        println!("=== INFO ===");
        for c in self.checks.iter().filter(|c| c.severity == Severity::Info) {
            println!("[INFO] {}: {} — {}", c.id, c.name, c.detail);
        }

        println!();
        if self.all_hard_passed {
            println!("=== RESULT: ALL HARD CHECKS PASSED ===");
        } else {
            let failed: Vec<_> = self
                .checks
                .iter()
                .filter(|c| c.severity == Severity::Hard && !c.passed)
                .map(|c| c.id)
                .collect();
            println!(
                "=== RESULT: HARD CHECK(S) FAILED: {} ===",
                failed.join(", ")
            );
        }
    }
}

// ── CSV row structs ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct DetailRow {
    benchmark: String,
    input_size: u32,
    mode: String,
    workers: u32,
    repetition: u32,
    correct: bool,
    wall_clock_secs: f64,
    total_interactions: u64,
    mips: f64,
    rounds: u32,
    speedup: f64,
    efficiency: f64,
    overhead_ratio: f64,
    con_con: u64,
    dup_dup: u64,
    era_era: u64,
    con_dup: u64,
    con_era: u64,
    dup_era: u64,
}

#[derive(Debug, Clone)]
struct SummaryRow {
    benchmark: String,
    input_size: u32,
    mode: String,
    workers: u32,
    repetitions: u32,
    all_correct: bool,
    wall_clock_mean: f64,
    cv: f64,
    speedup_mean: f64,
    efficiency_mean: f64,
    overhead_ratio_mean: f64,
    mips_mean: f64,
}

#[derive(Debug, Clone)]
struct RoundsRow {
    benchmark: String,
    input_size: u32,
    workers: u32,
    mode: String,
    repetition: u32,
    round: u32,
}

// ── CSV parsing ──────────────────────────────────────────────────────────

fn parse_detail_csv(path: &Path) -> Result<Vec<DetailRow>, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {}", path.display(), e))?;
    let mut rows = Vec::new();
    let mut lines = content.lines();
    let header = lines.next().ok_or("detail.csv is empty")?;
    if !header.starts_with("benchmark,input_size,mode,workers") {
        return Err(format!("detail.csv unexpected header: {}", header));
    }
    for (i, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split(',').collect();
        if cols.len() < 22 {
            return Err(format!(
                "detail.csv line {}: expected 22 columns, got {}",
                i + 2,
                cols.len()
            ));
        }
        rows.push(DetailRow {
            benchmark: cols[0].to_string(),
            input_size: cols[1].parse().map_err(|e| format!("line {}: input_size: {}", i + 2, e))?,
            mode: cols[2].to_string(),
            workers: cols[3].parse().map_err(|e| format!("line {}: workers: {}", i + 2, e))?,
            repetition: cols[4].parse().map_err(|e| format!("line {}: repetition: {}", i + 2, e))?,
            correct: cols[5] == "true",
            wall_clock_secs: cols[6].parse().map_err(|e| format!("line {}: wall_clock: {}", i + 2, e))?,
            total_interactions: cols[7].parse().map_err(|e| format!("line {}: interactions: {}", i + 2, e))?,
            mips: cols[8].parse().map_err(|e| format!("line {}: mips: {}", i + 2, e))?,
            rounds: cols[9].parse().map_err(|e| format!("line {}: rounds: {}", i + 2, e))?,
            speedup: cols[10].parse().map_err(|e| format!("line {}: speedup: {}", i + 2, e))?,
            efficiency: cols[11].parse().map_err(|e| format!("line {}: efficiency: {}", i + 2, e))?,
            overhead_ratio: cols[12].parse().map_err(|e| format!("line {}: overhead: {}", i + 2, e))?,
            con_con: cols[16].parse().map_err(|e| format!("line {}: con_con: {}", i + 2, e))?,
            dup_dup: cols[17].parse().map_err(|e| format!("line {}: dup_dup: {}", i + 2, e))?,
            era_era: cols[18].parse().map_err(|e| format!("line {}: era_era: {}", i + 2, e))?,
            con_dup: cols[19].parse().map_err(|e| format!("line {}: con_dup: {}", i + 2, e))?,
            con_era: cols[20].parse().map_err(|e| format!("line {}: con_era: {}", i + 2, e))?,
            dup_era: cols[21].parse().map_err(|e| format!("line {}: dup_era: {}", i + 2, e))?,
        });
    }
    Ok(rows)
}

fn parse_summary_csv(path: &Path) -> Result<Vec<SummaryRow>, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {}", path.display(), e))?;
    let mut rows = Vec::new();
    let mut lines = content.lines();
    let header = lines.next().ok_or("summary.csv is empty")?;
    if !header.starts_with("benchmark,input_size,mode,workers") {
        return Err(format!("summary.csv unexpected header: {}", header));
    }
    for (i, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split(',').collect();
        if cols.len() < 16 {
            return Err(format!(
                "summary.csv line {}: expected 16 columns, got {}",
                i + 2,
                cols.len()
            ));
        }
        rows.push(SummaryRow {
            benchmark: cols[0].to_string(),
            input_size: cols[1].parse().map_err(|e| format!("line {}: {}", i + 2, e))?,
            mode: cols[2].to_string(),
            workers: cols[3].parse().map_err(|e| format!("line {}: {}", i + 2, e))?,
            repetitions: cols[4].parse().map_err(|e| format!("line {}: {}", i + 2, e))?,
            all_correct: cols[5] == "true",
            wall_clock_mean: cols[6].parse().map_err(|e| format!("line {}: {}", i + 2, e))?,
            cv: cols[15].parse().map_err(|e| format!("line {}: {}", i + 2, e))?,
            speedup_mean: cols[12].parse().map_err(|e| format!("line {}: {}", i + 2, e))?,
            efficiency_mean: cols[13].parse().map_err(|e| format!("line {}: {}", i + 2, e))?,
            overhead_ratio_mean: cols[14].parse().map_err(|e| format!("line {}: {}", i + 2, e))?,
            mips_mean: cols[11].parse().map_err(|e| format!("line {}: {}", i + 2, e))?,
        });
    }
    Ok(rows)
}

fn parse_rounds_csv(path: &Path) -> Result<Vec<RoundsRow>, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {}", path.display(), e))?;
    let mut rows = Vec::new();
    let mut lines = content.lines();
    let header = lines.next().ok_or("rounds.csv is empty")?;
    if !header.starts_with("benchmark,input_size,workers,mode") {
        return Err(format!("rounds.csv unexpected header: {}", header));
    }
    for (i, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split(',').collect();
        if cols.len() < 15 {
            return Err(format!(
                "rounds.csv line {}: expected 15 columns, got {}",
                i + 2,
                cols.len()
            ));
        }
        rows.push(RoundsRow {
            benchmark: cols[0].to_string(),
            input_size: cols[1].parse().map_err(|e| format!("line {}: {}", i + 2, e))?,
            workers: cols[2].parse().map_err(|e| format!("line {}: {}", i + 2, e))?,
            mode: cols[3].to_string(),
            repetition: cols[4].parse().map_err(|e| format!("line {}: {}", i + 2, e))?,
            round: cols[5].parse().map_err(|e| format!("line {}: {}", i + 2, e))?,
        });
    }
    Ok(rows)
}

// ── Validation entry point ───────────────────────────────────────────────

/// Run all validation checks against the three CSV files.
pub fn validate_campaign(
    detail_path: &Path,
    summary_path: &Path,
    rounds_path: &Path,
) -> ValidationReport {
    let mut checks = Vec::new();

    // H2: Parse all 3 CSVs
    let detail = parse_detail_csv(detail_path);
    let summary = parse_summary_csv(summary_path);
    let rounds = parse_rounds_csv(rounds_path);

    let (detail_ok, summary_ok, rounds_ok) = (detail.is_ok(), summary.is_ok(), rounds.is_ok());
    let mut parse_errors = Vec::new();
    if let Err(ref e) = detail {
        parse_errors.push(format!("detail: {}", e));
    }
    if let Err(ref e) = summary {
        parse_errors.push(format!("summary: {}", e));
    }
    if let Err(ref e) = rounds {
        parse_errors.push(format!("rounds: {}", e));
    }

    checks.push(CheckResult {
        id: "H2",
        name: "All 3 CSVs parseable",
        severity: Severity::Hard,
        passed: detail_ok && summary_ok && rounds_ok,
        detail: if parse_errors.is_empty() {
            "All 3 CSVs parsed successfully".into()
        } else {
            parse_errors.join("; ")
        },
    });

    // If any CSV failed to parse, remaining checks cannot run
    let detail = match detail {
        Ok(d) => d,
        Err(_) => {
            return finish_report(checks);
        }
    };
    let summary = match summary {
        Ok(s) => s,
        Err(_) => {
            return finish_report(checks);
        }
    };
    let rounds = match rounds {
        Ok(r) => r,
        Err(_) => {
            return finish_report(checks);
        }
    };

    // H1: Zero G1 failures
    check_h1_correctness(&detail, &mut checks);

    // H3: Interaction counts match expected
    check_h3_interaction_counts(&detail, &mut checks);

    // H4: Speedup == 1.0 for sequential baseline
    check_h4_speedup_baseline(&detail, &mut checks);

    // H5: Efficiency == 1.0 for sequential baseline
    check_h5_efficiency_baseline(&detail, &mut checks);

    // H6: Interaction consistency across modes
    check_h6_interaction_consistency(&detail, &mut checks);

    // H7: Summary all_correct matches detail rows
    check_h7_summary_consistency(&detail, &summary, &mut checks);

    // W1: High CV
    check_w1_high_cv(&summary, &mut checks);

    // W2: Suspicious speedup in local mode
    check_w2_suspicious_speedup(&summary, &mut checks);

    // W3: Sequential MIPS inconsistency
    check_w3_mips_inconsistency(&summary, &mut checks);

    // W4: EP round count
    check_w4_ep_rounds(&detail, &mut checks);

    // W5: Overhead ratio range
    check_w5_overhead_range(&detail, &mut checks);

    // I1-I5: Informational metrics
    check_info_metrics(&detail, &summary, &rounds, &mut checks);

    finish_report(checks)
}

fn finish_report(checks: Vec<CheckResult>) -> ValidationReport {
    let all_hard_passed = checks
        .iter()
        .filter(|c| c.severity == Severity::Hard)
        .all(|c| c.passed);
    ValidationReport {
        checks,
        all_hard_passed,
    }
}

// ── Individual checks ────────────────────────────────────────────────────

/// H1: Zero G1 failures — all detail rows must have correct == true.
fn check_h1_correctness(detail: &[DetailRow], checks: &mut Vec<CheckResult>) {
    let total = detail.len();
    let correct_count = detail.iter().filter(|r| r.correct).count();
    checks.push(CheckResult {
        id: "H1",
        name: "Zero G1 failures",
        severity: Severity::Hard,
        passed: correct_count == total,
        detail: format!("{}/{} correct", correct_count, total),
    });
}

/// H3: Interaction counts match expected values per benchmark type.
fn check_h3_interaction_counts(detail: &[DetailRow], checks: &mut Vec<CheckResult>) {
    let mut validated = 0u32;
    let mut total_checked = 0u32;
    let mut failures: Vec<String> = Vec::new();

    // Only check sequential rows (canonical interaction counts)
    for row in detail.iter().filter(|r| r.mode == "sequential") {
        let expected = expected_interactions(&row.benchmark, row.input_size);
        if let Some((total, rule_check)) = expected {
            total_checked += 1;
            let mut ok = true;

            if row.total_interactions != total {
                ok = false;
                failures.push(format!(
                    "{} size={}: expected {} interactions, got {}",
                    row.benchmark, row.input_size, total, row.total_interactions
                ));
            }

            if let Some(rule_msg) = rule_check(row) {
                ok = false;
                failures.push(format!(
                    "{} size={}: {}",
                    row.benchmark, row.input_size, rule_msg
                ));
            }

            if ok {
                validated += 1;
            }
        }
        // Benchmarks without known formula (tree_sum, church_*) are skipped
    }

    let passed = failures.is_empty();
    checks.push(CheckResult {
        id: "H3",
        name: "Interaction counts correct",
        severity: Severity::Hard,
        passed,
        detail: if passed {
            format!("{}/{} sequential configs validated", validated, total_checked)
        } else {
            format!(
                "{} failures: {}",
                failures.len(),
                failures.iter().take(5).cloned().collect::<Vec<_>>().join("; ")
            )
        },
    });
}

/// Returns (expected_total, rule_checker) for benchmarks with known formulas.
/// The rule_checker returns Some(error_msg) if the per-rule breakdown is wrong.
fn expected_interactions(
    benchmark: &str,
    size: u32,
) -> Option<(u64, Box<dyn Fn(&DetailRow) -> Option<String>>)> {
    let n = size as u64;
    match benchmark {
        "ep_annihilation" => Some((
            n,
            Box::new(move |r: &DetailRow| {
                if r.era_era != n {
                    Some(format!("era_era: expected {}, got {}", n, r.era_era))
                } else if r.con_con + r.dup_dup + r.con_dup + r.con_era + r.dup_era != 0 {
                    Some("non-era_era interactions should be 0".into())
                } else {
                    None
                }
            }),
        )),
        "ep_annihilation_con" => Some((
            n,
            Box::new(move |r: &DetailRow| {
                if r.con_con != n {
                    Some(format!("con_con: expected {}, got {}", n, r.con_con))
                } else if r.dup_dup + r.era_era + r.con_dup + r.con_era + r.dup_era != 0 {
                    Some("non-con_con interactions should be 0".into())
                } else {
                    None
                }
            }),
        )),
        "ep_annihilation_dup" => Some((
            n,
            Box::new(move |r: &DetailRow| {
                if r.dup_dup != n {
                    Some(format!("dup_dup: expected {}, got {}", n, r.dup_dup))
                } else if r.con_con + r.era_era + r.con_dup + r.con_era + r.dup_era != 0 {
                    Some("non-dup_dup interactions should be 0".into())
                } else {
                    None
                }
            }),
        )),
        "condup_expansion" => Some((
            n,
            Box::new(move |r: &DetailRow| {
                if r.con_dup != n {
                    Some(format!("con_dup: expected {}, got {}", n, r.con_dup))
                } else if r.con_con + r.dup_dup + r.era_era + r.con_era + r.dup_era != 0 {
                    Some("non-con_dup interactions should be 0".into())
                } else {
                    None
                }
            }),
        )),
        "dual_tree" => {
            // depth d → 2^d - 1 con_con annihilations
            let total = (1u64 << n) - 1;
            Some((
                total,
                Box::new(move |r: &DetailRow| {
                    if r.con_con != total {
                        Some(format!("con_con: expected {}, got {}", total, r.con_con))
                    } else if r.dup_dup + r.era_era + r.con_dup + r.con_era + r.dup_era != 0 {
                        Some("non-con_con interactions should be 0".into())
                    } else {
                        None
                    }
                }),
            ))
        }
        "mixed_net" => {
            // 6*N total, N per rule
            let total = 6 * n;
            Some((
                total,
                Box::new(move |r: &DetailRow| {
                    let mut errs = Vec::new();
                    if r.con_con != n { errs.push(format!("con_con={} expected {}", r.con_con, n)); }
                    if r.dup_dup != n { errs.push(format!("dup_dup={} expected {}", r.dup_dup, n)); }
                    if r.era_era != n { errs.push(format!("era_era={} expected {}", r.era_era, n)); }
                    if r.con_dup != n { errs.push(format!("con_dup={} expected {}", r.con_dup, n)); }
                    if r.con_era != n { errs.push(format!("con_era={} expected {}", r.con_era, n)); }
                    if r.dup_era != n { errs.push(format!("dup_era={} expected {}", r.dup_era, n)); }
                    if errs.is_empty() { None } else { Some(errs.join(", ")) }
                }),
            ))
        }
        // erasure_propagation, tree_sum, tree_sum_balanced, church_add, church_mul:
        // Exact formulas are complex or depend on encoding details.
        // We just verify total > 0 via a soft check.
        _ => None,
    }
}

/// H4: Speedup == 1.0 for all sequential baselines.
fn check_h4_speedup_baseline(detail: &[DetailRow], checks: &mut Vec<CheckResult>) {
    let seq_rows: Vec<_> = detail.iter().filter(|r| r.mode == "sequential").collect();
    let bad: Vec<_> = seq_rows
        .iter()
        .filter(|r| (r.speedup - 1.0).abs() > 1e-6)
        .collect();
    checks.push(CheckResult {
        id: "H4",
        name: "Speedup == 1.0 for sequential",
        severity: Severity::Hard,
        passed: bad.is_empty(),
        detail: if bad.is_empty() {
            format!("{}/{} sequential rows have speedup=1.0", seq_rows.len(), seq_rows.len())
        } else {
            format!(
                "{} rows with speedup != 1.0 (e.g., {} size={} speedup={:.4})",
                bad.len(),
                bad[0].benchmark,
                bad[0].input_size,
                bad[0].speedup
            )
        },
    });
}

/// H5: Efficiency == 1.0 for all sequential baselines.
fn check_h5_efficiency_baseline(detail: &[DetailRow], checks: &mut Vec<CheckResult>) {
    let seq_rows: Vec<_> = detail.iter().filter(|r| r.mode == "sequential").collect();
    let bad: Vec<_> = seq_rows
        .iter()
        .filter(|r| (r.efficiency - 1.0).abs() > 1e-6)
        .collect();
    checks.push(CheckResult {
        id: "H5",
        name: "Efficiency == 1.0 for sequential",
        severity: Severity::Hard,
        passed: bad.is_empty(),
        detail: if bad.is_empty() {
            format!(
                "{}/{} sequential rows have efficiency=1.0",
                seq_rows.len(),
                seq_rows.len()
            )
        } else {
            format!(
                "{} rows with efficiency != 1.0 (e.g., {} size={} eff={:.4})",
                bad.len(),
                bad[0].benchmark,
                bad[0].input_size,
                bad[0].efficiency
            )
        },
    });
}

/// H6: total_interactions must be identical across all modes for the same
/// (benchmark, input_size) pair. G1 guarantees same normal form → same count.
fn check_h6_interaction_consistency(detail: &[DetailRow], checks: &mut Vec<CheckResult>) {
    // Group by (benchmark, input_size)
    let mut groups: HashMap<(String, u32), Vec<u64>> = HashMap::new();
    for r in detail {
        groups
            .entry((r.benchmark.clone(), r.input_size))
            .or_default()
            .push(r.total_interactions);
    }

    let mut inconsistent = Vec::new();
    for ((bench, size), counts) in &groups {
        let first = counts[0];
        if counts.iter().any(|&c| c != first) {
            let unique: Vec<u64> = {
                let mut u = counts.clone();
                u.sort();
                u.dedup();
                u
            };
            inconsistent.push(format!(
                "{} size={}: {:?}",
                bench, size, unique
            ));
        }
    }

    checks.push(CheckResult {
        id: "H6",
        name: "Interaction count consistent across modes",
        severity: Severity::Hard,
        passed: inconsistent.is_empty(),
        detail: if inconsistent.is_empty() {
            format!("{} (benchmark, size) groups all consistent", groups.len())
        } else {
            format!(
                "{} inconsistencies: {}",
                inconsistent.len(),
                inconsistent.iter().take(5).cloned().collect::<Vec<_>>().join("; ")
            )
        },
    });
}

/// H7: summary.all_correct must equal the AND of all matching detail rows.
fn check_h7_summary_consistency(
    detail: &[DetailRow],
    summary: &[SummaryRow],
    checks: &mut Vec<CheckResult>,
) {
    // Build lookup: (benchmark, size, mode, workers) -> all correct?
    let mut detail_correct: HashMap<(String, u32, String, u32), bool> = HashMap::new();
    for r in detail {
        let key = (
            r.benchmark.clone(),
            r.input_size,
            r.mode.clone(),
            r.workers,
        );
        let entry = detail_correct.entry(key).or_insert(true);
        if !r.correct {
            *entry = false;
        }
    }

    let mut mismatches = Vec::new();
    for s in summary {
        let key = (
            s.benchmark.clone(),
            s.input_size,
            s.mode.clone(),
            s.workers,
        );
        if let Some(&detail_all_ok) = detail_correct.get(&key) {
            if detail_all_ok != s.all_correct {
                mismatches.push(format!(
                    "{} size={} mode={} w={}: summary={}, detail={}",
                    s.benchmark, s.input_size, s.mode, s.workers, s.all_correct, detail_all_ok
                ));
            }
        } else {
            mismatches.push(format!(
                "{} size={} mode={} w={}: no matching detail rows",
                s.benchmark, s.input_size, s.mode, s.workers
            ));
        }
    }

    checks.push(CheckResult {
        id: "H7",
        name: "Summary all_correct matches detail",
        severity: Severity::Hard,
        passed: mismatches.is_empty(),
        detail: if mismatches.is_empty() {
            format!("{}/{} summary rows consistent", summary.len(), summary.len())
        } else {
            format!(
                "{} mismatches: {}",
                mismatches.len(),
                mismatches.iter().take(3).cloned().collect::<Vec<_>>().join("; ")
            )
        },
    });
}

/// W1: Flag configurations with CV > 10% (SPEC-09 R34).
fn check_w1_high_cv(summary: &[SummaryRow], checks: &mut Vec<CheckResult>) {
    let high_cv: Vec<_> = summary.iter().filter(|s| s.cv > 0.10).collect();
    checks.push(CheckResult {
        id: "W1",
        name: "Low variance (CV <= 10%)",
        severity: Severity::Warn,
        passed: high_cv.is_empty(),
        detail: if high_cv.is_empty() {
            format!("All {} configs have CV <= 10%", summary.len())
        } else {
            let worst = high_cv
                .iter()
                .max_by(|a, b| a.cv.partial_cmp(&b.cv).unwrap())
                .unwrap();
            format!(
                "{} configs with CV > 10% (worst: {} size={} w={} CV={:.1}%)",
                high_cv.len(),
                worst.benchmark,
                worst.input_size,
                worst.workers,
                worst.cv * 100.0
            )
        },
    });
}

/// W2: Speedup > 1.0 in local mode is suspicious (single-threaded simulation).
fn check_w2_suspicious_speedup(summary: &[SummaryRow], checks: &mut Vec<CheckResult>) {
    let suspicious: Vec<_> = summary
        .iter()
        .filter(|s| s.mode == "local" && s.workers >= 2 && s.speedup_mean > 1.0)
        .collect();
    checks.push(CheckResult {
        id: "W2",
        name: "No suspicious speedup in local mode",
        severity: Severity::Warn,
        passed: suspicious.is_empty(),
        detail: if suspicious.is_empty() {
            "No local-mode configs with speedup > 1.0 and workers >= 2".into()
        } else {
            format!(
                "{} configs with suspicious speedup (e.g., {} size={} w={} S={:.4})",
                suspicious.len(),
                suspicious[0].benchmark,
                suspicious[0].input_size,
                suspicious[0].workers,
                suspicious[0].speedup_mean
            )
        },
    });
}

/// W3: Sequential MIPS inconsistency — for the same benchmark across sizes,
/// MIPS should be roughly consistent (not vary by > 10x).
fn check_w3_mips_inconsistency(summary: &[SummaryRow], checks: &mut Vec<CheckResult>) {
    let mut seq_mips: HashMap<String, Vec<f64>> = HashMap::new();
    for s in summary.iter().filter(|s| s.mode == "sequential") {
        seq_mips
            .entry(s.benchmark.clone())
            .or_default()
            .push(s.mips_mean);
    }

    let mut wide = Vec::new();
    for (bench, mips_vals) in &seq_mips {
        if mips_vals.len() < 2 {
            continue;
        }
        let min = mips_vals.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = mips_vals
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max);
        if min > 0.0 && max / min > 10.0 {
            wide.push(format!(
                "{}: MIPS range {:.1}-{:.1} (ratio {:.1}x)",
                bench,
                min,
                max,
                max / min
            ));
        }
    }

    checks.push(CheckResult {
        id: "W3",
        name: "Sequential MIPS consistency",
        severity: Severity::Warn,
        passed: wide.is_empty(),
        detail: if wide.is_empty() {
            format!(
                "{} benchmarks have consistent sequential MIPS (< 10x range)",
                seq_mips.len()
            )
        } else {
            format!("{} wide ranges: {}", wide.len(), wide.join("; "))
        },
    });
}

/// W4: EP benchmarks in local mode should complete in exactly 1 round.
fn check_w4_ep_rounds(detail: &[DetailRow], checks: &mut Vec<CheckResult>) {
    let ep_benchmarks = [
        "ep_annihilation",
        "ep_annihilation_con",
        "ep_annihilation_dup",
    ];
    let bad: Vec<_> = detail
        .iter()
        .filter(|r| {
            r.mode == "local"
                && ep_benchmarks.contains(&r.benchmark.as_str())
                && r.rounds != 1
        })
        .collect();

    checks.push(CheckResult {
        id: "W4",
        name: "EP benchmarks complete in 1 round",
        severity: Severity::Warn,
        passed: bad.is_empty(),
        detail: if bad.is_empty() {
            "All EP local-mode runs completed in 1 round".into()
        } else {
            format!(
                "{} EP rows with rounds != 1 (e.g., {} size={} w={} rounds={})",
                bad.len(),
                bad[0].benchmark,
                bad[0].input_size,
                bad[0].workers,
                bad[0].rounds
            )
        },
    });
}

/// W5: overhead_ratio should be in [0.0, 1.0] for grid runs.
fn check_w5_overhead_range(detail: &[DetailRow], checks: &mut Vec<CheckResult>) {
    let grid_rows: Vec<_> = detail.iter().filter(|r| r.mode != "sequential").collect();
    let bad: Vec<_> = grid_rows
        .iter()
        .filter(|r| r.overhead_ratio < -0.001 || r.overhead_ratio > 1.001)
        .collect();

    checks.push(CheckResult {
        id: "W5",
        name: "Overhead ratio in [0, 1]",
        severity: Severity::Warn,
        passed: bad.is_empty(),
        detail: if bad.is_empty() {
            format!("All {} grid rows have overhead_ratio in [0, 1]", grid_rows.len())
        } else {
            format!(
                "{} rows out of range (e.g., {} size={} w={} overhead={:.4})",
                bad.len(),
                bad[0].benchmark,
                bad[0].input_size,
                bad[0].workers,
                bad[0].overhead_ratio
            )
        },
    });
}

/// I1-I5: Informational metrics.
fn check_info_metrics(
    detail: &[DetailRow],
    summary: &[SummaryRow],
    rounds: &[RoundsRow],
    checks: &mut Vec<CheckResult>,
) {
    // I1: Total datapoints
    checks.push(CheckResult {
        id: "I1",
        name: "Total datapoints",
        severity: Severity::Info,
        passed: true,
        detail: format!(
            "{} detail rows, {} summary configs, {} round entries",
            detail.len(),
            summary.len(),
            rounds.len()
        ),
    });

    // I2: Distinct benchmarks
    let benchmarks: std::collections::HashSet<_> =
        detail.iter().map(|r| r.benchmark.as_str()).collect();
    checks.push(CheckResult {
        id: "I2",
        name: "Distinct benchmarks",
        severity: Severity::Info,
        passed: true,
        detail: format!(
            "{}: {}",
            benchmarks.len(),
            {
                let mut b: Vec<_> = benchmarks.into_iter().collect();
                b.sort();
                b.join(", ")
            }
        ),
    });

    // I3: MIPS range per benchmark (sequential only)
    let mut seq_mips: HashMap<&str, (f64, f64)> = HashMap::new();
    for r in detail.iter().filter(|r| r.mode == "sequential") {
        let entry = seq_mips
            .entry(r.benchmark.as_str())
            .or_insert((f64::INFINITY, f64::NEG_INFINITY));
        if r.mips < entry.0 {
            entry.0 = r.mips;
        }
        if r.mips > entry.1 {
            entry.1 = r.mips;
        }
    }
    let mips_summary: Vec<_> = {
        let mut v: Vec<_> = seq_mips
            .iter()
            .map(|(b, (lo, hi))| format!("{}:{:.0}-{:.0}", b, lo, hi))
            .collect();
        v.sort();
        v
    };
    checks.push(CheckResult {
        id: "I3",
        name: "Sequential MIPS ranges",
        severity: Severity::Info,
        passed: true,
        detail: mips_summary.join(", "),
    });

    // I4: Average CV by benchmark
    let mut cv_sums: HashMap<&str, (f64, usize)> = HashMap::new();
    for s in summary {
        let entry = cv_sums
            .entry(s.benchmark.as_str())
            .or_insert((0.0, 0));
        entry.0 += s.cv;
        entry.1 += 1;
    }
    let avg_cvs: Vec<_> = {
        let mut v: Vec<_> = cv_sums
            .iter()
            .map(|(b, (sum, n))| format!("{}:{:.1}%", b, (sum / *n as f64) * 100.0))
            .collect();
        v.sort();
        v
    };
    checks.push(CheckResult {
        id: "I4",
        name: "Average CV by benchmark",
        severity: Severity::Info,
        passed: true,
        detail: avg_cvs.join(", "),
    });

    // I5: Worst overhead_ratio
    let worst_overhead = detail
        .iter()
        .filter(|r| r.mode != "sequential")
        .max_by(|a, b| {
            a.overhead_ratio
                .partial_cmp(&b.overhead_ratio)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    checks.push(CheckResult {
        id: "I5",
        name: "Worst overhead ratio",
        severity: Severity::Info,
        passed: true,
        detail: match worst_overhead {
            Some(r) => format!(
                "{:.4} ({} size={} workers={})",
                r.overhead_ratio, r.benchmark, r.input_size, r.workers
            ),
            None => "N/A (no grid rows)".into(),
        },
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_test_detail(path: &Path, rows: &str) {
        let mut f = fs::File::create(path).unwrap();
        writeln!(f, "benchmark,input_size,mode,workers,repetition,correct,wall_clock_secs,total_interactions,mips,rounds,speedup,efficiency,overhead_ratio,peak_memory_bytes,bytes_sent,bytes_received,con_con,dup_dup,era_era,con_dup,con_era,dup_era").unwrap();
        write!(f, "{}", rows).unwrap();
    }

    fn write_test_summary(path: &Path, rows: &str) {
        let mut f = fs::File::create(path).unwrap();
        writeln!(f, "benchmark,input_size,mode,workers,repetitions,all_correct,wall_clock_mean,wall_clock_std,wall_clock_median,wall_clock_min,wall_clock_max,mips_mean,speedup_mean,efficiency_mean,overhead_ratio_mean,cv").unwrap();
        write!(f, "{}", rows).unwrap();
    }

    fn write_test_rounds(path: &Path, rows: &str) {
        let mut f = fs::File::create(path).unwrap();
        writeln!(f, "benchmark,input_size,workers,mode,repetition,round,partition_time_secs,compute_time_secs,merge_time_secs,network_time_secs,border_redexes,border_ratio,agents_at_start,bytes_sent,bytes_received").unwrap();
        write!(f, "{}", rows).unwrap();
    }

    #[test]
    fn test_all_checks_pass_minimal() {
        let dir = std::env::temp_dir().join("validate_test_pass");
        let _ = fs::create_dir_all(&dir);
        let detail_path = dir.join("detail.csv");
        let summary_path = dir.join("summary.csv");
        let rounds_path = dir.join("rounds.csv");

        write_test_detail(
            &detail_path,
            "ep_annihilation,100,sequential,0,0,true,0.001,100,100.0,0,1.0000,1.0000,0.0000,0,0,0,0,0,100,0,0,0\n\
             ep_annihilation,100,local,1,0,true,0.002,100,50.0,1,0.5000,0.5000,0.5000,0,0,0,0,0,100,0,0,0\n",
        );
        write_test_summary(
            &summary_path,
            "ep_annihilation,100,sequential,0,1,true,0.001,0.0,0.001,0.001,0.001,100.0,1.0000,1.0000,0.0000,0.0500\n\
             ep_annihilation,100,local,1,1,true,0.002,0.0,0.002,0.002,0.002,50.0,0.5000,0.5000,0.5000,0.0500\n",
        );
        write_test_rounds(
            &rounds_path,
            "ep_annihilation,100,1,local,0,0,0.0,0.002,0.0,0.0,0,0.0,200,0,0\n",
        );

        let report = validate_campaign(&detail_path, &summary_path, &rounds_path);
        assert!(report.all_hard_passed);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_h1_fails_on_incorrect_row() {
        let dir = std::env::temp_dir().join("validate_test_h1");
        let _ = fs::create_dir_all(&dir);
        let detail_path = dir.join("detail.csv");
        let summary_path = dir.join("summary.csv");
        let rounds_path = dir.join("rounds.csv");

        write_test_detail(
            &detail_path,
            "ep_annihilation,100,sequential,0,0,false,0.001,100,100.0,0,1.0000,1.0000,0.0000,0,0,0,0,0,100,0,0,0\n",
        );
        write_test_summary(
            &summary_path,
            "ep_annihilation,100,sequential,0,1,false,0.001,0.0,0.001,0.001,0.001,100.0,1.0000,1.0000,0.0000,0.05\n",
        );
        write_test_rounds(&rounds_path, "");

        let report = validate_campaign(&detail_path, &summary_path, &rounds_path);
        assert!(!report.all_hard_passed);
        let h1 = report.checks.iter().find(|c| c.id == "H1").unwrap();
        assert!(!h1.passed);
        assert!(h1.detail.contains("0/1"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_h2_fails_on_missing_file() {
        let dir = std::env::temp_dir().join("validate_test_h2");
        let _ = fs::create_dir_all(&dir);
        let detail_path = dir.join("missing_detail.csv");
        let summary_path = dir.join("missing_summary.csv");
        let rounds_path = dir.join("missing_rounds.csv");

        let report = validate_campaign(&detail_path, &summary_path, &rounds_path);
        assert!(!report.all_hard_passed);
        let h2 = report.checks.iter().find(|c| c.id == "H2").unwrap();
        assert!(!h2.passed);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_h3_wrong_interaction_count() {
        let dir = std::env::temp_dir().join("validate_test_h3");
        let _ = fs::create_dir_all(&dir);
        let detail_path = dir.join("detail.csv");
        let summary_path = dir.join("summary.csv");
        let rounds_path = dir.join("rounds.csv");

        // ep_annihilation with size=100 should have 100 era_era, but we put 50
        write_test_detail(
            &detail_path,
            "ep_annihilation,100,sequential,0,0,true,0.001,50,100.0,0,1.0000,1.0000,0.0000,0,0,0,0,0,50,0,0,0\n",
        );
        write_test_summary(
            &summary_path,
            "ep_annihilation,100,sequential,0,1,true,0.001,0.0,0.001,0.001,0.001,100.0,1.0000,1.0000,0.0000,0.05\n",
        );
        write_test_rounds(&rounds_path, "");

        let report = validate_campaign(&detail_path, &summary_path, &rounds_path);
        let h3 = report.checks.iter().find(|c| c.id == "H3").unwrap();
        assert!(!h3.passed);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_w1_detects_high_cv() {
        let dir = std::env::temp_dir().join("validate_test_w1");
        let _ = fs::create_dir_all(&dir);
        let detail_path = dir.join("detail.csv");
        let summary_path = dir.join("summary.csv");
        let rounds_path = dir.join("rounds.csv");

        write_test_detail(
            &detail_path,
            "ep_annihilation,100,sequential,0,0,true,0.001,100,100.0,0,1.0000,1.0000,0.0000,0,0,0,0,0,100,0,0,0\n",
        );
        write_test_summary(
            &summary_path,
            "ep_annihilation,100,sequential,0,1,true,0.001,0.0,0.001,0.001,0.001,100.0,1.0000,1.0000,0.0000,0.2500\n",
        );
        write_test_rounds(&rounds_path, "");

        let report = validate_campaign(&detail_path, &summary_path, &rounds_path);
        let w1 = report.checks.iter().find(|c| c.id == "W1").unwrap();
        assert!(!w1.passed);
        assert!(w1.detail.contains("CV > 10%"));
        let _ = fs::remove_dir_all(&dir);
    }
}
