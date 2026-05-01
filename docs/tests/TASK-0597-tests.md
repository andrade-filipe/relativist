# TEST-SPEC-0597 — Tests for TASK-0597 — Thread `GridConfig.max_pending_lifetime` through legacy callers

**Task:** TASK-0597 (Phase B-2, P1 HIGH)
**Spec:** SPEC-21 §3.7 R37g (`MAX_PENDING_LIFETIME` pending-store memory bound, closes SC-016).
**Origin:** QA-D010-009 residual (commit `5a54111` wired the `_with_lifetime` wrapper end-to-end; legacy callers still hard-code `u32::MAX`).
**Test floor delta:** **+3 default** (no zero-copy gating needed).
**Prerequisites:** None on spec amendments. Recommended landing order: TASK-0596 first.

---

## Test inventory

| test_id | level | target file | prerequisite TASK | cfg gates |
|---|---|---|---|---|
| UT-0597-01 | unit | `relativist-core/src/merge/helpers.rs::tests::legacy_wrapper_forwards_max_pending_lifetime` | none | none |
| IT-0597-02 | integration | `relativist-core/tests/spec21_pending_lifetime_legacy_caller.rs::pending_store_bounded_by_lifetime_16` | none | none |
| IT-0597-03 | integration | `relativist-core/tests/spec21_pending_lifetime_legacy_caller.rs::pending_store_unbounded_under_u32_max_regression_sentinel` | none | none |
| IT-0597-04 | integration | `relativist-core/tests/spec21_pending_lifetime_legacy_caller.rs::no_surviving_u32_max_literal_in_streaming_paths` | none | none |

Total: **4 default tests**.

---

## Per-test specifications

### UT-0597-01 — `legacy_wrapper_forwards_max_pending_lifetime`

**Purpose.** Verify that the legacy entrypoint (`merge::generate_and_partition_chunked_with_delta` or the surviving wrapper) accepts a `max_pending_lifetime` parameter and forwards it byte-equivalently into the pending-store-eviction site.
**Setup.**
- Construct a minimal `GridConfig` with `max_pending_lifetime = 7` (a non-default, non-MAX value).
- Use a small benchmark stream (e.g. 50 chunks × ep_annihilation default).
**Action.** Invoke the legacy entrypoint with the configured `GridConfig`; capture the value seen at the pending-store eviction site (test-only hook or a `tracing` span field captured by a `tracing-test` subscriber).
**Assertions.**
- The captured `max_pending_lifetime` argument equals `7` at the eviction call site.
- No `u32::MAX` constant appears anywhere in the call chain (verified via the captured value, not a string scan — the string scan is IT-0597-04).
**Boundary case coverage.** Catches a buggy fix where the wrapper accepts the new parameter but forgets to forward it (still defaulting `u32::MAX` internally).
**Why it must exist.** Acceptance criterion #2 of TASK-0597 ("`GridConfig.max_pending_lifetime` is the single source of truth, propagated down to the pending-store eviction site").

---

### IT-0597-02 — `pending_store_bounded_by_lifetime_16`

**Purpose.** Behavior test: with `max_pending_lifetime = 16` and a stream of 100 chunks, the pending-store size at every eviction tick MUST never exceed 16 entries.
**Setup.**
- Build a streaming generation pipeline with 100 chunks; chunk size sized to produce non-trivial pending references (≥ 32 per chunk).
- `GridConfig.max_pending_lifetime = 16`.
- Instrument the pending-store with a peek hook (e.g., a `metrics`/`tracing` counter `pending_store.size`) sampled at every chunk boundary.
**Action.** Run `generate_and_partition_chunked_with_delta(stream, config)` (the legacy entrypoint) to completion.
**Assertions.**
- `max_observed_pending_store_size <= 16` (strict bound).
- The pending store is non-empty at some point during the run (sanity — the test is exercising the bound, not running on a degenerate input).
- All 100 chunks complete without error.
**Boundary case coverage.** Catches a buggy fix where the lifetime is propagated but the eviction logic uses the wrong comparator (e.g., `>` instead of `>=`).
**Why it must exist.** Acceptance criterion #3 of TASK-0597; SC-016 closure evidence.

