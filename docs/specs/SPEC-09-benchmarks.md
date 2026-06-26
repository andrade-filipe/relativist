# SPEC-09: Benchmark Suite

**Status:** Revised v3.2 — D-011 Phase F-1 Tier 3 measurement protocol amendment (R18a–R18g, R29a–R29c, R37c, §4.9)
**Depends on:** SPEC-00 (Glossary), SPEC-01 (Invariants), SPEC-02 (Net Representation), SPEC-03 (Reduction Engine), SPEC-04 (Partitioning), SPEC-05 (Merge and Grid Cycle), SPEC-06 (Wire Protocol), SPEC-07 (Deployment), SPEC-08 (Test Strategy), SPEC-12 (User I/O -- canonical generators), SPEC-14 (Arithmetic Encoding -- Church numeral benchmarks)
**Amends:** SPEC-21 §3.8 A4 (R2 — `Benchmark` trait gains `fn make_net_stream(&self, size, chunk_size) -> Box<dyn Iterator<Item = AgentBatch>>` with default impl that wraps `make_net`; SPEC-21 R10, R11, R12)
**Gray zones resolved:** Z4 (communication overhead vs. parallelism benefit), Z6 (scalability transfer from shared to distributed memory), Z7 (work granularity)
**References consumed:** REF-001 (Lafont 1990), REF-002 (Lafont 1997), REF-003 (HVM2), REF-005 (Mackie & Pinto 2002), REF-007 (Casanova 2002), REF-013 (Mackie 1997), REF-014 (Kahl 2015), REF-017 (Foster, Kesselman, Tuecke 2001)
**Discussions consumed:** DISC-003 v2 (strong confluence to distributed determinism, P1-P5), DISC-006 v2 (overhead anatomy, break-even analysis, workload profiles), DISC-008 v2 (shared-to-distributed transition, 6 operational dimensions)
**Arguments consumed:** ARG-001 (central argument, P1-P6, fundamental property), ARG-004 (practical viability and limits, workload classification A/B/C, break-even, granularity thresholds)
**Code analyses consumed:** AC-005 (Haskell benchmark framework, 5 benchmarks, 9 CSVs, ~110 datapoints, 8 limitations, experimental results), AC-014 (HigherOrderCO bench methodology, wall-clock sampling, 29 benchmarks, gaps identified)
**Revision history:** v3.1 (2026-04-10): the `Rounds (grid)` property row of every benchmark table has been split into `Rounds (grid, lenient)` and `Rounds (grid, strict, expected)` to make the distinction introduced by SPEC-05 R30a explicit in the benchmark contract. A new benchmark `cascade_cross` (R18) is introduced as the primary validation vehicle for strict-mode multi-round behavior. No existing benchmarks are removed and no metric fields of `BenchmarkResult` are changed. Source: plano curious-sleeping-patterson, Fase 1. v3.1.1 (2026-04-11): R17d (ChurchSumOfSquares) updated to match the actual implementation in `src/encoding/arithmetic.rs`. The benchmark pre-encodes each square `i^2` as a canonical Church numeral via `encode_church_into` and folds the chain with `wire_add_into`, instead of composing `wire_mul_into` inside `wire_add_into` as originally specified. Rationale: composing `wire_mul_into` inside an add chain produces reduced nets with nested DUP sharing boundaries mid-chain that the current `decode_shared_chain` cannot traverse (it only handles a single terminal DUP boundary produced by `mul(a,b)` alone), making value-based verification impossible without a decoder refactor that is out of scope for a demonstrative benchmark. The PortRef helpers `wire_add_into` and `wire_mul_into` still exist and `build_add` / `build_mul` remain thin wrappers over them; `wire_mul_into` is simply not called from the sum-of-squares path. No other benchmarks are affected. Source: plano curious-sleeping-patterson Risk #1.

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
    /// This is the source-of-truth materialization path; it remains a
    /// required method (no default impl) so the streaming default has a
    /// fallback. See SPEC-21 R11.
    fn make_net(&self, size: u32) -> Net;

    /// Generate the input net as a streaming iterator of `AgentBatch`.
    /// Default implementation (per SPEC-21 §3.8 A4 / R10): collect the
    /// eager net via `make_net` and slice it into chunks via the
    /// `default_chunked_iter` helper (lives in `src/bench/streaming.rs`,
    /// see SPEC-21 §3.2 R10). The default-impl path materializes the net
    /// then slices it (memory-equivalent to v1; no streaming benefit, but
    /// no break). Generators that benefit from native streaming (per
    /// SPEC-21 R12: `ep_annihilation` MUST override; others SHOULD)
    /// override this method. R10 ↔ R11 isomorphism contract: when the
    /// streaming variant is collected, it MUST produce a net isomorphic
    /// to `make_net(size)` (T6, §7.2 of SPEC-21).
    ///
    /// All 13 existing SPEC-09 implementations remain valid without
    /// per-implementation edits via this default impl.
    fn make_net_stream(
        &self,
        size: u32,
        chunk_size: usize,
    ) -> Box<dyn Iterator<Item = AgentBatch>> {
        Box::new(default_chunked_iter(self.make_net(size), chunk_size))
    }

    /// Default sizes for this benchmark (logarithmic variation).
    fn default_sizes(&self) -> Vec<u32>;

    /// Verify correctness: compare distributed result with sequential result.
    /// Returns true if the result is correct.
    fn verify(&self, sequential_result: &Net, distributed_result: &Net) -> bool;
}
```

> **Amendment A4 (SPEC-21 §3.8 A4 / R10, R11, R12):** Closes SC-008. The default-impl decision is the lower-friction choice and matches SPEC-09's posture of additive trait extensions. Without the default, the trait change would force ~520 LoC of mechanical implementation across 13 benchmarks even for those that derive no benefit from streaming. Total Phase B effort estimate: ~30 LoC for the trait amendment + `default_chunked_iter` helper, plus per-generator overrides on opt-in basis. The `AgentBatch` type is defined in SPEC-21 §4.1. Native-streaming overrides per benchmark:
>
> | Benchmark | Native streaming benefit | Override status |
> |-----------|--------------------------|-----------------|
> | `ep_annihilation` (R9 family) | YES — independent ERA-ERA pairs, no cross-batch wires | OVERRIDE per SPEC-21 R12 MUST |
> | `ep_annihilation_con` (R10) | YES — independent CON-CON pairs | OVERRIDE per R12 SHOULD |
> | `ep_annihilation_dup` (R11) | YES — independent DUP-DUP pairs | OVERRIDE per R12 SHOULD |
> | `dual_tree` | YES — but requires forward references (SPEC-21 §4.7) | OVERRIDE per R12 SHOULD |
> | `mixed_net` | NO — small fixed sizes; default-impl path acceptable | DEFAULT |
> | `church_add` (SPEC-14) | TBD — depends on encoder API stability | DEFAULT |
> | `church_mul` (SPEC-14) | TBD | DEFAULT |
> | `m5_*` (M5 milestone family) | YES — large nets benefit most | OVERRIDE per R12 SHOULD (deferred to M5 tasks) |
> | Remaining benchmarks | NO (small / synthetic) | DEFAULT |
>
> No regression risk: existing test counts pass via the default-impl path until a benchmark explicitly opts into override.

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

**R8.** Relativist MUST implement at least 12 benchmarks organized by overhead profile (10 general-purpose benchmarks plus `cascade_cross` as the strict-mode validation benchmark (R17c) plus `church_sum_of_squares` as the demonstrative arithmetic benchmark (R17d, SHOULD)). **(MUST)**

#### 3.2.1 Profile A -- Embarrassingly Parallel

**R9.** The **EP-Annihilation** benchmark MUST generate N pairs of ERA agents connected by principal ports. All redexes are independent. Expected result: 0 agents. Direct mapping from AC-005 `mkEPNet`. **(MUST)**

| Property | Value |
|----------|-------|
| Agents | 2N |
| Redexes | N |
| Rounds (grid, lenient) | 1 |
| Rounds (grid, strict, expected) | 1 (all redexes are internal under ContiguousIdStrategy; no cascades) |
| Border redexes | 0 (contiguous partitioning) |
| Rules exercised | ERA-ERA (void) |
| Profile | A |
| Default sizes | [100, 500, 1_000, 5_000, 10_000, 50_000, 100_000] |

**R10.** The **EP-Annihilation-CON** benchmark MUST generate N pairs of CON agents connected by principal ports, with auxiliary ports connected to FreePorts. Expected result: 0 CON agents, FreePorts reconnected in cross pattern. **(MUST)**

| Property | Value |
|----------|-------|
| Agents | 2N |
| Redexes | N |
| Rounds (grid, lenient) | 1 |
| Rounds (grid, strict, expected) | 1 (independent pairs, no cross-partition cascades) |
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
| Rounds (grid, lenient) | 1 |
| Rounds (grid, strict, expected) | 1 (independent pairs, no cross-partition cascades) |
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
| Rounds (grid, lenient) | 1 |
| Rounds (grid, strict, expected) | Variable (grows with N as expansion promotes internal wires to cross-partition wires) |
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
| Rounds (grid, lenient) | 1 |
| Rounds (grid, strict, expected) | >= d (one round per cascade level; exact count depends on partitioning and butterfly depth) |
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
| Rounds (grid, lenient) | 1 |
| Rounds (grid, strict, expected) | Variable (grows with depth of the CON cascade under cross-partition cuts) |
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
| Rounds (grid, lenient) | 1 |
| Rounds (grid, strict, expected) | Variable (CON-DUP commutation in different pair groups may or may not cross partition boundaries depending on N and the partition strategy) |
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
| Rounds (grid, lenient) | 1 |
| Rounds (grid, strict, expected) | Variable (one round per cross-partition step of the erasure cascade; maximum bounded by N+1 under ContiguousIdStrategy with cut-through placement) |
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
| Rounds (grid, lenient) | 1 |
| Rounds (grid, strict, expected) | Variable (grows with the number of beta-reduction stages whose CON-DUP commutations cross partition boundaries) |
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
| Rounds (grid, lenient) | 1 |
| Rounds (grid, strict, expected) | Variable (larger than ChurchAdd under the same partition strategy, due to a larger expansion factor and more nested CON-DUP stages) |
| Rules exercised | Same as ChurchAdd (all 6 rules via beta-reduction) |
| Profile | B (larger expansion factor than ChurchAdd) |
| Default sizes (a=b) | [5, 10, 20, 50] |

#### 3.2.7 Strict-Mode Validation Benchmark

**R17c (Cascade Cross Benchmark).** The **cascade_cross** benchmark MUST generate a synthetic net designed to stress-test multi-round behavior under strict BSP mode (SPEC-05 R30a). The benchmark consists of N stacked CON-DUP commutation stages arranged so that each stage's cascade falls into a different partition under the default `ContiguousIdStrategy`. Under strict mode, each stage takes at least one round to propagate; under lenient mode, the same work collapses into the single coordinator-side `reduce_all` pass. The benchmark ID is placed after R17a (ChurchAdd) and R17b (ChurchMul) in the "Mandatory Benchmarks" section (3.2) to keep the benchmark catalog contiguous; the next requirement (R18) is the canonical definition of `BenchmarkResult` in Section 3.3, which is unchanged. **(MUST)**

| Property | Value |
|----------|-------|
| Initial agents | ~4N |
| Initial redexes | N |
| Rounds (grid, lenient) | 1 |
| Rounds (grid, strict, expected) | >= 2 for N >= 2; grows with N |
| Border redexes | proportional to N |
| Rules exercised | CON-DUP commutation, CON-CON annihilation |
| Profile | B (expansion under distribution) |
| Default sizes | [10, 50, 100, 500, 1_000] |
| Correctness | full G1 (sizes kept small to avoid isomorphism intractability) |

**Purpose:** exercises the BSP multi-round loop by stacking CON-DUP commutation stages whose cascades fall into different partitions under `ContiguousIdStrategy`. Under strict_bsp, each stage takes at least one round to propagate. Under lenient mode, the same work collapses into the single coordinator `reduce_all` pass. cascade_cross is the only benchmark where strict mode measurably differs from lenient, making it the primary validation vehicle for SPEC-05's BSP-faithfulness claims (Section 4.5a Properties 1-4).

**Rationale:** Before this benchmark, there was no datapoint in the suite that could empirically distinguish `run_grid_lenient` from `run_grid_strict`. All other benchmarks either have rounds bounded above by 1 in both modes (Profile A: no cross-partition cascades) or have unpredictable strict-mode round counts that depend on size-sensitive partition boundaries (Profile B/C benchmarks: round counts vary but only at scales where full-G1 verification becomes intractable). cascade_cross is tuned for the opposite regime: sizes small enough that `nets_isomorphic` completes in sub-second, but topology deliberately crafted to force `rounds >= 2` under strict mode from `N >= 2`. This makes it the canonical regression test for R30a (SPEC-05) and the primary empirical support for D6 (SPEC-01 v3.1: `R_lenient(mu, n) <= R_strict(mu, n)`).

**Correctness:** The cascade_cross verifier MUST compare sequential and distributed results by full graph isomorphism (`nets_isomorphic`), the same as MixedNet and ErasurePropagation. The default sizes are deliberately kept small (maximum 1_000) so that the isomorphism check remains tractable regardless of the strict-mode round count.

**Benchmark matrix note:** cascade_cross MUST be executed in BOTH lenient and strict modes for the same configurations. The strict-mode run is the one that produces non-trivial `rounds > 1` data; the lenient-mode run serves as the non-regression baseline (proving that strict mode did not accidentally become the default for other benchmarks and confirming that the two modes reach the same Normal Form via G1 verification on every repetition).

#### 3.2.8 Demonstrative Arithmetic Benchmark

**R17d.** The **ChurchSumOfSquares** benchmark SHOULD generate a net encoding the sum of squares `sum_{i=1..N} i^2` by (a) pre-encoding each square `i^2` as a canonical Church numeral directly in Rust via `encode_church_into(net, i * i)` and (b) folding the resulting Church numerals right-to-left with `wire_add_into` across `i in (1..=N).rev()`, where N is the size parameter. The result MUST decode (via `decode_nat_or_shared`) to `N*(N+1)*(2*N+1)/6` AND the distributed result MUST equal the sequential result by value (with graph isomorphism as a fallback). NOTE: `wire_mul_into` is deliberately NOT used inside the fold even though it exists and is exercised by standalone `build_mul` tests. Composing `wire_mul_into` inside `wire_add_into` produces reduced nets whose inner `mul` DUP sharing boundaries end up mid-chain in the outer `add` result, which the current `decode_shared_chain` cannot traverse — it only handles a single terminal DUP boundary. A decoder extension for nested DUP boundaries is out of scope for this demonstrative benchmark. **(SHOULD)**

| Property | Value |
|----------|-------|
| Encoding | Pre-encoded canonical Church numerals for each `i^2` (via `encode_church_into`), folded right-to-left by a chain of `wire_add_into` calls |
| Problem | sum_{i=1..N} i^2 |
| Closed form | N*(N+1)*(2*N+1)/6 |
| Initial agents | `sum_{i=1..N}(2*i^2 + 1) + O(N)` = `N(N+1)(2N+1)/3 + O(N)` (squares are pre-encoded, so initial agent count already scales cubically in N) |
| Final agents | `~2 * N(N+1)(2N+1)/6 + 1` (unchanged: result is the Church numeral for the sum of squares) |
| Initial redexes | O(N) CON-CON beta-entry points, one per `wire_add_into` splice |
| Rounds (grid, lenient) | 1 |
| Rounds (grid, strict, expected) | O(N) (1D dependency chain across the add fold) |
| Rules exercised | CON-CON (beta), CON-DUP (add expansion over the pre-encoded Church operands) |
| Profile | B (expansion driven by the add fold over growing Church operands) |
| Default sizes | [5, 10, 30, 50, 100] |
| Correctness | value equivalence via `decode_nat_or_shared`, with `nets_isomorphic` fallback |
| Purpose | article/defense arithmetic demonstration (NOT part of frozen performance campaigns) |

**Purpose:** ChurchSumOfSquares is the single benchmark whose purpose is **explicitly demonstrative, not comparative**. It exists so that the TCC article and defense can exhibit the Relativist grid executing a recognizable arithmetic computation from start to finish and verifying the result against a closed-form formula (Archimedes/Faulhaber: `sum_{i=1..N} i^2 = N(N+1)(2N+1)/6`). The benchmark uses the existing `build_add` infrastructure via the new PortRef-based `wire_add_into` helper to fold the chain of N additions, while each square `i^2` is pre-encoded as a canonical Church numeral directly in Rust through `encode_church_into`. The sibling `wire_mul_into` helper (and `build_mul`) remain part of the codebase and are exercised by standalone multiplication tests and the ChurchMul benchmark, but they are not composed inside the sum-of-squares fold for the decoder-compatibility reasons documented in R17d. No new combinators are introduced.

**Scope boundary:** ChurchSumOfSquares MUST NOT be included in frozen performance campaigns (`v1_local_baseline`, `v1_stress`). It is exercised only via the CLI smoke tests (`relativist bench run --benchmark church_sum_of_squares`) documented in `USAGE_GUIDE.md` Section 11.8. Performance numbers reported for this benchmark are illustrative, not benchmark-grade.

**Rationale:** All other arithmetic benchmarks (ChurchAdd, ChurchMul) are comparative — they contribute to the scaling curves and frozen campaigns. ChurchSumOfSquares fills a different need: a single, visual, "grid computed this number" demonstration for the written and oral defense of the TCC. Sum of squares was chosen because (a) it has a closed-form formula (`N(N+1)(2N+1)/6`) that reviewers instantly recognize, (b) it naturally chains N additions over Church numerals whose magnitudes grow quadratically in N, producing a final Church result whose agent count grows cubically in N and delivering substantial Profile B work from a compact size parameter (e.g., N=100 yields a final net of ~680k agents), and (c) the expansion profile under reduction is strictly Profile B, matching the TCC's real target. The benchmark does not artificially force any agent-type coverage (no pair/fst/K combinator gymnastics); ERA-producing rules may or may not fire incidentally during reduction.

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

#### 3.3.X Streaming Representation Metrics (Tier 3 measurement protocol)

The Tier 3 hardening campaign (D-011) exercises the streaming generation pipeline (SPEC-21), the worker arena recycling discipline (SPEC-22 R1–R10c), and the Sparse construction representation (SPEC-22 §3.2). The legacy `peak_memory_bytes` field (R18) is the process-wide peak RSS (`VmHWM`) sampled AFTER reduction completes; this single watermark conflates construction-phase memory with reduction-phase memory and is therefore insufficient to validate the SPEC-21 R10 / R12 acceptance gate ("memory scales with `chunk_size`, not `total_agents`") or the SPEC-22 R32 free-list memory budget at the M5 milestone (100M agents on a 2 GB coordinator). Requirements R18a–R18g extend `BenchmarkResult` with the metrics needed to validate those gates without breaking the v1 detail.csv schema.

**R18a.** `BenchmarkResult` MUST carry a field `peak_memory_during_construction: u64` (bytes). The framework MUST capture this value at a single, well-defined program point, defined per construction path:

- **Eager path (`chunk_size == None`):** AFTER `Benchmark::make_net(size)` returns AND BEFORE any `reduce_all` / `run_grid` invocation, with no other heap allocation between the return and the sample.
- **Streaming path (`chunk_size == Some(N)`):** AFTER `merge::generate_and_partition_chunked_with_chunk_size_and_lifetime` returns (i.e., all `AgentBatch`es have been produced AND ingested by the partitioner AND all worker partitions have been finalized) AND BEFORE the first `AssignPartition` is dispatched. Sampling at the iterator-exhaustion point but before the partitioner has finalized worker partitions is FORBIDDEN: it underestimates the streaming-architecture's actual memory footprint and produces R18a numbers that are not comparable across implementations.
- **Sparse path (`representation == Sparse`):** AFTER `to_dense(id_range)` returns (i.e., the dense net is materialized) AND BEFORE any `reduce_all` invocation. The Sparse path's R18a thus captures the maximum of (SparseNet construction watermark, `to_dense` peak), since `VmHWM` is monotone-non-decreasing.

On Linux, the value MUST be obtained by reading `/proc/self/status` `VmHWM` at the exact program point above. On non-Linux platforms (Windows development environment), the value MUST be `0` and the limitation MUST be documented (consistent with R18 / §4.4 posture). This is the variable that validates SPEC-21 R10/R12 (memory scales with `chunk_size`) and SPEC-22 R32 (free-list memory budget at M5). **(MUST on Linux; MAY on non-Linux)**

**R18b.** `BenchmarkResult` MUST carry a field `peak_memory_during_reduction: u64` (bytes). The framework MUST capture this value AFTER the final `reduce_all` / `run_grid` call returns and BEFORE any merge cleanup. On Linux, the value MUST be obtained by reading `/proc/self/status` `VmHWM` at that program point. The DIFFERENCE `R18b - R18a` is the working-set delta induced by reduction itself (arena recycling, border state, BorderGraph allocation). The legacy R18 field `peak_memory_bytes` is preserved for backward CSV compatibility with `v1_local_baseline` and MUST be set to `max(R18a, R18b)` (the process-wide watermark over the full execution). This preserves the existing detail.csv column `peak_memory_bytes` unchanged in semantics for all v1-equivalent rodadas (which only exercise the eager path, where R18a ≈ R18b ≈ R18 by construction). **(MUST)**

**R18c.** `BenchmarkResult` MUST carry a field `agent_count_at_construction_complete: u64`. The framework MUST set this value at the same program point as R18a, by the following dispatching discipline (evaluated in order; first match wins):

1. **Sparse path (`representation == Sparse`):** `SparseNet::agents.len() as u64` (a `HashMap::len`, NOT the eventual dense-arena length). Sampled BEFORE the `to_dense(id_range)` conversion. This rule applies regardless of `chunk_size`. Closes F3 of Round 1 review by giving the Sparse axis priority over the streaming axis when both are active.
2. **Streaming path (`chunk_size == Some(N)`, `representation == Dense`):** the sum of `AgentBatch::agents.len()` across all batches produced during construction. Forward references that are subsequently resolved by overwrite are NOT double-counted: the sum is over distinct `AgentId`s emitted by the generator iterator.
3. **Eager path (`chunk_size == None`, `representation == Dense`):** `net.live_agents() as u64` (or equivalently `count_live_agents()` per SPEC-02 R16a, which excludes free-list slots).

Rationale: this is the input-size invariant against which R18d (working-set watermark) and R18a (memory-watermark) are compared in the acceptance gates of §4.9. **(MUST)**

**R18d.** `BenchmarkResult` MUST carry a field `live_agent_count_watermark: u64`. The framework MUST sample the live-agent count at each round boundary during reduction (at the same program point at which `agents_per_round` is recorded, R18) and record the maximum value observed across all rounds. The sampling primitive is `net.agents.iter().filter(|s| s.is_some()).count()` (or equivalently `count_live_agents()`, SPEC-02 R16a). For the Sparse path, R18d is sampled on the dense net produced by `to_dense`, NOT on the original `SparseNet`. R18d closes the empirical-validation gate for "Profile B (expansion + collapse)" benchmarks, where the live-agent count grows above `agent_count_at_construction_complete` mid-reduction before collapsing to the normal form. Without R18d, the only memory-related signal available is post-reduction process-wide RSS, which underestimates the working-set peak. **(MUST)**

**R18e.** `BenchmarkResult` MUST carry a field `representation: NetRepresentation` recording the construction-phase data structure used. The enum `NetRepresentation` is defined in §4.1 alongside `Mode`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum NetRepresentation {
    Dense,
    Sparse,
}
```

