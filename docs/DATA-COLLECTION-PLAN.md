# DATA-COLLECTION-PLAN.md

**Version:** 1.1
**Date:** 2026-04-09
**Author:** Spec-Driven Development pipeline
**Status:** Active
**Cross-references:** SPEC-09 (Benchmarks), SPEC-08 (Test Strategy), SPEC-01 (Invariants, G1), ARG-001 (P1-P6), Section 4.6 and Section 5 of the TCC article

---

## 1. Purpose and Scope

This document bridges SPEC-09 (what to measure and how) with Section 5 of the TCC article (how to present the results). It resolves ambiguities between the article text (Section 4.6, which describes the experimental campaign at a high level) and the spec (which defines the benchmark framework, metrics, and output format in detail). A developer reading this plan should know exactly:

1. What configurations to run (Section 3, Experimental Matrix).
2. What data files to produce and their exact schemas (Section 4, CSV Schemas).
3. What figures and tables Section 5 requires and which data feeds them (Section 5, Figure and Table Plan).
4. How to compute the statistics (Section 6, Statistical Analysis Plan).
5. How to compare with the Haskell prototype (Section 7, Haskell Comparison Methodology).
6. How to set up the environment (Section 8, Hardware and Environment Requirements).
7. The step-by-step execution protocol (Section 9, Execution Protocol).
8. The pass/fail criteria for the campaign (Section 10, Success Criteria).

This plan is NOT a spec. It does not introduce new requirements or override SPEC-09. Where SPEC-09 leaves a choice (e.g., default repetitions = 5 vs. publishable = 30), this plan makes a concrete decision for the TCC campaign.

---

## 2. Resolution of Ambiguities

The following conflicts or underspecified areas between the article text (Section 4.6) and SPEC-09 are resolved here.

### 2.1 Repetition Count

- **Article (Section 4.6):** "median of 10 repetitions after 2 warmup rounds, reporting 95% confidence intervals."
- **SPEC-09 (R30-R31):** Default 5 repetitions. SHOULD 30 for publishable results.
- **Resolution:** The TCC campaign uses **10 repetitions + 2 warmup** as stated in the article. SPEC-09's default of 5 is for development/debugging runs only. The value 10 is a pragmatic middle ground: sufficient for bootstrap CI computation, tractable in execution time (~1,500 configurations x 12 runs each = ~18,000 total executions including warmup).

### 2.2 Per-Round Vectors in CSV

- **SPEC-09 (R18):** `BenchmarkResult` contains `Vec<f64>` fields for per-round data (e.g., `partition_time_per_round`, `border_redexes_per_round`).
- **Problem:** CSV columns cannot natively hold variable-length vectors. JSON arrays in CSV columns complicate parsing in pandas/R.
- **Resolution:** Use **3 separate CSV files** (detail, rounds, summary) instead of embedding vectors. The `detail.csv` contains scalar aggregates per execution. The `rounds.csv` contains one row per round per execution, enabling per-round analysis without JSON-in-CSV. The `summary.csv` contains pre-aggregated statistics per configuration. This is cleaner than SPEC-09's single-file approach and fully compatible with `pandas.read_csv()`.

### 2.3 Haskell Comparison

- **SPEC-09 (R43-R45):** Comparison on shared benchmarks with shared sizes.
- **Article (Section 4.6):** Does not specify how to present the comparison.
- **Resolution:** Comparison is **qualitative only**. Compare trends (speedup direction, break-even behavior, overhead profile shape), NOT absolute times. Haskell is interpreted with lazy evaluation and O(N^2) `findRedexes`; Rust is compiled with an O(1) incremental redex queue. Absolute time comparison is scientifically meaningless. Present as a trend-comparison table (Tab 3 in Section 5 plan).

### 2.4 Super-Linear Speedup

- **Haskell prototype (AC-005):** Exhibits super-linear speedup (~K^2) due to O(N^2) `findRedexes` becoming O((N/K)^2) per worker.
- **Relativist:** Uses O(1) incremental redex queue (SPEC-02 R17). The algorithmic artifact that produced super-linear speedup in Haskell does not exist.
- **Resolution:** Super-linear speedup (speedup > K for K workers) is **NOT expected** in Rust. If observed, investigate: likely causes are cache effects (smaller partitions fit in L1/L2), branch prediction improvements, or measurement noise. Document this expectation explicitly in Section 5. The absence of super-linear speedup is a feature (more honest measurement), not a regression.

### 2.5 Statistical Confidence

- **Article (Section 4.6):** "95% confidence intervals for timing metrics."
- **SPEC-09 (R32):** Specifies mean, std, median, min, max. Does not specify CI method.
- **Resolution:** Use **bootstrap 95% CI** (10,000 resamples with replacement) on 10 repetitions. Median as central tendency (robust to timing outliers from OS scheduling, GC, etc.). Bootstrap is appropriate because (a) 10 repetitions is too few for CLT-based parametric CIs, and (b) timing distributions are typically right-skewed (long tail of slow runs), making the median more representative than the mean.

### 2.6 Benchmark Size Reduction for TCC Campaign

