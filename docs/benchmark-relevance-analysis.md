# Benchmark Relevance Analysis

**Date:** 2026-03-26
**Source:** SPEC-09-benchmarks.md (Revised v2)
**Purpose:** Classify the 9 benchmarks by relevance to the TCC's research question and experimental goals. Guide implementation priority and paper writing.

---

## Criteria for Classification

Each benchmark was evaluated against 6 criteria derived from `OBJETIVO_TCC.md` and the TCC's experimental needs:

| # | Criterion | Why it matters |
|---|-----------|----------------|
| C1 | **Tests the hypothesis directly** | Does it verify that distributed reduction produces the same result as sequential? (The Fundamental Property) |
| C2 | **Covers a unique overhead profile** | Does it add a profile (A/B/C) not already covered by other benchmarks? |
| C3 | **Fills a critical gap from the Haskell prototype** | Does it address a documented limitation (AC-005 gaps L1-L8)? |
| C4 | **Enables Haskell-vs-Rust comparison** | Does it have a direct counterpart in the prototype for qualitative comparison? |
| C5 | **Contributes unique insight about distribution** | Does it reveal something new about when/why distribution helps or hurts? |
| C6 | **Essential for honest reporting** | Is it needed to present both favorable and unfavorable results in the paper? |

---

## Tier 1 — Critical (must implement first)

These benchmarks are indispensable. Without them, the TCC cannot answer its research question or produce a credible experimental evaluation.

### 1. EP-Annihilation (ERA) — Profile A

| Criterion | Score | Justification |
|-----------|-------|---------------|
| C1 | Yes | Simplest possible correctness test: N independent ERA-ERA pairs, result must be 0 agents |
| C2 | Yes | Only Profile A representative from the prototype (embarrassingly parallel, 1 round, 0 borders) |
| C3 | Yes | Direct mapping from `mkEPNet`. Expanded sizes up to 100K |
| C4 | Yes | Same design as Haskell. Enables speedup trend comparison (super-linear vs linear) |
| C5 | Yes | Baseline for break-even analysis. Shows best-case distribution behavior |
| C6 | Yes | The "it works" benchmark. If this fails, nothing else matters |

**Why critical:** This is the foundational benchmark. It establishes the baseline for everything: correctness verification, speedup measurement, break-even analysis. It's the simplest case where distribution should clearly help, and the most direct comparison with the Haskell prototype. The disappearance of super-linear speedup (due to O(1) redex queue vs O(N^2) findRedexes) is itself a key finding for the paper.

**Datapoints:** ~350 (7 sizes x 5 worker configs x 2 modes x 5 reps)

---

### 2. DualTree — Profile C

| Criterion | Score | Justification |
|-----------|-------|---------------|
| C1 | Yes | Verifies correctness under maximally adversarial conditions (sequential dependency, many rounds, ~50% border ratio) |
| C2 | Yes | Only Profile C representative with cascade behavior (level-by-level reduction) |
| C3 | Yes | Direct mapping from `mkDualTreeNet` |
| C4 | Yes | Haskell showed 0.18x with 4 workers — key comparison point |
| C5 | Yes | Shows distribution overhead anatomy: where time is spent when distribution does NOT help |
| C6 | **Essential** | Without DualTree, the paper would only show favorable cases. Honest reporting demands showing slowdown |

**Why critical:** DualTree is the "adversarial witness." The TCC's hypothesis says correctness is preserved regardless of distribution — DualTree proves this even when speedup is negative. It demonstrates that strong confluence guarantees determinism even when it's slower. The paper needs this to be credible: showing only Profile A would be cherry-picking.

**Datapoints:** ~300 (6 sizes x 5 worker configs x 2 modes x 5 reps)

---

### 3. CON-DUP Expansion — Profile B

| Criterion | Score | Justification |
|-----------|-------|---------------|
| C1 | Yes | Verifies correctness when the net grows before collapsing (unique dynamic) |
| C2 | Yes | Only Profile B representative. The "unknown territory" with only 4 TCP datapoints in the prototype |
| C3 | Yes | Direct mapping from `mkExpansionNet`. Addresses gap L7 (CON-DUP undertested) |
| C4 | Yes | 4 Haskell datapoints -> ~300 Relativist datapoints. Resolves inconclusiveness |
| C5 | **High** | Profile B is explicitly conjectural (ARG-004). Resolving it is a key contribution |
| C6 | Yes | The middle ground between "distribution helps" (A) and "distribution hurts" (C) |

**Why critical:** Profile B is the TCC's most important empirical contribution. The Haskell prototype has only 4 TCP datapoints for CON-DUP, making its behavior inconclusive. Relativist will produce ~300 datapoints, resolving whether expansion-then-collapse workloads benefit from distribution. This is where the "when does distribution pay off?" question gets its most interesting answer.

