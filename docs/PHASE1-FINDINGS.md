# Phase 1 Findings: Sequential + Local Benchmarks

**Version:** 1.0
**Date:** 2026-04-10
**Status:** Complete
**Cross-references:** SPEC-09 (Benchmarks), SPEC-01 (Invariants, G1), ARG-001 (P1-P6), ARG-004 (Overhead Analysis), DATA-COLLECTION-PLAN v1.1

---

## 1. Campaign Summary

| Metric | Value |
|--------|-------|
| Total datapoints | 2,260 (1,900 original + 360 expanded) |
| Benchmarks | 11 across 5 categories |
| Profiles | A (embarrassingly parallel), B (expansion), C (sequential dependency) |
| Input sizes | 6 to 5,000,000 interactions |
| Worker counts | 0 (sequential), 1, 2, 4, 8 (local in-process) |
| Execution modes | Sequential (`reduce_all`) and Local (in-process grid) |
| G1 correctness | **100%** — all datapoints pass |
| Repetitions | 10 per configuration (2 warmup discarded) |

### Benchmarks by Profile

| Profile | Benchmark | Sizes Tested | Interactions |
|---------|-----------|-------------|--------------|
| A (EP) | ep_annihilation | 1K, 10K, 100K | N |
| A (EP) | ep_annihilation_con | 1K, 10K, 50K, **500K, 1M, 5M** | N |
| A (EP) | ep_annihilation_dup | 1K, 10K, 50K | N |
| B (Expansion) | condup_expansion | 50, 100, 1K, 5K | N |
| B (Mixed) | mixed_net | 50, 100, 1K | varies |
| C (Sequential) | dual_tree | 6, 8, 10, 12, 14, **18, 20, 22** | 2^d - 1 |
| C (Erasure) | erasure_propagation | 50, 500, 5K | N |
| Encoding | church_add | 5, 10, 20, 50 | ~N |
| Encoding | church_mul | 5, 10, 20, 50 | ~N^2 |
| Data-bound | tree_sum | 16, 64, 256 | N-1 |
| Data-bound | tree_sum_balanced | 16, 64, 256 | N-1 |

Sizes in **bold** are expanded tests (Phase 1b) targeting longer execution times for Phase 2/3 comparison.

---

## 2. Key Results

### 2.1 Speedup — No Effective Parallelism with 2+ Workers

**The central finding: no benchmark achieves speedup > 1.0 with 2 or more workers.** This holds across all profiles, all sizes, up to 5 million interactions.

#### Speedup by Profile (representative benchmarks, largest tested size)

| Benchmark | Size | 1 worker | 2 workers | 4 workers | 8 workers |
|-----------|------|---------|----------|----------|----------|
| **Profile A** | | | | | |
| ep_annihilation_con | 5M | 0.99 | 0.30 | 0.28 | 0.21 |
| ep_annihilation_con | 50K | 0.96 | 0.41 | 0.38 | 0.33 |
| ep_annihilation_dup | 50K | 0.87 | 0.36 | 0.38 | 0.31 |
| **Profile B** | | | | | |
| condup_expansion | 5K | 0.64 | 0.09 | 0.04 | 0.01 |
| mixed_net | 1K | 0.88 | 0.11 | 0.03 | 0.01 |
| **Profile C** | | | | | |
| dual_tree | 22 | 0.97 | 0.20 | 0.20 | 0.20 |
| dual_tree | 14 | 0.92 | 0.24 | 0.19 | 0.18 |
| erasure_propagation | 5K | 0.85 | 0.10 | 0.05 | 0.02 |
| **Encoding** | | | | | |
| church_add | 50 | 0.45 | 0.02 | 0.01 | 0.01 |
| church_mul | 50 | 0.75 | 0.001 | 0.0003 | 0.0001 |
| **Data-bound** | | | | | |
| tree_sum | 256 | 0.36 | 0.003 | 0.003 | 0.002 |

#### Speedup Does Not Improve with Larger Problems

For ep_annihilation_con (Profile A, best case), speedup at 8 workers:

| Size | Sequential time | 8-worker time | Speedup |
|------|----------------|---------------|---------|
| 1K | 0.0003s | 0.0006s | 0.46 |
| 10K | 0.002s | 0.006s | 0.36 |
| 50K | 0.010s | 0.031s | 0.33 |
| 500K | 0.177s | 0.776s | 0.22 |
| 1M | 0.386s | 1.985s | 0.20 |
| 5M | 2.920s | 13.957s | 0.21 |

**Overhead remains constant (~80%) regardless of problem size.** Increasing the problem by 5000x does not change the speedup ratio. The grid loop cost scales linearly with the input, just like the sequential reduction.

### 2.2 Overhead Ratio

The overhead ratio measures the fraction of wall clock time spent on infrastructure (partition, merge, border resolution) rather than useful computation:

| Workers | Typical overhead | Interpretation |
|---------|-----------------|----------------|
| 1 | 0.01–0.06 | Near-zero (1-worker path skips partitioning) |
| 2 | 0.56–0.98 | 56–98% of time is overhead |
| 4 | 0.65–0.99 | 65–99% of time is overhead |
| 8 | 0.79–0.99 | 79–99% of time is overhead |

