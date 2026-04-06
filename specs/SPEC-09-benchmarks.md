# SPEC-09: Benchmark Suite

**Status:** Revised v3
**Depends on:** SPEC-00 (Glossary), SPEC-01 (Invariants), SPEC-02 (Net Representation), SPEC-03 (Reduction Engine), SPEC-04 (Partitioning), SPEC-05 (Merge and Grid Cycle), SPEC-06 (Wire Protocol), SPEC-07 (Deployment), SPEC-08 (Test Strategy), SPEC-12 (User I/O -- canonical generators), SPEC-14 (Arithmetic Encoding -- Church numeral benchmarks)
**Gray zones resolved:** Z4 (communication overhead vs. parallelism benefit), Z6 (scalability transfer from shared to distributed memory), Z7 (work granularity)
**References consumed:** REF-001 (Lafont 1990), REF-002 (Lafont 1997), REF-003 (HVM2), REF-005 (Mackie & Pinto 2002), REF-007 (Casanova 2002), REF-013 (Mackie 1997), REF-014 (Kahl 2015), REF-017 (Foster, Kesselman, Tuecke 2001)
**Discussions consumed:** DISC-003 v2 (strong confluence to distributed determinism, P1-P5), DISC-006 v2 (overhead anatomy, break-even analysis, workload profiles), DISC-008 v2 (shared-to-distributed transition, 6 operational dimensions)
**Arguments consumed:** ARG-001 (central argument, P1-P6, fundamental property), ARG-004 (practical viability and limits, workload classification A/B/C, break-even, granularity thresholds)
**Code analyses consumed:** AC-005 (Haskell benchmark framework, 5 benchmarks, 9 CSVs, ~110 datapoints, 8 limitations, experimental results), AC-014 (HigherOrderCO bench methodology, wall-clock sampling, 29 benchmarks, gaps identified)

---

## 1. Purpose

This spec defines the complete benchmark suite for Relativist's experimental evaluation of distributed Interaction Combinator reduction. The suite has three goals: (1) verify the Fundamental Property (SPEC-01, G1) on every execution as the correctness criterion, (2) measure local and distributed reduction performance with quantitative metrics that address the gaps identified in the Haskell prototype (AC-005), and (3) produce structured data for Section 4 (Experimental Evaluation) of the TCC paper. The benchmarks cover the three overhead profiles identified in DISC-006 v2 and classified in ARG-004 (Profile A: embarrassingly parallel; Profile B: expansion with collapse; Profile C: sequential dependency), introduce networks that exercise all 6 interaction rules (correcting the prototype's ERA-ERA bias), provide scaling curves across 1-8 workers, include overhead breakdown per phase, and enable break-even analysis to answer the TCC's central empirical question: "when does distribution pay off?"

## 2. Definitions

Terms defined in SPEC-00 (Glossary) are used without redefinition. Terms introduced or refined in this spec:

| Term | Definition |
|------|-----------|
| **Benchmark** | A combination of (parametric net generator, correctness verifier, set of experimental configurations) that produces one or more datapoints. Each benchmark is identified by a `BenchmarkId`. |
| **Datapoint** | A single measured execution: (benchmark, size, num_workers, mode, repetition) -> BenchmarkResult. |
| **Overhead Profile** | Classification of a network with respect to distribution behavior. Profile A: embarrassingly parallel (independent redexes, 1 round, zero border redexes). Profile B: expansion with collapse (CON-DUP dominant, multiple rounds, emergent borders). Profile C: sequential dependency (level-dependent, many rounds, massive borders). (DISC-006 v2, Section 4.1; ARG-004, Part I, Step 6) |
| **MIPS** | Millions of Interactions Per Second. Throughput metric: `total_interactions / wall_clock_seconds / 1_000_000`. A critical gap in the Haskell prototype (AC-005 does not report it; AC-014 does not collect it). |
| **Border Ratio** | Fraction of border redexes over total redexes detected in a round: `border_redexes / (local_redexes + border_redexes)`. Quantifies partitioning quality (DISC-006 v2, Section 6.3). |
| **Overhead Ratio** | Fraction of time spent on overhead (partition + serialize + transfer + merge + border resolution) over total execution time: `T_overhead / T_total`. If > 0.5, distribution likely does not pay off (DISC-006 v2, Section 2.3). |
| **Efficiency** | Speedup divided by number of workers: `efficiency = speedup / K`. Perfect linear scaling yields efficiency = 1.0. Values above 1.0 indicate super-linear speedup (e.g., due to algorithmic artifacts such as findRedexes O(N^2) in the Haskell prototype). |
| **Worker Idle Time** | Time a worker spent idle waiting (between completion of its local reduction and start of the next round). Quantifies load imbalance. |
| **Warmup Run** | An execution discarded before timed measurements. Its purpose is to warm CPU caches and stabilize the system state. |
| **Sequential Baseline** | Execution of `reduce_all` on the complete net without partitioning. Serves as reference for speedup calculation. |
| **Break-Even Point** | The minimum network size (in redexes per worker) at which distributed execution yields speedup > 1.0 for a given mode and worker count. Determined empirically from scaling curves (DISC-006 v2, Section 3.3; ARG-004, Part II). |
| **Scaling Curve** | A plot of speedup (or MIPS, or overhead ratio) as a function of the number of workers for a fixed benchmark and size. The primary visualization for answering "when does distribution pay off?" |

---

## 3. Requirements

### 3.1 Benchmark Framework

**R1.** Relativist MUST implement a benchmark framework as a separate binary (`cargo bench` or `cargo run --bin bench`). The framework MUST support execution of individual benchmarks or the entire suite. **(MUST)**

**R2.** Each benchmark MUST be defined by a trait `Benchmark` with the following methods. **(MUST)**

```rust
/// Definition of a parametric benchmark.
pub trait Benchmark {
    /// Unique identifier of the benchmark.
    fn id(&self) -> BenchmarkId;

    /// Human-readable description for a given size.
    fn describe(&self, size: u32) -> String;

    /// Generate the input net for the specified size.
    fn make_net(&self, size: u32) -> Net;

    /// Default sizes for this benchmark (logarithmic variation).
    fn default_sizes(&self) -> Vec<u32>;

    /// Verify correctness: compare distributed result with sequential result.
    /// Returns true if the result is correct.
    fn verify(&self, sequential_result: &Net, distributed_result: &Net) -> bool;
}
```

**R3.** The framework MUST execute a sequential baseline (`reduce_all`) for each (benchmark, size) combination before any distributed execution. The sequential result MUST be preserved for correctness verification across all worker configurations. **(MUST)**

**R4.** The framework MUST verify the Fundamental Property (SPEC-01, G1) on EVERY datapoint: `reduce_all(net) ~= extract_result(run_grid(net, n))`, where `~=` denotes graph isomorphism (SPEC-08, `nets_isomorphic`) or benchmark-specific verification. An execution with `correct == false` MUST be reported as a failure. **(MUST)**

**R5.** The global success criterion of the suite MUST be: zero correctness failures across all datapoints. Any failure invalidates the entire suite and indicates a bug in the system. **(MUST)**

**R6.** The framework MUST support configuration via CLI (using `clap`) with the following parameters. **(MUST)**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `--benchmark` | `BenchmarkId` | `All` | Which benchmark to execute |
| `--sizes` | `Vec<u32>` | Defined per benchmark | Problem sizes |
| `--workers` | `Vec<u32>` | `[1, 2, 4, 8]` | Number of workers |
| `--mode` | `Mode` | `Local` | `Sequential`, `Local`, `TcpLocalhost`, `TcpNetwork` |
| `--warmup` | `u32` | `2` | Warmup runs discarded |
| `--repetitions` | `u32` | `5` | Timed repetitions |
| `--csv` | `Option<PathBuf>` | `None` | Path for CSV output |
| `--max-rounds` | `Option<u32>` | `None` | Grid loop round limit |

**R7.** The framework SHOULD integrate with the `criterion` crate for micro-benchmarks of the local reduction engine (SPEC-03). For distributed pipeline benchmarks, the framework MUST use direct timing with `std::time::Instant` (monotonic timer). **(SHOULD for criterion; MUST for Instant)**

### 3.2 Mandatory Benchmarks

**R8.** Relativist MUST implement at least 10 benchmarks organized by overhead profile. **(MUST)**

#### 3.2.1 Profile A -- Embarrassingly Parallel

**R9.** The **EP-Annihilation** benchmark MUST generate N pairs of ERA agents connected by principal ports. All redexes are independent. Expected result: 0 agents. Direct mapping from AC-005 `mkEPNet`. **(MUST)**

| Property | Value |
|----------|-------|
| Agents | 2N |
| Redexes | N |
| Rounds (grid) | 1 |
| Border redexes | 0 (contiguous partitioning) |
| Rules exercised | ERA-ERA (void) |
| Profile | A |
| Default sizes | [100, 500, 1_000, 5_000, 10_000, 50_000, 100_000] |

