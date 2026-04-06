# SPEC-09 v3 Task Impact Report

**Date:** 2026-04-05
**Spec:** SPEC-09-benchmarks.md (Revised v2 -> Revised v3)
**Critic review:** SPEC-09-round1-critic.md (17 issues)
**Defender response:** SPEC-09-round2-defender.md (14 accepted, 3 partially accepted)
**Tasks affected:** 22 existing + 2 new = 24 total

---

## Summary

| Category | Count |
|----------|-------|
| Tasks already reflecting v3 | 18 |
| Tasks updated in this pass | 2 |
| New tasks created | 2 |
| Tasks with no SPEC-09 v3 impact | 14 |
| **Total SPEC-09-referencing tasks** | **36** |

---

## New Tasks Created

### TASK-0221: Implement ChurchAdd benchmark (R17a MUST)
- **Source:** SC-004 (Church numeral benchmarks)
- **Spec change:** New R17a -- ChurchAdd benchmark using `build_add(N/2, N - N/2)` from SPEC-14
- **Priority:** P0 (MUST requirement)
- **Depends on:** TASK-0182, TASK-0185, TASK-0204, TASK-0203, Phase 1, Phase 2
- **Complexity:** S
- **Note:** Primary demonstration that Relativist performs real computation. Dual verification: `decode_nat` AND graph isomorphism.

### TASK-0222: Implement ChurchMul benchmark (R17b SHOULD)
- **Source:** SC-004 (Church numeral benchmarks)
- **Spec change:** New R17b -- ChurchMul benchmark using `build_mul(floor(sqrt(N)), floor(sqrt(N)))` from SPEC-14
- **Priority:** P1 (SHOULD requirement)
- **Depends on:** TASK-0182, TASK-0185, TASK-0205, TASK-0203, Phase 1, Phase 2
- **Complexity:** S
- **Note:** Complements ChurchAdd with deeper CON-DUP commutation cascades. Default sizes [5, 10, 20, 50] kept small due to O(a*b) interaction count.

---

## Tasks Updated in This Pass

### TASK-0180: Scaffold bench module structure
- **Change:** Added `church_add.rs` and `church_mul.rs` to the benchmark sub-module declarations and files-to-create list.
- **Source:** SC-004 (new benchmarks require new module files)

### TASK-0193: Implement TreeSum and TreeSumBalanced benchmarks
- **Change:** Updated Haskell comparison sizes from `[50, 200, 1000, 2000]` to `[16, 64, 256]` per R43 amendment (SC-016).
- **Source:** SC-016 (Haskell comparison sizes alignment with DATA-COLLECTION-PLAN)

---

## Tasks Already Reflecting v3 (Updated by Previous Agent)

These tasks were already updated during the SPEC-09 v3 review cycle:

### TASK-0181: Define BenchmarkId, Mode, and core enums
- `BenchmarkId` enum: 11 variants including `ChurchAdd` and `ChurchMul`
- `Mode` enum: 4 variants including `Sequential`
- References R26 (Sequential mode added in v3)

### TASK-0182: Define Benchmark trait
- References R37a (tiered isomorphism strategy, SC-013)
- References R37b (sequential baseline T6 validation, SC-017)
- `verify()` doc updated for ChurchAdd/ChurchMul verification

### TASK-0183: Define BenchmarkResult and metric structs
- References R39a/R39b three-file CSV schema (SC-005)
- Notes about per-round data in rounds.csv vs scalar aggregates in detail.csv

### TASK-0184: Define BenchmarkSuiteConfig and AggregatedStats
- `csv_rounds: Option<PathBuf>` field added (R39b)
- Bootstrap 95% CI fields as `Option<f64>` (R32a, SC-006)
- Median fields for mips, speedup, efficiency, overhead_ratio
- Three-tier repetition scheme comment (5/10/30, SC-001)
- `high_variance_warning` removed from struct (moved to runtime logic)

### TASK-0185: Implement graph isomorphism (nets_isomorphic)
- References R37a tiered isomorphism strategy (SC-013)
- References R37b sequential baseline T6 validation (SC-017)
- SPEC-08 v3 performance target reference

### TASK-0186: Implement statistical functions
- References R32a bootstrap CI (SC-006)
- Note about optional `bootstrap_ci_median` function

### TASK-0187: Implement memory measurement
- References SC-015 Windows fallback via `GetProcessMemoryInfo` or `sysinfo`
- MUST for TCC campaign (Linux), SHOULD for development (non-Linux)

### TASK-0194: Implement MixedNet benchmark
- Expected result block: 4N ERA agents (SC-007)
- Full derivation of expected state in acceptance criteria

### TASK-0195: Implement ErasurePropagation benchmark
- Expected result block: (N+1) ERA agents (SC-008)
- Explicit note: "expected result is NOT 0 agents"

### TASK-0196: Implement CSV output
- Three-file schema: detail.csv (R39a), rounds.csv (R39b), summary.csv (R40) (SC-005)
- CI columns as optional in summary.csv (R32a, SC-006)
- Rounds CSV skips Sequential mode

### TASK-0197: Implement derived metrics computation and aggregation
- Sequential baseline special cases: speedup=1.0, efficiency=1.0, overhead_ratio=0.0 (SC-009, SC-010)
- `seq_baseline_secs` is the **median** of sequential repetitions (SC-011)
- Median fields in aggregation (mips_median, speedup_median, etc.)
- Bootstrap CI fields as `Option<f64>` (R32a)

