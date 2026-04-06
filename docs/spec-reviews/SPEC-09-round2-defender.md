# SPEC-09 -- Round 2: Defender Response

**Date:** 2026-04-05
**Defender:** Especialista em Specs
**Target:** SPEC-09-benchmarks.md
**Critic review:** SPEC-09-round1-critic.md
**Spec version:** Revised v2 -> Revised v3

---

## Summary

| Response | Count |
|----------|-------|
| ACCEPTED | 14 |
| PARTIALLY ACCEPTED | 3 |
| NOT ADDRESSED | 0 |
| **Total issues** | **17** |

---

## Responses

### SC-001: Repetition count conflict between SPEC-09 and DATA-COLLECTION-PLAN
**Severity:** CRITICAL
**Response:** ACCEPTED
**Action taken:** Amended R31 to align SPEC-09, DATA-COLLECTION-PLAN, and the article text on a three-tier repetition scheme:
- MUST minimum: 5 (for development/debugging runs)
- SHOULD for TCC campaign: at least 10 (sufficient for bootstrap 95% CI computation, consistent with Section 4.6 of the article and DATA-COLLECTION-PLAN Section 2.1)
- Recommended for CLT-based parametric CI: 30

This resolves the three-way conflict: the spec now explicitly blesses 10 as the TCC campaign value (matching the article and plan), while retaining 5 as the minimum for development and 30 as the gold standard for CLT-based analysis. Also updated Section 5.3 rationale and Section 6.2 metrics table to reflect the three-tier scheme.
**Spec sections modified:** Section 3.5 (R31), Section 5.3, Section 6.2

### SC-002: Sequential mode listed in CLI but absent from the R26 mode table
**Severity:** CRITICAL
**Response:** ACCEPTED
**Action taken:** Added `Sequential` as the first entry in R26's mode table with description: "Pure sequential: `reduce_all` without grid. Bypasses partitioning, merge, and all protocol infrastructure. Ignores `--workers` (always 0 workers). This mode produces the baseline for speedup calculations." Changed R26's introductory text from "three execution modes" to "four execution modes." Added a clarifying note below the table: "`--mode Sequential` is the canonical way to request the sequential baseline. `--workers 0` with any distributed mode MUST be treated as equivalent to `--mode Sequential` for convenience." This resolves the ambiguity between mode-based and worker-count-based control of the sequential baseline.
**Spec sections modified:** Section 3.4.3 (R26)

### SC-003: SPEC-09 generator pseudocode duplicates SPEC-12's canonical generators
**Severity:** HIGH
**Response:** ACCEPTED
**Action taken:** Marked Section 4.2 as **(Informative)** with an explicit cross-spec note: "The canonical generator implementations are defined in SPEC-12 (R35-R42a). The pseudocode below is illustrative only. Benchmarks MUST use the generators from `src/io/examples.rs` via `Benchmark::make_net`, which delegates to the shared generator functions. If any discrepancy exists between the pseudocode here and SPEC-12's Rust generators, SPEC-12 is authoritative." Updated Section 4.6 directory structure to show `io/examples.rs` as the generator location with an explanatory note about the relationship between `bench/benchmarks/*.rs` (trait implementations) and `io/examples.rs` (canonical generators). This eliminates the duplication concern: SPEC-09's pseudocode is informative context, not normative specification.
**Spec sections modified:** Section 4.2 (header and cross-spec note), Section 4.6 (directory structure and note)

### SC-004: No Church numeral benchmarks despite SPEC-14 defining generators
**Severity:** HIGH
**Response:** ACCEPTED
**Action taken:** Added two new benchmarks:
- **R17a (MUST):** ChurchAdd benchmark. Generates `build_add(N/2, N - N/2)` using SPEC-14 encoding. Profile B. Default sizes [10, 50, 100, 500]. Verified by `decode_nat` AND graph isomorphism. Added `ChurchAdd` to the `BenchmarkId` enum.
- **R17b (SHOULD):** ChurchMul benchmark. Generates `build_mul(a, b)` with O(a*b) interactions. Profile B. Default sizes [5, 10, 20, 50]. Also verified by `decode_nat` AND isomorphism. Added `ChurchMul` to the `BenchmarkId` enum.

These are placed in a new Section 3.2.6 (Arithmetic Encoding Benchmark). Updated R8 from "at least 7" to "at least 10" benchmarks. Updated the experimental matrix (Section 4.7) to include both. Updated the Haskell mapping table (Section 6.1) to list both as new benchmarks. Resolved Open Question 6 (previously marked as recommended; now resolved). Added SPEC-14 to the "Depends on" header.