**R10.** The **EP-Annihilation-CON** benchmark MUST generate N pairs of CON agents connected by principal ports, with auxiliary ports connected to FreePorts. Expected result: 0 CON agents, FreePorts reconnected in cross pattern. **(MUST)**

| Property | Value |
|----------|-------|
| Agents | 2N |
| Redexes | N |
| Rounds (grid) | 1 |
| Border redexes | 0 |
| Rules exercised | CON-CON (annihilation cross) |
| Profile | A |
| Default sizes | [100, 500, 1_000, 5_000, 10_000, 50_000] |

**Rationale:** AC-005, Gap L6 identifies that the prototype only uses ERA in the primary EP benchmark. CON-CON pairs exercise the cross-reconnection that is distinct from trivial ERA-ERA annihilation. This directly addresses DISC-003 v2, Section 5.2, point 2: insufficient coverage of the 6 rules.

**R11.** The **EP-Annihilation-DUP** benchmark MUST generate N pairs of DUP agents connected by principal ports, with auxiliary ports connected to FreePorts. Expected result: 0 DUP agents, FreePorts reconnected in parallel pattern. **(MUST)**

| Property | Value |
|----------|-------|
| Agents | 2N |
| Redexes | N |
| Rounds (grid) | 1 |
| Border redexes | 0 |
| Rules exercised | DUP-DUP (annihilation parallel) |
| Profile | A |
| Default sizes | [100, 500, 1_000, 5_000, 10_000, 50_000] |

**Rationale:** Complements EP-Annihilation-CON. The asymmetry between CON-CON (cross) and DUP-DUP (parallel) reconnection is essential for the system's universality (REF-002, p. 90). Testing both is necessary to validate the correctness of the two non-trivial annihilation rules.

#### 3.2.2 Profile B -- Expansion with Collapse

**R12.** The **CON-DUP Expansion** benchmark MUST generate N pairs of CON-DUP connected by principal ports, with auxiliary ports connected to FreePorts. The net expands (CON-DUP rule: +2 agents per interaction) before collapsing via annihilation. Direct mapping from AC-005 `mkExpansionNet`. **(MUST)**

| Property | Value |
|----------|-------|
| Initial agents | 2N |
| Initial redexes | N |
| Rounds (grid) | Variable (> 1 for large N) |
| Border redexes | Variable (emergent from expansion) |
| Rules exercised | CON-DUP (commutation), CON-CON, DUP-DUP (post-expansion annihilation) |
| Profile | B |
| Default sizes | [10, 50, 100, 500, 1_000, 5_000] |

**Rationale:** The only benchmark where the net grows before collapsing (AC-005). Exercises the CON-DUP rule (commutation) plus resulting annihilations. Addresses AC-005 Gap L7 (CON-DUP undertested) and DISC-006 v2 Section 3.5 (CON-DUP paradox: expansion creates more work but also more communication). This is the primary benchmark for validating Profile B behavior, which remains conjectural with only 4 TCP datapoints in the Haskell prototype (ARG-004, Part I, Step 6).

#### 3.2.3 Profile C -- Sequential Dependency

**R13.** The **DualTree** benchmark MUST generate two perfect binary trees of CON agents with depth d, connected at the roots. Reduction cascades level by level with cross-connect. Direct mapping from AC-005 `mkDualTreeNet`. **(MUST)**

| Property | Value |
|----------|-------|
| Agents | 2 * (2^d - 1) |
| Interactions | 2^d - 1 |
| Rounds (grid) | d (minimum) |
| Border redexes | ~50% per level (cross-connect butterfly pattern) |
| Rules exercised | CON-CON (annihilation cross, cascade) |
| Profile | C |
| Default sizes (depth) | [4, 6, 8, 10, 12, 14] |

**Rationale:** Maximally adversarial workload for distribution (DISC-006 v2, Section 2.2). Demonstrates that strong confluence guarantees correctness even when distribution produces slowdown (0.18x in the Haskell prototype with 4 workers, AC-005). The per-level cost model (DISC-006 v2, Section 2.2) is more appropriate than Amdahl's Law because the serial fraction varies with K. Essential for honest reporting: the TCC must show both favorable and unfavorable cases.

#### 3.2.4 Data-Bound Benchmarks

**R14.** The **TreeSum** benchmark MUST generate a net equivalent to the Haskell prototype's `mkTree` (AC-004): a chain of CON agents (adders) connected to ERA-ERA pairs (work units). **(MUST)**

**Note:** TreeSum uses a pragmatic encoding inherited from the Haskell prototype (documented limitation in the TCC article, Section 5.3). The concepts of "adders" and "values" are semantic conventions external to pure IC theory. The `extract_result` function interprets the reduced net's structure by convention, not by a formal IC encoding. For formal arithmetic computation on IC nets, see the Church numeral benchmarks (SPEC-14, R17a below). **TreeSum's correctness verification MUST use graph isomorphism with the sequential result (`nets_isomorphic`), not semantic value extraction.** This ensures correctness is verified by the fundamental property (G1), regardless of encoding conventions.

| Property | Value |
|----------|-------|
| Agents | ~2N + N (CON + ERA pairs) |
| Redexes | N (ERA-ERA) + CON cascade |
| Rounds (grid) | Variable |
| Rules exercised | ERA-ERA, CON-ERA (propagation) |
| Profile | A/B (depends on size) |
| Default sizes | [4, 8, 16, 32, 64, 128, 256] |

**R15.** The **TreeSumBalanced** benchmark SHOULD be a variant of TreeSum with balanced distribution of ERA-ERA pairs across partitions. Equivalent to the Haskell prototype's `mkTreeBalanced` (AC-004). **(SHOULD)**

#### 3.2.5 Benchmarks Exercising All 6 Rules

**R16.** The **MixedNet** benchmark MUST generate a net containing redexes of ALL 6 interaction rule types in controlled proportions. **(MUST)**

The net MUST contain at least:
- CON-CON pairs (annihilation cross)
- DUP-DUP pairs (annihilation parallel)
- ERA-ERA pairs (void)
- CON-DUP pairs (commutation)
- CON-ERA pairs (erasure)
- DUP-ERA pairs (erasure)

| Property | Value |
|----------|-------|
| Agents | ~12N (2 agents per pair, 6 pair types, N pairs of each) |
| Redexes | 6N |
| Rounds (grid) | Variable (CON-DUP generates cascade) |
| Border redexes | Variable |
| Rules exercised | ALL 6 |
| Profile | B (due to CON-DUP) |
| Default sizes | [10, 50, 100, 500, 1_000] |

**Expected result:** 4N ERA agents, each connected to a unique FreePort, with 0 agents of other types. Derivation:
- ERA-ERA pairs (N): void -- 0 agents remaining.
- CON-CON pairs (N): annihilation cross -- 0 agents remaining (FreePorts cross-reconnected).
- DUP-DUP pairs (N): annihilation parallel -- 0 agents remaining (FreePorts parallel-reconnected).
- CON-DUP pairs (N): commutation produces 4 agents (2 CON + 2 DUP) per pair in a cross-connected pattern, generating 2 new redexes (CON-CON and DUP-DUP annihilation). After the cascade, 0 agents remaining.
- CON-ERA pairs (N): erasure produces 2 ERA agents per pair, each connected to a FreePort. 2N ERA agents.
- DUP-ERA pairs (N): erasure produces 2 ERA agents per pair, each connected to a FreePort. 2N ERA agents.
- Total final agents: 4N (all ERA), 0 redexes (normal form).

All pairs are fully independent (unique FreePort IDs per SPEC-12 R41), so post-CON-DUP agents interact only within their own group.

**Rationale:** Directly addresses DISC-003 v2 Section 5.2 point 2 ("insufficient coverage of the 6 rules in a distributed context"), AC-005 Gap L6, and SPEC-08 Requirement R21. No benchmark in the Haskell prototype exercises all 6 rules in a single net.

**R17.** The **ErasurePropagation** benchmark MUST generate a net with N CON (or DUP) agents connected in a chain, with an ERA at one end. Reduction propagates the ERA through the chain, exercising the CON-ERA (or DUP-ERA) rule repeatedly. **(MUST)**

| Property | Value |
|----------|-------|
| Agents | N + 1 (N CON/DUP + 1 ERA) |
| Redexes | 1 (initial), cascade of N |
| Rounds (grid) | Depends on distribution |
| Border redexes | Variable (propagation may cross boundaries) |
| Rules exercised | CON-ERA or DUP-ERA (erasure propagation) |
| Profile | C (sequential cascade) |
| Default sizes | [10, 50, 100, 500, 1_000, 5_000] |

**Expected result:** The net reaches normal form with (N+1) ERA agents, each connected to a FreePort, and 0 redexes. Derivation: the initial ERA interacts with CON_0 (CON-ERA rule), removing CON_0 and producing 2 new ERA agents -- one continues the cascade to CON_1 (via the chain port), and one terminates at CON_0's free port. This process repeats down the chain. The last CON (CON_{N-1}) has both auxiliary ports connected to FreePorts, so its erasure produces 2 ERAs connected to FreePorts. Total: N ERAs from the free-port branches along the chain (one per CON), plus 1 ERA from the last step of the cascade. The verifier MUST check: (a) all remaining agents are ERA, (b) the net is in normal form (0 redexes), (c) the result matches the sequential baseline by graph isomorphism.