- **SPEC-09 (Section 4.7):** Full matrix with 7 sizes per benchmark, 5 reps = ~4,200 datapoints.
- **Resolution for TCC:** The TCC campaign uses a **curated subset** of sizes focused on the most informative data points. Sizes are chosen to:
  - Include the smallest size (to show overhead-dominated regime).
  - Include at least one mid-range size (break-even region).
  - Include the largest practical size (to show scaling behavior).
  - Use 3-5 sizes per benchmark instead of 5-7.
  This reduces the matrix to ~1,500 datapoints (10 reps each), which is tractable in a single campaign day while still being a ~14x improvement over the Haskell prototype's ~110 datapoints.

### 2.7 Sequential Baseline as workers=0

- **SPEC-09 (R23):** The 0-worker case is the sequential baseline.
- **Resolution:** In the CSV and experimental matrix, `workers=0` denotes the `Sequential` mode (pure `reduce_all`, no grid overhead). This is the reference for all speedup calculations. The `Sequential` mode is only run with `workers=0`. The `Local`, `TcpLocalhost`, and `TcpNetwork` modes require `workers >= 1`.

---

## 3. Experimental Matrix

The table below defines all configurations for the TCC campaign. Each row is a benchmark with its specific sizes, worker counts, execution modes, repetitions, and resulting datapoint count.

**Notation:**
- `workers=0` = Sequential baseline (no grid)
- `Seq` = Sequential mode, `Local` = Local grid, `TcpLH` = TcpLocalhost
- Reps = 10 timed repetitions per configuration (+ 2 warmup, discarded)
- Datapoints = sizes x (1 sequential + workers x modes_distributed) x reps

### 3.1 Profile A -- Embarrassingly Parallel

| Benchmark | Sizes | Workers | Modes | Reps | Configs | Datapoints |
|-----------|-------|---------|-------|------|---------|------------|
| EP-ERA | 1K, 10K, 100K | 0, 1, 2, 4, 8 | Seq, Local, TcpLH | 10 | 3x(1+4x2)=27 | 270 |
| EP-CON | 1K, 10K, 50K | 0, 1, 2, 4, 8 | Seq, Local, TcpLH | 10 | 27 | 270 |
| EP-DUP | 1K, 10K, 50K | 0, 1, 2, 4, 8 | Seq, Local, TcpLH | 10 | 27 | 270 |

**Notes:**
- EP-CON/DUP use 50K max instead of 100K because auxiliary port reconnection adds overhead.
- Profile A is the ideal case: all redexes independent, 1 round, 0 border redexes. Expected outcome: near-linear speedup for large sizes.

### 3.2 Profile B -- Expansion with Collapse

| Benchmark | Sizes | Workers | Modes | Reps | Configs | Datapoints |
|-----------|-------|---------|-------|------|---------|------------|
| CON-DUP | 50, 100, 1K, 5K | 0, 1, 2, 4, 8 | Seq, Local, TcpLH | 10 | 4x(1+4x2)=36 | 360 |
| MixedNet | 50, 100, 1K | 0, 1, 2, 4, 8 | Seq, Local, TcpLH | 10 | 27 | 270 |

**Notes:**
- CON-DUP Expansion is the **primary focus** of the TCC (Profile B: expansion + collapse). The Haskell prototype has only 4 TCP datapoints for this benchmark (AC-005). Relativist will produce ~360 datapoints.
- MixedNet exercises all 6 rules and is classified as Profile B due to CON-DUP cascades.
- Sizes for CON-DUP include 4 values to better characterize the break-even curve.

### 3.3 Profile C -- Sequential Dependency

| Benchmark | Sizes (depth) | Workers | Modes | Reps | Configs | Datapoints |
|-----------|---------------|---------|-------|------|---------|------------|
| DualTree | d=6, 8, 10, 12, 14 | 0, 1, 2, 4, 8 | Seq, Local, TcpLH | 10 | 5x(1+4x2)=45 | 450 |
| ErasureProp | 50, 500, 5K | 0, 1, 2, 4, 8 | Seq, Local, TcpLH | 10 | 27 | 270 |

**Notes:**
- DualTree uses 5 depths for granular scaling curves. Depth 14 = 32,766 agents, 14 rounds minimum.
- ErasurePropagation tests sequential cascade of CON-ERA/DUP-ERA rules.
- Profile C demonstrates honest reporting: distribution produces slowdown for these workloads.

### 3.4 Data-Bound Benchmarks

| Benchmark | Sizes | Workers | Modes | Reps | Configs | Datapoints |
|-----------|-------|---------|-------|------|---------|------------|
| TreeSum | 16, 64, 256 | 0, 1, 2, 4, 8 | Seq, Local, TcpLH | 10 | 27 | 270 |
| TreeSumBal | 16, 64, 256 | 0, 1, 2, 4, 8 | Seq, Local, TcpLH | 10 | 27 | 270 |

**Notes:**
- Sizes chosen to overlap with Haskell prototype (AC-005) for qualitative comparison.
- TreeSum uses pragmatic encoding (documented limitation in article Section 5.3).

### 3.5 Summary