---

### IT-0597-03 — `pending_store_unbounded_under_u32_max_regression_sentinel`

**Purpose.** Negative-control regression sentinel: with `max_pending_lifetime = u32::MAX`, the pending store grows unbounded (= total emitted pending refs across all chunks). Documents that the behavior was PREVIOUSLY broken and the new bound IS load-bearing.
**Setup.**
- Same harness as IT-0597-02 with 100 chunks.
- `GridConfig.max_pending_lifetime = u32::MAX`.
**Action.** Run the streaming generation; record peak pending-store size.
**Assertions.**
- `max_observed_pending_store_size > 16` (strictly more than the bounded case).
- Specifically: `max_observed_pending_store_size >= total_pending_refs_emitted - small_constant_for_natural_eviction_window` (i.e., behavior approaches the unbounded-store baseline).
**Boundary case coverage.** Witness test that the bound in IT-0597-02 is meaningful; without this test, IT-0597-02 could pass on a workload that naturally never accumulates 16 pending refs.
**Why it must exist.** Documents the QA-D010-009 root cause as a regression sentinel — if a future refactor accidentally re-introduces unbounded behavior, this test reveals the new bound is not actually load-bearing on the chosen workload (i.e., need a stronger test).

---

### IT-0597-04 — `no_surviving_u32_max_literal_in_streaming_paths`

**Purpose.** Source-level guard: scan `relativist-core/src/merge/` and `relativist-core/src/partition/streaming.rs` for any literal `u32::MAX` in proximity to a `pending_lifetime` or `MAX_PENDING_LIFETIME` token. Emits a CI-time test failure if any are found.
**Setup.**
- A `tests/` integration test that uses `std::fs::read_to_string` over the in-tree source files (allowed pattern — there is precedent in the repo for build-time / source-scanning tests; if not, this can become a `build.rs`-time check or a clippy lint instead).
**Action.** Read the named files; for each, regex-scan for the pattern `(?i)u32::MAX.*?(pending_lifetime|MAX_PENDING_LIFETIME)|max_pending_lifetime.*?u32::MAX` within a 200-character window.
**Assertions.**
- Match count == 0 across all scanned files.
- If found, the failure message names the file + line so the regression is debuggable.
**Boundary case coverage.** Catches a future PR that adds a new caller and forgets to thread the lifetime through.
**Why it must exist.** Acceptance criterion #1 of TASK-0597 ("No surviving `u32::MAX` literal for `max_pending_lifetime` in any legacy caller of streaming generation"). The grep-based source guard is the only mechanical enforcement.

**Implementation note for developer.** If a source-text scan test is judged stylistically out-of-bounds for the project, an acceptable alternative is to introduce a clippy::disallowed_methods rule on `u32::MAX` in those files via `clippy.toml` — but a single integration test is simpler and the test-spec recommends it.

---

## Coverage matrix

| test_id | R37g | SC-016 | TASK §AC-1 | TASK §AC-2 | TASK §AC-3 |
|---|---|---|---|---|---|
| UT-0597-01 | ✅ | | | ✅ | |
| IT-0597-02 | ✅ | ✅ | | | ✅ |
| IT-0597-03 | ✅ | ✅ | | | |
| IT-0597-04 | | | ✅ | | |

Every acceptance criterion has ≥1 test.

---

## Out-of-scope tests (deferred to other tasks)

- Tests of the `_with_chunk_size_and_lifetime` wrapper itself → already covered by D-010 SPEC-21 tests landed in `5a54111`.
- Tests of bench-harness lifetime propagation → **TASK-0604** (Phase C-2/C-4).
- Tests of streaming `partition::build_subnet` populating `free_list` per-partition → **TASK-0481** (already landed in `v2-development`).
