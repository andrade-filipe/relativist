# TEST-SPEC-T8: Chunk size independence (property test)

**SPEC-21 §7.3 ID:** T8.
**Owning task:** TASK-0541 + TASK-0554.
**Parent spec:** SPEC-21 §7.3 T8.
**Type:** property.
**Theory anchor:** ARG-001 G1; ARG-002 Q7.

---

## Inputs / Fixtures

- A proptest harness varying `(benchmark, size, num_workers, chunk_size)` over a stable seed.
- The sequential baseline `reduce_all(make_net(...))`.

## Property tests

| ID | Property | Generator | Assertion |
|----|----------|-----------|-----------|
| PT-T8-01 | Result is invariant under chunk_size | proptest: fixed `(benchmark ∈ {ep_annihilation, dual_tree})`, fixed `size ∈ [10..100]`, fixed `num_workers ∈ [1..8]`, varying `chunk_size ∈ [1..size]` | `is_behaviorally_equal(streaming_result, sequential_baseline) == true` for ALL chunk_size values. |
| PT-T8-02 | Interaction count is invariant under chunk_size | same generator | `interactions_streaming == interactions_sequential`. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | `chunk_size = 1` (one agent per chunk) | correctness holds; performance worst. |
| EC-2 | `chunk_size = size` (single chunk) | correctness holds; equivalent to the default-impl path. |
| EC-3 | `chunk_size > size` | one chunk only; same as EC-2. |

## Invariants asserted

- G1 (full-cycle equivalence under any chunk_size).
- Partition quality affects performance but never correctness.

## Determinism notes

Proptest seed MUST be stable for reproducibility. Use `proptest::test_runner::Config::with_cases(N)` with a fixed seed; document the seed in the test code.

## Cross-test dependencies

- **TEST-SPEC-0541** (ep_annihilation streaming override) — PT-0541-01 implements this for ep_annihilation.
- **TEST-SPEC-0554** (orchestrator) — PT-0554-01 implements this in general.
- **TEST-SPEC-T6** (streaming vs batch) — sibling: T6 fixes one chunk_size; T8 varies it.
