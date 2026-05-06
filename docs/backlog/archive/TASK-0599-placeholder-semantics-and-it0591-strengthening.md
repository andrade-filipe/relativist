# TASK-0599 — `worker_a/b` placeholder semantics + IT-0591 vacuous-coverage strengthening (QA-D010-010, QA-D010-012)

**Phase:** B-4a (D-011 hardening — first of three sub-tasks)
**Bundle:** D-011 — Tier 3 Hardening + Bench Enablement
**Status:** PENDING
**Priority:** P2 (MEDIUM — invariant strengthening, not user-visible)
**Spec:** SPEC-21 §3 placeholder discipline; IT-0591 invariant test (cross-feature isomorphism for `streaming-no-recycle`).
**Origin:** QA-D010-010 (`worker_a/b` placeholder semantics under-specified) + QA-D010-012 (vacuous IT-0591 coverage — assertions trivially hold and don't exercise the discriminant).
**Estimated complexity:** M (~80 LoC production + ~60 LoC tests — the bulk is test strengthening)
**Estimated stages duration:** Stages 2→3→4→5→6 over ~0.5 day.

---

## Context

Two related but distinct findings, bundled because both touch the same streaming partition test surface:

- **QA-D010-010** — `worker_a` and `worker_b` are used as placeholders for "any two distinct workers" in streaming-mode tests but the documentation, naming, and pinned semantics are under-specified. Current code conflates "the first two workers in iteration order" with "an explicitly chosen pair" — risking flaky tests if iteration order shifts.
- **QA-D010-012** — IT-0591 (`streaming-no-recycle` cross-feature isomorphism) currently runs a degenerate input where pop-counter is trivially zero in *both* arms, making the test vacuously pass even if the gate were broken. Strengthen the input to one where the runtime gate would actually matter (i.e., `recycle_under_delta=true` with non-trivial chunk-cross interactions) so the assertion is discriminating.

## Dependencies

- None on D-011 amendments.
- Independent of TASK-0596/0597/0598. Parallel-OK with B-4b/B-4c.

## Files in scope

| File | Change |
|------|--------|
| `relativist-core/src/partition/streaming.rs` (test module) | Document `worker_a/b` placeholder semantics with a doc-comment block; rename to `worker_first` / `worker_second` if the iteration-order assumption is not safe; pin to explicit `WorkerId(0)` / `WorkerId(1)` in tests where the pair must be deterministic. |
| `relativist-core/tests/spec21_streaming_no_recycle_feature.rs` (or equivalent IT-0591 home) | Strengthen the test input: use a chunked stream with cross-chunk principal pairs that DO trigger free-list recycle attempts under `recycle_under_delta=true`; add an explicit assertion on pop-counter discriminant (with-feature → 0 pops; without-feature → ≥1 pop). |
| `docs/tests/` (test-generator output expected) | Document the new IT-0591 invariant strengthening for traceability. |

## Files explicitly OUT of scope

- `relativist-core/src/net/free_list.rs` — not changing semantics.
- TASK-0591 feature-gate code itself — not modifying its production logic.

## Acceptance criteria

1. `worker_a/b` placeholder usages either renamed or carry a doc-comment block stating their semantics; no test relies on iteration-order coincidence.
2. IT-0591 input is strengthened so that with-feature and without-feature arms produce **observably different** counter values; the test asserts on the discriminant explicitly.
3. IT-0591 assertion passes under both `cargo test --features streaming-no-recycle` AND `cargo test` (no-feature default).
4. New / strengthened tests pass; existing tests pass with zero regression.

## Test floor delta expected

**+3 to +5 tests** added (mostly assertions on the discriminant counter, plus the renamed placeholder semantics test).

## Notes

- The "vacuous test" pattern is a known anti-pattern from D-010 QA. The fix is structural (input strengthening), not a quick patch.
- After this lands, IT-0591 becomes a true regression guard for the feature gate.