| Category | Benchmarks | Datapoints |
|----------|-----------|------------|
| Profile A | EP-ERA, EP-CON, EP-DUP | 810 |
| Profile B | CON-DUP, MixedNet | 630 |
| Profile C | DualTree, ErasureProp | 720 |
| Data-bound | TreeSum, TreeSumBal | 540 |
| **Total (Local + TcpLH)** | **9 benchmarks** | **~2,700** |

**Campaign Phases.** The experimental campaign follows a 3-phase progression that decomposes overhead layer by layer (SPEC-09 R26-R27):

- **Phase 1 (Sequential + Local):** Pure `reduce_all` baseline and in-process grid simulation. Measures algorithmic overhead only. **Status: COMPLETE** (1900 datapoints, validated via `relativist validate`).
- **Phase 2 (TcpLocalhost via Docker):** Coordinator and workers as separate Docker containers on the same machine, communicating via TCP loopback. Adds serialization + TCP overhead (~0.01ms RTT). Uses `docker compose` orchestration. Estimated: ~400-600 additional datapoints on a selected subset of benchmarks.
- **Phase 3 (TcpNetwork on physical machines):** Workers on distinct physical machines over real LAN. Adds real network latency (~0.1-1ms RTT). Required by SPEC-09 R27 (MUST). Conditional on hardware availability; if not feasible, report TcpLocalhost results and document as limitation (Section 10.2 fallback). Estimated: ~120 additional datapoints.

**Note on bench CLI:** The `relativist bench` command supports Sequential and Local modes only (in-process). TcpLocalhost and TcpNetwork campaigns require external orchestration via `docker compose` or manual coordinator+worker process management. The coordinator/worker subcommands are fully functional for this purpose.

---

## 4. CSV Schemas

Three CSV files are produced. All files use comma delimiter, UTF-8 encoding, dot for decimals, and are directly loadable by `pandas.read_csv()`.

### 4.1 detail.csv -- One Row Per Execution

One row per (benchmark, input_size, workers, mode, repetition). This is the raw data file.

| Column | Type | Description |
|--------|------|-------------|
| `benchmark` | string | Benchmark ID (e.g., "EP-ERA", "CON-DUP", "DualTree") |
| `input_size` | u32 | Problem size (pairs for EP, depth for DualTree, items for TreeSum) |
| `mode` | string | "Sequential", "Local", "TcpLocalhost", "TcpNetwork" |
| `workers` | u32 | Number of workers (0 for Sequential) |
| `repetition` | u32 | Repetition index (0-9) |
| `correct` | bool | G1 verification passed (true/false) |
| `wall_clock_secs` | f64 | Total wall-clock time in seconds |
| `total_interactions` | u64 | Total interactions (reductions) performed |
| `mips` | f64 | Millions of Interactions Per Second |
| `rounds` | u32 | Grid loop rounds (0 for Sequential) |
| `peak_memory_bytes` | u64 | Peak RSS of the process |
| `bytes_sent` | u64 | Total bytes sent (0 for Sequential/Local) |
| `bytes_received` | u64 | Total bytes received (0 for Sequential/Local) |
| `speedup` | f64 | T_seq_median / T_this (1.0 for Sequential) |
| `efficiency` | f64 | speedup / workers (1.0 for Sequential) |
| `overhead_ratio` | f64 | 1.0 - (T_compute / T_total), 0.0 for Sequential |
| `con_con` | u64 | CON-CON interactions |
| `dup_dup` | u64 | DUP-DUP interactions |
| `era_era` | u64 | ERA-ERA interactions |
| `con_dup` | u64 | CON-DUP interactions |
| `con_era` | u64 | CON-ERA interactions |
| `dup_era` | u64 | DUP-ERA interactions |

**Example row:**
```
EP-ERA,10000,Local,4,3,true,0.002341,10000,4.27,1,0,0,0,2.15,0.537,0.12,0,0,10000,0,0,0
```

### 4.2 rounds.csv -- One Row Per Round Per Execution

One row per (benchmark, input_size, workers, mode, repetition, round). Only populated for distributed modes (Local, TcpLocalhost, TcpNetwork). Enables per-round analysis of overhead evolution, border ratio dynamics, and net size changes.

| Column | Type | Description |
|--------|------|-------------|
| `benchmark` | string | Benchmark ID |
| `input_size` | u32 | Problem size |
| `workers` | u32 | Number of workers |
| `mode` | string | Execution mode |
| `repetition` | u32 | Repetition index |
| `round` | u32 | Round number (0-indexed) |
| `partition_time_secs` | f64 | Time spent partitioning the net |
| `compute_time_secs` | f64 | Time spent in local reduction (max across workers) |
| `merge_time_secs` | f64 | Time spent in merge + border resolution |
| `network_time_secs` | f64 | Time spent in serialization + transfer (0 for Local) |
| `border_redexes` | u32 | Border redexes detected after merge |
| `border_ratio` | f64 | border_redexes / (local_redexes + border_redexes) |
| `agents_at_start` | u32 | Agents in the net at the start of this round |
| `bytes_sent` | u64 | Bytes sent in this round |
| `bytes_received` | u64 | Bytes received in this round |

