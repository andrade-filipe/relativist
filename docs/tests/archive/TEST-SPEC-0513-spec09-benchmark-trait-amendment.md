# TEST-SPEC-0513: SPEC-09 Benchmark trait amendment (make_net_stream default impl)

**SPEC-21 §7 ID:** plumbing only (gates T6, T8 transitively via TEST-SPEC-0540).
**Owning task:** TASK-0513.
**Parent spec:** SPEC-21 §3.2 R10, R11; §3.8 A4; SC-008 closure.
**Type:** unit + CI lint (regression compile gate).
**Theory anchor:** AC-014 (Bench Methodology — canonical reference for streaming bench harness).

---

## Inputs / Fixtures

- The amended `Benchmark` trait surface in `src/bench/mod.rs`.
- All 13 existing `Benchmark` implementations (the v1 baseline catalog: `ep_annihilation_pure`, `ep_annihilation_con`, `dual_tree`, `con_dup_cascade`, ... — exact list owned by SPEC-09).
- A `cargo build -p relativist-core` regression gate.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0513-01 | `default_impl_signature_matches_spec` | the trait definition | `cargo doc --no-deps -p relativist-core` rendered HTML | the trait method `make_net_stream(&self, total_size: u32, chunk_size: u32) -> Box<dyn Iterator<Item = AgentBatch>>` is present with a default implementation block. |
| UT-0513-02 | `default_impl_returns_single_batch` | a benchmark that does NOT override (e.g., a synthetic `IdentityBenchmark` for the test) calling `make_net_stream(20, 5)` | `iter.collect::<Vec<_>>().len()` | `== 1` (default impl wraps `make_net(total_size)` in a single-element iterator). |
| UT-0513-03 | `default_impl_single_batch_contains_full_net` | UT-0513-02 | inspect `iter.next().unwrap()` (the only batch) | the batch carries all agents and connections that `self.make_net(20)` would produce; serialized agent count matches. |
| UT-0513-04 | `default_impl_chunk_size_argument_ignored` | call `make_net_stream(20, 1)` and `make_net_stream(20, 100)` on the same non-overriding benchmark | compare collected batches | both produce a single batch of size 20; the `chunk_size` argument is ignored by the default impl (this is the deliberate placeholder behavior; native overrides honor `chunk_size`). |
| UT-0513-05 | `all_13_existing_benchmarks_compile_unchanged` | the v1 `Benchmark` impl list | `cargo build -p relativist-core` after adding R10's default-impl method | builds clean; no impl in the catalog needs hand-editing (the trait amendment is fully default-implemented). |
| UT-0513-06 | `r11_make_net_signature_unchanged` | the trait | grep `fn make_net(&self, size: u32) -> Net;` | present and signature byte-for-byte unchanged; R11 reframes its semantics but does not modify the signature. |
| UT-0513-07 | `default_impl_doctest_present` | the trait method's Rustdoc block | grep for an `# Examples` section showing the default impl invocation pattern | present (R10 mandates a doc-test illustrating the default's single-batch wrap). |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | `make_net_stream(0, 0)` on a non-overriding benchmark | the single batch is empty (`AgentBatch::empty()`); pipeline downstream MUST tolerate this without panic. |
| EC-2 | `make_net_stream(u32::MAX, 1)` on a non-overriding benchmark for a benchmark whose `make_net` would OOM at that size | the default impl invokes `make_net(u32::MAX)` and OOMs as expected; this is acceptable because the default does NOT promise streaming behavior; the documented mitigation is to override the method (TASK-0541, TASK-0542). |
| EC-3 | Adding a 14th benchmark that overrides `make_net_stream` | the trait amendment MUST permit overriding without breaking R11 contract; UT-0513-05 is a regression gate (failure indicates an accidental backward-incompat change). |

## Invariants asserted

- R10 (Benchmark trait gains `make_net_stream` with a default implementation; closes SC-008).
- R11 (`make_net` is the source-of-truth materialization path; signature unchanged).
- §3.8 A4 (the trait amendment is a default-impl, not a per-impl rewrite).
- D1 (Split/Merge Identity, extended for streaming) — preserved by the R10 ↔ R11 isomorphism contract; behavioral coverage in TEST-SPEC-0540.

## ARG/DISC/REF citation

- AC-014 (Bench Methodology) — the per-benchmark override path (TASK-0541, TASK-0542) follows the AC-014 throughput-measurement pattern; the default impl satisfies SC-008 with ~30 LoC vs ~520 LoC of mechanical implementation.

## Determinism notes

Pure synchronous trait amendment. UT-0513-05 is a CI-lint test (compile-time): `cargo build` must succeed; failure indicates a per-impl edit was accidentally required, signaling SC-008 closure regression.

## Cross-test dependencies

- TEST-SPEC-0540 (default-impl path equivalence) — behavioral mirror; UT-0513-02/03 here are structural; TEST-SPEC-0540 enforces `make_net_stream(...)` collapsed-to-Net is `nets_isomorphic` to `make_net(...)`.
- TEST-SPEC-0541, TEST-SPEC-0542 (per-benchmark overrides) — depend on the default impl existing; both REPLACE the default with native streaming. UT-0513-05 must hold even after these overrides ship (the 13 baseline impls remain untouched).
