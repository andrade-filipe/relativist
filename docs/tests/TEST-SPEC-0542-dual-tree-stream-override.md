# TEST-SPEC-0542: dual_tree_stream native override (forward references via Pending)

**SPEC-21 §7 ID:** T3 (forward reference resolution); T7 partial.
**Owning task:** TASK-0542.
**Parent spec:** SPEC-21 §3.2 R12, R14 (forward references); §4.7 forward-reference resolution.
**Type:** unit + integration + property.
**Theory anchor:** ARG-002 Q3 (bidirectional FreePort — streaming-extended); ARG-001 (G1 full-cycle equivalence).

---

## Inputs / Fixtures

- `dual_tree_stream(8, chunk_size=4)` → at minimum 2 chunks; the dual_tree topology requires forward references (parent → child wires across chunks).
- `dual_tree_stream(64, chunk_size=8)` → 8+ chunks; stress test for the pending lifetime.
- A constant `MAX_PENDING_LIFETIME` (test-only) bounding the chunk-window over which a pending entry may live.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0542-01 | `t3_forward_reference_resolution` | a `dual_tree_stream(8)` run with chunk_size=4 | locate a batch with a `Pending { target_agent: AgentId(50) }` directive (or any cross-batch wire); track its resolution | the `Pending` directive is converted to a wire (internal or border) AFTER the batch containing the target agent is processed. |
| UT-0542-02 | `t3_pending_resolved_no_orphans_at_end` | full `dual_tree_stream(8)` consumed | post-stream: inspect the pipeline's pending-connection store | empty (R19: empty-pending-store assertion). |
| UT-0542-03 | `r15_monotonicity_dual_tree` | a `dual_tree_stream(8)` run | record (min_id, max_id) per batch | `max_id_in_batch_k < min_id_in_batch_(k+1)` for every consecutive pair. |
| UT-0542-04 | `pending_lifetime_bounded` | a `dual_tree_stream(64)` run | for every Pending directive emitted at chunk N referencing agent ID X (resolved at chunk M): record `M - N` | `M - N <= MAX_PENDING_LIFETIME` (a documented bound; e.g., 4 for dual_tree's tree depth). The bound is a property of the generator topology, not the pipeline. |
| UT-0542-05 | `pending_max_simultaneous_count` | the `dual_tree(64)` run | max `|pending_store|` across the run | `< MAX_PENDING_LIFETIME * max_forward_refs_per_chunk` (loose upper bound; documents memory behavior). |
| UT-0542-06 | `t7_end_to_end_reduction_equivalence_dual_tree_8` | (a) `reduce_all(make_net(dual_tree(8)))`; (b) `run_grid` with the streaming pipeline using `dual_tree_stream(8, chunk_size=2)` | compare results | `nets_isomorphic` AND identical interaction counts (SPEC-01 T7). |
| UT-0542-07 | `each_internal_wire_classified_correctly` | the `dual_tree_stream(8)` run | for each cross-chunk wire: determine whether endpoints land on same or different workers | the test asserts the classification (internal vs border) matches what `install_connection` (TEST-SPEC-0553) computes. |

## Property tests

| ID | Property | Generator | Assertion |
|----|----------|-----------|-----------|
| PT-0542-01 | Forward-ref resolution holds for any dual_tree size | proptest: `depth ∈ [2..6]` (dual_tree(2^depth) sizes), `chunk_size ∈ [2..size]` | empty-pending-store post-stream; `nets_isomorphic` to sequential baseline. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | `chunk_size = 1` for `dual_tree(8)` | maximum forward-reference depth (every parent → child wire is cross-chunk); the pipeline still succeeds; pending store may briefly hold ~size/2 entries. |
| EC-2 | `chunk_size > size` (single batch) | all wires resolved at construction; no Pending directives emitted. |
| EC-3 | A `Pending` directive whose target NEVER appears (synthetic malformed generator) | TEST-SPEC-T4 covers this; TEST-SPEC-0542 does NOT cover the malformed-generator case. |

## Invariants asserted

- R12 (streaming variant for dual_tree).
- R14 (forward references via Pending).
- R15 (generator-phase monotonicity preserved across batches).
- R19 (empty-pending-store post-stream).
- C2 (Complete Wire Coverage) — preserved via forward-ref resolution path.
- C1 (Complete Agent Coverage) — preserved.
- T3 (Forward reference resolution).
- T7 (End-to-end reduction equivalence) — partial.

## ARG/DISC/REF citation

- ARG-002 Q3 (bidirectional FreePort — extended to streaming).
- ARG-001 (G1 full-cycle equivalence — UT-0542-06 is a direct G1 assertion).

## Determinism notes

The full pipeline run (UT-0542-06) MUST use a controlled tokio runtime (`#[tokio::test(flavor = "current_thread")]`) when run as `run_grid`. The merge protocol's BSP barrier is single-threaded by construction; multi-thread runtime adds no parallelism but does add scheduling non-determinism.

`MAX_PENDING_LIFETIME` is a topology-derived constant: for `dual_tree(2^depth)` with `chunk_size = c`, the bound is `ceil(2^depth / c)` in the worst case (full tree spread). UT-0542-04 documents the bound; the implementing test code MUST compute the bound from the input parameters, NOT hardcode an integer.

## Cross-test dependencies

- TEST-SPEC-0540 (default-impl path) — superseded by this native override for dual_tree.
- TEST-SPEC-T3 (forward reference resolution behavioral) — this TEST-SPEC IS T3 partial; spec-catalog T3 generalizes.
- TEST-SPEC-T4 (empty pending store assertion / malformed generator) — sibling test for the negative case.
- TEST-SPEC-T7 (end-to-end reduction equivalence) — UT-0542-06 IS the dual_tree-specific T7 instance.
- TEST-SPEC-0553 (install_connection) — UT-0542-07 cross-checks classification with that helper.