**Example row:**
```
DualTree,14,4,TcpLocalhost,0,3,0.000120,0.000450,0.000230,0.000000,128,0.47,16382,0,0
```

### 4.3 summary.csv -- One Row Per Configuration (Aggregated)

One row per (benchmark, input_size, workers, mode), aggregated from the 10 repetitions in detail.csv. This file is the primary input for figures and tables in Section 5.

| Column | Type | Description |
|--------|------|-------------|
| `benchmark` | string | Benchmark ID |
| `input_size` | u32 | Problem size |
| `workers` | u32 | Number of workers |
| `mode` | string | Execution mode |
| `n_reps` | u32 | Number of repetitions (should be 10) |
| `correct_all` | bool | All repetitions passed G1 (AND of all `correct` values) |
| `wall_clock_median` | f64 | Median wall-clock time (seconds) |
| `wall_clock_ci95_lo` | f64 | Lower bound of 95% bootstrap CI |
| `wall_clock_ci95_hi` | f64 | Upper bound of 95% bootstrap CI |
| `wall_clock_std` | f64 | Standard deviation |
| `wall_clock_min` | f64 | Minimum |
| `wall_clock_max` | f64 | Maximum |
| `mips_median` | f64 | Median MIPS |
| `mips_std` | f64 | MIPS standard deviation |
| `speedup_median` | f64 | Median speedup |
| `speedup_ci95_lo` | f64 | Lower bound of 95% bootstrap CI for speedup |
| `speedup_ci95_hi` | f64 | Upper bound of 95% bootstrap CI for speedup |
| `efficiency_median` | f64 | Median efficiency |
| `overhead_ratio_median` | f64 | Median overhead ratio |
| `peak_memory_median` | f64 | Median peak memory (bytes) |
| `bytes_sent_median` | f64 | Median bytes sent |
| `bytes_received_median` | f64 | Median bytes received |
| `speedup_above_one` | bool | Whether speedup_median > 1.0 |

---

## 5. Figure and Table Plan for Section 5

Each figure and table in Section 5 of the TCC article is defined below with its data source, axes, filters, and purpose. The ID column corresponds to the expected order in the article.

| ID | Type | Source CSV | X-axis | Y-axis / Content | Filters | Purpose in Article |
|----|------|-----------|--------|-------------------|---------|-------------------|
| Tab 1 | Table | detail.csv | -- | Pass/fail counts per benchmark, aggregated | All configs | **G1 correctness verification.** Zero failures = hypothesis confirmed. SPEC-01 G1, SPEC-09 R4-R5. |
| Fig 1 | Line plot | summary.csv | workers (0,1,2,4,8) | speedup_median (+ CI error bars) | Profile A benchmarks, largest size per benchmark | **Ideal case: embarrassingly parallel scaling.** Shows near-linear speedup. Answers: "does distribution work for independent redexes?" |
| Fig 2 | Line plot | summary.csv | workers (0,1,2,4,8) | speedup_median (+ CI error bars) | Profile B benchmarks, largest size per benchmark | **Main focus: expansion/collapse scaling.** Shows Profile B behavior (the key TCC contribution). 3 lines: CON-DUP 5K, MixedNet 1K. |
| Fig 3 | Line plot | summary.csv | workers (0,1,2,4,8) | speedup_median (+ CI error bars) | Profile C benchmarks, largest size per benchmark | **Worst case: sequential dependency.** Shows DualTree slowdown (expected <1.0). Demonstrates honest reporting. |
| Fig 4 | Stacked bar | rounds.csv | round (x-axis) | Stacked: partition + compute + merge + network time | CON-DUP 5K, 4 workers, 1 rep (median rep) | **Phase overhead breakdown.** Shows where time is spent per round. Illustrates the BSP cycle anatomy from DISC-006 v2. |
| Fig 5 | Line plot | rounds.csv | round | border_ratio | DualTree d=14 vs CON-DUP 5K, 4 workers | **Boundary effects comparison.** DualTree: ~50% constant. CON-DUP: starts 0%, grows with expansion. Illustrates why Profile C is adversarial. |
| Fig 6 | Line plot | summary.csv | workers (0,1,2,4,8) | mips_median | All profiles, largest sizes, mode=Local | **Throughput scaling.** Shows absolute performance (not relative to baseline). Positions Relativist in the MIPS spectrum. |
| Fig 7 | Line plot | summary.csv | input_size (log scale) | speedup_median, with horizontal line at 1.0 | Profile A + Profile B, workers=4, mode=Local | **Break-even analysis.** Identifies the minimum network size where distribution pays off. Key empirical contribution for Z7 (granularity). |
| Tab 2 | Table | summary.csv | -- | speedup, MIPS, efficiency, overhead_ratio | Best configuration per profile (highest speedup) | **Results at a glance.** Summary table showing the best achievable performance for each profile. |
| Tab 3 | Table | Separate analysis | -- | Haskell trend vs Rust trend | TreeSum + DualTree at shared sizes | **Qualitative Haskell comparison.** Trend arrows showing whether speedup increases/decreases similarly. Explains disappearance of super-linear speedup. |

### 5.1 Figure Production Notes