R18e MUST be set from the `BenchmarkSuiteConfig::representation` field (D-011 Phase C plan, `relativist-core/src/bench/mod.rs:231-251`). Default value: `Dense`. **(MUST)**

**R18f.** `BenchmarkResult` MUST carry a field `chunk_size: Option<u32>` recording the streaming construction-path chunk size used. `None` denotes the eager path; `Some(N)` denotes the streaming path with `N`-agent batches. R18f MUST be set from the `BenchmarkSuiteConfig::chunk_size` field. **(MUST)**

**R18g.** `BenchmarkResult` MUST carry a field `recycle_policy: RecyclePolicy` recording the worker arena recycling discipline used. The Rust enum `RecyclePolicy` is FIRST DEFINED HERE (SPEC-09 §4.1) for cross-module use; the SEMANTICS of the two variants are governed by SPEC-22 R10b/R10c (the SPEC-22 spec describes the BEHAVIOR; SPEC-09 owns the enum TYPE for `BenchmarkSuiteConfig` and `BenchmarkResult` provenance). Wire-format / persistence concerns, if any, defer to SPEC-22. The variants are:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RecyclePolicy {
    DisableUnderDelta,
    BorderClean,
}
```

R18g MUST be set from the `BenchmarkSuiteConfig::recycle_policy` field. Default value: `DisableUnderDelta`. **(MUST)**

**Backward compatibility with v1 CSVs (joinability gate).** R18a–R18g extend the `detail.csv` schema (R39a) by appending seven new columns to the RIGHT of the existing v1 column order. R39a is the source of truth for the v1 column order; the seven new columns are listed in R39a's schema block following the v1 column sequence. The new columns appear in the order:

```
peak_memory_during_construction,peak_memory_during_reduction,
agent_count_at_construction_complete,live_agent_count_watermark,
representation,chunk_size,recycle_policy
```

This guarantees that `v1_local_baseline/phase2_detail.csv` (which has only the v1 columns) remains joinable to Tier 3 detail.csv by the composite key `(benchmark, input_size, workers, mode, repetition)` without any column-mapping shim — pandas `pd.merge(on=[...])` ignores the rightmost 7 columns of the Tier 3 frame when v1 lacks them, and the leftmost 22 columns are byte-identical in order and semantics. The `summary.csv` (R40) and `rounds.csv` (R39b) schemas are NOT extended by R18a–R18g; per-round memory sampling is intentionally out of scope for this campaign. **(MUST)**

**Field-population discipline for v1-equivalent rodadas.** When `chunk_size == None`, `representation == Dense`, and `recycle_policy == DisableUnderDelta` (i.e., the v1 path), the framework MUST populate R18a–R18d with their meaningful values (memory watermarks and agent counts MUST be measured), and MUST populate R18e–R18g with their default values (`Dense`, `None`, `DisableUnderDelta`). v1-equivalent rodadas thereby produce a strict superset of the v1 detail.csv columns; downstream Python tools that join Tier 3 detail.csv against `v1_local_baseline` detail.csv MAY simply ignore the rightmost 7 columns to recover the v1 schema. **(MUST)**

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

#### 3.4.5 Chunk size

**R29a.** The bench harness MUST expose a variable `chunk_size: Option<u32>` controlling the streaming construction path defined in SPEC-21 §3.5 (chunked pipeline). The semantics of the value are:

- `chunk_size == None` selects the EAGER construction path. The net is materialized in full via `Benchmark::make_net(size)` before any reduction call; this is the v1-equivalent baseline and the only path exercised by the frozen `v1_local_baseline` campaign. **(MUST)**
- `chunk_size == Some(N)` selects the STREAMING construction path. The net is generated as a sequence of `AgentBatch` chunks of `N` agents each via `Benchmark::make_net_stream(size, N)` and partitioned incrementally per batch via `merge::generate_and_partition_chunked_with_chunk_size_and_lifetime` (SPEC-21 R10–R12, R37g). **(MUST when streaming is exercised)**

The chunk size MUST be held constant within a single bench rodada (i.e., across all repetitions of all (benchmark, size, workers, mode) triples in one CSV output). Comparative experiments that vary `chunk_size` across rodadas are out of scope for the D-011 production rodada and are deferred to a future memory-scaling study. The default value for the v2 bench rodada is `Some(10000)`; the eager baseline is `None`. The selected value MUST be recorded in every `BenchmarkResult` row via the new field `chunk_size` (R18f).

#### 3.4.6 Recycle policy

**R29b.** The bench harness MUST expose a variable `recycle_policy ∈ {DisableUnderDelta, BorderClean}` controlling worker-side arena recycling discipline as defined in SPEC-22 R10b/R10c. The semantics are:

- `DisableUnderDelta` (default): recycling is suppressed when a partition is in `delta_mode` AND the reused id belongs to the border-referenced set. This is the conservative SPEC-22 R10b posture. **(MUST as default)**
- `BorderClean`: recycling proceeds unconditionally except for slots whose ids are protected tombstones on the active border (SPEC-22 R10c). MAY be exercised in D-011 Phase C smoke testing only; the production rodada uses the default. **(MAY)**

The selected policy MUST be recorded in every `BenchmarkResult` row via the new field `recycle_policy` (R18g). Production-grade rodadas that mix policies in the same CSV are forbidden; if both policies are exercised, they MUST be written to distinct CSV outputs to preserve provenance.

#### 3.4.7 Representation

**R29c.** The bench harness MUST expose a variable `representation ∈ {Dense, Sparse}` controlling the construction-phase data structure as defined in SPEC-22 §3.2:

- `Dense` (default): the standard `Net` (`Vec<Option<Agent>>`) representation, v1-equivalent. All benchmarks MUST support `Dense`. **(MUST)**
- `Sparse`: the `SparseNet` (`HashMap`-backed) representation. Construction occurs into a `SparseNet`; the structure is converted via `to_dense(id_range)` (SPEC-22 R20) before reduction begins. The `Sparse` value is exercised ONLY in the SparseNet micro-bench scope of D-011 Phase D, restricted to the `dual_tree` benchmark, and is NOT part of the main 13-benchmark experimental matrix (§4.7). **(MAY for `Sparse`)**

The selected representation MUST be recorded in every `BenchmarkResult` row via the new field `representation` (R18e). Sparse-path datapoints MUST be written to a distinct CSV output from dense-path datapoints to preserve provenance and avoid joining Sparse rows against `v1_local_baseline` Dense rows.

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

**R37.** Verification MUST use the benchmark-specific verifier (`Benchmark::verify`). For benchmarks with an empty normal form (EP, DualTree), the verifier MUST check that the result net has 0 agents. For benchmarks with a non-trivial result (TreeSum, MixedNet, ErasurePropagation, ChurchAdd), the verifier MUST compare with the sequential result by graph isomorphism (SPEC-08, `nets_isomorphic`) or by a benchmark-specific metric. For the demonstrative ChurchSumOfSquares benchmark (R17d), the verifier MUST first decode the result via `decode_nat_or_shared` and compare the decoded value against the closed-form `N*(N+1)*(2*N+1)/6` and against the decoded sequential result, falling back to `nets_isomorphic` only if decoding fails. **(MUST)**

**R37a.** Graph isomorphism for correctness verification MAY use a lightweight check when performance is a concern: (a) same agent count per symbol, (b) same wire count, (c) same free port count, (d) same redex count (must be 0 for normal form). Full structural isomorphism (canonical graph comparison via `nets_isomorphic`) SHOULD be used when the lightweight check passes and the result is small (< 1000 agents). For empty-result benchmarks (EP, DualTree), the verifier SHOULD simply check `agent_count == 0`. **(MAY for lightweight; SHOULD for full isomorphism on small nets)**

**R37b.** For the sequential baseline, correctness MUST be verified by comparing the result of each repetition against the first sequential result by graph isomorphism. This validates invariant T6 (uniqueness of normal form, SPEC-01): all sequential runs MUST produce the same normal form regardless of reduction order. A mismatch indicates a reduction engine bug. **(MUST)**

**R37c.** When `representation == Sparse` OR `chunk_size.is_some()` (i.e., any Tier 3 construction path is active), the verifier MUST additionally assert that the net OBSERVED at the end of construction — that is, AT the program point of R18a (defined per path in R18a's bullet list, before any `reduce_all` / `run_grid` invocation) — is graph-isomorphic to the eager-constructed reference net produced by `Benchmark::make_net(size)`. The check MUST use the standard isomorphism primitive `nets_isomorphic` defined in SPEC-08; the lightweight fast-path of R37a (matching agent counts per symbol, wire counts, free-port counts, and redex counts without the canonical structural pass) is permitted on nets larger than 5000 agents, mirroring R37a's threshold. For the Sparse path, the isomorphism check MUST be performed AFTER `to_dense(id_range)` and BEFORE any reduction call, on the dense net produced by the conversion.

**Sequencing constraint (closes F2 of Round 1 review).** The reference eager net's allocation MUST NOT poison R18a. The framework MUST sequence the operations as follows:

1. Build the Tier 3 net (streaming or sparse path).
2. Sample `VmHWM` and write to `peak_memory_during_construction` (R18a).
3. Build the reference eager net via `Benchmark::make_net(size)`.
4. Run `nets_isomorphic` between the Tier 3 net (post-`to_dense` for Sparse; post-finalize for streaming) and the reference eager net.
5. Discard the reference eager net (drop the binding).
6. Proceed to `run_grid` / `reduce_all`.

After step 2, R18a is frozen for the row. The reference net's allocation in step 3 is permitted to raise process-wide `VmHWM` but MUST NOT influence R18a (already captured) and MUST NOT influence R18b (which is sampled AFTER reduction completes — the reference net's allocation pre-dates step 6 by construction, so its watermark contribution is captured by the legacy `peak_memory_bytes` field but NOT by R18b unless the reduction itself rises above it).

Implementations MAY alternatively perform the isomorphism check in a separate "verification run" of the same benchmark, with R18a sampled in a "measurement run" that omits the reference allocation; this is operationally heavier but produces identical `BenchmarkResult` values for R18a/R18b/R37c-validity. Both alternatives satisfy R37c.

Rationale: streaming-built nets MAY have different internal `AgentId` assignments and different arena layouts than their eager equivalents (SPEC-21 R6, R10) but MUST be reduction-equivalent (SPEC-21 R10 / R27 D1). Without this gate, a streaming bug that produces a structurally-different but reducible net would silently pass R37 (which only checks the post-reduction normal form) — for example, a streaming partitioner that drops a wire but happens to preserve the post-reduction agent count for an empty-result benchmark like `ep_annihilation`. R37c closes this hole by requiring construction-phase structural equivalence to the eager reference. The check is conditional (only when Tier 3 paths are active) to avoid imposing the cost on v1-equivalent rodadas, where eager construction IS the reference and the check would be tautological. **(MUST when `representation == Sparse` OR `chunk_size.is_some()`)**

**R38.** A correctness failure on any datapoint MUST halt the suite and report full details: benchmark, size, workers, mode, repetition, and the nature of the divergence (agent count, topology, etc.). **(MUST)**

### 3.7 Output

**R39.** The framework MUST produce CSV output organized into three files. **(MUST)**

**R39a.** `detail.csv`: One row per (benchmark, input_size, workers, mode, repetition). This is the raw data file. Schema:

```
benchmark,input_size,mode,workers,repetition,correct,
wall_clock_secs,total_interactions,mips,
rounds,speedup,efficiency,overhead_ratio,
peak_memory_bytes,bytes_sent,bytes_received,
con_con,dup_dup,era_era,con_dup,con_era,dup_era,
peak_memory_during_construction,peak_memory_during_reduction,
agent_count_at_construction_complete,live_agent_count_watermark,
representation,chunk_size,recycle_policy
```

The leftmost 22 columns (through `dup_era`) are the v1 schema, frozen by `results/locked/v1_local_baseline/phase2_detail.csv`. The rightmost 7 columns (`peak_memory_during_construction` onward) are the Tier 3 measurement protocol additions per R18a–R18g (D-011 Phase F-1). v1-equivalent rodadas (eager path, dense, default recycle policy) MUST still populate the rightmost 7 columns with their default values per R18a–R18g; the rightmost columns MUST NOT be omitted from a v1-equivalent rodada's CSV.

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
    /// R17c: strict-mode multi-round validation benchmark.
    CascadeCross,
    /// R17d: demonstrative sum-of-squares benchmark (non-comparative, not in frozen campaigns).
    ChurchSumOfSquares,
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

/// Construction-phase data structure (R18e, R29c).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum NetRepresentation {
    /// Standard `Net` (Vec<Option<Agent>>), v1-equivalent.
    Dense,
    /// SparseNet (HashMap-backed); converted via `to_dense` before reduction.
    /// SPEC-22 §3.2.
    Sparse,
}

/// Worker arena recycling discipline (R18g, R29b). Owned by SPEC-22 §3.1
/// (R10a); re-exported into the bench module for `BenchmarkSuiteConfig`
/// and `BenchmarkResult` provenance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RecyclePolicy {
    /// Suppress recycling under delta_mode for border-referenced ids
    /// (SPEC-22 R10b conservative posture, default).
    DisableUnderDelta,
    /// Recycle whenever the slot is not on the active border
    /// (SPEC-22 R10c protected-tombstone posture, smoke-test only).
    BorderClean,
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
    /// Streaming construction chunk size (R29a). `None` selects the EAGER
    /// path; `Some(N)` selects the STREAMING path with N-agent batches.
    /// Default: `None` for v1-equivalent rodadas; `Some(10000)` for the
    /// v2 production rodada.
    pub chunk_size: Option<u32>,
    /// Worker arena recycling discipline (R29b). Default:
    /// `RecyclePolicy::DisableUnderDelta`.
    pub recycle_policy: RecyclePolicy,
    /// Construction-phase data structure (R29c). Default:
    /// `NetRepresentation::Dense`. `Sparse` is exercised only in the
    /// D-011 Phase D SparseNet micro-bench (dual_tree only).
    pub representation: NetRepresentation,
    /// Pending-reference lifetime budget for the streaming pipeline
    /// (SPEC-21 R37g). Default: 16. Honored only when `chunk_size.is_some()`.
    pub max_pending_lifetime: u32,
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
      cascade_cross.rs       # CascadeCross (R17c, strict-mode validation)
      church_sum_of_squares.rs # ChurchSumOfSquares (R17d, demonstrative, SHOULD)
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
| CascadeCross (strict + lenient) | B | 5 | 13 x 2 modes | 5 | 650 |
| ChurchSumOfSquares (demonstrative, not in frozen campaigns) | B | 5 | n/a (smoke only) | n/a | n/a |
| **Total** | | | | | **~4810** |

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

### 4.9 Streaming Architecture (Tier 3 measurement protocol)

(Reference: SPEC-21 §3.5 chunked pipeline / R10–R12 / R37g, SPEC-22 §3.1 free-list lifecycle / R10b–R10c, SPEC-22 §3.2 SparseNet / R20.)

The bench harness MUST select between two execution paths based on `BenchmarkSuiteConfig::chunk_size` (R29a):

- `chunk_size == None` selects the **EAGER** path. The net is materialized in full via `Benchmark::make_net(size)`, then `run_grid` (or `reduce_all` for the sequential baseline) is invoked on the resulting `Net`. Memory peak (R18a) occurs at construction. This path is the v1-equivalent baseline and the only path exercised by `results/locked/v1_local_baseline/`.

- `chunk_size == Some(N)` selects the **STREAMING** path. The net is generated as a sequence of `AgentBatch` chunks of `N` agents each via `Benchmark::make_net_stream(size, N)` (SPEC-09 R2 amendment, SPEC-21 §3.8 A4). Each batch is partitioned incrementally as it is produced, via `merge::generate_and_partition_chunked_with_chunk_size_and_lifetime`, and dispatched per-chunk to workers. Memory peak (R18a) is bounded by the largest chunk plus retained borders, per SPEC-21 R10 / R12.

The `BenchmarkSuiteConfig::recycle_policy` field (R29b) selects the worker-side arena recycling discipline (SPEC-22 R10a / R10b / R10c):

- `RecyclePolicy::DisableUnderDelta` (default): recycling is suppressed when a partition is in `delta_mode` AND the reused id is in the border-referenced set. This is the conservative SPEC-22 R10b posture and MUST be the production-rodada default.
- `RecyclePolicy::BorderClean`: recycling proceeds whenever the slot is not on the active border (the protected-tombstone set per SPEC-22 R10c). This is the optimistic posture and is exercised in D-011 Phase C smoke testing only.

The `BenchmarkSuiteConfig::representation` field (R29c) selects the construction-phase data structure:

- `NetRepresentation::Dense` (default): standard `Net` (`Vec<Option<Agent>>`), v1-equivalent. All 13 benchmarks support this representation.
- `NetRepresentation::Sparse`: `SparseNet` (`HashMap`-backed) construction (SPEC-22 §3.2). The structure is converted via `to_dense(id_range)` (SPEC-22 R20) before reduction. The Sparse path is exercised ONLY in the SparseNet micro-bench scope (D-011 Phase D), restricted to the `dual_tree` benchmark, and is NOT part of the main 13-benchmark experimental matrix (§4.7).

#### 4.9.1 Pending-reference budget across batches

The streaming pipeline accumulates forward references (`ConnectionDirective::Pending`) when a wire is emitted in chunk `i` but its target agent has not yet been emitted (it will appear in chunk `j > i`). Per SPEC-21 R37g, the pipeline MUST bound the lifetime of any pending entry to at most `max_pending_lifetime` chunks (default value: `16`, configurable via `GridConfig::max_pending_lifetime`). The bench harness MUST propagate the configured value into the streaming partitioner via `merge::generate_and_partition_chunked_with_chunk_size_and_lifetime`. Exceeding the budget MUST cause the streaming partitioner to return the error `PartitionError::PendingLifetimeExceeded` (this error variant is part of the Phase B-2 fix scope per the D-011 plan and is not yet present in the develop branch as of 2026-04-30; the bench harness MUST surface this error as a benchmark-level fatal failure, halting the suite per R38 with a diagnostic identifying the violating benchmark and chunk size).

When `chunk_size == None` (eager path), the `BenchmarkSuiteConfig::max_pending_lifetime` field MUST be ignored: it MUST NOT be propagated, MUST NOT be validated against any bound, and MUST NOT alter the eager construction path's behavior. The field's default value (`16`) is reported in CSV provenance only when streaming is active; eager-path rodadas are silent on the field. (Closes F6 of Round 1 review.)

#### 4.9.2 Acceptance gates per path

The Tier 3 measurement protocol defines the following acceptance gates. Each gate is a per-benchmark assertion that MUST be checked by the post-processing pipeline (Python scripts under `scripts/plot_results.py`, or equivalent) AFTER the rodada's CSVs are produced. The gates are NOT enforced by the Rust binary at runtime; they are validation criteria for the rodada's output.

- **EAGER acceptance gate (vs. v1_local_baseline).** For every (benchmark, size, workers, mode) row produced by an EAGER-path rodada (`chunk_size == None`, `representation == Dense`, `recycle_policy == DisableUnderDelta`), the median `wall_clock_secs`, median `mips`, and `peak_memory_bytes` MUST exhibit no statistically significant regression versus the corresponding row in `results/locked/v1_local_baseline/phase2_summary.csv`. "No regression" is defined as: the Tier 3 median `wall_clock_secs` is within the v1 bootstrap 95% CI band, the Tier 3 median `mips` is within the v1 bootstrap 95% CI band, and the Tier 3 `peak_memory_bytes` is within ±10% of the v1 value. A regression on any axis MUST be investigated before the rodada is locked. If `v1_local_baseline/phase2_summary.csv` lacks the bootstrap CI columns (`wall_clock_ci95_lo/hi`, `mips_ci95_lo/hi`) — which is the case for the v1 frozen artifact, since R32a is SHOULD and v1 deferred CI computation to Python post-processing — the post-processing step MUST recompute the v1 CI from `phase2_detail.csv` raw data per R32a's offline-bootstrap escape hatch, with 10,000 resamples on the median. **(MUST; closes F7 of Round 1 review)**

- **STREAMING acceptance gate (memory-scaling validation).** For every (benchmark, size, workers, mode) row produced by a STREAMING-path rodada (`chunk_size == Some(N)`), `peak_memory_during_construction` (R18a) MUST be bounded by `O(chunk_size + |retained_borders|)`, independent of `total_agents`. The empirical validation protocol is: run the same benchmark at fixed `chunk_size` while doubling `input_size` (e.g., from `agent_count_at_construction_complete = 10000` to `20000` to `40000`); the median R18a values across the three rodada points MUST stay within a 2× envelope. Failure of this gate indicates that `max_pending_lifetime` is too high, that the generator violates the SPEC-21 R37g forward-reference discipline, or that border retention is unbounded — all of which MUST be triaged before the streaming numbers are reported. **(MUST when `chunk_size.is_some()`)**

- **SPARSE acceptance gate (micro-bench, Phase D).** For the SparseNet micro-bench scope (D-011 Phase D, `dual_tree` benchmark only), the median `peak_memory_during_construction` (R18a) under `representation == Sparse` MUST be strictly less than 80% of the median R18a under `representation == Dense` at the same `input_size`. This is the empirical validation of SPEC-22's "memory savings on sparse-degree workloads" claim. Failure of this gate means SparseNet does not deliver the expected memory savings on `dual_tree` and MUST be triaged before the micro-bench is reported. **(MUST when `representation == Sparse` AND `benchmark == DualTree`)**

#### 4.9.3 Provenance discipline

Tier 3 rodadas (any rodada with `chunk_size.is_some()` OR `representation == Sparse`) MUST be written to CSV files distinct from any v1-equivalent rodada, to prevent silent provenance drift. The recommended naming convention is `{phase}_{path}_{detail|rounds|summary}.csv` where `path ∈ {eager, streaming_chunkN, sparse_dual_tree}`. Mixing paths in the same CSV is forbidden by R29b and R29c (recycle policy and representation are constant within a CSV); chunk_size MAY be constant within a CSV per R29a.

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
| (does not exist) | CascadeCross | New. R17c. Synthetic multi-round BSP stress test. The only benchmark whose strict-mode round count measurably differs from lenient-mode (`rounds == 1`). Primary validation vehicle for SPEC-05 R30a and SPEC-01 D6 v3.1. Profile B. |
| (does not exist) | ChurchSumOfSquares | New. SPEC-09 R17d. Demonstrative sum-of-squares: squares pre-encoded as canonical Church numerals, chained by `wire_add_into` fold. Profile B. Not part of frozen campaigns. |

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