**Note:** The expected result is NOT 0 agents. ERA agents with arity 0 connected to FreePorts are in normal form; they cannot participate in any interaction and remain in the net as the reduction's output.

**Rationale:** The erasure rules (CON-ERA and DUP-ERA) produce 2 new ERA agents per interaction, propagating erasure in a cascade. This pattern is distinct from annihilation (EP) and expansion (CON-DUP), and is common in real programs with garbage collection of unused subterms. Complements MixedNet with a specific propagation pattern.

#### 3.2.6 Arithmetic Encoding Benchmark

**R17a.** The **ChurchAdd** benchmark MUST generate a net encoding Church numeral addition: `build_add(N/2, N - N/2)` where N is the size parameter, using the Church numeral encoding defined in SPEC-14. The result MUST be verified by `decode_nat` (SPEC-14 R22) producing the expected sum, AND by graph isomorphism with the sequential result. **(MUST)**

| Property | Value |
|----------|-------|
| Initial agents | O(N) (two Church numerals + add combinator) |
| Initial redexes | O(1) (beta-reduction entry point) |
| Rounds (grid) | Variable (multiple, due to CON-DUP expansion phase) |
| Border redexes | Variable (emergent from CON-DUP commutation during beta-reduction) |
| Rules exercised | CON-DUP (commutation during duplication), CON-CON, DUP-DUP (post-expansion annihilation), CON-ERA, DUP-ERA (erasure of unused branches) |
| Profile | B (DUP-CON expansion during beta-reduction, then collapse) |
| Default sizes | [10, 50, 100, 500] |

**Rationale:** Church arithmetic is the primary demonstration that Relativist performs real computation (not just graph manipulation). SPEC-14 Section 1 identifies Church arithmetic as "essential for the TCC experimental evaluation (SPEC-09) and defense." The ChurchAdd benchmark produces Profile B nets via a semantically meaningful computation, complementing the synthetic CON-DUP Expansion benchmark with a workload that has a verifiable numeric result. The generators reside in `src/io/examples.rs` (SPEC-12 R35-R36, SPEC-14 R26-R27) and are shared with the `compute` subcommand.

**R17b.** The **ChurchMul** benchmark SHOULD generate a net encoding Church numeral multiplication: `build_mul(a, b)` where a and b are derived from the size parameter. ChurchMul produces O(a*b) interactions from an O(a+b) initial net, providing a natural expansion scaling curve. The result MUST be verified by `decode_nat` AND by graph isomorphism with the sequential result. **(SHOULD)**

| Property | Value |
|----------|-------|
| Initial agents | O(a + b) |
| Initial redexes | O(1) |
| Rounds (grid) | Variable (many, due to larger expansion) |
| Rules exercised | Same as ChurchAdd (all 6 rules via beta-reduction) |
| Profile | B (larger expansion factor than ChurchAdd) |
| Default sizes (a=b) | [5, 10, 20, 50] |

### 3.3 Mandatory Metrics

**R18.** Each datapoint MUST produce a `BenchmarkResult` structure with all metrics necessary for experimental analysis. **(MUST)**

```rust
/// Complete result of a single benchmark execution.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BenchmarkResult {
    // --- Identification ---
    /// Benchmark identifier.
    pub benchmark: BenchmarkId,
    /// Problem size (semantics defined per benchmark).
    pub input_size: u32,
    /// Execution mode.
    pub mode: Mode,
    /// Number of workers (0 for sequential).
    pub workers: u32,
    /// Repetition index (0-indexed).
    pub repetition: u32,

    // --- Correctness ---
    /// The Fundamental Property was verified and holds.
    pub correct: bool,

    // --- Timing ---
    /// Total wall-clock time in seconds.
    pub wall_clock_secs: f64,

    // --- Interactions ---
    /// Total interactions (reductions) performed.
    pub total_interactions: u64,
    /// MIPS: Millions of Interactions Per Second.
    pub mips: f64,
    /// Interactions broken down by rule type.
    pub interactions_by_rule: InteractionsByRule,

    // --- Grid metrics ---
    /// Number of grid loop rounds (0 for sequential).
    pub rounds: u32,
    /// Border redexes detected per round.
    pub border_redexes_per_round: Vec<u32>,
    /// Border ratio per round: border_redexes / total_redexes_in_round.
    pub border_ratio_per_round: Vec<f64>,

    // --- Memory ---
    /// Peak memory usage in bytes (entire process).
    pub peak_memory_bytes: u64,
    /// Agents in the net at the start of each round.
    pub agents_per_round: Vec<usize>,

    // --- Communication (populated only in distributed modes) ---
    /// Total bytes sent (all messages, all rounds).
    pub bytes_sent: u64,
    /// Total bytes received.
    pub bytes_received: u64,
    /// Bytes sent per round.
    pub bytes_sent_per_round: Vec<u64>,
    /// Bytes received per round.
    pub bytes_received_per_round: Vec<u64>,

    // --- Time breakdown per phase ---
    /// Partitioning time per round (seconds).
    pub partition_time_per_round: Vec<f64>,
    /// Local reduction time (all workers) per round (seconds).
    pub compute_time_per_round: Vec<f64>,
    /// Merge + border resolution time per round (seconds).
    pub merge_time_per_round: Vec<f64>,
    /// Network communication time per round (seconds, distributed modes).
    pub network_time_per_round: Vec<f64>,

    // --- Per-worker metrics ---
    /// Per-worker statistics per round.
    pub worker_stats: Vec<Vec<WorkerBenchStats>>,

    // --- Derived metrics (computed after execution) ---
    /// Speedup vs sequential baseline: T_seq / T_grid.
    pub speedup: f64,
    /// Efficiency: speedup / workers.
    pub efficiency: f64,
    /// Overhead ratio: T_overhead / T_total, where T_overhead = T_total - T_compute.
    pub overhead_ratio: f64,
}

/// Interactions broken down by rule type.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct InteractionsByRule {
    pub con_con: u64,
    pub dup_dup: u64,
    pub era_era: u64,
    pub con_dup: u64,
    pub con_era: u64,
    pub dup_era: u64,
}

/// Per-worker statistics for a single round of a benchmark execution.
#[derive(Debug, Clone, serde::Serialize)]
pub struct WorkerBenchStats {
    pub worker_id: u32,
    /// Agents in partition before local reduction.
    pub agents_before: usize,
    /// Agents in partition after local reduction.
    pub agents_after: usize,
    /// Local redexes reduced by this worker.
    pub local_interactions: u64,
    /// Local reduction time for this worker (seconds).
    pub compute_time_secs: f64,
    /// Idle time for this worker (seconds).
    /// Computed as: max(all_worker_compute_times) - this_worker_compute_time.
    pub idle_time_secs: f64,
}
```

**R19.** The following metrics MUST be reported for EACH datapoint. **(MUST)**

| Metric | Field | Unit | Source | Gap addressed |
|--------|-------|------|--------|---------------|
| Wall-clock time | `wall_clock_secs` | seconds | `std::time::Instant` | Present in prototype |
| Total interactions | `total_interactions` | count | SPEC-03 interaction counter | Present in prototype |
| MIPS | `mips` | M interactions/s | Derived: interactions / time / 1e6 | AC-005 does not highlight; AC-014 does not collect |
| Interactions by rule | `interactions_by_rule` | count per type | SPEC-03, R16 (per-rule counter) | Prototype does not discriminate |
| Peak memory | `peak_memory_bytes` | bytes | OS query (`/proc/self/status` or equivalent) | No IC benchmark collects this |
| Bytes transferred | `bytes_sent`, `bytes_received` | bytes | Wire protocol counters (SPEC-06) | Prototype collects in distributed CSV |
| Border ratio | `border_ratio_per_round` | fraction [0,1] | Derived: borders / total redexes | DISC-006 v2, Section 6.3 |
| Worker idle time | `WorkerBenchStats.idle_time_secs` | seconds | Derived: max_worker_time - this_worker_time | Prototype does not measure |
| Overhead ratio | `overhead_ratio` | fraction [0,1] | Derived: (total - compute) / total | DISC-006 v2, Section 2.3 |
| Speedup | `speedup` | ratio | Derived: T_seq / T_grid | Present in prototype |
| Efficiency | `efficiency` | ratio | Derived: speedup / workers | Not in prototype |
| Correctness | `correct` | bool | SPEC-08, isomorphism or benchmark-specific verifier | Present in prototype |

**R20.** Derived metrics MUST be computed by the framework after each execution, not by the user. **(MUST)**