**Datapoints:** ~300 (6 sizes x 5 worker configs x 2 modes x 5 reps)

---

### 4. MixedNet — All 6 Rules

| Criterion | Score | Justification |
|-----------|-------|---------------|
| C1 | **Essential** | The ONLY benchmark that verifies correctness across all 6 interaction rules simultaneously in a distributed context |
| C2 | Partial | Profile B (due to CON-DUP), but the value is rule coverage, not profile |
| C3 | Yes | No prototype equivalent. Addresses DISC-003 v2 Section 5.2 point 2 ("insufficient rule coverage") and gap L6 |
| C4 | No | New benchmark, no Haskell counterpart |
| C5 | Medium | Validates system completeness, not a specific distribution pattern |
| C6 | Yes | If the system only works for ERA-ERA and CON-CON, it's not a general IC reducer |

**Why critical:** The TCC claims to implement Interaction Combinators — all 3 symbols, all 6 rules. Without MixedNet, the distributed correctness claim only covers a subset of the rules. This is the benchmark that validates the Fundamental Property in its full generality. The prototype's biggest weakness was testing only ERA-ERA in the primary EP benchmark.

**Datapoints:** ~250 (5 sizes x 5 worker configs x 2 modes x 5 reps)

---

### 5. TreeSum — Profile A/B (Data-Bound)

| Criterion | Score | Justification |
|-----------|-------|---------------|
| C1 | Yes | Verifies correctness with a non-trivial result (sum of values, not just empty net) |
| C2 | Partial | A/B depending on size. Adds the "practical computation" dimension |
| C3 | Yes | Direct mapping from `mkTree` (AC-004) |
| C4 | Yes | Same design as Haskell. Key comparison point for practical workloads |
| C5 | Yes | The only benchmark that simulates a real computation (map/reduce). Bridges theory to practice |
| C6 | Yes | Shows the system doing actual work, not just synthetic rule exercises |

**Why critical:** TreeSum is the benchmark that connects IC reduction to a real-world pattern (map/reduce). While EP, DualTree, and CON-DUP test pure IC dynamics, TreeSum demonstrates that the system can compute something meaningful. The paper needs at least one benchmark where the reader can say "this is useful for real work." Also exercises CON-ERA (erasure/propagation) rules absent from EP.

**Datapoints:** ~350 (7 sizes x 5 worker configs x 2 modes x 5 reps)

---

## Tier 2 — Important (implement after Tier 1)

These benchmarks strengthen the evidence and fill gaps, but the paper could survive without them if time is constrained.

### 6. EP-Annihilation-CON — Profile A

| Criterion | Score | Justification |
|-----------|-------|---------------|
| C1 | Yes | Verifies CON-CON annihilation (cross-reconnection) in distribution |
| C2 | No | Same profile as EP-Annihilation (ERA). Profile A, 1 round, 0 borders |
| C3 | Yes | Addresses gap L6 (EP only tests ERA-ERA) |
| C4 | No | New benchmark, no Haskell counterpart |
| C5 | Low | Distribution behavior identical to EP-ERA. The difference is the reconnection pattern, not the distribution dynamics |
| C6 | Partial | Adds rule coverage but doesn't change the distribution narrative |

**Why Tier 2:** The distribution behavior is identical to EP-ERA (all independent, 1 round, 0 borders). What changes is the reconnection pattern (cross vs void). This is important for correctness coverage but doesn't add new insight about distribution dynamics. If EP-ERA passes and MixedNet passes (which includes CON-CON pairs), this benchmark is confirmatory rather than revelatory.

**Datapoints:** ~300

---

### 7. EP-Annihilation-DUP — Profile A

| Criterion | Score | Justification |
|-----------|-------|---------------|
| C1 | Yes | Verifies DUP-DUP annihilation (parallel-reconnection) in distribution |
| C2 | No | Same profile as EP-ERA and EP-CON |
| C3 | Yes | Addresses gap L6 |
| C4 | No | New benchmark |
| C5 | Low | Same distribution dynamics as EP-ERA and EP-CON |
| C6 | Partial | Completes the annihilation trio |

**Why Tier 2:** Same reasoning as EP-CON. The asymmetry between cross (CON-CON) and parallel (DUP-DUP) reconnection is important for IC theory (universality, REF-002 p.90), but from a distribution perspective, the behavior is identical. MixedNet already exercises DUP-DUP.

**Datapoints:** ~300

---

### 8. ErasurePropagation — Profile C

| Criterion | Score | Justification |
|-----------|-------|---------------|
| C1 | Yes | Verifies correctness under cascade erasure (ERA propagation through CON chain) |
| C2 | Partial | Profile C, but DualTree already covers sequential dependency |
| C3 | No | New benchmark (no prototype counterpart) |
| C4 | No | New benchmark |
| C5 | Medium | Tests a pattern common in real programs (GC of unused subterms). Different cascade shape than DualTree (linear chain vs binary tree) |
| C6 | Partial | Adds depth to Profile C but DualTree already proves the point |

