# TEST-SPEC-0599 — Tests for TASK-0599 — `worker_a/b` placeholder semantics + IT-0591 strengthening

**Task:** TASK-0599 (Phase B-4a, P2)
**Spec:** SPEC-21 §3 placeholder discipline; IT-0591 cross-feature isomorphism (`streaming-no-recycle` gate); SPEC-22 R10b/R10c (free-list LIFO recycle order).
**Origin:** QA-D010-010 (`worker_a/b` placeholder semantics under-specified) + QA-D010-012 (IT-0591 vacuous coverage — assertions trivially hold).
**Test floor delta:** **+4 default** (3 strengthened/new IT assertions + 1 new placeholder-semantics test).
**Prerequisites:** None (parallel-OK with B-4b/B-4c).

---

## Test inventory

| test_id | level | target file | prerequisite TASK | cfg gates |
|---|---|---|---|---|
| UT-0599-01 | unit | `relativist-core/src/partition/streaming.rs::tests::worker_pair_is_pinned_to_worker_id_zero_and_one` | none | none |
| IT-0599-02 | integration | `relativist-core/tests/spec21_streaming_no_recycle_feature.rs::it0591_input_triggers_recycle_attempts_under_baseline` | none | `#[cfg(not(feature = "streaming-no-recycle"))]` |
| IT-0599-03 | integration | `relativist-core/tests/spec21_streaming_no_recycle_feature.rs::it0591_pop_counter_is_zero_under_feature` | none | `#[cfg(feature = "streaming-no-recycle")]` |
| IT-0599-04 | integration | `relativist-core/tests/spec21_streaming_no_recycle_feature.rs::it0591_discriminant_assertion_is_non_vacuous` | none | none (compiles on both feature settings; uses runtime `cfg!` to branch the assertion) |

Total: **4 default tests** (UT-0599-01 + IT-0599-02 OR IT-0599-03 depending on feature + IT-0599-04 unconditional).

---

## Per-test specifications

### UT-0599-01 — `worker_pair_is_pinned_to_worker_id_zero_and_one`

**Purpose.** Pin the `worker_a` / `worker_b` placeholder semantics: in tests that use the streaming partition harness with two workers, `worker_a` MUST be `WorkerId(0)` and `worker_b` MUST be `WorkerId(1)`, regardless of HashMap iteration order or other non-determinism in the harness.
**Setup.** Invoke the streaming-partition test helper that yields the `(worker_a, worker_b)` pair (or, if the renamed-to-`worker_first`/`worker_second` direction is taken, those bindings).
**Action.** `let (worker_a, worker_b) = setup_two_worker_harness();`
**Assertions.**
- `worker_a == WorkerId(0)` (explicit, NOT iteration-order-dependent).
- `worker_b == WorkerId(1)` (same).
- `worker_a != worker_b` (defensive — must always be a true pair).
- A doc-comment in the test cites QA-D010-010 and the placeholder discipline rationale.
**Boundary case coverage.** Catches a regression where `setup_two_worker_harness` switches from `BTreeMap` to `HashMap` and the iteration order swaps — which would silently flip `worker_a/b` and break any test that asserts on per-worker behavior.
**Why it must exist.** Acceptance criterion #1 (placeholder semantics are explicit; no test relies on iteration-order coincidence). This is the test that *codifies the rename/pinning decision* in the test suite.

---

### IT-0599-02 — `it0591_input_triggers_recycle_attempts_under_baseline`

**Purpose.** Strengthen IT-0591: build a chunked stream input with cross-chunk principal pairs that DO trigger free-list recycle attempts. Under the BASELINE (no `streaming-no-recycle` feature), the pop-counter must be **non-zero** — proving the input is not vacuous.
**Setup.**
- Configure a streaming partition with `recycle_under_delta = true`.
- Construct a `Net` whose generator produces principal pairs spanning at least 2 chunks (e.g. CON-CON pairs where one agent is in chunk 1 and its partner is in chunk 2; chunk size ≤ 8 so pairs cross).
- Run reduction to completion.
**Action.** Capture the post-run pop-counter from the streaming partition's free_list (e.g. `partition.free_list_pop_count()` or the equivalent counter exposed for tests).
**Assertions.**
- `pop_count >= 1` — at least one free_list pop attempt occurred (input is non-trivial).
- `pop_count == expected_pops` where `expected_pops` is the deterministic count derived from the input shape (test-generator: pick a generator with exactly `K=4` cross-chunk recycle attempts; assert `pop_count == 4`).
- Final reduced net is graph-isomorphic to the dense-baseline reduction.
**Boundary case coverage.** Catches the QA-D010-012 vacuity: under the OLD trivial input, `pop_count == 0` regardless of feature setting, so the test passed for the wrong reason. Asserting `pop_count >= 1` here makes that regression observable.
**cfg gate.** `#[cfg(not(feature = "streaming-no-recycle"))]`.
**Why it must exist.** Acceptance criterion #2 (with-feature and without-feature arms produce observably different counter values). This is the "without-feature" arm.