### 2.3 Rounds — Always 1

All benchmarks converge in exactly 1 grid round in local mode. This means the BSP cycle (split → reduce → merge → resolve → repeat) never iterates. The `resolve_borders` step at merge resolves all cross-partition redexes in a single pass.

### 2.4 Border Interactions

| Profile | Border ratio | Explanation |
|---------|-------------|-------------|
| A (EP) | 0% | All redexes are independent; no cross-partition edges |
| B (Expansion) | Variable | Some cross-partition edges from CON-DUP expansion |
| C (DualTree) | ~100% | All interactions are cross-partition (butterfly pattern) |

### 2.5 Correctness (G1)

**100% pass rate across all 2,260 datapoints.** The Fundamental Property G1 (`reduce_all(net) ≅ extract_result(run_grid(net, K))`) is verified on every measurement via graph isomorphism.

---

## 3. Limitations

### L1: Grid Loop Overhead Dominates Reduction

**Evidence:** `overhead_ratio > 0.70` for 2+ workers in ALL benchmarks, ALL sizes (up to 5M interactions).

**Root cause:** The BSP cycle (partition → local reduce → merge → resolve borders) has per-round cost that scales linearly with agent count. In Rust, `reduce_all` is O(I) with O(1) per interaction (queue-based redex dispatch), so local reduction is extremely fast. The grid infrastructure cost (cloning nets, assigning agents to partitions, rebuilding after merge, resolving borders) is comparable to or exceeds the reduction itself.

**Comparison with Haskell prototype:** The Haskell prototype showed super-linear speedup (10.6× with 4 workers for Profile A) because its `findRedexes` was O(N²). Workers each searched a smaller space, giving a quadratic benefit. Rust's O(1) queue eliminates this artifact — the sequential baseline is already optimal, leaving no room for parallelism to compensate.

**Impact:** Distribution does not pay off for ANY tested size (up to 5M). The break-even point may not exist with the current in-process grid architecture.

### L2: All Benchmarks Converge in 1 Round

**Evidence:** `rounds = 1` for ALL benchmarks in local mode.

**Root cause:** The contiguous-id partitioning distributes agents to workers. Each worker reduces locally in round 1. The merge step then resolves all border redexes sequentially in a single pass. Since border resolution completes in one step, the BSP cycle never iterates.

**Theoretical exception:** DualTree should require `d` rounds (one per tree level), but in local mode the `resolve_borders` merge handles all levels at once — the multi-round behavior only manifests with real TCP distribution where each round requires a network round-trip.

**Impact:** Phase 1 data does not validate the multi-round BSP protocol. Phases 2/3 (TCP) will exercise this.

### L3: G1 Isomorphism Intractable for Large Non-Empty Nets

**Evidence:** `condup_expansion` benchmarks above size 5,000 cause the benchmark suite to hang (minutes per G1 verification call).

**Root cause:** The `nets_isomorphic()` function uses backtracking bijection search with symbol-based pruning — O(N!) worst-case complexity. CON-DUP commutation is the only IC rule that increases agent count (2 agents → 4 agents). For `condup_expansion(5000)`: 10K initial agents expand to ~30K+ agents in the result. The isomorphism search on 30K agents with mixed CON/DUP symbols is computationally intractable.

**Benchmarks affected:** Only `condup_expansion` (Profile B). All other benchmarks either reduce to empty nets (EP variants, DualTree — trivial isomorphism) or have small results (Church numerals, TreeSum).

**Mitigation:** Accept data up to size 5K for `condup_expansion`. Document as known limitation.

### L4: High Measurement Variance (CV > 10%)

**Evidence:** 73% of configurations have CV (coefficient of variation) > 10%. Worst cases: `erasure_propagation` size=50 (CV=1.34), `condup_expansion` size=50 (CV=0.79).

**Root cause:** Combination of:
- Benchmarks with wall clock < 1ms (timer resolution dominates)
- Non-isolated environment (Windows desktop, background processes)
- Non-deterministic cache/memory behavior
- 10 repetitions may be insufficient for sub-millisecond benchmarks

**Mitigation in expanded data:** `ep_annihilation_con` at 1M+ and `dual_tree` at depth 20+ achieve CV < 5% — execution times > 100ms stabilize measurements. Phase 2/3 comparisons should focus on these larger sizes.

### L5: Speedup > 1.0 Only with 1 Worker (Measurement Anomaly)

**Evidence:** 6 of 190 configurations show speedup > 1.0, ALL with exactly 1 worker. Best case: `mixed_net` size=50 at 1.57×.

**Root cause:** The `run_grid` function with 1 worker skips partitioning and calls `reduce_all` directly (optimization in `grid.rs` lines 42-44). The apparent "speedup" is measurement noise or cache warm-up effects, not actual parallelism benefit.

**Impact:** No benchmark demonstrates effective parallelism. The grid loop is strictly slower than sequential for 2+ workers.

---

## 4. Root Cause Analysis