Church arithmetic is the primary demonstration that Relativist performs real computation. Including ChurchAdd as MUST and ChurchMul as SHOULD provides a semantically meaningful Profile B workload that complements the synthetic CON-DUP Expansion benchmark.
**Spec sections modified:** Section 3.2.6 (new -- R17a, R17b), Section 3.2 (R8), Section 4.1 (BenchmarkId enum), Section 4.7 (experimental matrix), Section 6.1 (Haskell mapping table), Section 7 (OQ-6 resolved), Header (Depends on)

### SC-005: CSV schema conflict between SPEC-09 and DATA-COLLECTION-PLAN
**Severity:** HIGH
**Response:** ACCEPTED
**Action taken:** Replaced the single-file CSV schema (old R39) with a three-file schema aligned with DATA-COLLECTION-PLAN Section 4:
- **R39a:** `detail.csv` -- one row per (benchmark, input_size, workers, mode, repetition). Schema contains scalar aggregates per execution. The per-phase timing columns (`t_partition`, etc.) that were in the old single-file schema are now in `rounds.csv`.
- **R39b:** `rounds.csv` -- one row per (benchmark, input_size, workers, mode, repetition, round). Per-round phase breakdown (partition, compute, merge, network times), border redexes, border ratio, agents at start. Only populated for distributed modes.
- **R40:** `summary.csv` -- aggregated statistics per configuration. Now includes median fields and optional bootstrap CI columns (`wall_clock_ci95_lo/hi`, `speedup_ci95_lo/hi`, `mips_ci95_lo/hi`). CI columns are SHOULD; if bootstrap is deferred to Python, they MAY be absent.

Updated `BenchmarkSuiteConfig` to include `csv_rounds: Option<PathBuf>`. Updated Section 4.3 execution protocol to write 3 files. Updated Section 5.5 rationale from "unified CSV" to "three CSV files."

This resolves the conflict: SPEC-09 now specifies the same three-file structure as the DATA-COLLECTION-PLAN, ensuring Python analysis scripts will find the expected schemas.
**Spec sections modified:** Section 3.7 (R39 replaced by R39a/R39b, R40 rewritten), Section 4.1 (BenchmarkSuiteConfig), Section 4.3 (CSV output call), Section 4.6 (csv.rs description), Section 5.5

### SC-006: Statistical methodology conflict -- SPEC-09 uses mean/std, DATA-COLLECTION-PLAN uses median/bootstrap CI
**Severity:** HIGH
**Response:** ACCEPTED
**Action taken:** Added R32a (SHOULD): "The framework SHOULD compute bootstrap 95% confidence intervals for `wall_clock_secs`, `mips`, and `speedup` using 10,000 resamples on the median." Includes rationale from DATA-COLLECTION-PLAN Section 6.3: (a) 10 repetitions is too few for CLT-based parametric CIs, (b) timing distributions are typically right-skewed. If bootstrap is not implemented in Rust, it MAY be deferred to the Python post-processing scripts, provided `detail.csv` contains all raw per-repetition data. Updated the median description in R32 to mark it as the "primary measure of central tendency." Added `time_ci95_lo/hi`, `mips_ci95_lo/hi`, `speedup_ci95_lo/hi`, `mips_median`, `speedup_median`, `efficiency_median`, `overhead_ratio_median` to the `AggregatedStats` struct (as `Option<f64>` for CI fields to support deferred computation).

This resolves the methodology conflict: SPEC-09 now recommends bootstrap CIs while allowing flexibility in where the computation happens (Rust or Python).
**Spec sections modified:** Section 3.5 (R32 updated, R32a added), Section 4.1 (AggregatedStats struct)

### SC-007: MixedNet expected result is underspecified
**Severity:** HIGH
**Response:** ACCEPTED
**Action taken:** Added a complete "Expected result" block to R16 with a step-by-step derivation of the final state: 4N ERA agents (2N from CON-ERA erasure + 2N from DUP-ERA erasure), each connected to a unique FreePort, 0 agents of other types, 0 redexes. The derivation covers all 6 pair types: ERA-ERA -> void, CON-CON -> annihilation, DUP-DUP -> annihilation, CON-DUP -> commutation -> cascade -> void, CON-ERA -> 2 ERAs, DUP-ERA -> 2 ERAs. Also noted that pairs are fully independent (unique FreePort IDs per SPEC-12 R41), so post-CON-DUP agents interact only within their own group, not across groups.