- `mips = total_interactions as f64 / wall_clock_secs / 1_000_000.0`
- `speedup = baseline_sequential_time / wall_clock_secs` (for workers >= 1). For speedup computation, `baseline_sequential_time` is the **median** wall-clock time of all sequential repetitions for the same (benchmark, size) combination.
- `efficiency = speedup / workers as f64`
- `overhead_ratio = 1.0 - (sum(compute_time_per_round) / wall_clock_secs)` (for grid executions, workers >= 1)
- `border_ratio_per_round[i] = border_redexes_per_round[i] as f64 / (local_redexes_in_round[i] + border_redexes_per_round[i]) as f64`
- `WorkerBenchStats.idle_time_secs = max(all_worker_compute_times) - this_worker_compute_time`

**Sequential baseline special cases (workers == 0):** For the sequential baseline, `speedup` MUST be 1.0, `efficiency` MUST be 1.0, and `overhead_ratio` MUST be 0.0 (by convention, since the baseline has no parallelism overhead to measure and is 100% compute). The formulas for `speedup`, `efficiency`, and `overhead_ratio` apply only to grid executions (workers >= 1). This avoids division-by-zero (efficiency = speedup / 0) and incorrect values (overhead_ratio formula produces 1.0 for sequential because `compute_time_per_round` is empty).

### 3.4 Experimental Variables

**R21.** The suite MUST systematically vary the following experimental variables. **(MUST)**

#### 3.4.1 Number of Workers

**R22.** Each benchmark MUST be executed with 1, 2, 4, and 8 workers. The 1-worker case MUST be executed in grid mode (with trivial partitioning, SPEC-04 R2) to measure the protocol overhead in isolation. **(MUST)**

**R23.** The 0-worker case (pure sequential, `reduce_all` without grid) MUST be included as baseline. **(MUST)**

#### 3.4.2 Network Size

**R24.** Each benchmark MUST be executed with at least 5 different sizes in logarithmic variation (e.g., 100, 500, 1000, 5000, 10000). Logarithmic variation is necessary for plotting scaling curves on log-log graphs. **(MUST)**

**R25.** The default sizes for each benchmark MUST be defined by the benchmark (via `default_sizes()`) to reflect the semantics of the size parameter (N = pairs for EP, depth for DualTree, N = items for TreeSum). **(MUST)**

#### 3.4.3 Execution Mode

**R26.** The suite MUST support four execution modes. **(MUST)**

| Mode | Description | Workers | Communication |
|------|-------------|---------|---------------|
| `Sequential` | Pure sequential: `reduce_all` without grid. Bypasses partitioning, merge, and all protocol infrastructure. Ignores `--workers` (always 0 workers). This mode produces the baseline for speedup calculations. | None (0) | None |
| `Local` | Simulated grid in a single process. Workers are sequential function calls (or parallel via `rayon`). | Simulated | None |
| `TcpLocalhost` | Grid with workers as separate processes on the same host, communicating via TCP localhost. | Real processes | TCP 127.0.0.1 |
| `TcpNetwork` | Grid with workers on distinct physical machines, communicating via TCP over a real network. | Real processes on separate machines | TCP over LAN |

**Note:** `--mode Sequential` is the canonical way to request the sequential baseline. `--workers 0` with any distributed mode (Local, TcpLocalhost, TcpNetwork) MUST be treated as equivalent to `--mode Sequential` for convenience.

**R27.** The suite MUST support the `TcpNetwork` mode for execution on separate physical machines with at least 4 and 8 workers. Deployment follows the bare-metal procedure defined in SPEC-07 (R41, Section 4.12). **(MUST)**

#### 3.4.4 Partitioning Strategy

**R28.** The baseline partitioning strategy MUST be round-robin by ID (SPEC-04). **(MUST)**

**R29.** The suite MAY include alternative strategies (redex-aware, etc.) as future experimental variables. **(MAY)**

### 3.5 Statistical Methodology

**R30.** Each datapoint MUST be produced by a measurement protocol with warmup and repetitions. **(MUST)**

The protocol MUST be:

```
For each (benchmark, size, workers, mode):
    1. Generate net: net = benchmark.make_net(size)
    2. Execute sequential baseline (if not already executed for this (benchmark, size))
    3. Warmup: execute W times (default W=2), discarding results
    4. Measure: execute R times (default R=5), collecting BenchmarkResult
    5. Verify correctness on EACH repetition
    6. Aggregate: compute statistics over the R repetitions
```

**R31.** The minimum number of repetitions MUST be 5 (for development and debugging runs). For publishable results in the TCC paper, the number SHOULD be at least 10 (sufficient for bootstrap 95% CI computation, consistent with the article text in Section 4.6 and DATA-COLLECTION-PLAN Section 2.1). For production-grade benchmarks with CLT-based parametric confidence intervals, 30 is recommended. **(MUST for 5; SHOULD for 10 in the TCC campaign; 30 recommended for CLT-based CI)**

**R32.** The framework MUST compute and report the following aggregation statistics over repetitions. **(MUST)**

| Statistic | Computation | Purpose |
|-----------|-------------|---------|
| Mean | `mean(values)` | Central tendency |
| Standard deviation | `std(values)` | Dispersion |
| Median | `median(values)` | Robust central tendency (resilient to outliers). **Primary** measure of central tendency for all timing metrics in the TCC (robust to right-skewed timing distributions). |
| Minimum | `min(values)` | Best case |
| Maximum | `max(values)` | Worst case |

**R32a.** The framework SHOULD compute bootstrap 95% confidence intervals for `wall_clock_secs`, `mips`, and `speedup` using 10,000 resamples on the median. Bootstrap is appropriate because (a) 10 repetitions is too few for CLT-based parametric CIs, and (b) timing distributions are typically right-skewed (long tail of slow runs), making the median more representative than the mean (DATA-COLLECTION-PLAN Section 6.3). If bootstrap CI computation is not implemented in Rust, it MAY be deferred to the Python post-processing scripts (`scripts/plot_results.py`), provided the detail.csv contains all raw per-repetition data needed for offline bootstrap computation. **(SHOULD)**

**R33.** The statistics MUST be computed for `wall_clock_secs`, `mips`, `speedup`, `efficiency`, and `overhead_ratio`. **(MUST)**

**R34.** The framework SHOULD also report the coefficient of variation (CV = std/mean) for `wall_clock_secs`. If CV > 0.10 (variation > 10%), the framework SHOULD emit a high-variance warning. **(SHOULD)**

**R35.** For benchmarks with very fast execution (< 1ms), the framework SHOULD use the `criterion` crate for micro-benchmarking with automatic iteration control. **(SHOULD)**

### 3.6 Correctness on Every Execution

**R36.** Correctness verification MUST be executed on EACH repetition of EACH datapoint, not just once per configuration. **(MUST)**

**R37.** Verification MUST use the benchmark-specific verifier (`Benchmark::verify`). For benchmarks with an empty normal form (EP, DualTree), the verifier MUST check that the result net has 0 agents. For benchmarks with a non-trivial result (TreeSum, MixedNet, ErasurePropagation, ChurchAdd), the verifier MUST compare with the sequential result by graph isomorphism (SPEC-08, `nets_isomorphic`) or by a benchmark-specific metric. **(MUST)**

**R37a.** Graph isomorphism for correctness verification MAY use a lightweight check when performance is a concern: (a) same agent count per symbol, (b) same wire count, (c) same free port count, (d) same redex count (must be 0 for normal form). Full structural isomorphism (canonical graph comparison via `nets_isomorphic`) SHOULD be used when the lightweight check passes and the result is small (< 1000 agents). For empty-result benchmarks (EP, DualTree), the verifier SHOULD simply check `agent_count == 0`. **(MAY for lightweight; SHOULD for full isomorphism on small nets)**

**R37b.** For the sequential baseline, correctness MUST be verified by comparing the result of each repetition against the first sequential result by graph isomorphism. This validates invariant T6 (uniqueness of normal form, SPEC-01): all sequential runs MUST produce the same normal form regardless of reduction order. A mismatch indicates a reduction engine bug. **(MUST)**

**R38.** A correctness failure on any datapoint MUST halt the suite and report full details: benchmark, size, workers, mode, repetition, and the nature of the divergence (agent count, topology, etc.). **(MUST)**

### 3.7 Output

**R39.** The framework MUST produce CSV output organized into three files. **(MUST)**

**R39a.** `detail.csv`: One row per (benchmark, input_size, workers, mode, repetition). This is the raw data file. Schema:

```
benchmark,input_size,mode,workers,repetition,correct,
wall_clock_secs,total_interactions,mips,
rounds,speedup,efficiency,overhead_ratio,
peak_memory_bytes,bytes_sent,bytes_received,
con_con,dup_dup,era_era,con_dup,con_era,dup_era
```

**R39b.** `rounds.csv`: One row per (benchmark, input_size, workers, mode, repetition, round). Only populated for distributed modes (Local, TcpLocalhost, TcpNetwork). Enables per-round analysis of overhead evolution and border ratio dynamics without JSON-in-CSV. Schema:

```
benchmark,input_size,workers,mode,repetition,round,
partition_time_secs,compute_time_secs,merge_time_secs,network_time_secs,
border_redexes,border_ratio,agents_at_start,bytes_sent,bytes_received
```

