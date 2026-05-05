# TEST-SPEC-0540: Benchmark::make_net_stream default-impl path equivalence

**SPEC-21 §7 ID:** T6 partial (full T6 in TEST-SPEC-T6).
**Owning task:** TASK-0540.
**Parent spec:** SPEC-21 §3.2 R10, R11; §7.2 isomorphism contract.
**Type:** unit + CI lint (compile-regression gate for the 13 baseline impls).
**Theory anchor:** ARG-002 (split/merge identity — extended to the streaming wrap).

---

## Inputs / Fixtures

- A non-overriding benchmark: `ep_annihilation_pure(20)` (uses the default `make_net_stream` impl).
- A non-overriding benchmark: `dual_tree(8)` (uses the default impl prior to TASK-0542).
- The `nets_isomorphic` helper (SPEC-00 §6.12 / SPEC-22 R21 `is_behaviorally_equal`).

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0540-01 | `default_impl_path_equivalence_ep_annihilation` | `ep_annihilation_pure` benchmark | (a) collect `make_net_stream(20, 5)` into a `Net` (concatenate batches, install resolved connections); (b) call `make_net(20)` | `nets_isomorphic(net_from_stream, net_from_make_net) == true`. (D1 extended for streaming via the default impl.) |
| UT-0540-02 | `default_impl_path_equivalence_dual_tree` | `dual_tree` benchmark (BEFORE TASK-0542 ships its native override) | same comparison at `dual_tree(8)` | `nets_isomorphic == true`. NOTE: once TASK-0542 ships, `dual_tree` no longer goes through the default impl; this test then becomes vestigial unless re-targeted at a still-non-overriding benchmark. |
| UT-0540-03 | `default_impl_returns_single_batch` | non-overriding benchmark | `make_net_stream(20, 5).count()` | `== 1` (default impl is a single-batch wrapper; the chunk_size argument is ignored at this layer). |
| UT-0540-04 | `default_impl_chunk_size_argument_ignored` | non-overriding benchmark | `make_net_stream(20, 1).collect::<Vec<_>>()` and `make_net_stream(20, 100).collect::<Vec<_>>()` | both produce a single-element Vec; structural equality between the two outputs. |
| UT-0540-05 | `all_13_existing_benchmarks_compile_unchanged` | the v1 baseline catalog | `cargo build -p relativist-core` | success; no per-impl edit needed (CI lint regression gate). |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | A benchmark with `size = 0` | default impl produces a single empty batch; `nets_isomorphic` on two empty Nets returns true. |
| EC-2 | A benchmark with `make_net` that panics | `make_net_stream` MUST propagate the panic at first iteration (default impl does NOT catch panics). |
| EC-3 | A future 14th benchmark not yet in the catalog | UT-0540-05 MUST be updated to include it, OR the lint MUST scan dynamically. The static path is preferred for explicit coverage. |

## Invariants asserted

- R10 (Benchmark trait gains `make_net_stream` with default impl).
- R11 (`make_net` is the source-of-truth materialization path).
- D1 (Split/Merge Identity, extended for streaming) — preserved by R10/R11 isomorphism contract.
- §7.2 T6 isomorphism — partial coverage at the benchmark layer.

## ARG/DISC/REF citation

- ARG-002 (split/merge identity).

## Determinism notes

Pure synchronous (the default impl is `Box::new(std::iter::once(self.make_net(total_size)))`, no async wrapping). `nets_isomorphic` is deterministic (canonical-form comparison).

UT-0540-05 is a CI-lint gate: `cargo build` must succeed. Failure indicates an accidental backward-incompat trait amendment, signaling SC-008 closure regression.

## Cross-test dependencies

- TEST-SPEC-0513 (Benchmark trait amendment) — prerequisite.
- TEST-SPEC-0541 (ep_annihilation native override) — REPLACES the default-impl path for that benchmark.
- TEST-SPEC-0542 (dual_tree native override) — same; UT-0540-02 becomes vestigial after TASK-0542 lands.
- TEST-SPEC-T6 (streaming vs batch isomorphism) — full pipeline-level coverage.