- **Error bars:** All line plots include error bars representing the 95% bootstrap CI (from `*_ci95_lo` and `*_ci95_hi` columns in summary.csv).
- **Color scheme:** Profile A = green, Profile B = blue, Profile C = red. Consistent across all figures.
- **Log scales:** Fig 7 (break-even) uses log-scale x-axis. All other figures use linear axes.
- **Tool:** Figures are produced by a Python script (`scripts/plot_results.py`) reading the CSV files. The Rust binary does NOT generate figures directly.
- **Fig 4 selection:** For the stacked bar chart, select the repetition whose wall_clock_secs is closest to the median of all repetitions for that configuration.

---

## 6. Statistical Analysis Plan

### 6.1 Central Tendency

**Median** is used as the primary measure of central tendency for all timing metrics (wall_clock_secs, mips, speedup, efficiency, overhead_ratio). Rationale: timing distributions are typically right-skewed due to OS scheduling jitter, cache cold starts, and background process interference. The median is robust to these outliers.

### 6.2 Dispersion

- **Standard deviation** is reported for all timing metrics (for comparability with other work).
- **Min and max** are reported to bound the full range of observations.
- **Coefficient of variation** (CV = std / mean) is computed for wall_clock_secs. If CV > 0.10, a high-variance warning is emitted (SPEC-09 R34). High variance suggests environmental noise or non-deterministic behavior that should be investigated.

### 6.3 Confidence Intervals

**Bootstrap 95% CI** with 10,000 resamples on 10 repetitions:

```python
import numpy as np

def bootstrap_ci(data, n_resamples=10000, ci=0.95):
    """Compute bootstrap confidence interval for the median."""
    medians = []
    for _ in range(n_resamples):
        sample = np.random.choice(data, size=len(data), replace=True)
        medians.append(np.median(sample))
    alpha = (1 - ci) / 2
    lo = np.percentile(medians, 100 * alpha)
    hi = np.percentile(medians, 100 * (1 - alpha))
    return lo, hi
```

This is computed for `wall_clock_secs`, `speedup`, `mips`, and stored in the `*_ci95_lo` / `*_ci95_hi` columns of summary.csv.

### 6.4 Outlier Handling

**No removal.** All 10 repetitions are kept and reported. The median naturally handles outliers without data exclusion. If a specific repetition is anomalous (e.g., wall_clock_secs is 10x the median), it is noted in the analysis but not removed from the dataset.

### 6.5 Significance Testing

**Not applicable.** This is a descriptive study, not a hypothesis test comparing two populations. The key threshold is binary: speedup > 1.0 (distribution pays off) or speedup <= 1.0 (distribution does not pay off). This is determined directly from the data, not via a p-value.

### 6.6 Reporting in Figures and Tables

- All figures include error bars (CI 95% bounds).
- Tables include median with CI bounds in parentheses: e.g., "2.15 (1.98--2.31)".
- Standard deviation is reported in supplementary columns but not displayed in primary figures (to avoid visual clutter).

---

## 7. Haskell Comparison Methodology

### 7.1 Shared Benchmarks and Sizes

Run the Haskell prototype (`grid_computing_interaction_combinators_prototype_v1`) on the following shared configurations:

| Benchmark | Haskell sizes | Workers | Mode |
|-----------|---------------|---------|------|
| TreeSum | 16, 64, 256 | 1, 2, 4 | Grid Local |
| DualTree | d=6, 8, 10, 12, 14 | 1, 2, 4 | Grid Local |
| EP-ERA | 1K, 10K | 1, 2, 4 | Grid Local |
| CON-DUP | 50, 100, 1K | 1, 2, 4 | Grid Local |

**Same machine** for both Haskell and Rust executions. This controls for hardware differences.

### 7.2 Comparison Metrics

Compare **trends**, not absolute values:

| Metric | How to compare | Presentation |
|--------|---------------|--------------|
| Speedup direction | Does speedup increase or decrease with workers? | Trend arrow: up, down, flat |
| Super-linear presence | Does speedup exceed K for K workers? | Yes/No per benchmark |
| Break-even size | Approximate size where speedup crosses 1.0 | Order of magnitude |
| Overhead profile | Which phase dominates overhead? | Dominant phase name |
| Correctness | All executions correct? | Pass/Fail |

### 7.3 What NOT to Compare

- **Absolute wall-clock time.** Haskell is interpreted (GHC runtime, lazy evaluation, garbage collector). Rust is compiled (zero-cost abstractions, no GC). A 10-100x difference in absolute time is expected and uninteresting.
- **Absolute MIPS.** Same reason as above.
- **Serialization payload size.** Haskell uses `[Int]` (8 bytes per Int on 64-bit). Rust uses bincode (compact binary). Not comparable.

### 7.4 Super-Linear Speedup Explanation

The Haskell prototype exhibits super-linear speedup for EP-ERA because `findRedexes` scans all wires (O(N^2) total for N reductions). When the net is split into K partitions, each worker scans O(N/K) wires, yielding O(N^2/K^2) total -- a K^2 improvement, not K.