### TASK-0198: Implement benchmark suite runner
- R37b: sequential T6 validation (each sequential repetition verified against first)
- Three CSV files written (R39a, R39b, R40)
- 11 benchmarks referenced (9 original + ChurchAdd + ChurchMul)
- Sequential baseline uses median time

### TASK-0199: Implement CLI binary and Criterion micro-benchmarks
- `--mode` accepts `sequential` (R26, SC-002)
- `--csv-rounds` flag for rounds.csv (R39b)
- `--csv-summary` flag for summary.csv (R40)

---

## Cross-Phase Tasks with SPEC-09 v3 References (No Updates Needed)

These tasks reference SPEC-09 but were not directly affected by v3 changes, or were already updated during their own spec review cycles:

| Task | Phase | Notes |
|------|-------|-------|
| TASK-0060 | Phase 4 | GridMetrics struct -- referenced by SPEC-09, no v3 changes needed |
| TASK-0092 | Phase 5 | run_coordinator -- metrics structure unchanged |
| TASK-0094 | Phase 5 | GridMetrics network extensions -- no v3 changes needed |
| TASK-0114 | Phase 6 | run_generate_command -- already aligned with SPEC-12 generators |
| TASK-0159 | Phase 8 | OTel trace context -- P2, no v3 impact |
| TASK-0171 | Phase 9 | ep_annihilation generator -- canonical in SPEC-12, no v3 changes |
| TASK-0172 | Phase 9 | ep_annihilation_con/dup generators -- no v3 changes |
| TASK-0173 | Phase 9 | con_dup_expansion generator -- no v3 changes |
| TASK-0174 | Phase 9 | dual_tree generator -- no v3 changes |
| TASK-0175 | Phase 9 | mixed_rules generator -- no v3 changes |
| TASK-0176 | Phase 9 | tree_sum generators -- no v3 changes (generator sizes independent of R43 benchmark sizes) |
| TASK-0177 | Phase 9 | erasure_propagation + Church generators -- no v3 changes |
| TASK-0211 | Phase 11 | ARITH-* benchmark scenarios -- uses SPEC-14, no v3 impact |
| TASK-0212 | Phase 5 | SerializingChannelTransport -- no v3 impact |

---

## SPEC-09 v3 Change -> Task Mapping

| Change (SC-NNN) | Spec Section | Tasks Affected | Status |
|-----------------|-------------|----------------|--------|
| SC-001: 3-tier reps (5/10/30) | R31, Sec 5.3, 6.2 | TASK-0184, TASK-0198 | Done |
| SC-002: Sequential mode in R26 | R26, Sec 3.4.3 | TASK-0181, TASK-0199 | Done |
| SC-003: Generators (Informative) | Sec 4.2, 4.6 | No task impact (spec-only annotation) | N/A |
| SC-004: ChurchAdd/ChurchMul | R17a, R17b, Sec 3.2.6 | TASK-0180, TASK-0181, TASK-0198, **TASK-0221** (new), **TASK-0222** (new) | Done |
| SC-005: CSV 3-file schema | R39a, R39b, R40 | TASK-0183, TASK-0184, TASK-0196, TASK-0198, TASK-0199 | Done |
| SC-006: Bootstrap 95% CI | R32a | TASK-0184, TASK-0186, TASK-0196, TASK-0197 | Done |
| SC-007: MixedNet expected 4N ERA | R16 | TASK-0194 | Done |
| SC-008: EP expected N+1 ERA | R17 | TASK-0195 | Done |
| SC-009: Speedup for workers=0 | R20 | TASK-0197 | Done |
| SC-010: Overhead ratio for sequential | R20 | TASK-0197 | Done |
| SC-011: baseline_sequential_time = median | R20 | TASK-0197, TASK-0198 | Done |
| SC-012: TreeSum pragmatic encoding note | R14 | No task impact (spec-only note) | N/A |
| SC-013: Tiered isomorphism (R37a) | R37a | TASK-0185, TASK-0182 | Done |
| SC-014: SPEC-05 cross-ref clarification | Sec 4.3 | No task impact (spec-only note) | N/A |
| SC-015: Memory measurement Windows fallback | Sec 4.4 | TASK-0187 | Done |
| SC-016: TreeSum Haskell sizes [16,64,256] | R43 | **TASK-0193** (updated) | Done |
| SC-017: Sequential T6 validation (R37b) | R37b | TASK-0185, TASK-0182, TASK-0198 | Done |

---

## BACKLOG.md Changes

1. Added TASK-0221 to Phase 10 table (P0, ChurchAdd MUST)
2. Added TASK-0222 to Phase 10 table (P1, ChurchMul SHOULD)
3. Updated total task count from 201 to 203

---

## Verification Checklist

- [x] All 17 critic issues (SC-001 through SC-017) mapped to tasks
- [x] All new requirements (R17a, R17b, R32a, R37a, R37b, R39a, R39b) have corresponding task coverage
- [x] Expected result corrections (MixedNet 4N ERA, EP N+1 ERA) reflected in tasks
- [x] Sequential mode (R26) reflected in enum and CLI tasks
- [x] Three-file CSV schema (detail, rounds, summary) reflected in CSV and config tasks
- [x] TreeSum Haskell comparison sizes corrected to [16, 64, 256]
- [x] Scaffolding task includes new benchmark module files
- [x] BACKLOG.md updated with new tasks and correct count
- [x] No spec files or code files were edited (territory respected)