---

### IT-0599-03 — `it0591_pop_counter_is_zero_under_feature`

**Purpose.** The feature-on counterpart: with the `streaming-no-recycle` feature enabled, the SAME input (same generator, same chunk size, same `recycle_under_delta`) must produce `pop_count == 0`. The feature gate suppresses the recycle path entirely.
**Setup.** Identical to IT-0599-02 but the test compiles only with the feature on.
**Action.** Same as IT-0599-02; capture pop-counter.
**Assertions.**
- `pop_count == 0` — strict zero (not "small", not "less than baseline" — exactly zero).
- The final reduced net is **still** graph-isomorphic to the dense-baseline reduction (the feature gate must NOT change correctness — only the recycle path is suppressed).
- Total agent count post-reduction is identical to IT-0599-02's reduction (no agents leaked or doubled).
**Boundary case coverage.** Catches a buggy feature implementation that suppresses pops but also drops in-flight redexes (would surface as a divergent reduced net).
**cfg gate.** `#[cfg(feature = "streaming-no-recycle")]`.
**Why it must exist.** Acceptance criterion #3 (IT-0591 passes under both feature settings). This is the "with-feature" arm.

---

### IT-0599-04 — `it0591_discriminant_assertion_is_non_vacuous`

**Purpose.** Meta-test that explicitly asserts the discriminant: the pop-counter values from IT-0599-02 and IT-0599-03 are observably different. This single test contains BOTH paths via `cfg!(feature = ...)` runtime branching (compiles on both feature settings) and asserts the headline discriminant property.
**Setup.** Same generator as IT-0599-02/03 in a single test body. Run the streaming reduction.
**Action.** Capture `pop_count`. Branch on `cfg!(feature = "streaming-no-recycle")` for the assertion.
**Assertions.**
- If feature is ON: `pop_count == 0`.
- If feature is OFF: `pop_count >= 1`.
- A `static_assertions` style or a doc-comment block declares: "the difference between feature-on and feature-off MUST be observable on this input — a future regression that makes BOTH branches yield 0 (vacuous test) is caught by IT-0599-02 failing."
**Boundary case coverage.** Single test body that documents the cross-feature contract. Even if IT-0599-02 / IT-0599-03 are deleted by accident, IT-0599-04 carries the headline assertion forward.
**Why it must exist.** Acceptance criterion #2 verbatim ("the test asserts on the discriminant explicitly"). This is the regression guard against the QA-D010-012 vacuity pattern returning.

---

## Coverage matrix

| test_id | AC-1 (placeholder semantics) | AC-2 (discriminant) | AC-3 (both features pass) | AC-4 (no regression) |
|---|---|---|---|---|
| UT-0599-01 | ✅ | | | ✅ |
| IT-0599-02 | | ✅ | ✅ (no-feature arm) | ✅ |
| IT-0599-03 | | ✅ | ✅ (with-feature arm) | ✅ |
| IT-0599-04 | | ✅ | ✅ | |

Every acceptance criterion has ≥1 test.

---

## Out-of-scope tests (deferred to other tasks)

- Generic free_list LIFO ordering tests → TASK-0596 (UT-0596-03 already covers).
- Tests of the `streaming-no-recycle` feature gate's *production code logic* → out of scope; the production logic landed in TASK-0591 and is not modified here.
- ABI-drift tests for counters → TASK-0598.

---

## Known spec ambiguity (adversarial flag)

- SPEC-21 §3 does not specify whether `worker_a/b` should be the rename direction (`worker_first/second`) or stay as-is with explicit doc-comments. The task text says either is acceptable. UT-0599-01 is written to be agnostic of the chosen direction (asserts on `WorkerId(0/1)` values, not on identifier names) — this is the safe testing posture, but flag for Stage 3 developer to add a doc-comment in the test naming the chosen convention.
- The exact counter API (`partition.free_list_pop_count()` vs. an in-test instrumentation hook) is not specified. The test assumes a counter is reachable from test code; if the production code only exposes the counter under `cfg(debug_assertions)`, this test must be `#[cfg(debug_assertions)]`-gated, which would conflict with TASK-0598's "always-present fields" strategy. Cross-task coordination required: TASK-0599 IT depends on TASK-0598's counter-field strategy.