Relativist uses an incremental redex queue (SPEC-02 R17) with O(1) amortized cost per redex discovery. The algorithmic artifact that produces super-linear speedup does not exist. Expected Relativist speedup for EP-ERA: linear (~K), not quadratic (~K^2).

**If super-linear speedup IS observed in Relativist:** investigate. Likely causes:
1. **Cache effects:** Smaller partitions fit entirely in L1/L2 cache.
2. **Memory allocation patterns:** Smaller arenas have better allocation locality.
3. **Measurement noise:** For very fast executions (< 1ms), timing granularity may dominate.

Document the investigation and finding in Section 5 regardless of outcome.

### 7.5 Presentation (Tab 3)

| Benchmark | Size | Metric | Haskell Trend | Rust Trend | Notes |
|-----------|------|--------|--------------|------------|-------|
| EP-ERA | 10K | Speedup vs workers | up (super-linear) | up (linear) | O(N^2) findRedexes artifact gone |
| DualTree | d=14 | Speedup vs workers | down (0.18x at 4w) | down (less severe) | Faster reduce_all, no remap overhead |
| CON-DUP | 1K | Speedup vs workers | inconclusive (4 pts) | up/flat/down (TBD) | Key contribution: resolve Profile B |
| TreeSum | 256 | Speedup vs workers | up (moderate) | up/flat (TBD) | Pragmatic encoding, not pure IC |

Trend arrows: up = speedup increases with workers, down = speedup decreases, flat = no significant change.

---

## 8. Hardware and Environment Requirements

### 8.1 Primary Machine

Document in the article (Section 4, "Experimental Setup"):

| Property | What to Record |
|----------|---------------|
| CPU model | e.g., AMD Ryzen 7 5800X, 8 cores / 16 threads |
| CPU base/boost clock | e.g., 3.8 GHz / 4.7 GHz |
| RAM | e.g., 32 GB DDR4-3200 |
| Disk | e.g., NVMe SSD (not relevant for computation, but for Docker image loading) |
| OS | e.g., Windows 11 Pro + WSL2 Ubuntu 22.04, or native Linux |
| Kernel | e.g., Linux 5.15.0-xxx |

### 8.2 Software Versions

| Software | How to Pin |
|----------|-----------|
| Rust toolchain | Pin via `rust-toolchain.toml` (e.g., `1.78.0`) |
| Docker | Record `docker --version` output |
| Docker Compose | Record `docker compose version` output |
| OS packages | Record kernel version, glibc version |

### 8.3 Network (for TcpNetwork mode)

| Property | What to Record |
|----------|---------------|
| Network type | LAN (Gigabit Ethernet, WiFi, etc.) |
| Measured latency | `ping -c 100 <peer>`, report min/avg/max |
| Measured bandwidth | `iperf3 -c <peer>`, report throughput |
| Number of physical machines | 4 or 8 |
| Machine specs | Document each machine (CPU, RAM) if heterogeneous |

### 8.4 Isolation Protocol

Before running the campaign:
1. Close all unnecessary applications (browsers, IDEs, communication tools).
2. Set power plan to "High Performance" (prevent CPU throttling).
3. Disable automatic updates during the run.
4. On Linux: set CPU governor to `performance` (`cpupower frequency-set -g performance`).
5. Verify low background load: `uptime` should show load average < 0.5.
6. Record `uptime` and `free -h` before and after the campaign.

---

## 9. Execution Protocol

Step-by-step procedure for the TCC campaign. Follow exactly.

### 9.1 Pre-Campaign

1. **Build release binary:**
   ```bash
   cargo build --release
   ```
2. **Verify all unit and integration tests pass:**
   ```bash
   cargo test
   ```
3. **Verify G1 on a small smoke test:**
   ```bash
   cargo run --release --bin bench -- --benchmark EP-ERA --sizes 100 --workers 1,2 --mode Local --repetitions 1 --warmup 0
   ```
   Confirm: `correct=true` for all rows.
4. **Record environment** (hardware, software versions, date/time).
5. **Apply isolation protocol** (Section 8.4).

### 9.2 Campaign Execution

Execute benchmarks in order of increasing complexity. This ensures that if the campaign must be interrupted, the most informative data (simple benchmarks) is already collected.

**Phase 1: Profile A (Embarrassingly Parallel)**

```bash
# EP-ERA: largest, most data
cargo run --release --bin bench -- \
  --benchmark EP-ERA \
  --sizes 1000,10000,100000 \
  --workers 0,1,2,4,8 \
  --mode Sequential,Local,TcpLocalhost \
  --warmup 2 --repetitions 10 \
  --csv-detail results/detail.csv \
  --csv-rounds results/rounds.csv

# EP-CON
cargo run --release --bin bench -- \
  --benchmark EP-CON \
  --sizes 1000,10000,50000 \
  --workers 0,1,2,4,8 \
  --mode Sequential,Local,TcpLocalhost \
  --warmup 2 --repetitions 10 \
  --csv-detail results/detail.csv \
  --csv-rounds results/rounds.csv

# EP-DUP
cargo run --release --bin bench -- \
  --benchmark EP-DUP \
  --sizes 1000,10000,50000 \
  --workers 0,1,2,4,8 \
  --mode Sequential,Local,TcpLocalhost \
  --warmup 2 --repetitions 10 \
  --csv-detail results/detail.csv \
  --csv-rounds results/rounds.csv
```