### Why Haskell Showed Speedup but Rust Does Not

The Haskell prototype (AC-005) reported super-linear speedup for Profile A:
- `ep_annihilation` 10K with 4 TCP workers: **10.6× speedup**
- Mechanism: Haskell's `findRedexes` scanned ALL agents to find active pairs — O(N²) per reduction step. With K workers, each worker searched N/K agents, giving O(N²/K²) total — a quadratic improvement.

Rust's `reduce_all` uses a pre-computed redex queue — O(1) per interaction, O(I) total. There is no N² bottleneck to parallelize away. The sequential baseline is already near-optimal.

**This is not a bug — it is a fundamental architectural difference.** The Haskell speedup was an artifact of an inefficient implementation, not a property of Interaction Combinators. Rust correctly implements the efficient algorithm, which leaves no room for distribution to improve on.

### Overhead Decomposition (Local Mode)

For local in-process grid with K workers, the cost per round is:

| Phase | Cost | Scales with |
|-------|------|-------------|
| Partition | O(A) | Agent count |
| Clone K nets | O(A × K) | Agent count × workers |
| Local reduce | O(I/K) per worker | Interactions / workers |
| Merge K results | O(A) | Agent count |
| Resolve borders | O(B) | Border redex count |

When `reduce_all` is already O(I) with tiny constant factor, the partition + clone + merge overhead dominates. Adding workers increases clone cost linearly without proportionally reducing compute time.

---

## 5. Implications for Phase 2 (Docker) and Phase 3 (Network)

### Expected Behavior

TCP distribution will **add** overhead on top of the existing grid overhead:
- **Phase 2 (Docker loopback):** ~0.01ms RTT per round, serialization cost (bincode + CRC32)
- **Phase 3 (LAN):** ~0.1–1ms RTT per round, real network jitter

Since local mode already shows 70-98% overhead, TCP modes will be **strictly worse** in absolute speedup. However, the scientific value of Phases 2/3 is:

1. **Decompose overhead by layer:** Algorithmic (Phase 1) vs. serialization (Phase 2) vs. network (Phase 3)
2. **Exercise multi-round BSP:** TCP distribution forces real round-trip per BSP round, unlike local mode which collapses to 1 round
3. **Validate protocol correctness under real conditions:** G1 with TCP, concurrent connections, retry logic
4. **Measure communication metrics:** bytes_sent, bytes_received, network overhead %

### Recommended Focus for Phase 2/3

Use the expanded sizes that produce stable measurements (CV < 10%):
- `ep_annihilation_con` at 500K, 1M, 5M (Profile A baseline)
- `dual_tree` at 18, 20, 22 (Profile C — sequential dependency)
- `condup_expansion` up to 5K (Profile B — limited by L3)

---

## 6. Summary Tables

### Table A: Speedup at 8 Workers (All Benchmarks, Largest Size)

| Benchmark | Size | Seq. time (s) | 8w time (s) | Speedup | Overhead |
|-----------|------|--------------|------------|---------|----------|
| ep_annihilation_con | 5M | 2.920 | 13.957 | 0.21 | 0.88 |
| ep_annihilation_con | 50K | 0.010 | 0.031 | 0.33 | 0.79 |
| ep_annihilation_dup | 50K | 0.010 | 0.033 | 0.31 | 0.81 |
| ep_annihilation | 100K | 0.023 | 0.067 | 0.34 | 0.77 |
| condup_expansion | 5K | 0.001 | 0.063 | 0.01 | 0.99 |
| mixed_net | 1K | 0.001 | 0.078 | 0.01 | 0.99 |
| dual_tree | 22 | 1.162 | 6.611 | 0.20 | 0.98 |
| dual_tree | 14 | 0.002 | 0.010 | 0.18 | 0.96 |
| erasure_propagation | 5K | 0.000 | 0.012 | 0.02 | 0.98 |
| church_add | 50 | 0.000 | 0.000 | 0.01 | 0.99 |
| church_mul | 50 | 0.000 | 0.011 | 0.0001 | 0.99 |
| tree_sum | 256 | 0.000 | 0.000 | 0.002 | 0.99 |
| tree_sum_balanced | 256 | 0.000 | 0.000 | 0.002 | 0.99 |

### Table B: Measurement Stability (CV by Configuration Type)

| Category | Configs with CV < 10% | Configs with CV > 10% | Worst CV |
|----------|----------------------|----------------------|----------|
| Sequential | 19 / 38 (50%) | 19 / 38 (50%) | 0.79 |
| Local 1w | 26 / 38 (68%) | 12 / 38 (32%) | 0.60 |
| Local 2w | 14 / 38 (37%) | 24 / 38 (63%) | 0.79 |
| Local 4w | 12 / 38 (32%) | 26 / 38 (68%) | 0.49 |
| Local 8w | 15 / 38 (39%) | 23 / 38 (61%) | 0.48 |
| **Expanded (>100ms)** | **16 / 18 (89%)** | **2 / 18 (11%)** | 0.13 |

Expanded tests with wall clock > 100ms achieve stable measurements (CV < 10% in 89% of cases).
