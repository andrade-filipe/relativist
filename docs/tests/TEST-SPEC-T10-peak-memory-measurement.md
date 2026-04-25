# TEST-SPEC-T10: Peak memory measurement (performance)

**SPEC-21 §7.4 ID:** T10.
**Owning task:** TASK-0552 + TASK-0554 (full T10 lives in TASK-0584 / out-of-scope wave 2).
**Parent spec:** SPEC-21 §3.3 R22 (one-batch-in-flight); §7.4 T10.
**Type:** benchmark / instrumented integration.
**Theory anchor:** AC-014 (Bench Methodology).

---

## Inputs / Fixtures

- A peak-allocation tracker (`jemalloc_ctl::stats::allocated`, or a custom `GlobalAlloc` wrapper recording peak).
- A reference run: `ep_annihilation_stream(10_000, chunk_size=100)` with 4 workers.
- A non-streaming baseline: `make_net(ep_annihilation_pure(10_000))` allocates the full net upfront.

## Unit Tests

| ID | Test | Then |
|----|------|------|
| UT-T10-01 | Instrument the streaming pipeline; record peak allocation during the run | peak `<= O(chunk_size + sum(accumulator_sizes) + border_count + pending_size)`. The bound MUST NOT include the full-net allocation that the non-streaming baseline does. |
| UT-T10-02 | Verify peak does NOT scale with `total_agents` beyond accumulator growth | run with `total_agents` ∈ {1_000, 10_000, 100_000} and same chunk_size; peak grows linearly (not super-linearly) and is dominated by accumulator size, not chunk size. |
| UT-T10-03 | Compare to non-streaming baseline | streaming peak `< 0.5 *` baseline peak (the test asserts a meaningful reduction, not strict equality with the formula). |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | `chunk_size > total_agents` (single chunk) | streaming degenerates to non-streaming; peak ≈ baseline peak. |
| EC-2 | A FENNEL strategy with large assignment_cache | peak includes cache (O(total_agents) per R6); the test asserts the documented bound. |

## Invariants asserted

- R22 (one-batch-in-flight memory bound).

## Determinism notes

Memory profiling is platform-specific. The test MAY be `#[cfg(unix)]`-gated and not run on Windows CI. Document the gate. Peak measurement uses jemalloc stats; configure the global allocator to jemalloc for this test.

The exact peak number is workload- and allocator-dependent; tests assert ORDER-OF-MAGNITUDE bounds, not absolute values. Use `tracing::info!` to log measured peaks for trend analysis (not as an assertion).

## Cross-test dependencies

- **TEST-SPEC-0552** (finalize) — UT-0552-* covers the finalize-time memory threshold.
- **TEST-SPEC-0554** (orchestrator) — UT-0554-07 covers the loose pipeline-peak bound.
- **TASK-0584** (out of scope wave 1) — strict T10 implementation with formal peak bound.
- AC-014 cross-reference: methodology for measurement.
