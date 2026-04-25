# TEST-SPEC-0531: FennelStreamingStrategy (advanced; locality-aware)

**SPEC-21 §7 ID:** T9 (strategy independence) partial; full T9 requires TEST-SPEC-T9 spec-catalog test.
**Owning task:** TASK-0531.
**Parent spec:** SPEC-21 §3.1 R5, R6, R7, R8, R9; §4.4 FENNEL design; Q3 fixed-alpha calibration.
**Type:** unit + property.
**Theory anchor:** REF-TBD (FENNEL/LDG REF-NNN registration is TCC-root cleanup; SC-020 deferred per spec annotation). The strategy follows the FENNEL streaming graph partitioning algorithm (Tsourakakis et al., 2014) with `alpha = 1.0` per Q3.

---

## Inputs / Fixtures

- A `dual_tree(64)` net synthesized as a stream of `AgentBatch`es (chunk size = 16, 4 chunks).
- A fresh `FennelStreamingStrategy::with_alpha(1.0)`.
- `num_workers = 4`.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0531-01 | `r6_cache_size_matches_total_agents` | full 64-agent run | post-finalize: inspect the strategy's internal `assignment_cache.len()` (test-only accessor or field) | `== 64` (one entry per agent). |
| UT-0531-02 | `r6_cache_memory_bound` | the same run | compute approx bytes used by the cache | `<= total_agents * 8` bytes (R6: 8x memory reduction vs full net). |
| UT-0531-03 | `r8_determinism_repeated_invocation` | the same dual_tree(64) batch sequence twice | inspect outputs | byte-identical `Vec<WorkerId>` outputs across runs (including the tiebreak path). |
| UT-0531-04 | `c1_complete_coverage` | the 64-agent run | union of all assignments | covers every agent_id in 0..63 with no omissions and no duplicates (R7). |
| UT-0531-05 | `tiebreak_is_deterministic` | a synthetic batch where two workers have identical FENNEL scores for the same agent | inspect chosen worker | the tiebreak picks the lowest `WorkerId` (or whatever the §4.4 spec mandates — implementation MUST document this in Rustdoc; UT-0531-05 grep-asserts the docs match the runtime behavior). |
| UT-0531-06 | `fixed_alpha_load_imbalance_acceptable` | dual_tree(64) run with alpha=1.0, num_workers=4 | inspect `finalize().per_worker_agent_counts` | `max(counts) <= 2 * mean(counts)` (Q3 sanity bound; not catastrophic load imbalance). |
| UT-0531-07 | `r9_pure_core_grep_gate` | the impl source file | grep for `tokio::`, `async fn`, `.await` | NONE. |
| UT-0531-08 | `o_batch_size_per_call_complexity` | a single `allocate_batch` invocation with batch size B and num_workers K | empirical timing | per-call cost `O(B * K + B * log(K))` (formal analysis-style assertion; tests measure proportional scaling NOT absolute time). |

## Property tests

| ID | Property | Generator | Assertion |
|----|----------|-----------|-----------|
| PT-0531-01 | C1 holds for any batch sequence (with monotone IDs) | proptest: random batches `[10..200]` agents total, `num_workers ∈ [1..8]`, alpha = 1.0 | union covers every emitted agent_id exactly once. |
| PT-0531-02 | Determinism holds for any input | same generator | running twice produces identical `Vec<WorkerId>`. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | A batch with no edges to any previously-seen agent (full standalone batch) | FENNEL falls back to capacity-only scoring; assignment is round-robin-equivalent for this batch. |
| EC-2 | `alpha = 0.0` (no capacity penalty) | strategy reduces to "assign to the worker with most neighbors"; cache memory unchanged; UT-0531-01 still passes. |
| EC-3 | First batch (no prior neighbors) | every agent has score = 0 across all workers; tiebreak path drives assignment (UT-0531-05). |
| EC-4 | A future calibration shows alpha=1.0 is materially worse than batch FENNEL | per Q3, FENNEL drops to FUTURE scope; this TEST-SPEC remains valid only if alpha=1.0 stays in v2. |

## Invariants asserted

- R5 (FENNEL strategy SHOULD).
- R6 (cache memory bound).
- R7 (C1 cross-batch coverage).
- R8 (determinism with deterministic tiebreak).
- R9 (pure Core).
- C1 (Complete Agent Coverage) — preserved.

## ARG/DISC/REF citation

- REF-TBD (FENNEL — Tsourakakis et al. 2014). NOTE: FENNEL/LDG REF-NNN registration is a TCC-root cleanup task per SC-020 deferral. The strategy citation in production Rustdoc MUST use `REF-TBD` placeholder until the TCC-root REF is registered; tests do not block on this.

## Determinism notes

R8 mandates determinism INCLUDING the tiebreak. The implementation MUST use a stable tiebreak (e.g., lowest WorkerId) AND MUST NOT use any RNG, no `HashMap` iteration order dependencies, no wall-clock dependencies. UT-0531-05 grep-asserts the Rustdoc documents the chosen tiebreak.

The internal `assignment_cache` is a `HashMap<AgentId, WorkerId>`. UT-0531-01 reads `.len()` (deterministic regardless of iteration order). Any test that iterates the cache MUST sort by `AgentId` before assertion (HashMap iteration order is non-deterministic per RandomState).

## Cross-test dependencies

- TEST-SPEC-0524 (trait surface) — prerequisite.
- TEST-SPEC-0530 (RoundRobin) — sibling strategy.
- TEST-SPEC-T9 (strategy independence behavioral) — full T9 spec-catalog test compares RoundRobin and FENNEL outputs for isomorphism.
- TEST-SPEC-0542 (dual_tree streaming override) — produces the input `AgentBatch` stream consumed in UT-0531-* fixtures.