**Note:** The per-phase time columns (`t_partition`, `t_compute`, `t_merge`, `t_network`) from the previous version's single-file schema are now in `rounds.csv` per round, enabling finer-grained analysis. The `detail.csv` contains only scalar aggregates per execution.

**Differences from the Haskell prototype:**
- Three-file schema (detail, rounds, summary) vs. two distinct schemas for local and distributed (AC-005)
- Includes `repetition` (vs. single execution, AC-005 Gap L8)
- Includes `mips` (vs. absent, AC-014 Cross-Cutting Concern 1)
- Includes `efficiency` (vs. absent)
- Includes `peak_memory_bytes` (vs. absent, AC-014 Cross-Cutting Concern 3)
- Includes per-rule counters (vs. total only, AC-005)
- Includes `overhead_ratio` and `border_ratio` (vs. computed externally)
- Per-round data in separate file (vs. variable-length columns or external processing)

**R40.** The framework MUST produce a `summary.csv` aggregation file with statistics grouped by (benchmark, size, workers, mode). The summary MUST include: median, standard deviation, min, max for `wall_clock_secs`, `mips`, `speedup`, `efficiency`, and `overhead_ratio`. If bootstrap CI computation is implemented in Rust (R32a), the summary MUST also include `wall_clock_ci95_lo`, `wall_clock_ci95_hi`, `speedup_ci95_lo`, `speedup_ci95_hi`, `mips_ci95_lo`, `mips_ci95_hi`. If bootstrap is deferred to Python, these columns MAY be absent from the Rust-produced summary.csv and computed offline. **(MUST for descriptive statistics; SHOULD for CI columns)**

**R41.** The framework SHOULD produce formatted table output to the terminal for real-time monitoring. **(SHOULD)**

**R42.** The CSVs produced MUST be compatible with analysis in Python (pandas), R, or spreadsheets. Delimiter: comma. Encoding: UTF-8. Decimals: dot. **(MUST)**

### 3.8 Comparison with Haskell Prototype

**R43.** The benchmarks EP-Annihilation, DualTree, CON-DUP Expansion, TreeSum, and TreeSumBalanced MUST use the same problem sizes as the Haskell prototype (AC-005) in at least a subset of executions, to enable direct comparison. **(MUST)**

Comparison sizes:

| Benchmark | Haskell sizes (AC-005) |
|-----------|------------------------|
| EP-Annihilation | 100, 500, 1000, 2000, 5000, 10000 |
| DualTree | depth 8, 10, 12, 14 |
| CON-DUP Expansion | 50, 100, 1000 |
| TreeSum | 16, 64, 256 |

**R44.** Comparison MUST be qualitative (trends, overhead profiles, break-even patterns) and NOT direct in absolute time (different languages, different hardware). Comparable metrics: speedup, efficiency, overhead ratio, border ratio, correctness. **(MUST)**

**R45.** The benchmark suite MUST produce a Haskell-vs-Rust comparison table for Section 4 of the TCC paper, showing for each shared benchmark: speedup trend (Haskell vs Rust at comparable worker counts), overhead ratio trend, border ratio, correctness status, and whether super-linear speedup persists or disappears (expected: disappears due to incremental redex queue, ARG-004 Part I Steps 2-3). **(MUST)**

### 3.9 Break-Even Analysis

**R46.** The benchmark suite MUST produce data sufficient to identify the break-even point for each benchmark and mode combination. **(MUST)**

The break-even point is the smallest input size at which `speedup > 1.0` for a given (benchmark, workers, mode). It is identified by:
1. Running each benchmark at logarithmically spaced sizes (R24).
2. Computing speedup for each (size, workers, mode).
3. Plotting speedup vs. size to identify the crossing of the `speedup = 1.0` line.

**R47.** For each benchmark, the framework SHOULD report the estimated break-even size per worker count in the aggregation CSV and terminal output. **(SHOULD)**

**Rationale:** DISC-006 v2 Section 3.3 estimates thresholds of ~500-1000 redexes/worker for the Haskell prototype (with findRedexes O(N^2)) and ~5000-10000 redexes/worker for an optimized Rust implementation (with O(1) redex queue). ARG-004 Part II emphasizes that these are order-of-magnitude estimates that MUST be validated empirically. The break-even analysis is the primary empirical contribution addressing Z7 (granularity).

### 3.10 Overhead Breakdown

**R48.** For each distributed datapoint, the framework MUST record and report time spent in each phase of the grid protocol. **(MUST)**

The phases are:

| Phase | Description | Source |
|-------|-------------|--------|
| `partition` | Splitting the net into K sub-nets + generating boundary FreePorts (SPEC-04) | DISC-006 v2, Section 1.1 |
| `serialize` | Converting each partition to bincode bytes (SPEC-06) | DISC-006 v2, Section 1.1 |
| `transfer` | Sending/receiving partitions via TCP (SPEC-06) | DISC-006 v2, Section 1.1 |
| `compute` | Local reduction on workers (SPEC-03, `reduce_all`) | DISC-006 v2, Section 1.1 |
| `merge` | Recombining partitions in the coordinator (SPEC-05) | DISC-006 v2, Section 1.1 |
| `border` | Resolving border redexes (SPEC-05) | DISC-006 v2, Section 1.1 |

**Note:** In the Haskell prototype, `remap` is a separate phase (AC-005). In Relativist, ID remapping is integrated into merge (SPEC-05) via pre-allocated ID ranges, so there is no separate remap phase. However, `serialize` and `transfer` are reported together as `network` in the per-round breakdown (since serialization is performed as part of the send operation). The fine-grained breakdown (6 phases) SHOULD be available via a `--verbose-breakdown` flag; the default breakdown is 4 phases: `partition`, `compute`, `merge` (including border), `network` (including serialize + transfer).

