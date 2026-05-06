# TASK-0604 — Bench harness path selection (eager vs streaming) + `ep_annihilation_stream` wiring (Phase C-2 + C-4)

**Phase:** C-2 + C-4 (D-011 bench harness wiring — core path selection logic)
**Bundle:** D-011 — Tier 3 Hardening + Bench Enablement
**Status:** PENDING
**Priority:** P0 (the central wiring task — without this, `chunk_size` flag is inert)
**Spec:** SPEC-09 R18a–R18g, R37c (committed `82b2d27`); SPEC-21 R23–R26 chunked-pipeline orchestration; SPEC-21 R37g pending-lifetime bound.
**Origin:** D-011 plan §C-2 (path selection) + §C-4 (`ep_annihilation_stream` wiring).
**Estimated complexity:** M (~120 LoC production + ~80 LoC tests)
**Estimated stages duration:** Stages 2→3→4→5→6 over ~1 day.

---

## Context

Per D-011 plan: today `bench/suite.rs:313-438` calls `run_grid(net, GridConfig::default(), &strategy)` unconditionally — the eager path. To exercise Tier 3, the harness must:

- **C-2**: Branch on `config.chunk_size`. If `Some(N)` → invoke `merge::generate_and_partition_chunked_with_chunk_size_and_lifetime` (wired in `5a54111`) using the benchmark's stream (`benchmark.make_net_stream(size, chunk_size)`); pass `config.max_pending_lifetime` and `config.recycle_policy`. Otherwise → keep the current `run_grid` call but propagate `config.max_pending_lifetime` into `GridConfig` (instead of `default()`).
- **C-4**: Wire the `ep_annihilation_stream` benchmark stream-impl into the bench dispatch path (the other 12 benchmarks already use the `default_chunked_iter` from `ec81eb3`, per the plan).

The two are bundled because they are tightly coupled — C-4 requires C-2's branch to be live.

## Dependencies

- **TASK-0602 (C-1)** — REQUIRED for the new fields.
- **TASK-0603 (C-3)** — RECOMMENDED — without C-3, the new fields are populated only via in-code defaults (still testable, just not user-visible). C-3 can land in parallel.
- **TASK-0597 (B-2)** — RECOMMENDED — guarantees `max_pending_lifetime` is honored end-to-end in the streaming generator path. Without B-2, C-2's eager-path branch still works but the streaming branch may bias memory measurements.
- **TASK-0596 (B-1)** — REQUIRED for `--mode tcp` benchmark execution to be meaningful, but C-2 itself only needs the local path to compile and pass.
- **SPEC commit `82b2d27`** — already landed.

## Files in scope

| File | Change |
|------|--------|
| `relativist-core/src/bench/suite.rs:313-438` | Add path-selection branch: `if let Some(chunk_size) = config.chunk_size { ... } else { ... }`. Propagate `config.max_pending_lifetime` and `config.recycle_policy` into both branches' `GridConfig`. |
| `relativist-core/src/bench/streaming.rs` (or wherever `ep_annihilation_stream` lives) | Wire into the bench dispatch table so `BenchmarkId::EpAnnihilation` with `chunk_size: Some(_)` invokes the stream-impl override; default-impl path remains unchanged. |
| `relativist-core/tests/spec09_bench_streaming_path.rs` (new) | Integration: bench with `chunk_size: Some(100)` + `ep_annihilation` + `workers: 2` produces a valid CSV with G1 isomorphism passing; result is bit-isomorphic to the eager-path counterpart at the merged-result level. |
| `relativist-core/tests/spec09_bench_eager_path_regression.rs` (new) | Regression: bench WITHOUT `--chunk-size` (eager path) produces output bit-equivalent to the previous baseline (defends against the C-2 plan's identified silent-regression risk). |

## Files explicitly OUT of scope

- `BenchmarkSuiteConfig` definition — TASK-0602.
- CLI flag wiring — TASK-0603.
- Memory probe — TASK-0605.
- Sparse-net path in suite — TASK-0606.
- Wiring streaming impls for benchmarks OTHER than `ep_annihilation` — explicitly deferred per the plan ("the other 12 benchmarks use the default-impl path via `default_chunked_iter` already working since `ec81eb3`").

## Acceptance criteria

1. Path-selection branch is implemented in `bench/suite.rs`; the eager path is the default (`chunk_size: None`).
2. `config.max_pending_lifetime` is propagated into `GridConfig` in BOTH branches (no `u32::MAX` left in the bench path).
3. `config.recycle_policy` is propagated into the streaming branch's `GridConfig`.
4. `ep_annihilation_stream` is invoked by the streaming branch when the benchmark id is `EpAnnihilation` and `chunk_size.is_some()`.
5. Smoke command from D-011 plan: `cargo run --bin relativist -- bench --benchmark ep_annihilation --sizes 1000 --workers 2 --chunk-size 100 --representation dense` runs to completion in <60s and produces a valid CSV with G1 (full or weak per `skip_g1`) passing.
6. Regression test asserts eager-path output (no `--chunk-size`) is bit-equivalent to the pre-D-011 baseline.
7. Streaming-vs-eager isomorphism test asserts the merged-net structure is the same regardless of path choice (T6/T8 from SPEC-21).
8. All existing tests pass with zero regression. ≥1683 default / ≥1726 zero-copy.

## Test floor delta expected

**+8 to +12 tests** added (path-selection, eager regression, streaming smoke, streaming-vs-eager isomorphism, and a few edge sizes).

## Notes

- This is the riskiest task of Phase C — the silent eager-path regression is the headline risk. Phase C-2's mitigation (the eager-path bit-equivalence regression test) is non-negotiable.
- After this lands, Phase D (sparse) and Phase F-2 (full bench rodada) become possible.
- The streaming path's CSV will populate the new SPEC-09 R18a–R18g columns; the column writers themselves are part of the existing CSV writer infrastructure (no new task) but verify they emit valid values for the new fields when streaming is active (Stage 5 QA scope).