**Phase 2: Profile B (Expansion with Collapse)**

```bash
# CON-DUP Expansion (PRIMARY FOCUS)
cargo run --release --bin bench -- \
  --benchmark CON-DUP \
  --sizes 50,100,1000,5000 \
  --workers 0,1,2,4,8 \
  --mode Sequential,Local,TcpLocalhost \
  --warmup 2 --repetitions 10 \
  --csv-detail results/detail.csv \
  --csv-rounds results/rounds.csv

# MixedNet
cargo run --release --bin bench -- \
  --benchmark MixedNet \
  --sizes 50,100,1000 \
  --workers 0,1,2,4,8 \
  --mode Sequential,Local,TcpLocalhost \
  --warmup 2 --repetitions 10 \
  --csv-detail results/detail.csv \
  --csv-rounds results/rounds.csv
```

**Phase 3: Profile C (Sequential Dependency)**

```bash
# DualTree
cargo run --release --bin bench -- \
  --benchmark DualTree \
  --sizes 6,8,10,12,14 \
  --workers 0,1,2,4,8 \
  --mode Sequential,Local,TcpLocalhost \
  --warmup 2 --repetitions 10 \
  --csv-detail results/detail.csv \
  --csv-rounds results/rounds.csv

# ErasurePropagation
cargo run --release --bin bench -- \
  --benchmark ErasureProp \
  --sizes 50,500,5000 \
  --workers 0,1,2,4,8 \
  --mode Sequential,Local,TcpLocalhost \
  --warmup 2 --repetitions 10 \
  --csv-detail results/detail.csv \
  --csv-rounds results/rounds.csv
```

**Phase 4: Data-Bound Benchmarks**

```bash
# TreeSum
cargo run --release --bin bench -- \
  --benchmark TreeSum \
  --sizes 16,64,256 \
  --workers 0,1,2,4,8 \
  --mode Sequential,Local,TcpLocalhost \
  --warmup 2 --repetitions 10 \
  --csv-detail results/detail.csv \
  --csv-rounds results/rounds.csv

# TreeSumBalanced
cargo run --release --bin bench -- \
  --benchmark TreeSumBal \
  --sizes 16,64,256 \
  --workers 0,1,2,4,8 \
  --mode Sequential,Local,TcpLocalhost \
  --warmup 2 --repetitions 10 \
  --csv-detail results/detail.csv \
  --csv-rounds results/rounds.csv
```

### 9.3 G1 Verification Rule

**On EVERY execution (including warmup), verify G1:**

```
reduce_all(net) ~= extract_result(run_grid(net, n))
```

where `~=` is graph isomorphism (SPEC-08, `nets_isomorphic`).

**If ANY G1 failure occurs: STOP IMMEDIATELY.**

A G1 failure is a correctness bug in the system (reduction engine, partitioning, merge, or wire protocol). Do not continue the campaign. Debug, fix, re-run all tests, and restart the campaign from scratch. There is no "acceptable failure rate" for correctness.

### 9.4 Post-Campaign

1. **Generate summary.csv** from detail.csv:
   ```bash
   python scripts/compute_summary.py results/detail.csv results/summary.csv
   ```
   This script computes medians, CIs, and all derived columns.

2. **Run Haskell prototype** on shared benchmarks (Section 7.1):
   ```bash
   cd codigo/grid_computing_interaction_combinators_prototype_v1
   cabal run benchmark -- --csv ../relativist/results/haskell_baseline.csv
   ```

3. **Generate figures and tables:**
   ```bash
   python scripts/plot_results.py results/summary.csv results/rounds.csv results/figures/
   ```

4. **Verify all success criteria** (Section 10).

5. **Record environment** again (date/time, uptime, any anomalies).

### 9.5 Phase 2: TcpLocalhost Campaign (Docker)

Run selected benchmarks with coordinator and workers as separate Docker containers:

1. Build Docker image: `docker compose build`.
2. Generate input net: `relativist generate <benchmark> -n <size> -o data/input.bin`.
3. Run with N workers: `NUM_WORKERS=N docker compose up`.
4. Collect output net and metrics from `data/` volume mount.
5. Verify G1: compare output with sequential baseline via `relativist validate` or manual isomorphism check.
6. Repeat for each (benchmark, size, workers) configuration with 10 repetitions.
7. Append results to the same CSV files with `mode=tcp_localhost`.

**Selected benchmarks for Phase 2** (subset showing interesting Phase 1 results):
- Profile A: ep_annihilation_con (1K, 5K)
- Profile B: condup_expansion (100, 500, 1K)
- Profile C: dual_tree (8, 10, 12)
- Encoding: church_add (3+4, 5+5), church_mul (3*4)
- Workers: 1, 2, 4, 8
- Repetitions: 10

### 9.6 Phase 3: TcpNetwork Campaign (Physical Machines)

Required by SPEC-09 R27 (MUST). Conditional on hardware availability.

