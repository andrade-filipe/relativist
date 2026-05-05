# TASK-0617 — D-011-FU-RELEASE-TESTS: make `cargo test --release` compile

**Phase:** D-012 (Instrumentation Restore) — Stage 3 DEV scope
**Bundle:** D-012 — Instrumentation Restore
**Status:** TODO
**Priority:** P0 (MEDIUM severity per handoff §2 row 3, but blocks the CI release lane independently — recommended to ship FIRST in the bundle)
**Closes red flag:** Not from `D011-final-baseline-analysis` directly, but logged as a separate follow-up in `docs/next-steps.md` 2026-05-05 entry ("`cargo test --release` is broken pre-D-011 …; does not affect production binary or this bench. Logged as a separate follow-up.") and surfaced again in `docs/handoffs/2026-05-05-D012-instrumentation-restore-handoff.md` §2 row 3.
**Spec:** none (compilation-only; behavior unchanged).
**Depends on:** **none — can ship first**, independently of TASK-0615/0616/0618. This unblocks the CI release lane and any future bench wanting `debug_assertions=false` invariant verification.
**Estimated complexity:** S (~10 LoC production, 0 LoC test — the existing test floor exercises the fix once it compiles).

---

## Context

At HEAD `b079cdc`, `cargo test --release` does not compile. Two pre-existing defects unrelated to D-011 are responsible. Neither affects the production binary's correctness or wall-time; they only break the release-mode test build. The defects are:

1. **`net/debug.rs:282-319` — debug-only `mod tests` not symmetrically gated.** The `impl Net { … debug_assert_* methods … }` block at the top of the file is gated `#[cfg(debug_assertions)]`. The `mod tests` at the bottom is gated `#[cfg(test)]` only. In release+test builds, `debug_assertions=false`, so the `impl` block vanishes — but the test module still tries to reference its symbols. Compilation fails.

2. **`coordinator.rs:1871-1873` — non-exhaustive match.** Test `ut_0577_08_rejected_transition_dispatching_first_request_work` matches on `PullCoordinatorError` covering only `UnexpectedEvent`. The `WorkerIdMismatch` variant was added by QA-D010-002 closure in commit `7fca43e`, after this test was written. The test was never re-built in release mode, so the non-exhaustive match never surfaced as a compile error.

Both fixes are surgical, well-defined by the handoff, and require no design decision. The rationale for shipping this first: it unblocks any release-mode CI lane (currently absent) and lets future tasks (TASK-0615/0616) exercise `cargo test --release` for invariant-free regression.

## Files in scope (file:line pointers)

| File | Change |
|------|--------|
| `relativist-core/src/net/debug.rs:282` | **MODIFY.** Change `#[cfg(test)]` to `#[cfg(all(test, debug_assertions))]` on the `mod tests` declaration. Body is unchanged. |
| `relativist-core/src/coordinator.rs:1871-1873` | **MODIFY.** Extend the `match err.unwrap_err()` arm to handle `PullCoordinatorError::WorkerIdMismatch { .. }` with `panic!("unexpected WorkerIdMismatch from DispatchingFirst + RequestWork; expected UnexpectedEvent");`. The existing `UnexpectedEvent` arm is unchanged. |

## Files explicitly OUT of scope

- Anything else. Two files, two surgical edits. **No new tests** — the existing test floor exercises the fix once it compiles.
- The semantics of the `WorkerIdMismatch` variant (added by QA-D010-002) are not in scope; this task only acknowledges its existence in the test arm.
- Adding a CI release lane — that's a separate CICD task (out of D-012 scope).

## Acceptance criteria

1. `cargo test --release` compiles cleanly at HEAD after this change. (Pre-change: fails to compile.)
2. `cargo test --release` runs to completion. Test count must be ≥ 1784 (default debug floor) modulo the 12 `mod tests` functions in `net/debug.rs:282-319` that are now correctly gated out in release. Verify the exact post-fix count and document it in the commit body.
3. **Numeric record:** the commit body MUST report the exact `test result: ok. N passed; 0 failed` line from `cargo test --release`. This becomes the new `cargo test --release` floor row in `next-steps.md` and `CLAUDE.md` (Build & Test section).
4. `cargo test` (debug, default) test count unchanged: **≥ 1784 default**.
5. `cargo test --features zero-copy` unchanged: **≥ 1828**.
6. `cargo test --features streaming-no-recycle` unchanged: **≥ 1775**.
7. v1 floor (690) inviolable.
8. `cargo clippy --all-targets --all-features --release -- -D warnings` clean (verify the release path lints cleanly too — pre-existing lint debt is in scope to surface, not to fix; if any release-only lint surfaces, file as a separate follow-up).
9. `cargo fmt --check` clean.

## Test floor delta

**0 default** (no new tests). Possibly **−12 release** as the 12 functions in `net/debug.rs::tests` are now correctly gated out of release-mode tests (they only ever ran in debug — they assert `debug_assertions`-gated methods). Adjust `cargo test --release` floor to the post-fix observed count.

## Implementation hints

1. The debug.rs fix is a one-character-position attribute change. Verify the existing `mod tests` body has no symbols that exist independently of the `#[cfg(debug_assertions)]` `impl` block; if it does, those tests should be moved out (but inspection of the file at `282-319` suggests all tests reference `debug_assert_*` methods exclusively).
2. For the coordinator.rs fix, prefer `panic!` over `unreachable!` because the test is asserting a specific FSM transition; if `WorkerIdMismatch` is observed, that's a real bug in the FSM, and a panic with diagnostic message is more useful than `unreachable!` (which prints a generic message).
3. After the fix, run `cargo test --release -- --list` to enumerate the test set and capture the exact count for the commit body.

## Estimated LoC

- Production: ~10 LoC (1 attribute change + 1 `match` arm with diagnostic panic).
- Tests: 0 LoC.
- Total: ~10 LoC. Trivially under the 200 LoC ceiling.

## Why ship first

1. **Independent.** No production-code dependency on TASK-0615/0616/0618.
2. **Unblocks CI release lane.** CICD agent can subsequently add a release-mode lane to GitHub Actions.
3. **Lets TASK-0615/0616 exercise release tests.** Both add timing instrumentation that, in release mode, will exhibit different overhead than debug. Having `cargo test --release` working before the metric tasks land lets the developer verify their changes don't blow up in release.
4. **Smallest scope.** ~10 LoC, zero design decisions, fastest to land and review.

## Cross-references

- `docs/analysis/D011-final-baseline-analysis-2026-05-04.md` — does NOT explicitly call out this defect; it was surfaced separately during the D-011 LOCK pass and recorded in `docs/next-steps.md` 2026-05-05 ("`cargo test --release` is **broken pre-D-011** …").
- `results/locked/v2_d011_final_baseline_2026-05-04/MANIFEST.md` "Test-floor status" section (mentions release-mode breakage).
- `docs/handoffs/2026-05-05-D012-instrumentation-restore-handoff.md` §2 row 3, §3 D-011-FU-RELEASE-TESTS subsection (full verbatim diff of both edits).
