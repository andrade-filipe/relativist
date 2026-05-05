# TEST-SPEC-0601 — Tests for TASK-0601 — LIFO non-protected stalemate edge case

**Task:** TASK-0601 (Phase B-4c, P3)
**Spec:** SPEC-21 §3 streaming dispatch fairness invariant.
**Origin:** QA-D010-016 — under LIFO dispatch with no protected-window guarantee, a worker can be perpetually starved of completing a stale chunk.
**Test floor delta:** **+3 default** (1 starvation reproduction + 1 fix-validation + 1 FIFO-non-regression).
**Prerequisites:** None (parallel-OK with B-4a/B-4b).

---

## Test inventory

| test_id | level | target file | prerequisite TASK | cfg gates |
|---|---|---|---|---|
| UT-0601-01 | unit | `relativist-core/src/partition/streaming.rs::tests::lifo_dispatch_skips_protected_in_flight_chunks` | none | none |
| IT-0601-02 | integration | `relativist-core/tests/spec21_lifo_starvation_edge.rs::stale_chunk_completes_under_adversarial_arrivals` | none | none |
| IT-0601-03 | integration | `relativist-core/tests/spec21_lifo_starvation_edge.rs::fifo_path_unchanged_no_perf_regression` | none | none |

Total: **3 default tests**.

---

## Per-test specifications

### UT-0601-01 — `lifo_dispatch_skips_protected_in_flight_chunks`

**Purpose.** Unit-level: with two chunks in the pending store — one in-flight (protected) and one freshly arrived (newer) — LIFO dispatch must select the newer **un-protected** chunk, NOT the in-flight one.
**Setup.**
- Construct a pending store with two chunks: `chunk_old` (push order 1, mark in-flight via the new `chunk_in_flight: bool` flag), `chunk_new` (push order 2, NOT in-flight).
- Configure dispatch policy = LIFO.
**Action.** `let dispatched = store.dispatch_next();`
**Assertions.**
- `dispatched == Some(chunk_new)` (the newer un-protected chunk wins LIFO among non-protected).
- `chunk_old` remains in the store with `chunk_in_flight == true`.
- After `chunk_old`'s in-flight flag is cleared (e.g. on completion), the next `dispatch_next()` returns `chunk_old` (it can now be re-dispatched if needed, OR — per the spec — it has already completed and is removed; the test must match the production semantics).
**Boundary case coverage.** Catches a buggy implementation of LIFO that ignores the protected flag and dispatches the in-flight chunk twice (which would be the original bug).
**Why it must exist.** Acceptance criterion #1 (LIFO dispatch implements protected-window guarantee). This is the unit-level witness.

---

### IT-0601-02 — `stale_chunk_completes_under_adversarial_arrivals`

**Purpose.** Headline regression test: the QA-D010-016 starvation scenario. A slow worker is dispatched a chunk; while it is processing, the coordinator generates and pushes N=20 newer chunks at high frequency. Under the old (buggy) LIFO, the slow worker's chunk would be perpetually superseded; under the fix, the slow worker's chunk completes in bounded steps.
**Setup.**
- In-process coordinator + 1 slow worker (instrumented with a configurable per-chunk delay, e.g. 50 ms).
- Adversarial generator that pushes a new chunk every 5 ms for 100 ms (≈ 20 new chunks during the slow worker's processing time).
- Dispatch policy = LIFO with protected-window.
**Action.** Run the streaming pipeline to completion; capture (a) the order in which chunks complete, (b) the total dispatch round count.
**Assertions.**
- The first-dispatched chunk completes within `O(N)` rounds (bounded — assert `rounds_until_first_chunk_completed <= 30` or some explicit small bound, where N=20).
- All 20+ chunks eventually complete (no permanent starvation — `pending_store.len() == 0` at end).
- The slow worker's first chunk's completion timestamp is BEFORE all 20 adversarial chunks' completion timestamps (or at least: the slow chunk does not finish dead-last — bound is "completes within N=20 rounds of its dispatch").
- A doc-comment cites QA-D010-016 verbatim: "under LIFO + no protected-window, this test would loop indefinitely; with the protected-window fix, it terminates."
**Boundary case coverage.** This IS the bug reproduction. Without the fix, the test loops or times out. With the fix, the test passes within a deterministic bound.
**Why it must exist.** Acceptance criterion #2 (regression test demonstrates the previously starvation-prone scenario terminates within bounded rounds).

---

### IT-0601-03 — `fifo_path_unchanged_no_perf_regression`

**Purpose.** Sanity test: the FIFO dispatch path is NOT modified by this task. Run the same adversarial workload as IT-0601-02 but with FIFO dispatch; assert behavior is identical to the pre-fix FIFO baseline.
**Setup.** Same as IT-0601-02 but `dispatch_policy = FIFO`.
**Action.** Run to completion; capture round count and completion order.
**Assertions.**
- Total round count under FIFO is within ±5% of the pre-fix FIFO baseline (a numeric reference value committed at task-implementation time, e.g. `assert!(rounds <= baseline * 1.05)`).
- Completion order is `[chunk_0, chunk_1, ..., chunk_N]` (strict FIFO — first pushed completes first, modulo worker concurrency).
- No use of the protected-window flag in the FIFO control flow (verifiable by a tracing-log assertion: the `protected_skip` event count is 0).
**Boundary case coverage.** Catches a buggy fix that adds protected-window logic to BOTH paths and slows FIFO. The task explicitly says "FIFO dispatch path — already correct" — this test enforces the task's scope boundary.
**Why it must exist.** Acceptance criterion #3 (existing tests pass with zero regression; no measurable performance overhead in the FIFO path).

---

## Coverage matrix

| test_id | AC-1 (LIFO + protected) | AC-2 (bounded completion) | AC-3 (FIFO unchanged) |
|---|---|---|---|
| UT-0601-01 | ✅ | | |
| IT-0601-02 | ✅ | ✅ | |
| IT-0601-03 | | | ✅ |

Every acceptance criterion has ≥1 test.

---

## Out-of-scope tests (deferred to other tasks)

- Tests of chunk-size selection logic → orthogonal; not modified by this task.
- Tests of the streaming-mode generator → SPEC-21 existing tests.
- Property tests on dispatch fairness for general workloads → out of scope; the task is a narrow edge fix and a property test would over-scope.

---

## Known spec ambiguity (adversarial flag)

- SPEC-21 §3 does not numerically bound "O(N) coordinator steps." The task text says "bounded steps regardless of new arrivals" — IT-0601-02 picks `N=20` and an explicit upper bound of `30 rounds` as a witness. **Flag for Stage 3:** if the production semantics yields a different concrete bound (e.g. O(2N) or O(N + k)), the assertion bound must be regenerated. Document the chosen bound in the test's doc-comment.
- The "in-flight" tag mechanism (the task notes `chunk_in_flight: bool` flag favored over a counter "unless QA Stage 5 demands stronger ordering") is a Stage 3 implementation detail. UT-0601-01 references the boolean flag by name; if the developer picks a counter, regenerate the test access pattern (`chunk.in_flight_count > 0` instead of `chunk.in_flight == true`).
- The `dispatch_policy` enum value names (`LIFO` vs `Lifo` vs `LastInFirstOut`) are not pinned by the spec. Use the production name at implementation time.