**R49.** The framework MUST produce per-round overhead breakdown data for each distributed execution, enabling visualization of overhead evolution across rounds (e.g., DualTree's decreasing payload per round, DISC-006 v2 Section 1.2). **(MUST)**

---

## 4. Design

### 4.1 Types

```rust
/// Benchmark identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum BenchmarkId {
    EPAnnihilation,
    EPAnnihilationCon,
    EPAnnihilationDup,
    ConDupExpansion,
    DualTree,
    TreeSum,
    TreeSumBalanced,
    MixedNet,
    ErasurePropagation,
    ChurchAdd,
    ChurchMul,
}

/// Execution mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Mode {
    /// Pure sequential: reduce_all without grid.
    Sequential,
    /// Simulated grid in a single process.
    Local,
    /// Grid with separate processes via TCP localhost.
    TcpLocalhost,
    /// Grid with physical machines via real-network TCP.
    TcpNetwork,
}

/// Configuration for a benchmark suite run.
#[derive(Debug, Clone)]
pub struct BenchmarkSuiteConfig {
    /// Which benchmarks to execute.
    pub benchmarks: Vec<BenchmarkId>,
    /// Worker counts to test.
    pub workers: Vec<u32>,
    /// Modes to test.
    pub modes: Vec<Mode>,
    /// Warmup runs discarded.
    pub warmup: u32,
    /// Timed repetitions.
    pub repetitions: u32,
    /// Path for detail CSV output (one row per repetition). R39a.
    pub csv_detail: Option<PathBuf>,
    /// Path for rounds CSV output (one row per round per execution). R39b.
    pub csv_rounds: Option<PathBuf>,
    /// Path for aggregation CSV output (one row per configuration). R40.
    pub csv_summary: Option<PathBuf>,
    /// Grid loop round limit.
    pub max_rounds: Option<u32>,
}

/// Aggregated statistics over a set of repetitions.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AggregatedStats {
    pub benchmark: BenchmarkId,
    pub input_size: u32,
    pub mode: Mode,
    pub workers: u32,
    pub num_repetitions: u32,
    pub correct_count: u32,
    // Wall-clock time
    pub time_mean: f64,
    pub time_std: f64,
    pub time_median: f64,
    pub time_min: f64,
    pub time_max: f64,
    pub time_cv: f64,
    // Bootstrap 95% CI (R32a, SHOULD -- may be computed offline by Python)
    pub time_ci95_lo: Option<f64>,
    pub time_ci95_hi: Option<f64>,
    // MIPS
    pub mips_mean: f64,
    pub mips_median: f64,
    pub mips_std: f64,
    pub mips_ci95_lo: Option<f64>,
    pub mips_ci95_hi: Option<f64>,
    // Speedup
    pub speedup_mean: f64,
    pub speedup_median: f64,
    pub speedup_std: f64,
    pub speedup_ci95_lo: Option<f64>,
    pub speedup_ci95_hi: Option<f64>,
    // Efficiency
    pub efficiency_mean: f64,
    pub efficiency_median: f64,
    pub efficiency_std: f64,
    // Overhead ratio
    pub overhead_ratio_mean: f64,
    pub overhead_ratio_median: f64,
    pub overhead_ratio_std: f64,
    // Memory
    pub peak_memory_mean: f64,
    // Communication
    pub bytes_sent_mean: f64,
    pub bytes_received_mean: f64,
    // Break-even indicator
    pub speedup_above_one: bool,
}
```

### 4.2 Net Generators (Informative)

> **Cross-spec note:** The canonical generator implementations are defined in SPEC-12 (R35-R42a). The pseudocode below is **illustrative only** and is provided to convey the intent of each benchmark. Benchmarks MUST use the generators from `src/io/examples.rs` (SPEC-12 R36) via `Benchmark::make_net`, which delegates to the shared generator functions. If any discrepancy exists between the pseudocode here and SPEC-12's Rust generators, SPEC-12 is authoritative.

Each benchmark implements the `Benchmark` trait. Net generators are illustrated per benchmark:

#### EP-Annihilation (ERA)

```
fn make_net(n: u32) -> Net:
    for i in 0..n:
        a = create_agent(Era, id=2*i)
        b = create_agent(Era, id=2*i+1)
        connect(AgentPort(a, 0), AgentPort(b, 0))  // redex
    // Result: n redexes, 2n agents, all independent
```

#### EP-Annihilation-CON

```
fn make_net(n: u32) -> Net:
    for i in 0..n:
        a = create_agent(Con, id=2*i)
        b = create_agent(Con, id=2*i+1)
        connect(AgentPort(a, 0), AgentPort(b, 0))  // redex CON-CON
        // Auxiliary ports connected to FreePorts
        connect(AgentPort(a, 1), FreePort(4*i))
        connect(AgentPort(a, 2), FreePort(4*i+1))
        connect(AgentPort(b, 1), FreePort(4*i+2))
        connect(AgentPort(b, 2), FreePort(4*i+3))
```

#### EP-Annihilation-DUP

Identical to CON, but with `Dup` instead of `Con`. Post-annihilation reconnection is parallel instead of cross.

#### CON-DUP Expansion

```
fn make_net(n: u32) -> Net:
    for i in 0..n:
        a = create_agent(Con, id=2*i)
        b = create_agent(Dup, id=2*i+1)
        connect(AgentPort(a, 0), AgentPort(b, 0))  // redex CON-DUP
        connect(AgentPort(a, 1), FreePort(4*i))
        connect(AgentPort(a, 2), FreePort(4*i+1))
        connect(AgentPort(b, 1), FreePort(4*i+2))
        connect(AgentPort(b, 2), FreePort(4*i+3))
```

#### DualTree

```
fn make_net(depth: u32) -> Net:
    if depth < 1: return empty net
    tree_size = 2^depth - 1
    // Tree A: agents [0..tree_size-1], symbol Con
    // Tree B: agents [tree_size..2*tree_size-1], symbol Con
    build_tree(base_a=0, depth, Con)
    build_tree(base_b=tree_size, depth, Con)
    connect(AgentPort(0, 0), AgentPort(tree_size, 0))  // root-root: single redex
    // Internal wires: array-based layout (node i -> children 2i+1, 2i+2)
    // Leaves connected to FreePorts
```

#### MixedNet

```
fn make_net(n: u32) -> Net:
    // n pairs of each rule type
    offset = 0
    for each type in [ConCon, DupDup, EraEra, ConDup, ConEra, DupEra]:
        for i in 0..n:
            create pair of the type with ids starting at offset
            offset += 2  // (or more for types with auxiliary ports)
    // All unmatched auxiliary ports connect to FreePorts
```

#### ErasurePropagation

```
fn make_net(n: u32) -> Net:
    // Chain of n CON: CON_0 -> CON_1 -> ... -> CON_(n-1)
    // ERA connected to principal port of CON_0
    era = create_agent(Era)
    for i in 0..n:
        con_i = create_agent(Con)
    // Connect ERA.port0 <-> CON_0.port0 (first redex)
    connect(AgentPort(era, 0), AgentPort(con_0, 0))
    // Connect chain: CON_i.port1 <-> CON_(i+1).port0
    for i in 0..n-1:
        connect(AgentPort(con_i, 1), AgentPort(con_{i+1}, 0))
    // CON_i.port2 -> FreePort (free auxiliary port)
    // CON_(n-1).port1 and port2 -> FreePorts
```

### 4.3 Execution Protocol

> **Note:** The `run_grid(net, workers, mode)` call below is pseudocode for the benchmark framework's wrapper function, not a direct invocation of SPEC-05's `run_grid(net: Net, num_workers: u32, strategy: impl PartitionStrategy)`. The wrapper selects the appropriate transport (in-memory channel for Local mode, TCP for TcpLocalhost/TcpNetwork) and then delegates to the grid cycle (SPEC-05 R25) via the system architecture (SPEC-13).

```
fn run_benchmark_suite(config: BenchmarkSuiteConfig):
    results_detail: Vec<BenchmarkResult> = []
    results_summary: Vec<AggregatedStats> = []

    for each benchmark in config.benchmarks:
        for each size in benchmark.default_sizes():
            net = benchmark.make_net(size)

            // Sequential baseline
            seq_results = []
            for r in 0..config.warmup:
                reduce_all(net.clone())  // discarded
            for r in 0..config.repetitions:
                result = measure_sequential(net.clone())
                assert!(result.correct, "Correctness failure in sequential!")
                seq_results.push(result)
            seq_baseline = median(seq_results.map(|r| r.wall_clock_secs))
            seq_net = reduce_all(net.clone())  // reference result

            for each mode in config.modes:
                for each w in config.workers:
                    // Warmup
                    for r in 0..config.warmup:
                        run_grid(net.clone(), w, mode)  // discarded

                    // Measurement
                    repetition_results = []
                    for r in 0..config.repetitions:
                        result = measure_grid(
                            net.clone(), w, mode, &seq_net, seq_baseline
                        )
                        assert!(result.correct,
                            "Correctness failure! bench={}, size={}, w={}, \
                             mode={:?}, rep={}", ...)
                        repetition_results.push(result)
                        results_detail.push(result)

                    // Aggregation
                    stats = aggregate(repetition_results)
                    results_summary.push(stats)

    // Output (3 CSV files per R39-R40)
    write_csv_detail(config.csv_detail, &results_detail)
    write_csv_rounds(config.csv_rounds, &results_detail)  // extract per-round data
    write_csv_summary(config.csv_summary, &results_summary)
```

### 4.4 Memory Measurement

```rust
/// Obtain the peak memory usage of the current process.
/// On Linux: reads /proc/self/status (VmHWM).
/// On other OSes: returns 0 (metric unavailable).
fn get_peak_memory_bytes() -> u64
```

Memory measurement MUST be performed after each benchmark execution. The implementation depends on the OS and MAY return 0 on platforms that do not support memory introspection. For Docker executions (SPEC-07), Linux is guaranteed and `/proc/self/status` (VmHWM) is available. On Windows (development platform), the framework SHOULD attempt to use `GetProcessMemoryInfo` from the Windows API or a cross-platform crate (`sysinfo`); if unavailable, it MUST return 0 and the limitation MUST be documented. The `peak_memory_bytes` metric is MUST for the TCC campaign (Linux/Docker) and SHOULD for development-time runs on non-Linux platforms.

### 4.5 Timing

```rust
use std::time::Instant;

fn measure_sequential(net: Net) -> BenchmarkResult {
    let start = Instant::now();
    let (result_net, stats) = reduce_all_with_stats(net);
    let elapsed = start.elapsed();

    BenchmarkResult {
        wall_clock_secs: elapsed.as_secs_f64(),
        total_interactions: stats.total_interactions,
        mips: stats.total_interactions as f64 / elapsed.as_secs_f64() / 1_000_000.0,
        interactions_by_rule: stats.by_rule,
        speedup: 1.0,
        efficiency: 1.0,
        // ... populate remaining fields
    }
}

fn measure_grid(
    net: Net,
    workers: u32,
    mode: Mode,
    seq_result: &Net,
    seq_baseline: f64,
) -> BenchmarkResult {
    let start = Instant::now();
    let (result_net, metrics) = run_grid(net, workers, mode);
    let elapsed = start.elapsed();

    let correct = benchmark.verify(seq_result, &result_net);
    let speedup = seq_baseline / elapsed.as_secs_f64();
    let efficiency = speedup / workers as f64;

    BenchmarkResult {
        wall_clock_secs: elapsed.as_secs_f64(),
        speedup,
        efficiency,
        correct,
        // ... derive remaining fields from metrics (GridMetrics, SPEC-05)
    }
}
```

The `Instant` timer is monotonic and is not affected by system clock adjustments (AC-014: equivalent to the `hrtime` used in HigherOrderCO's Bench).

### 4.6 Directory Structure

```
codigo/relativist/
  src/
    bench/
      mod.rs             # Benchmark trait, BenchmarkResult, AggregatedStats
      suite.rs           # BenchmarkSuiteConfig, run_benchmark_suite
      csv.rs             # CSV writing (detail, rounds, summary)
      stats.rs           # Statistical functions (mean, std, median, bootstrap CI)
      memory.rs          # get_peak_memory_bytes
    bench/benchmarks/
      mod.rs             # Re-exports all benchmarks (each implements Benchmark trait)
      ep_annihilation.rs     # EP-Annihilation (ERA, CON, DUP)
      condup_expansion.rs    # CON-DUP Expansion
      dual_tree.rs           # DualTree
      tree_sum.rs            # TreeSum and TreeSumBalanced
      mixed_net.rs           # MixedNet (6 rules)
      erasure_propagation.rs # ErasurePropagation
      church_add.rs          # ChurchAdd (SPEC-14)
      church_mul.rs          # ChurchMul (SPEC-14, SHOULD)
    io/
      examples.rs        # Canonical net generator functions (SPEC-12 R35-R42a)
  src/bin/
    bench.rs             # CLI binary for benchmarks (clap)
  benches/
    criterion_local.rs   # Criterion micro-benchmarks for local reduction
```

**Note:** The `bench/benchmarks/*.rs` files implement the `Benchmark` trait (configuration, sizes, verification). The actual net generation logic resides in `src/io/examples.rs` (SPEC-12 R36); benchmark implementations call `generate_<name>(size)` from there. This prevents generator duplication between the CLI `generate` subcommand and the benchmark suite.

### 4.7 Complete Experimental Matrix

The table below summarizes the complete experimental plan. The total number of datapoints depends on sizes and repetitions.

| Benchmark | Profile | Sizes | Configs/size | Reps | Datapoints |
|-----------|---------|-------|-------------|------|------------|
| EP-Annihilation | A | 7 | 13 | 5 | 455 |
| EP-Annihilation-CON | A | 6 | 13 | 5 | 390 |
| EP-Annihilation-DUP | A | 6 | 13 | 5 | 390 |
| CON-DUP Expansion | B | 6 | 13 | 5 | 390 |
| DualTree | C | 6 | 13 | 5 | 390 |
| TreeSum | A/B | 7 | 13 | 5 | 455 |
| TreeSumBalanced | A/B | 7 | 13 | 5 | 455 |
| MixedNet | B | 5 | 13 | 5 | 325 |
| ErasurePropagation | C | 6 | 13 | 5 | 390 |
| ChurchAdd | B | 4 | 13 | 5 | 260 |
| ChurchMul (SHOULD) | B | 4 | 13 | 5 | 260 |
| **Total** | | | | | **~4160** |

**Note on config count:** Each benchmark has 13 configurations per size: 1 sequential baseline (mode=Sequential, workers=0) + 4 worker counts (1,2,4,8) x 3 distributed modes (Local, TcpLocalhost, TcpNetwork) = 13. The Sequential mode (R26) is the baseline; it does not combine with non-zero worker counts.

**Note on TCC campaign:** The DATA-COLLECTION-PLAN defines a curated subset of this full matrix with 10 repetitions and 3 sizes per benchmark, producing ~2,700 datapoints -- a ~25x improvement over the Haskell prototype's ~110 datapoints. The full matrix above is for reference; the campaign matrix in DATA-COLLECTION-PLAN Section 3 is the operative plan for the TCC.

Comparison: the Haskell prototype produced ~110 datapoints (AC-005). Relativist will produce ~2,700 (TCC campaign with 10 reps) to ~4,160 (full matrix with 5 reps). The four modes (Sequential, Local, TcpLocalhost, TcpNetwork) allow isolating the cost of each layer: partitioning/merge overhead (Sequential vs Local), serialization + TCP overhead (Local vs TcpLocalhost), real network latency (TcpLocalhost vs TcpNetwork).

### 4.8 Visualizations for the Paper

The benchmark suite SHOULD produce or enable the following visualizations for Section 4 of the TCC paper:

1. **Scaling curves** (speedup vs. workers, per benchmark and size): demonstrates Profile A/B/C behavior and answers "does distribution help?"
2. **Overhead breakdown stacked bar charts** (partition/compute/merge/network per round): shows where time is spent and which phase dominates.
3. **Break-even plots** (speedup vs. size at fixed worker count): identifies the minimum viable network size per benchmark.
4. **Border ratio evolution** (border_ratio vs. round for DualTree and ErasurePropagation): shows how boundary effects evolve during multi-round execution.
5. **Haskell vs. Rust comparison table**: qualitative comparison of speedup trends, showing whether super-linear speedup disappears with the incremental redex queue.
6. **Efficiency heatmap** (benchmark x workers, color = efficiency): provides an at-a-glance view of where distribution is viable.

These visualizations MAY be produced by an external Python/R script reading the CSV output, not necessarily by the Rust binary itself.

---

## 5. Rationale

### 5.1 Why 7+ benchmarks instead of 5

The Haskell prototype has 5 benchmarks (AC-005): EP-Annihilation, TreeSum, TreeSumBalanced, CON-DUP Expansion, DualTree. Relativist adds EP-Annihilation-CON, EP-Annihilation-DUP, MixedNet, and ErasurePropagation. The reasons:

1. **Rule coverage.** The original EP uses only ERA-ERA. EP-CON and EP-DUP exercise the two other annihilation rules (CON-CON and DUP-DUP). MixedNet exercises all 6 rules in a single net. This addresses DISC-003 v2, Section 5.2 point 2 (insufficient coverage).

2. **Propagation pattern.** ErasurePropagation tests a cascade pattern (ERA propagation through CON/DUP chains) that is distinct from both annihilation (EP) and expansion (CON-DUP). This pattern is common in real reductions (garbage collection of unused subterms).

3. **All 6 rules in grid.** No benchmark in the prototype exercises all 6 rules simultaneously in a distributed context. MixedNet guarantees this coverage.

### 5.2 Why MIPS as a mandatory metric

AC-005 reports `int_per_sec` but does not highlight it as a primary metric. AC-014 (HigherOrderCO Bench) does not collect interaction counts at all. MIPS is essential because:

1. **Size-independent.** Wall-clock time grows with net size; MIPS allows comparing efficiency across different sizes.
2. **Captures communication overhead.** If MIPS drops with more workers, communication overhead is consuming throughput.
3. **Enables HVM comparison.** HVM2/HVM4 report MIPS as their primary throughput metric. Having MIPS in Relativist positions the system in the performance spectrum.

### 5.3 Why statistical repetitions

AC-005, Gap L8: "Each scenario is executed a single time. No mean, standard deviation, confidence intervals." AC-014 recommends 3-10 runs with 0.5s minimum.

A single datapoint can be affected by: OS context switches, CPU cache cold/hot state, network variability (in TCP mode), and background processes. Without repetitions, signal cannot be distinguished from noise. The minimum of 5 repetitions allows computing mean and standard deviation; 10 repetitions (the TCC campaign value) are sufficient for bootstrap 95% CI computation on the median; 30 repetitions allow CLT-based parametric confidence intervals.

### 5.4 Why correctness on every repetition

Correctness is not statistical -- it is boolean. If the system produces an incorrect result in 1 of 30 repetitions, there is a concurrency or determinism bug. Verifying on every repetition maximizes the probability of detecting intermittent bugs, especially in TCP mode where message ordering may vary.

### 5.5 Why three CSV files instead of one

The Haskell prototype has two distinct schemas (local with 17 columns, distributed with 17 different columns). This complicates comparative analysis. Relativist uses three files (detail, rounds, summary) that separate concerns: `detail.csv` contains scalar aggregates per execution (one row per datapoint), `rounds.csv` contains per-round phase breakdowns (enabling per-round analysis without JSON-in-CSV), and `summary.csv` contains pre-aggregated statistics per configuration (primary input for figures). This three-file approach was adopted from DATA-COLLECTION-PLAN Section 4 and resolves the problem of embedding variable-length vectors in CSV columns. All three files use a consistent identifier scheme `(benchmark, input_size, workers, mode)` enabling easy joins.

### 5.6 Why minimum granularity is not specified a priori

DISC-006 v2 Section 3.3 estimates thresholds of ~500-1000 redexes/worker for break-even in the Haskell prototype and ~5000-10000 redexes/worker for Rust with a real network. However, these thresholds are a function of network latency and per-reduction cost in the Rust implementation, both unknown until implementation. The benchmark suite MUST allow measuring the break-even point empirically by varying net sizes, not assuming it a priori. The logarithmic sizes (R24), the overhead ratio metric (R19), and the break-even analysis (R46-R47) serve exactly this purpose: the point where `overhead_ratio < 0.5` and `speedup > 1.0` is the empirical granularity threshold.

### 5.7 Why efficiency as a metric

Speedup alone can be misleading. An 8-worker configuration with speedup 4.0x has efficiency 0.5, meaning half the computational resources are wasted. Efficiency enables comparing different worker counts on an equal footing and identifies the point of diminishing returns. For Profile A, efficiency should be near 1.0 (or above for the Haskell prototype's super-linear case). For Profile C, efficiency will be well below 1.0. For Profile B, the efficiency curve is the primary unknown -- filling this gap is a key contribution of the TCC (ARG-004, Part I, Step 6: Profile B is conjectural with only 4 TCP datapoints).

### 5.8 Why TcpNetwork is mandatory

The TCC's objective is to evaluate distributed IC reduction in a Grid Computing environment. Running benchmarks only on `Local` (in-process) and `TcpLocalhost` (same machine) would validate the algorithm but not the distributed system. Key differences that only `TcpNetwork` on physical machines can reveal:

1. **Real network latency.** TcpLocalhost uses loopback (~0.01ms RTT). A LAN has ~0.1-1ms RTT. This 10-100x difference directly affects the break-even point and overhead ratio.
2. **Real bandwidth constraints.** Loopback bandwidth is effectively unlimited (~10 Gbps+). A LAN may be 1 Gbps or less, making serialization payload size a real factor.
3. **System heterogeneity.** Different machines may have different CPU speeds, memory, and OS scheduling behavior, affecting worker idle time and load balance.
4. **Isolation of overhead layers.** With 3 modes, the paper can decompose overhead: `Local` measures pure algorithm cost, `TcpLocalhost` adds serialization + TCP overhead, `TcpNetwork` adds real network latency. This progression (SPEC-07, Section 5.6) is essential for the overhead anatomy described in DISC-006 v2.

Without `TcpNetwork` data, the break-even analysis (Section 3.9) would be based on loopback measurements, which systematically underestimate communication overhead and produce optimistic thresholds that do not transfer to real grid deployments.

### 5.9 Why break-even analysis is a first-class concern

The TCC's research question (OBJETIVO_TCC.md) asks whether IC properties "allow building a model of distributed reduction for Grid Computing where the result is deterministic." The answer to correctness is covered by the Fundamental Property verification. But a practical follow-up is: "when is this model also efficient?" The break-even analysis (R46-R47) directly addresses this question, connecting the theoretical framework (ARG-001, P1-P5 ensure correctness unconditionally) with practical viability (ARG-004: efficiency depends on workload profile, network size, and overhead). Without break-even analysis, the paper would only show "it works" without quantifying "when it works well."

---

## 6. Haskell Prototype Reference

### 6.1 Direct mapping

| Haskell benchmark (AC-005) | Relativist benchmark | What changes |
|----------------------------|----------------------|-------------|
| `mkEPNet numPairs ERA` | EP-Annihilation | Same design. Sizes expanded (up to 100K). |
| (does not exist) | EP-Annihilation-CON | New. Fills prototype gap L6. |
| (does not exist) | EP-Annihilation-DUP | New. Fills prototype gap L6. |
| `mkExpansionNet` | CON-DUP Expansion | Same design. Sizes expanded. |
| `mkDualTreeNet` | DualTree | Same design. |
| `mkTree` (AC-004) | TreeSum | Same design. |
| `mkTreeBalanced` (AC-004) | TreeSumBalanced | Same design. |
| (does not exist) | MixedNet | New. All 6 rules. |
| (does not exist) | ErasurePropagation | New. Erasure cascade. |
| (does not exist) | ChurchAdd | New. SPEC-14. Arithmetic computation via Church encoding. Profile B. |
| (does not exist) | ChurchMul (SHOULD) | New. SPEC-14. Multiplication with O(a*b) interactions. Profile B. |

### 6.2 Metrics: prototype vs. Relativist

| Metric | Haskell prototype | Relativist |
|--------|-------------------|-----------|
| Wall-clock time | Yes (`getCurrentTime`) | Yes (`Instant`) |
| Total interactions | Yes (`brInteractions`) | Yes |
| Interactions/sec | Yes (`brIntPerSec`) | Yes, as MIPS |
| Interactions by rule | **No** | **Yes** (per rule type) |
| Speedup | Yes (`brSpeedup`) | Yes |
| Efficiency | **No** | **Yes** (speedup / workers) |
| Overhead % | Yes (`brOverheadPct`) | Yes, as `overhead_ratio` |
| Worker balance (CV) | Yes (`brBalance`) | Yes, via `idle_time` per worker |
| Phase breakdown | Yes (partition/compute/remap/merge/border) | Yes (partition/compute/merge/network; no remap) |
| Bytes sent/recv | Yes (distributed CSV) | Yes (all modes) |
| Border ratio | **No** | **Yes** (per round) |
| Peak memory | **No** | **Yes** |
| MIPS | **No** | **Yes** |
| Worker idle time | **No** | **Yes** |
| Repetitions/stats | **No** (1 execution) | **Yes** (min 5, TCC campaign 10, ideal 30) |
| Correctness | Yes (`brCorrect`) | Yes (every repetition) |
| Break-even analysis | **No** | **Yes** |

### 6.3 Prototype limitations that Relativist addresses

| Limitation AC-005 | How Relativist addresses it | Requirement |
|-------------------|----------------------------|-------------|
| L1: findRedexes O(N^2) | Incremental redex queue (SPEC-02, R17) -- linear speedup instead of super-linear | SPEC-02 |
| L2: Serialization via [Int] | bincode/serde (SPEC-06) -- ~50% smaller payload | SPEC-06 |
| L5: getCurrentTime without criterion | criterion for micro-benchmarks; Instant + repetitions for macro | R7, R30-R31 |
| L6: EP only tests ERA-ERA | EP-CON, EP-DUP, MixedNet | R10, R11, R16 |
| L7: CON-DUP undertested | Expanded sizes, more datapoints | R12 |
| L8: No statistical repetition | Minimum 5, ideal 30 repetitions | R30-R31 |

### 6.4 What Relativist does NOT directly compare

1. **Absolute time.** Haskell vs. Rust on different hardware is not directly comparable. The comparison is of trends and profiles, not absolute numbers.
2. **Numerical speedup.** The Haskell prototype has super-linear speedup (artifact of O(N^2) findRedexes, ARG-004 Part I Steps 2-3). Relativist will have at most linear speedup (better absolute performance, worse relative speedup). This is NOT a regression -- it is a more honest measurement.
3. **Serialization format.** The prototype uses `[Int]`; Relativist uses bincode. Bytes transferred are not directly comparable.

### 6.5 Expected differences in results

Based on the analysis in DISC-006 v2, DISC-008 v2, and ARG-004, the following differences are expected:

| Aspect | Haskell prototype | Relativist (expected) |
|--------|-------------------|----------------------|
| EP speedup trend | Super-linear (~K^2) | Linear (~K) |
| EP absolute time | Slower (GC, lazy eval, O(N^2)) | Faster (arena, O(1) queue) |
| DualTree slowdown | 0.18x with 4w | Still slowdown but less severe (faster reduce_all, no remap overhead) |
| CON-DUP behavior | 4 TCP datapoints, inconclusive | ~300 datapoints, Profile B resolved |
| Break-even point | ~500-1000 redexes/worker | ~5000-10000 redexes/worker (estimated, DISC-006 v2 Sec 3.3) |
| Border ratio (DualTree) | Not measured | ~50% per level (expected from cross-connect butterfly) |
| Overhead dominant phase | border (31% in DualTree) | TBD empirically |

---

## 7. Open Questions

1. **Maximum practical sizes.** The default sizes (up to 100K for EP, up to depth 14 for DualTree) are estimates. The maximum practical size depends on: (a) available memory, (b) acceptable time per datapoint (< 10 minutes as a guideline). Sizes MUST be adjusted after the first empirical benchmarks. **(Does NOT block implementation; adjust iteratively.)**

2. **Memory measurement in TCP mode.** In TcpLocalhost and TcpNetwork modes, the coordinator's peak memory includes the full net, but worker peak memory is measured in separate processes. The framework MUST collect peak memory and `WorkerBenchStats` from worker processes via a `WorkerMetrics` message (SPEC-06) sent alongside `PartitionResult`. This is required for complete metric reporting in `TcpNetwork` mode. **(Resolved: metrics piggyback on the return message.)**

3. **MixedNet: rule proportions.** The spec defines equal proportions (N pairs of each type). Different proportions (e.g., 80% ERA-ERA + 10% CON-DUP + 10% rest) may be more representative of real programs. This MAY be parameterized via an optional field in the `Benchmark` trait. **(Does NOT block implementation.)**

4. **Benchmarks adapted from HVM Bench.** AC-014 identifies `tree_fold`, `cnot_24`, `fib_nat`, and `lambda_eval` as high-adaptability candidates for pure ICs. Encoding these computations as IC nets is a non-trivial effort that MAY be included as future work or a suite extension. **(Does NOT block implementation; out of scope for v1.)**

5. ~~**TcpNetwork mode on physical machines.**~~ **Resolved.** TcpNetwork is now MUST (R27). The benchmark suite MUST execute on at least 4 and 8 physical machines using bare-metal deployment (SPEC-07, R41). This is the primary experimental scenario for the TCC, producing data with real network latency and bandwidth constraints.

6. ~~**Church arithmetic benchmarks (SPEC-14).**~~ **Resolved.** ChurchAdd is now a MUST benchmark (R17a) and ChurchMul is a SHOULD benchmark (R17b). Both use generators from SPEC-14 R26-R27, shared via `src/io/examples.rs` (SPEC-12 R35-R36). Results are verified by `decode_nat` (SPEC-14 R22) AND graph isomorphism with the sequential baseline.
