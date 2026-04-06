# SPEC-09 -- Round 1: Spec Critic Review

**Date:** 2026-04-05
**Reviewer:** Spec Critic (adversarial)
**Target:** SPEC-09-benchmarks.md (status: Revised v2)
**Predecessors consulted:** SPEC-00, SPEC-01, SPEC-05, SPEC-12, SPEC-14
**Cross-documents consulted:** DATA-COLLECTION-PLAN.md, BACKLOG.md

---

## Overall Assessment

SPEC-09 is a well-structured benchmark specification that addresses the critical gaps of the Haskell prototype (AC-005) with 9 benchmarks, 12 mandatory metrics, 3 execution modes, and statistical methodology. However, it contains several inconsistencies with SPEC-12 (net generator ownership), SPEC-14 (missing Church numeral benchmarks), and DATA-COLLECTION-PLAN.md (repetition count, CSV schema, and statistical methods). It also has internal gaps in the specification of the `Sequential` mode (listed in the Mode enum but absent from R26's mode table), incomplete specification of how MixedNet's post-CON-DUP cascades affect the expected agent count, and a missing link between SPEC-09's generator pseudocode and SPEC-12's canonical Rust generators.

**Verdict:** NEEDS REVISION

---

## Issues

### SC-001: Repetition count conflict between SPEC-09 and DATA-COLLECTION-PLAN
**Severity:** CRITICAL
**Axis:** Consistency
**Section:** 3.5 (R30-R31), DATA-COLLECTION-PLAN Section 2.1
**Requirement:** R30, R31
**Problem:** SPEC-09 R30 specifies the default repetition count as 5 (via CLI default `--repetitions 5`). R31 says "The minimum number of repetitions MUST be 5. For publishable results in the paper, the number SHOULD be 30." The DATA-COLLECTION-PLAN resolves this to **10 repetitions**, stating: "SPEC-09's default of 5 is for development/debugging runs only." However, the DATA-COLLECTION-PLAN also says: "This plan is NOT a spec. It does not introduce new requirements or override SPEC-09."

This creates a contradiction: the plan claims not to override SPEC-09, but then designates 10 repetitions as the campaign value, which is neither the MUST minimum (5) nor the SHOULD recommendation (30). An implementer reading only SPEC-09 would use 5 or 30; the plan says 10. Additionally, the article text (Section 4.6) says "10 repetitions" while SPEC-09 R31 says SHOULD 30. The three documents give three different answers.

**Impact if unresolved:** The TCC article will claim 10 repetitions. SPEC-09 says SHOULD 30 for publishable results. A reviewer could question why the published results use only 10 repetitions when the spec recommends 30.
**Suggested resolution:** Amend SPEC-09 R31 to: "For publishable results in the TCC paper, the number SHOULD be at least 10. For production-grade benchmarks with CLT-based confidence intervals, 30 is recommended." This aligns all three documents (spec, plan, article) on 10 as the TCC value while keeping 30 as a stronger recommendation.

---

### SC-002: Sequential mode listed in CLI but absent from the R26 mode table
**Severity:** CRITICAL
**Axis:** Completeness
**Section:** 3.4.3 (R26), 3.1 (R6), 4.1 (Mode enum)
**Requirement:** R6, R26
**Problem:** R6 lists `--mode` with default `Local` and options `Sequential, Local, TcpLocalhost, TcpNetwork`. The Mode enum in Section 4.1 includes `Sequential` as a variant. However, R26's mode table defines only **three** execution modes: `Local`, `TcpLocalhost`, and `TcpNetwork`. `Sequential` is not in the table.

Furthermore, R23 says "The 0-worker case (pure sequential, `reduce_all` without grid) MUST be included as baseline", implying sequential is controlled by `workers=0`, not by `--mode Sequential`. But the Mode enum has an explicit `Sequential` variant, and the DATA-COLLECTION-PLAN uses `mode=Sequential` with `workers=0` in its CSV schema and campaign commands.

The spec conflates two mechanisms: is the sequential baseline controlled by `--workers 0` (regardless of mode), or by `--mode Sequential`? R23 implies the former; the Mode enum and DATA-COLLECTION-PLAN imply the latter.

**Impact if unresolved:** The implementer does not know whether `Sequential` is a mode or a worker count. If `Sequential` is a mode, what happens when `--mode Sequential --workers 4`? If it is only `workers=0`, why does the Mode enum contain a `Sequential` variant?
**Suggested resolution:** Add `Sequential` to the R26 mode table with description: "Pure sequential: `reduce_all` without grid. Ignores `--workers` (always 0 workers). This mode bypasses partitioning, merge, and all protocol infrastructure." Clarify in R23 that `--mode Sequential` is the canonical way to request the baseline, and that `--workers 0` is an alias for `--mode Sequential` for convenience.

---

### SC-003: SPEC-09 generator pseudocode duplicates SPEC-12's canonical generators
**Severity:** HIGH
**Axis:** Consistency
**Section:** 4.2 (Net Generators)
**Requirement:** R9-R17 (benchmark definitions), SPEC-12 R35-R42
**Problem:** SPEC-09 Section 4.2 provides pseudocode for each net generator (EP-Annihilation, CON-DUP Expansion, DualTree, MixedNet, ErasurePropagation). SPEC-12 Section 3.5 (R35-R42) provides full Rust implementations of the same generators. SPEC-12 R35 explicitly states: "Generator functions MUST be pure functions with signature `fn generate_<name>(size: u32) -> Net`, reusable by both the `generate` subcommand and the benchmark suite (SPEC-09, `Benchmark::make_net`)." SPEC-12 R36 adds: "The generators MUST be implemented in the `io/examples.rs` submodule, NOT duplicated between the CLI and the benchmark suite."

However, the two specs disagree on some details:

1. **ErasurePropagation:** SPEC-09's pseudocode connects `CON_i.port1 <-> CON_(i+1).port0` (chain via left auxiliary). SPEC-12 R42a's Rust code and docstring say: "Each CON_i.p1 connects to CON_{i+1}.p0 (feeds next principal port)." These match. But SPEC-09 R17 says "an ERA at one end" without specifying which auxiliary port feeds the chain, while SPEC-12's generator explicitly uses p1 for the chain and p2 for the free port branch. A subtle mismatch could arise if someone implements from SPEC-09's less precise pseudocode.

2. **DualTree:** SPEC-09 says `connect(AgentPort(0, 0), AgentPort(tree_size, 0))` using hardcoded ID 0, but SPEC-12 R39 uses a recursive `build_tree` function that returns an `AgentId`, and the root connection is `connect(root_a.p0, root_b.p0)`. The hardcoded `id=0` in SPEC-09 assumes the first agent created gets ID 0, which may not hold if `Net::new()` starts `next_id` at a value other than 0.

3. **Ownership:** SPEC-09 Section 4.6 places generators in `src/bench/benchmarks/`, while SPEC-12 R36 places them in `src/io/examples.rs`. These are different modules.

**Impact if unresolved:** Two specs define the same generators with slightly different specifications. An implementer could write generators in `src/bench/benchmarks/` (per SPEC-09) that differ from the canonical generators in `src/io/examples.rs` (per SPEC-12), violating SPEC-12 R36's no-duplication rule.
**Suggested resolution:** SPEC-09 Section 4.2 should be marked as **(Informative)** and include a note: "The canonical generator implementations are defined in SPEC-12 (R35-R42). The pseudocode here is illustrative only. Benchmarks MUST use the generators from `src/io/examples.rs` via `Benchmark::make_net`, which delegates to the shared generators." Remove or clearly label the pseudocode as non-normative.

---

### SC-004: No Church numeral benchmarks despite SPEC-14 defining generators
**Severity:** HIGH
**Axis:** Completeness | Consistency
**Section:** 3.2 (Mandatory Benchmarks)
**Requirement:** R8 (at least 7 benchmarks)
**Problem:** SPEC-14 (Arithmetic Encoding) defines three net generators: `ChurchNat`, `ChurchAdd`, `ChurchMul` (SPEC-14 R26-R27). SPEC-12 R33 includes these as `ExampleNet` variants available via the `generate` subcommand, and explicitly says they "MUST be usable from both the `generate` subcommand (SPEC-12) and the benchmark suite (SPEC-09, `Benchmark::make_net`)." SPEC-14 R27 states generators "MUST be usable from [...] the benchmark suite (SPEC-09)."

However, SPEC-09's 9 mandatory benchmarks (R8-R17) do not include any Church numeral benchmark. The `BenchmarkId` enum in SPEC-09 Section 4.1 has no `ChurchAdd`, `ChurchMul`, or `ChurchExp` variant. The experimental matrix in Section 4.7 does not include Church benchmarks. The DATA-COLLECTION-PLAN's experimental matrix (Section 3) also omits them entirely.

Church arithmetic is the primary demonstration that Relativist performs real computation (not just graph manipulation). SPEC-14 Section 1 says Church arithmetic is "essential for the TCC experimental evaluation (SPEC-09) and defense." If Church benchmarks are not in SPEC-09, the TCC defense cannot demonstrate distributed arithmetic computation.

**Impact if unresolved:** SPEC-14 promises Church benchmarks will be part of SPEC-09, but SPEC-09 does not include them. The TCC's `compute` subcommand will exist but its results will not be benchmarked or appear in Section 5 of the article. The defense demonstration (e.g., "Relativist computed 500 + 500 = 1000 distributedly") would be anecdotal, not part of the formal benchmark suite.
**Suggested resolution:** Add at least one Church arithmetic benchmark to SPEC-09 R8's mandatory list (total: 10 benchmarks). Suggested addition:

**ChurchAdd** benchmark: generate `build_add(N/2, N - N/2)` where N is the size parameter. Verify result by `decode_nat`. Profile B (DUP-CON expansion during beta-reduction). Default sizes: [10, 50, 100, 500]. Add `ChurchAdd` to the `BenchmarkId` enum.

Optionally add `ChurchMul` as a SHOULD benchmark for larger expansion patterns.

---

### SC-005: CSV schema conflict between SPEC-09 and DATA-COLLECTION-PLAN
**Severity:** HIGH
**Axis:** Consistency
**Section:** 3.7 (R39-R42), DATA-COLLECTION-PLAN Section 4
**Requirement:** R39, R40
**Problem:** SPEC-09 R39 defines a single CSV schema with columns including `t_partition, t_compute, t_merge, t_network`. SPEC-09 R18's `BenchmarkResult` struct stores per-round vectors (`partition_time_per_round: Vec<f64>`, etc.). SPEC-09 does not specify how per-round vectors are flattened into CSV columns.

The DATA-COLLECTION-PLAN resolves this by splitting into **3 separate CSV files** (detail.csv, rounds.csv, summary.csv) and explicitly states: "This is cleaner than SPEC-09's single-file approach." However:

1. SPEC-09 R39 says "unified schema" (singular CSV). The plan says 3 files.
2. SPEC-09 R40 says "aggregation CSV file" (suggesting 2 files: detail + aggregation). The plan adds a 3rd file (rounds.csv).
3. The detail.csv schema in the DATA-COLLECTION-PLAN does not include `t_partition, t_compute, t_merge, t_network` columns that SPEC-09 R39 mandates. Instead, these per-phase timings are only in the rounds.csv per-round file.
4. The summary.csv in the plan uses `wall_clock_median, wall_clock_ci95_lo, wall_clock_ci95_hi` which are not in SPEC-09's `AggregatedStats` struct (which has `time_mean, time_std, time_median, time_min, time_max` but no CI columns).

**Impact if unresolved:** The Rust implementation of the benchmark framework must produce CSV files. SPEC-09 says one detail CSV + one aggregation CSV. The plan says three CSVs with different schemas. The implementer must choose, and if they follow SPEC-09, the plan's Python analysis scripts will not work (they expect `rounds.csv`). If they follow the plan, they violate SPEC-09 R39's column specification.
**Suggested resolution:** Amend SPEC-09 R39 to specify 3 CSV files (detail, rounds, summary) as defined in the DATA-COLLECTION-PLAN Section 4. Update R39's column list to match the plan's detail.csv schema. Add a new R39a for rounds.csv. Update R40 to reference summary.csv with bootstrap CI columns.

---

### SC-006: Statistical methodology conflict -- SPEC-09 uses mean/std, DATA-COLLECTION-PLAN uses median/bootstrap CI
**Severity:** HIGH
**Axis:** Consistency
**Section:** 3.5 (R32-R33), DATA-COLLECTION-PLAN Sections 2.5 and 6
**Requirement:** R32, R33
**Problem:** SPEC-09 R32 specifies that the framework MUST compute: mean, standard deviation, median, min, max. R33 says these MUST be computed for wall_clock_secs, mips, speedup, efficiency, and overhead_ratio. The `AggregatedStats` struct in Section 4.1 has fields for `time_mean`, `time_std`, `time_median`, `time_min`, `time_max`.

The DATA-COLLECTION-PLAN Section 6.1 states: "**Median** is used as the primary measure of central tendency for all timing metrics." Section 2.5 specifies "bootstrap 95% CI (10,000 resamples with replacement) on 10 repetitions" and declares: "Bootstrap is appropriate because (a) 10 repetitions is too few for CLT-based parametric CIs."

However, SPEC-09 R31 says "30 repetitions allow confidence intervals via the Central Limit Theorem" -- implying parametric CIs (Gaussian assumption). The plan rejects CLT-based CIs in favor of bootstrap CIs, but SPEC-09 does not mention bootstrap at all. SPEC-09's `AggregatedStats` struct has no CI fields (`ci95_lo`, `ci95_hi`). The plan's summary.csv has `wall_clock_ci95_lo, wall_clock_ci95_hi, speedup_ci95_lo, speedup_ci95_hi` which have no counterpart in SPEC-09.

**Impact if unresolved:** The framework built from SPEC-09 will compute mean/std/median/min/max but not bootstrap CIs. The article and plan require CIs with error bars on every figure. The Python scripts in the plan expect CI columns in summary.csv that the Rust framework (per SPEC-09) will not produce.
**Suggested resolution:** Add bootstrap CI computation to SPEC-09. Amend R32 to include: "The framework SHOULD compute bootstrap 95% confidence intervals for `wall_clock_secs`, `mips`, and `speedup` using 10,000 resamples on the median." Add `time_ci95_lo`, `time_ci95_hi`, `speedup_ci95_lo`, `speedup_ci95_hi`, `mips_ci95_lo`, `mips_ci95_hi` to the `AggregatedStats` struct. Note that the bootstrap computation MAY be deferred to the Python post-processing script if not implemented in Rust.

---

### SC-007: MixedNet expected result is underspecified
**Severity:** HIGH
**Axis:** Testability
**Section:** 3.2.5 (R16)
**Requirement:** R16, R37
**Problem:** R16 defines MixedNet as containing N pairs of each of the 6 rule types. The table says "Agents: ~12N" and "Profile: B (due to CON-DUP)." R37 says verification for benchmarks with a non-trivial result "MUST compare with the sequential result by graph isomorphism."

However, the MixedNet's expected result is never fully characterized. The initial net has 6N redexes, but:
- ERA-ERA pairs: produce 0 agents (void).
- CON-CON pairs: produce 0 agents (annihilation, free ports cross-reconnected).
- DUP-DUP pairs: produce 0 agents (annihilation, free ports parallel-reconnected).
- CON-ERA pairs: produce 2 ERA agents that connect to free ports, then have no redexes (normal form).
- DUP-ERA pairs: produce 2 ERA agents that connect to free ports, then have no redexes (normal form).
- CON-DUP pairs: produce 4 agents (2 CON + 2 DUP), which form new redexes among themselves (CON-CON and DUP-DUP annihilation cascades via the cross-wiring).

The note in SPEC-12 R41 states that all free port IDs are unique, so "the pairs are fully independent" -- meaning post-CON-DUP agents only interact within their own group, not across groups. But the CON-DUP commutation creates 4 agents in a cross-connected pattern that generates 2 new redexes (one CON-CON and one DUP-DUP). After those annihilate, the result is 0 agents from the CON-DUP groups.

So the expected final result is: 4N ERA agents (2 from each CON-ERA pair + 2 from each DUP-ERA pair), all connected to free ports, with 0 other agents. But SPEC-09 never states this. The verifier has no specified expected output.

**Impact if unresolved:** The `verify` method for MixedNet must compare with the sequential baseline by isomorphism, but without knowing the expected topology, the test assertion is opaque (it passes or fails without the developer knowing what the correct answer should look like). This also means unit tests for the benchmark cannot be written independently of the reduction engine.
**Suggested resolution:** Add to R16: "Expected result: 4N ERA agents (2 from CON-ERA erasure + 2 from DUP-ERA erasure), each connected to a unique FreePort. All other agent types are eliminated: ERA-ERA -> void, CON-CON -> annihilation, DUP-DUP -> annihilation, CON-DUP -> commutation -> annihilation cascade -> void. Total final agents: 4N."

---

### SC-008: ErasurePropagation expected result "0 agents" is incorrect
**Severity:** HIGH
**Axis:** Testability | Correctness
**Section:** 3.2.5 (R17), SPEC-12 R42a
**Requirement:** R17
**Problem:** SPEC-12 R42a's docstring says: "After reduction: ERA propagates through the chain, producing 2 ERAs per step, which erase the free-port branches. Result: 0 agents." However, this is incorrect.

Consider a chain of length 2: ERA -- CON_0 -- CON_1. The ERA interacts with CON_0 (CON-ERA rule): CON_0 is removed, 2 new ERA agents are created, one connecting to CON_0.p1 (which goes to CON_1.p0) and one connecting to CON_0.p2 (which goes to a free port). The ERA connecting to CON_1.p0 forms a new ERA-CON redex. The ERA connecting to the free port is in normal form (ERA has arity 0, connected to a free port, no redex partner).

After all cascading reductions, we are left with (N+1) ERA agents, each connected to a FreePort. The initial ERA generates 2 ERAs per CON-ERA interaction, but one of those ERAs continues the cascade and the other terminates at a free port. The last CON in the chain produces 2 ERAs: one for p1 (free port) and one for p2 (free port). Plus the cascade ERA that reached the last CON. So: N free-port-connected ERAs from the p2 ports along the chain, plus 1 ERA from the last CON's p1, plus 1 ERA from the last CON's p2. Wait -- the last CON (CON_{N-1}) has both p1 and p2 connected to free ports per SPEC-12 R42a. So the ERA-CON_{N-1} interaction produces 2 ERAs, each connected to a free port. Total: N ERAs connected to free ports (one from each CON's p2 branch, except the last which produces 2).

The exact count is N+1 ERA agents connected to free ports. This is NOT 0 agents.

**Impact if unresolved:** The verifier for ErasurePropagation would check `agent_count == 0` and fail on every execution, causing the benchmark suite to halt (R38). The implementer would waste time debugging a "correctness failure" that is actually a spec error.
**Suggested resolution:** Fix the expected result for ErasurePropagation. The correct result is: the net contains some number of ERA agents (each connected to a free port, in normal form with 0 redexes). The exact count depends on the chain structure. The verifier should check: (a) all remaining agents are ERA, (b) the net is in normal form (0 redexes), (c) agent count matches the sequential baseline.

---

### SC-009: Speedup formula for workers=1 grid mode is undefined
**Severity:** MEDIUM
**Axis:** Testability
**Section:** 3.5 (R20), 3.4.1 (R22-R23)
**Requirement:** R20, R22
**Problem:** R20 defines: `speedup = baseline_sequential_time / wall_clock_secs` (for workers > 0). R22 says: "The 1-worker case MUST be executed in grid mode (with trivial partitioning) to measure the protocol overhead in isolation." R23 says: "The 0-worker case (pure sequential) MUST be included as baseline."

This means: workers=0 is the sequential baseline with `speedup = 1.0`. Workers=1 in grid mode has `speedup = T_seq / T_grid_1`. This speedup will be < 1.0 (grid overhead with 1 worker and no parallelism benefit). This is correct and intended.

However, the `BenchmarkResult` struct has `workers: u32` and `speedup: f64`. For the sequential baseline (workers=0), R20 sets `speedup = 1.0`. But the `measure_sequential` function in Section 4.5 also sets `efficiency = 1.0`. The formula `efficiency = speedup / workers` would be `1.0 / 0 = division by zero` for the sequential case. The code works around this by hardcoding `efficiency = 1.0`, but this is not documented as a special case in R20.

**Impact if unresolved:** A naive implementation of `efficiency = speedup / workers as f64` for the sequential baseline (workers=0) would produce infinity or NaN. The formula needs an explicit guard.
**Suggested resolution:** Add to R20: "For the sequential baseline (workers == 0), speedup MUST be 1.0 and efficiency MUST be 1.0 (by convention, since the baseline has no parallelism overhead to measure)."

---

### SC-010: Overhead ratio formula is incorrect for sequential mode
**Severity:** MEDIUM
**Axis:** Testability
**Section:** 3.5 (R20)
**Requirement:** R20
**Problem:** R20 defines `overhead_ratio = 1.0 - (sum(compute_time_per_round) / wall_clock_secs)`. For the sequential baseline (workers=0), `compute_time_per_round` is an empty vector (no rounds). `sum([])` = 0.0. So `overhead_ratio = 1.0 - (0.0 / wall_clock_secs) = 1.0`.

This says the sequential baseline has 100% overhead and 0% compute time, which is semantically wrong. The sequential baseline is 100% compute, 0% overhead. The `measure_sequential` function in Section 4.5 sets `efficiency = 1.0` and `speedup = 1.0` but does not set `overhead_ratio`. The DATA-COLLECTION-PLAN detail.csv schema says: "overhead_ratio: 0.0 for Sequential."

The formula produces 1.0 for sequential; the plan expects 0.0. These contradict each other.

**Impact if unresolved:** The overhead ratio for sequential mode will be either 1.0 (if computed from the formula) or 0.0 (if hardcoded per the plan). Incorrect overhead ratio in the CSV would skew any analysis that includes sequential rows.
**Suggested resolution:** Add to R20: "For the sequential baseline (workers == 0, no grid rounds), overhead_ratio MUST be 0.0 by convention. The formula `1.0 - (sum(compute_time_per_round) / wall_clock_secs)` applies only to grid executions (workers >= 1)."

---

### SC-011: G1 verification timing -- verification cost included in wall_clock_secs?
**Severity:** MEDIUM
**Axis:** Testability
**Section:** 3.6 (R36), 4.5 (Timing)
**Requirement:** R36, R4
**Problem:** R36 says "Correctness verification MUST be executed on EACH repetition." Section 4.5's `measure_grid` function shows:

```rust
let start = Instant::now();
let (result_net, metrics) = run_grid(net, workers, mode);
let elapsed = start.elapsed();
let correct = benchmark.verify(seq_result, &result_net);
```

The `verify` call occurs AFTER `elapsed` is computed, so verification time is NOT included in `wall_clock_secs`. Good.

However, R4 says: "The framework MUST verify the Fundamental Property (SPEC-01, G1) on EVERY datapoint: `reduce_all(net) ~= extract_result(run_grid(net, n))`." The `reduce_all(net)` for the sequential baseline must run before any grid executions (per R3: "The sequential result MUST be preserved for correctness verification across all worker configurations").

The issue: Section 4.3's execution protocol runs the sequential baseline separately (steps 2-5 of the protocol), then runs grid modes. The sequential baseline's wall-clock time is used as `seq_baseline` for speedup calculation. But the protocol also runs the sequential baseline with warmup and repetitions, meaning the sequential net is reduced multiple times. Is the `seq_baseline` the median of sequential repetitions, or a single run?

Section 4.3 says: `seq_baseline = median(seq_results.map(|r| r.wall_clock_secs))`. This is the median of the sequential repetitions. But: the speedup formula in R20 says `speedup = baseline_sequential_time / wall_clock_secs`. Is `baseline_sequential_time` the median? SPEC-09 does not specify. The DATA-COLLECTION-PLAN detail.csv says: `speedup: T_seq_median / T_this`. So it is the median. But SPEC-09's R20 formula does not say "median."

**Impact if unresolved:** If the implementer uses a single sequential run (not the median), speedup values will have higher variance. If they use the mean instead of the median, results will differ from the plan's expectation. Small inconsistency, but affects reproducibility.
**Suggested resolution:** Amend R20 to explicitly state: "For speedup computation, `baseline_sequential_time` is the median wall-clock time of all sequential repetitions for the same (benchmark, size) combination."

---

### SC-012: TreeSum benchmark uses pragmatic encoding, not pure IC
**Severity:** MEDIUM
**Axis:** Completeness | Consistency
**Section:** 3.2.4 (R14)
**Requirement:** R14
**Problem:** R14 says TreeSum "MUST generate a net equivalent to the Haskell prototype's `mkTree` (AC-004): a chain of CON agents (adders) connected to ERA-ERA pairs (work units). The result is verified by `extract_result == sum(values)`."

This description uses "adders" and "values" which are semantic concepts that do not exist in pure Interaction Combinators. In pure IC, agents have only symbols (CON, DUP, ERA) and ports -- there is no concept of "addition" or "numeric value." The Haskell prototype's `mkTree` uses a pragmatic encoding where the "sum" is computed by a convention external to IC theory (AC-005 notes this).

SPEC-14 now defines Church numeral encoding for arithmetic, which IS a pure IC encoding of computation. TreeSum's encoding is undocumented -- neither SPEC-09 nor any other spec defines what "extract_result" means for TreeSum, nor how a "sum" is extracted from a normal-form IC net of CON and ERA agents.

The DATA-COLLECTION-PLAN Section 3.4 acknowledges this: "TreeSum uses pragmatic encoding (documented limitation in article Section 5.3)."

**Impact if unresolved:** The `extract_result` function for TreeSum has no formal specification. The verifier (`benchmark.verify`) cannot be implemented without knowing how to interpret the reduced net as a numeric value. This is not a correctness issue (isomorphism with the sequential result still works), but it means the "result verification" for TreeSum is purely structural, not semantic.
**Suggested resolution:** Add to R14: "Note: TreeSum uses a pragmatic encoding inherited from the Haskell prototype. The `extract_result` function interprets the reduced net's structure by convention. For formal arithmetic computation, see the Church numeral benchmarks (SPEC-14). TreeSum's correctness verification MUST use graph isomorphism with the sequential result, not semantic value extraction."

---

### SC-013: Benchmark trait `verify` method takes `&Net` for sequential result but Section 4.3 clones the net
**Severity:** MEDIUM
**Axis:** Consistency
**Section:** 3.1 (R2), 4.3 (Execution Protocol)
**Requirement:** R2
**Problem:** R2 defines the `Benchmark` trait with:
```rust
fn verify(&self, sequential_result: &Net, distributed_result: &Net) -> bool;
```

The sequential result is passed by shared reference. Section 4.3's protocol says: `seq_net = reduce_all(net.clone())` -- this produces one sequential result. Then for each (mode, workers) combination, the protocol calls `benchmark.verify(seq_result, &result_net)`. Since `verify` takes `&Net` (immutable reference), the sequential result is reused across all configurations. This is correct.

However, the `verify` method signature implies it can compare two arbitrary nets. For benchmarks where the expected result is empty (EP, DualTree), the verifier just checks `distributed_result.agent_count() == 0` -- it does not need the sequential result at all. For benchmarks like MixedNet, it needs graph isomorphism. The trait signature forces all benchmarks to accept a sequential result even when they do not need it.

More importantly: graph isomorphism (`nets_isomorphic`) is mentioned in R37 and SPEC-08, but SPEC-09 does not specify the algorithm or its complexity. For large nets, isomorphism can be expensive. The spec should clarify whether isomorphism is exact (canonical form comparison) or approximate (agent count + wire count + per-symbol counts).

**Impact if unresolved:** Minor. The trait design works functionally but is overloaded. The more serious concern is the unspecified isomorphism algorithm for large benchmark results.
**Suggested resolution:** Add a note to R37: "Graph isomorphism for correctness verification MAY use the lightweight check: (a) same agent count per symbol, (b) same wire count, (c) same free port count, (d) same redex count (must be 0 for normal form). Full structural isomorphism (canonical graph comparison) SHOULD be used only when the lightweight check passes and the result is small (< 1000 agents). For empty-result benchmarks (EP, DualTree), the verifier SHOULD simply check agent_count == 0."

---

### SC-014: SPEC-09 does not reference SPEC-05 for grid cycle integration
**Severity:** MEDIUM
**Axis:** Consistency
**Section:** Header (Depends on)
**Requirement:** (meta)
**Problem:** SPEC-09's "Depends on" header lists SPEC-05 as a dependency: "SPEC-05 (Merge and Grid Cycle)." However, SPEC-05 does not exist at the path `SPEC-05-grid-cycle.md` -- the actual file is `SPEC-05-merge.md`. The spec file name does not match the dependency reference. While the spec text uses the correct full title ("Merge and Grid Cycle"), the file naming inconsistency could confuse tooling or automated cross-reference checks.

Additionally, SPEC-09 Section 4.3 references `run_grid` (from SPEC-05 R25) but uses a 3-argument version: `run_grid(net, workers, mode)`. SPEC-05 R25 defines `run_grid(net: Net, num_workers: u32, strategy: impl PartitionStrategy)`. The `mode` parameter (Local/TcpLocalhost/TcpNetwork) is not part of SPEC-05's `run_grid` -- it determines the transport layer, which is a higher-level concern (SPEC-06/SPEC-13). SPEC-09 conflates the grid cycle function with the execution mode selection.

**Impact if unresolved:** Minor inconsistency in the `run_grid` function signature between specs. The mode parameter must be resolved at a layer above `run_grid`.
**Suggested resolution:** SPEC-09 Section 4.3 should clarify that `run_grid(net, workers, mode)` is pseudocode for the benchmark framework's wrapper, not a direct call to SPEC-05's `run_grid`. The wrapper selects the appropriate transport (in-memory channel for Local, TCP for TcpLocalhost/TcpNetwork) and then delegates to the grid cycle.

---

### SC-015: Peak memory measurement is platform-dependent with no fallback
**Severity:** LOW
**Axis:** Completeness
**Section:** 4.4 (Memory Measurement)
**Requirement:** R19 (peak_memory_bytes)
**Problem:** Section 4.4 says: "On Linux: reads /proc/self/status (VmHWM). On other OSes: returns 0 (metric unavailable)." R19 lists `peak_memory_bytes` as a MUST metric. But if the benchmark runs on Windows (the developer's current platform per the environment info), the metric will always be 0.

This means all development-time benchmarks will have `peak_memory_bytes = 0`, making the metric untestable during development. Only Docker/Linux execution will populate it.

**Impact if unresolved:** No correctness issue. However, during development, the developer cannot verify that memory measurement works. It will only be validated at campaign time (Docker deployment).
**Suggested resolution:** Add a Windows implementation using `GetProcessMemoryInfo` from the `winapi` crate (or `windows` crate), or use the `sysinfo` crate for cross-platform memory info. Alternatively, mark `peak_memory_bytes` as SHOULD (not MUST) with a note: "If the OS does not support memory introspection, the field MUST be 0 and the limitation documented."

---

### SC-016: Haskell comparison sizes do not fully match DATA-COLLECTION-PLAN
**Severity:** LOW
**Axis:** Consistency
**Section:** 3.8 (R43)
**Requirement:** R43
**Problem:** SPEC-09 R43 specifies Haskell comparison sizes for TreeSum as [50, 200, 1000, 2000]. The DATA-COLLECTION-PLAN Section 3.4 uses TreeSum sizes [16, 64, 256]. Section 7.1 of the plan lists Haskell comparison sizes as [16, 64, 256] for TreeSum. These are different from SPEC-09 R43's [50, 200, 1000, 2000].

The plan's TreeSum sizes match the DATA-COLLECTION-PLAN's main experimental matrix but do NOT match SPEC-09 R43's "Haskell sizes." It is unclear whether the plan's Haskell comparison will use [16, 64, 256] (from its own matrix) or [50, 200, 1000, 2000] (from SPEC-09 R43).

**Impact if unresolved:** If the Haskell prototype is run with sizes [50, 200, 1000, 2000] but Relativist only has data for [16, 64, 256], the qualitative comparison in Tab 3 will have no overlapping sizes, making trend comparison impossible.
**Suggested resolution:** Either (a) add [50, 200, 1000, 2000] to the DATA-COLLECTION-PLAN's TreeSum sizes for the Haskell comparison subset, or (b) update SPEC-09 R43 to use [16, 64, 256] for TreeSum (matching the plan), and run the Haskell prototype with those same sizes.

---

### SC-017: No explicit G1 verification mechanism for the `Sequential` mode baseline
**Severity:** LOW
**Axis:** Invariant Preservation
**Section:** 3.6 (R36-R37), 4.3
**Requirement:** R36
**Problem:** R36 says "Correctness verification MUST be executed on EACH repetition." R4 says G1 verification is `reduce_all(net) ~= extract_result(run_grid(net, n))`. But the sequential baseline IS `reduce_all(net)` -- there is no `run_grid` to compare against. How is the sequential baseline verified?

Section 4.3 runs the sequential baseline with warmup and repetitions, and the code says: `assert!(result.correct, "Correctness failure in sequential!")`. But what does `correct` mean for the sequential case? There is no second result to compare against. Is it comparing different sequential runs to each other (T6 uniqueness of normal form)? Or is it always `true` by definition?

**Impact if unresolved:** The sequential baseline's `correct` field is computed but its semantics are undefined. If `correct = true` always for sequential, the "zero correctness failures across all datapoints" criterion (R5) is trivially satisfied for sequential rows.
**Suggested resolution:** Add to R36 or R37: "For the sequential baseline, correctness MUST be verified by comparing the result of each repetition against the first sequential result by graph isomorphism. This validates invariant T6 (uniqueness of normal form): all sequential runs MUST produce the same normal form regardless of reduction order."

---

## Summary

| Severity | Count |
|----------|-------|
| CRITICAL | 2 |
| HIGH | 6 |
| MEDIUM | 5 |
| LOW | 4 |

## Mandatory (must fix before implementation)

- **SC-001:** Repetition count conflict (5 vs 10 vs 30) between SPEC-09, DATA-COLLECTION-PLAN, and article
- **SC-002:** Sequential mode present in CLI/enum but absent from the mode definition table (R26)
- **SC-003:** Generator pseudocode in SPEC-09 duplicates and subtly conflicts with SPEC-12's canonical generators
- **SC-004:** No Church numeral benchmarks despite SPEC-14 requiring benchmark integration
- **SC-005:** CSV schema conflict -- SPEC-09 says 1-2 files, plan says 3 files with different schemas
- **SC-006:** Statistical methodology conflict -- SPEC-09 omits bootstrap CIs that the plan and article require
- **SC-007:** MixedNet expected result never specified (verifier untestable)
- **SC-008:** ErasurePropagation "0 agents" expected result is mathematically incorrect

## Recommended (should fix)

- **SC-009:** Speedup/efficiency formulas undefined for workers=0 (division by zero)
- **SC-010:** Overhead ratio formula produces 1.0 for sequential (should be 0.0)
- **SC-011:** `baseline_sequential_time` not specified as median or mean
- **SC-012:** TreeSum's pragmatic encoding has no formal spec for `extract_result`
- **SC-013:** Graph isomorphism algorithm for large nets is unspecified
- **SC-014:** `run_grid` function signature mismatch with SPEC-05 (mode parameter)
- **SC-016:** Haskell comparison sizes for TreeSum do not match DATA-COLLECTION-PLAN
- **SC-017:** Sequential baseline correctness verification semantics undefined

---

## Checklist

### Consistency
- [x] BenchmarkResult fields match SPEC-05 GridMetrics structure
- [x] InteractionsByRule covers all 6 rules from SPEC-01 T5
- [x] BenchmarkId enum matches the 9 benchmarks defined in R8-R17
- [x] Mode enum includes all modes referenced in R6 and R26
- [x] Benchmark trait interface matches usage in Section 4.3
- [ ] **FAIL:** Repetition count inconsistent across SPEC-09, DATA-COLLECTION-PLAN, and article (SC-001)
- [ ] **FAIL:** Sequential mode in CLI/enum but not in R26 mode table (SC-002)
- [ ] **FAIL:** Generator pseudocode conflicts with SPEC-12 canonical generators (SC-003)
- [ ] **FAIL:** No Church benchmarks despite SPEC-14 R27 requiring SPEC-09 integration (SC-004)
- [ ] **FAIL:** CSV schema (1-2 files) conflicts with plan (3 files) (SC-005)
- [ ] **FAIL:** Statistical methods omit bootstrap CIs required by plan and article (SC-006)
- [ ] **FAIL:** Haskell comparison sizes for TreeSum differ from plan (SC-016)
- [x] Worker counts [0, 1, 2, 4, 8] consistent across R22-R23 and experimental matrix
- [x] Mode names (Local, TcpLocalhost, TcpNetwork) consistent with SPEC-06/SPEC-07

### Testability
- [x] R4 (G1 on every datapoint): testable via isomorphism check
- [x] R5 (zero correctness failures): testable via global assertion
- [x] R22-R23 (worker counts): testable by running with each worker count
- [x] R30-R31 (warmup + repetitions): testable by inspecting CSV row count
- [x] R34 (CV warning): testable by inducing high variance
- [ ] **FAIL:** MixedNet verifier: expected result unspecified (SC-007)
- [ ] **FAIL:** ErasurePropagation verifier: expected result "0 agents" is wrong (SC-008)
- [ ] **PARTIAL:** Speedup for workers=0 hardcoded but not documented (SC-009)
- [ ] **PARTIAL:** Overhead ratio for workers=0 formula produces wrong value (SC-010)
- [ ] **PARTIAL:** Sequential baseline correctness check semantics undefined (SC-017)
- [x] R38 (halt on failure): testable by injecting a bug
- [x] R43-R45 (Haskell comparison): testable by running shared sizes

### Completeness
- [x] All 3 overhead profiles (A, B, C) have at least 2 benchmarks each
- [x] All 6 interaction rules are exercised (MixedNet covers all 6)
- [x] Experimental matrix covers 9 benchmarks x 5 worker counts x 3 modes
- [x] Metrics cover timing, throughput, correctness, memory, communication, per-worker, per-round
- [x] Break-even analysis specified (R46-R47)
- [x] Overhead breakdown specified (R48-R49)
- [ ] **FAIL:** Church numeral benchmarks missing (SC-004)
- [ ] **FAIL:** MixedNet expected normal form not characterized (SC-007)
- [ ] **FAIL:** ErasurePropagation expected normal form incorrect (SC-008)
- [ ] **PARTIAL:** TreeSum `extract_result` semantics unspecified (SC-012)
- [x] Haskell comparison table specified (R43-R45)
- [x] Visualizations for Section 4 enumerated (Section 4.8)

### Invariant Preservation
- [x] G1 verified on every datapoint (R4, R36)
- [x] T7 (invariant step count) verifiable via `total_interactions` consistency across modes
- [x] D1 (split/merge identity) implicitly tested by grid mode with workers=1
- [x] T6 (unique normal form) verifiable by comparing sequential and distributed results
- [ ] **PARTIAL:** G1 verification for sequential baseline is tautological (SC-017)
- [x] Correctness halt policy (R38) prevents publishing invalid results
- [x] Per-rule interaction counters enable validation of rule coverage (all 6 rules exercised)
