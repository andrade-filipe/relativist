# TEST-SPEC-0541: ep_annihilation_stream native streaming override

**SPEC-21 §7 ID:** T5 partial (full T5 in TEST-SPEC-T5); T8 partial (joint with TASK-0567).
**Owning task:** TASK-0541.
**Parent spec:** SPEC-21 §3.2 R12, R13 (no cross-batch wires); §4.5 ep_annihilation example.
**Type:** unit + property.
**Theory anchor:** ARG-002 (independent ERA-ERA pairs are the simplest C2 case).

---

## Inputs / Fixtures

- `ep_annihilation_stream(20, chunk_size=4)` → 5 batches of 4 agents each (each batch = 2 ERA-ERA pairs).
- `ep_annihilation_stream(100, chunk_size=10)` → 10 batches.
- `ep_annihilation_stream(7, chunk_size=4)` → 4 batches (last is partial; depending on impl, partial may be size 2 or padded).

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0541-01 | `pair_batch_invariant` | `ep_annihilation_stream(20, chunk_size=4)` | iterate batches; for each, count `agents.len()` | every batch has even agent count; `agents.len() % 2 == 0`. |
| UT-0541-02 | `resolved_only_no_pending` | the same run | iterate `connection_directives` and match | every directive is `Resolved`; zero `Pending` (R13: no cross-batch wires for ep_annihilation). |
| UT-0541-03 | `r15_monotonicity_across_batches` | the same run | record `(min_id, max_id)` per batch | `max_id_in_batch_k < min_id_in_batch_(k+1)` for every consecutive pair (R15 generator-phase). |
| UT-0541-04 | `total_agents_eq_2x_size` | the run with `size=20` | sum `batch.agents.len()` over all batches | `== 40` (ep_annihilation produces `2 * size` agents). |
| UT-0541-05 | `each_batch_pairs_are_resolved_internally` | a single batch with 4 agents | inspect connection_directives | exactly 2 `Resolved` directives, each connecting two agents within the batch's id_range. |
| UT-0541-06 | `partial_last_batch_handled` | `ep_annihilation_stream(7, chunk_size=4)` | iterate batches | total agents = 14 (`2 * 7`); last batch is partial (size 2 or 6 depending on impl); the test asserts WHICHEVER pattern the impl chose, AS LONG AS the total matches `2 * size`. |
| UT-0541-07 | `chunk_size_one_works` | `ep_annihilation_stream(10, chunk_size=1)` | iterate | not panic (chunk_size=1 means one pair per chunk = 2 agents per batch). |

## Property tests

| ID | Property | Generator | Assertion |
|----|----------|-----------|-----------|
| PT-0541-01 | T8 chunk-size independence (joint with TEST-SPEC-T8) | proptest: `size ∈ [1..200]`, `chunk_size ∈ [1..size]`, `num_workers ∈ [1..8]` | the merged result of `generate_and_partition_chunked` is `nets_isomorphic` to `reduce_all(make_net(ep_annihilation_pure(size)))`. (Joint coverage via TEST-SPEC-T8.) |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | `size = 0` | the iterator is empty (zero batches). Pipeline downstream MUST tolerate. |
| EC-2 | `size = 1` (1 pair = 2 agents) | one batch of 2 agents; one `Resolved` directive. |
| EC-3 | `chunk_size > size` | one batch of `2 * size` agents; no partial batches. |
| EC-4 | A future generator variant (`ep_annihilation_con` with CON instead of ERA) | depending on whether TASK-0541 covers only the ERA variant: if yes, the CON variant uses the default impl and is covered by TEST-SPEC-0540 / future overrides; this test stays scoped to the ERA streamer. |

## Invariants asserted

- R12 (each generator gains streaming variant; ep_annihilation MUST).
- R13 (no cross-batch wires for ep_annihilation — informative, asserted via UT-0541-02).
- R15 (generator-phase monotonicity).
- C1 (preserved — every pair's two agents enter exactly one batch).
- C2 (preserved — every wire is internal-resolved).

## ARG/DISC/REF citation

- ARG-002 (split/merge identity — ep_annihilation is the simplest case where C2 trivially holds).

## Determinism notes

Pure synchronous, no tokio, no RNG (the generator is structural). Tests use sized integer inputs only.

## Cross-test dependencies

- TEST-SPEC-0540 (default-impl path) — superseded by this native override for ep_annihilation.
- TEST-SPEC-T5 (streaming pipeline produces valid partitions) — uses ep_annihilation as the canonical fixture.
- TEST-SPEC-T8 (chunk size independence) — PT-0541-01 is the partial coverage; full T8 in TEST-SPEC-T8.
- TEST-SPEC-0521 (AgentBatch struct) — produces these batches; UT-0521-* covers the type-level invariants this test relies on.
- TEST-SPEC-0544 (R15 monotonicity discipline) — UT-0541-03 is one input case to that property.