**Why Tier 2:** ErasurePropagation tests an important real-world pattern (garbage collection), and its linear chain shape is structurally different from DualTree's binary tree. However, from the TCC's perspective, DualTree already demonstrates that sequential dependencies cause slowdown under distribution. ErasurePropagation confirms this with a different topology but doesn't fundamentally change the story.

**Datapoints:** ~300

---

## Tier 3 — Nice-to-Have (implement if time permits)

### 9. TreeSumBalanced — Profile A/B

| Criterion | Score | Justification |
|-----------|-------|---------------|
| C1 | Yes | Verifies correctness with balanced partitioning |
| C2 | No | Same profile as TreeSum |
| C3 | Yes | Direct mapping from `mkTreeBalanced` (AC-004) |
| C4 | Yes | Has Haskell counterpart |
| C5 | Low | Tests whether balanced distribution of work improves speedup. The answer depends on the partitioning strategy, which is round-robin by default (SPEC-04). With round-robin, the difference from TreeSum may be minimal |
| C6 | Low | TreeSum already covers the data-bound case |

**Why Tier 3:** TreeSumBalanced is a variant of TreeSum that pre-balances the work distribution. With Relativist's round-robin partitioning (SPEC-04), the natural distribution may already be reasonably balanced, making the difference between TreeSum and TreeSumBalanced small. This benchmark becomes more relevant if/when alternative partitioning strategies are implemented (SPEC-09, R29). It's a SHOULD in the spec, not a MUST.

**Datapoints:** ~350

---

## Summary Table

| Tier | Benchmark | Profile | Rules Tested | Haskell Comparison | Unique Contribution |
|------|-----------|---------|--------------|-------------------|---------------------|
| **1** | EP-Annihilation | A | ERA-ERA | Yes | Baseline, break-even, super-linear disappearance |
| **1** | DualTree | C | CON-CON cascade | Yes | Adversarial case, honest reporting |
| **1** | CON-DUP Expansion | B | CON-DUP + annihilation | Yes | Resolves Profile B (4 -> 300 datapoints) |
| **1** | MixedNet | B | ALL 6 | No | Full rule coverage in distribution |
| **1** | TreeSum | A/B | ERA-ERA, CON-ERA | Yes | Practical computation (map/reduce) |
| **2** | EP-Annihilation-CON | A | CON-CON | No | Cross-reconnection correctness |
| **2** | EP-Annihilation-DUP | A | DUP-DUP | No | Parallel-reconnection correctness |
| **2** | ErasurePropagation | C | CON-ERA / DUP-ERA | No | Cascade erasure pattern |
| **3** | TreeSumBalanced | A/B | ERA-ERA, CON-ERA | Yes | Partitioning quality effect |

---

## Datapoint Budget

| Tier | Benchmarks | Datapoints (5 reps) | Datapoints (30 reps) |
|------|-----------|---------------------|----------------------|
| Tier 1 | 5 | ~1,550 | ~9,300 |
| Tier 2 | 3 | ~900 | ~5,400 |
| Tier 3 | 1 | ~350 | ~2,100 |
| **Total** | **9** | **~2,800** | **~16,800** |

**Recommendation:** Implement Tier 1 first (5 benchmarks, ~1,550 datapoints). This covers all 3 profiles, all 6 rules, enables Haskell comparison, and provides break-even analysis. Tier 2 adds robustness. Tier 3 adds depth on partitioning effects.

---

## Implementation Order (within Tier 1)

1. **EP-Annihilation** — Simplest net generator, simplest verification (0 agents). Use this to validate the entire benchmark framework pipeline (generate -> reduce -> verify -> measure -> CSV).
2. **TreeSum** — Non-trivial result verification (sum check). Validates the framework works with meaningful outputs.
3. **DualTree** — Multi-round grid execution. Validates the framework handles multiple rounds and border resolution.
4. **CON-DUP Expansion** — Expanding nets. Validates the framework handles dynamic net growth.
5. **MixedNet** — All 6 rules. The most complex generator, best saved for last.

---

## What Each Tier Adds to the Paper

### With only Tier 1 (5 benchmarks):
- Section 4 can show: scaling curves for all 3 profiles, break-even analysis, overhead breakdown, Haskell comparison, all-rule correctness verification
- The paper's experimental claims are fully supported

### Adding Tier 2 (+3 benchmarks):
- Strengthens rule coverage claims (dedicated EP for CON and DUP)
- Adds a second Profile C shape (linear chain vs binary tree)
- More robust evidence, but same conclusions

### Adding Tier 3 (+1 benchmark):
- Opens discussion about partitioning strategy effects
- Minor addition unless alternative partitioning is implemented