This makes the MixedNet verifier fully testable: the expected output is precisely characterized, enabling both unit tests (check agent count = 4N, all ERA) and integration tests (graph isomorphism with sequential result).
**Spec sections modified:** Section 3.2.5 (R16 -- added Expected result block)

### SC-008: ErasurePropagation expected result "0 agents" is mathematically incorrect
**Severity:** HIGH
**Response:** ACCEPTED
**Action taken:** Added a complete "Expected result" block to R17 with the correct characterization: (N+1) ERA agents, each connected to a FreePort, 0 redexes. The derivation explains the cascade mechanism: each CON-ERA interaction removes 1 CON and produces 2 ERAs -- one continues the cascade (connecting to the next CON's principal port), one terminates at the consumed CON's free port branch. The last CON has both auxiliary ports as FreePorts, producing 2 ERAs. Total: N+1 ERA agents.

Added an explicit note: "The expected result is NOT 0 agents. ERA agents with arity 0 connected to FreePorts are in normal form; they cannot participate in any interaction and remain in the net as the reduction's output." The verifier MUST check: (a) all remaining agents are ERA, (b) net is in normal form (0 redexes), (c) result matches sequential baseline by graph isomorphism.

This prevents the implementer from writing `assert!(agent_count == 0)` for ErasurePropagation, which would fail on every execution.
**Spec sections modified:** Section 3.2.5 (R17 -- added Expected result block and Note)

### SC-009: Speedup formula for workers=1 grid mode is undefined
**Severity:** MEDIUM
**Response:** ACCEPTED
**Action taken:** Added a "Sequential baseline special cases (workers == 0)" paragraph to R20: "For the sequential baseline, `speedup` MUST be 1.0, `efficiency` MUST be 1.0, and `overhead_ratio` MUST be 0.0 (by convention, since the baseline has no parallelism overhead to measure and is 100% compute). The formulas for `speedup`, `efficiency`, and `overhead_ratio` apply only to grid executions (workers >= 1). This avoids division-by-zero (efficiency = speedup / 0) and incorrect values (overhead_ratio formula produces 1.0 for sequential because `compute_time_per_round` is empty)."

Note: the workers=1 grid mode case was already correctly handled -- it uses the standard formulas and produces speedup < 1.0, which is the intended measurement of protocol overhead in isolation.
**Spec sections modified:** Section 3.3 (R20 -- added special-case paragraph)

### SC-010: Overhead ratio formula is incorrect for sequential mode
**Severity:** MEDIUM
**Response:** ACCEPTED
**Action taken:** Covered by the same R20 amendment as SC-009. The special-case paragraph explicitly states: "overhead_ratio MUST be 0.0" for sequential (workers == 0), and "The formula `1.0 - (sum(compute_time_per_round) / wall_clock_secs)` applies only to grid executions (workers >= 1)." This prevents the formula from producing 1.0 (100% overhead) for the sequential baseline, which is semantically the opposite of the truth (0% overhead, 100% compute). Aligned with DATA-COLLECTION-PLAN detail.csv schema which specifies "overhead_ratio: 0.0 for Sequential."
**Spec sections modified:** Section 3.3 (R20 -- same amendment as SC-009)

### SC-011: G1 verification timing -- `baseline_sequential_time` not specified as median or mean
**Severity:** MEDIUM
**Response:** ACCEPTED
**Action taken:** Amended R20's speedup formula to explicitly state: "For speedup computation, `baseline_sequential_time` is the **median** wall-clock time of all sequential repetitions for the same (benchmark, size) combination." This aligns with DATA-COLLECTION-PLAN detail.csv which specifies `speedup: T_seq_median / T_this` and with the execution protocol in Section 4.3 which already computes `seq_baseline = median(seq_results.map(|r| r.wall_clock_secs))`. The ambiguity is resolved: median, not mean.
**Spec sections modified:** Section 3.3 (R20 -- speedup formula clarified)

### SC-012: TreeSum benchmark uses pragmatic encoding, not pure IC
**Severity:** MEDIUM
**Response:** ACCEPTED
**Action taken:** Added a "Note" block to R14 explaining that TreeSum uses a pragmatic encoding inherited from the Haskell prototype. The note clarifies: (a) "adders" and "values" are semantic conventions external to pure IC theory, (b) `extract_result` interprets the net by convention, not by a formal IC encoding, (c) for formal arithmetic computation, see the Church numeral benchmarks (SPEC-14, R17a), (d) TreeSum's correctness verification MUST use graph isomorphism with the sequential result, not semantic value extraction. The DATA-COLLECTION-PLAN Section 3.4 already acknowledges this as a "documented limitation in article Section 5.3."
**Spec sections modified:** Section 3.2.4 (R14 -- added Note block)

### SC-013: Benchmark trait `verify` method -- graph isomorphism algorithm unspecified
**Severity:** MEDIUM
**Response:** PARTIALLY ACCEPTED
**Action taken:** Added R37a specifying a tiered isomorphism strategy: (a) lightweight check (same agent count per symbol, same wire count, same free port count, same redex count == 0) MAY be used when performance is a concern, (b) full structural isomorphism via `nets_isomorphic` (SPEC-08) SHOULD be used when the lightweight check passes and the result is small (< 1000 agents), (c) for empty-result benchmarks (EP, DualTree), the verifier SHOULD simply check `agent_count == 0`.

The trait signature itself was not changed. The critic noted it is "overloaded" (sequential result parameter unused for empty-result benchmarks), but the unified signature is a deliberate design choice: it keeps the trait simple and allows future benchmarks to use the sequential result for comparison without trait changes. The minor inefficiency of passing an unused reference is negligible.
**Spec sections modified:** Section 3.6 (R37a added)

### SC-014: SPEC-09 does not reference SPEC-05 correctly for grid cycle integration
**Severity:** MEDIUM
**Response:** ACCEPTED
**Action taken:** Added a clarifying note to Section 4.3 explaining that `run_grid(net, workers, mode)` in the execution protocol is pseudocode for the benchmark framework's wrapper function, not a direct invocation of SPEC-05's `run_grid(net: Net, num_workers: u32, strategy: impl PartitionStrategy)`. The wrapper selects the appropriate transport (in-memory channel for Local, TCP for TcpLocalhost/TcpNetwork) and delegates to the grid cycle via the system architecture (SPEC-13). The file naming inconsistency noted by the critic (SPEC-05 title includes "Grid Cycle" but file is `SPEC-05-merge.md`) is a pre-existing naming convention that does not affect correctness; no change made.
**Spec sections modified:** Section 4.3 (added clarifying note above execution protocol)

### SC-015: Peak memory measurement is platform-dependent with no fallback
**Severity:** LOW
**Response:** PARTIALLY ACCEPTED
**Action taken:** Updated Section 4.4 to recommend a Windows fallback: "On Windows (development platform), the framework SHOULD attempt to use `GetProcessMemoryInfo` from the Windows API or a cross-platform crate (`sysinfo`); if unavailable, it MUST return 0 and the limitation MUST be documented." Clarified that `peak_memory_bytes` is MUST for the TCC campaign (Linux/Docker, where `/proc/self/status` is available) and SHOULD for development-time runs on non-Linux platforms.

The fix differs from the critic's suggestion in that `peak_memory_bytes` remains a MUST metric (not downgraded to SHOULD), because the TCC campaign runs on Docker/Linux where the metric is guaranteed available. The Windows fallback is a SHOULD convenience for development.
**Spec sections modified:** Section 4.4 (memory measurement clarification)

### SC-016: Haskell comparison sizes do not fully match DATA-COLLECTION-PLAN
**Severity:** LOW
**Response:** ACCEPTED
**Action taken:** Changed R43's TreeSum Haskell comparison sizes from `[50, 200, 1000, 2000]` to `[16, 64, 256]`, matching DATA-COLLECTION-PLAN Section 7.1. The rationale: the plan's TreeSum sizes are chosen to overlap with the TCC campaign's TreeSum sizes (DATA-COLLECTION-PLAN Section 3.4: `[16, 64, 256]`), ensuring both Haskell and Rust data exist at the same sizes for trend comparison. The original sizes `[50, 200, 1000, 2000]` would have no Rust counterparts in the campaign matrix, making comparison impossible.
**Spec sections modified:** Section 3.8 (R43 -- TreeSum sizes)

### SC-017: No explicit G1 verification mechanism for the sequential mode baseline
**Severity:** LOW
**Response:** PARTIALLY ACCEPTED
**Action taken:** Added R37b: "For the sequential baseline, correctness MUST be verified by comparing the result of each repetition against the first sequential result by graph isomorphism. This validates invariant T6 (uniqueness of normal form, SPEC-01): all sequential runs MUST produce the same normal form regardless of reduction order."

The fix differs slightly from the critic's suggestion in that the comparison is against the first sequential result (not an arbitrary reference), which is natural since the first result is computed before any repetitions. This makes T6 validation explicit: if two sequential runs produce different normal forms for the same input, the reduction engine has a bug.
**Spec sections modified:** Section 3.6 (R37b added)

---

## Changes Made to SPEC-09

### Header
- Status changed from "Revised v2" to "Revised v3"
- Added SPEC-12 and SPEC-14 to "Depends on" list

### Section 3.2 (Mandatory Benchmarks)
- R8: Changed "at least 7 benchmarks" to "at least 10 benchmarks"
- R14: Added Note block about pragmatic encoding and graph isomorphism verification (SC-012)
- R16: Added "Expected result" block with full derivation (4N ERA agents) (SC-007)
- R17: Added "Expected result" block with correct characterization (N+1 ERA agents) and Note about non-zero result (SC-008)
- R17a: New MUST requirement -- ChurchAdd benchmark (SC-004)
- R17b: New SHOULD requirement -- ChurchMul benchmark (SC-004)
- New Section 3.2.6 (Arithmetic Encoding Benchmark)

### Section 3.3 (Mandatory Metrics)
- R20: Added "Sequential baseline special cases" paragraph specifying speedup=1.0, efficiency=1.0, overhead_ratio=0.0 for workers==0 (SC-009, SC-010)
- R20: Specified `baseline_sequential_time` as median of sequential repetitions (SC-011)

### Section 3.4.3 (Execution Mode)
- R26: Changed "three execution modes" to "four execution modes"
- R26: Added `Sequential` mode to the mode table with description (SC-002)
- R26: Added note about `--workers 0` equivalence

### Section 3.5 (Statistical Methodology)
- R31: Rewritten with three-tier repetition scheme (5/10/30) (SC-001)
- R32: Updated median description as "primary measure of central tendency"
- R32a: New SHOULD requirement -- bootstrap 95% CI computation with deferral option (SC-006)

### Section 3.6 (Correctness on Every Execution)
- R37: Updated benchmark list to include ErasurePropagation and ChurchAdd
- R37a: New MAY/SHOULD requirement -- tiered isomorphism strategy (SC-013)
- R37b: New MUST requirement -- sequential baseline T6 validation (SC-017)

### Section 3.7 (Output)
- R39: Replaced single-file schema with three-file schema (R39a detail.csv, R39b rounds.csv) (SC-005)
- R40: Rewritten for summary.csv with median fields and optional bootstrap CI columns (SC-005, SC-006)

### Section 4.1 (Types)
- `BenchmarkId` enum: Added `ChurchAdd` and `ChurchMul` variants
- `BenchmarkSuiteConfig`: Added `csv_rounds: Option<PathBuf>` field
- `AggregatedStats`: Added `time_ci95_lo/hi`, `mips_median`, `mips_ci95_lo/hi`, `speedup_median`, `speedup_ci95_lo/hi`, `efficiency_median`, `overhead_ratio_median` fields (CI fields as `Option<f64>`)

### Section 4.2 (Net Generators)
- Renamed to "Net Generators (Informative)"
- Added cross-spec note declaring SPEC-12 as authoritative for generators (SC-003)

### Section 4.3 (Execution Protocol)
- Added clarifying note about `run_grid(net, workers, mode)` being benchmark wrapper pseudocode (SC-014)
- Updated CSV output call to write 3 files

### Section 4.4 (Memory Measurement)
- Added Windows fallback recommendation and MUST/SHOULD distinction for campaign vs development (SC-015)

### Section 4.6 (Directory Structure)
- Added `church_add.rs` and `church_mul.rs` to bench/benchmarks/
- Added `io/examples.rs` reference
- Added note about generator delegation relationship (SC-003)

### Section 4.7 (Experimental Matrix)
- Added ChurchAdd (MUST) and ChurchMul (SHOULD) rows
- Updated mode count from 3 to 4
- Added notes on TCC campaign subset and mode count
- Updated comparison text

### Section 5.3 (Why statistical repetitions)
- Updated text to reference three-tier scheme (5/10/30)

### Section 5.5 (Why CSV schema)
- Rewritten from "unified CSV" to "three CSV files" with rationale

### Section 6.1 (Haskell mapping table)
- Added ChurchAdd and ChurchMul rows

### Section 6.2 (Metrics comparison table)
- Updated repetitions text to "min 5, TCC campaign 10, ideal 30"

### Section 7 (Open Questions)
- OQ-6: Marked as resolved (Church benchmarks now MUST/SHOULD)

### Section 3.8 (Haskell Comparison)
- R43: Changed TreeSum sizes from [50, 200, 1000, 2000] to [16, 64, 256] (SC-016)