1. Deploy Relativist on 4 and 8 physical machines (SPEC-07, bare-metal or Docker).
2. Measure baseline network latency (`ping`, `iperf3`) and record in environment log.
3. Run selected configurations (benchmarks with speedup > 1.0 in Phase 2).
4. Append results to the same CSV files with `mode=tcp_network`.
5. Record network latency and bandwidth measurements per run.
6. If hardware unavailable: report TcpLocalhost results only and document as limitation (Section 10.2 fallback).

---

## 10. Success Criteria

### 10.1 Hard Requirements (MUST pass for publishable results)

| Criterion | How to Verify | SPEC Reference |
|-----------|--------------|----------------|
| Zero G1 failures | `detail.csv`: `correct` column is `true` for ALL rows | SPEC-09 R4-R5, SPEC-01 G1 |
| All 3 CSV files generated | `detail.csv`, `rounds.csv`, `summary.csv` exist and parse correctly | SPEC-09 R39-R42 |
| At least 1 benchmark shows speedup > 1.0 | `summary.csv`: at least 1 row with `speedup_above_one == true` | Expected for Profile A, large sizes |
| DualTree shows slowdown | `summary.csv`: DualTree rows with workers > 1 have speedup < 1.0 | Expected from AC-005 results; demonstrates honest reporting |
| All 9 benchmarks produce data | Each of the 9 benchmark IDs appears in `detail.csv` | SPEC-09 R8-R17 |
| At least ~2,500 datapoints | Row count of `detail.csv` >= 2,500 | Campaign matrix target |

### 10.2 Soft Requirements (SHOULD pass; document if not)

| Criterion | How to Verify | Fallback if Not Met |
|-----------|--------------|---------------------|
| Break-even point identified for Profile A | speedup crosses 1.0 between smallest and largest sizes | Report as "break-even below minimum tested size" |
| Break-even point identified for Profile B | speedup crosses 1.0 for CON-DUP | Report as "overhead dominates at all tested sizes" or "speedup at all tested sizes" |
| Bootstrap CIs computed for all timing metrics | `summary.csv` has non-null `*_ci95_lo` and `*_ci95_hi` columns | Use min/max as bounds instead |
| Haskell comparison table completed | Tab 3 has data for all shared benchmarks | Mark cells as "N/A" if Haskell prototype cannot run |
| All 10 figures/tables from Section 5 plan producible | Each figure/table has sufficient source data | Remove figure from article and explain in Section 5.3 (Limitations) |
| CV < 0.10 for wall-clock measurements | `summary.csv`: check `wall_clock_std / wall_clock_median` | Investigate environment noise; re-run with better isolation |
| TcpNetwork data on physical machines | At least 60 TcpNetwork datapoints | Report TcpLH results only; note in limitations |

---

## Appendix A: SPEC-09 Requirement Cross-Reference

Key SPEC-09 requirements and how this plan addresses them:

| SPEC-09 Req | Topic | Plan Section |
|-------------|-------|-------------|
| R1-R2 | Benchmark framework binary + trait | (Implementation concern, not data plan) |
| R3-R5 | Sequential baseline + G1 on every datapoint | Section 9.3 |
| R6 | CLI parameters | Section 9.2 (command examples) |
| R8-R17 | 9 mandatory benchmarks | Section 3 (all 9 included) |
| R18-R20 | BenchmarkResult + derived metrics | Section 4.1 (detail.csv schema) |
| R22-R23 | Workers 0,1,2,4,8 | Section 3 (all worker counts) |
| R24-R25 | Logarithmic size variation | Section 3 (sizes chosen per benchmark) |
| R26-R27 | 3 execution modes + TcpNetwork | Section 3 + Section 9.5 |
| R30-R31 | Warmup + repetitions | Section 2.1 (10 reps + 2 warmup) |
| R32-R34 | Aggregation statistics + CV warning | Section 6 |
| R36-R38 | Correctness on every repetition + halt on failure | Section 9.3 |
| R39-R42 | CSV output | Section 4 (3 CSV files) |
| R43-R45 | Haskell comparison | Section 7 |
| R46-R47 | Break-even analysis | Section 5 (Fig 7) |
| R48-R49 | Overhead breakdown per round | Section 4.2 (rounds.csv) + Section 5 (Fig 4) |

## Appendix B: Estimated Campaign Duration

Rough estimates assuming ~0.01s per small execution and ~10s per large execution:

| Phase | Configurations | Executions (incl. warmup) | Est. Time |
|-------|---------------|--------------------------|-----------|
| Profile A | 81 configs x 12 runs | ~972 | ~30 min |
| Profile B | 63 configs x 12 runs | ~756 | ~60 min |
| Profile C | 72 configs x 12 runs | ~864 | ~120 min |
| Data-bound | 54 configs x 12 runs | ~648 | ~20 min |
| Summary generation | -- | -- | ~5 min |
| Haskell comparison | ~60 configs x 1 run | ~60 | ~30 min |
| **Total** | | | **~4-5 hours** |

These are rough estimates. Actual duration depends on hardware speed, net sizes, and the number of grid rounds for multi-round benchmarks (DualTree, CON-DUP). Plan for a full day with buffer.
