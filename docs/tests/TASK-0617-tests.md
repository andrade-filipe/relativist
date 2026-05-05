# TEST-SPEC-0617 — Tests for TASK-0617 — D-011-FU-RELEASE-TESTS: make `cargo test --release` compile

**Task:** TASK-0617 (D-012 Instrumentation Restore — Stage 3 DEV scope; recommended FIRST in the bundle).
**Spec:** none (compilation-only; behavior unchanged).
**Closes follow-up:** Not from `D011-final-baseline-analysis` directly. Logged in `docs/next-steps.md` 2026-05-05 ("`cargo test --release` is broken pre-D-011 …; does not affect production binary or this bench. Logged as a separate follow-up.") and re-surfaced in `docs/handoffs/2026-05-05-D012-instrumentation-restore-handoff.md` §2 row 3.
**Origin:** Handoff §3 D-011-FU-RELEASE-TESTS (full verbatim diff for both edits).
**Test floor delta:** **0 default** (no NEW Rust tests required — the existing 1784-default floor is the regression gate; once `cargo test --release` compiles, the existing test set exercises the fix). Possibly **−12 release** as the 12 functions in `net/debug.rs::tests` are now correctly gated out of release-mode builds.
**Prerequisites:**
- None. Independent of TASK-0615 / 0616 / 0618. Recommended to ship FIRST.
- No spec change.

---

## Test inventory

| test_id | level | target | prerequisite TASK | cfg gates |
|---|---|---|---|---|
| VP-0617-01 | verification protocol (NOT a Rust test) | `cargo test --release` end-to-end smoke | none | release |
| VP-0617-02 | verification protocol (NOT a Rust test) | Test-count parity vs default profile | none | release |
| VP-0617-03 | verification protocol (NOT a Rust test) | `cargo clippy --all-targets --all-features --release -- -D warnings` clean | none | release |

**Totals:** 0 UT, 0 IT, 0 PT, 3 VP. Net floor delta: **0 default; −≤12 release** (post-fix observed count vs current undefined-because-broken release count). The "VP" category here means **Verification Protocol** — instructions the developer + reviewer must execute by hand (or via CI), with the captured output documented in the implementation commit body. No new `#[test]` function is added by this task.

**Why no new Rust tests.** Constraint statement (operator prompt): "minimal test spec — verifies that `cargo test --release` runs to completion at HEAD with no compile errors and all tests pass; no NEW Rust tests required (the existing 1784 floor exercises the fix)." Adding a Rust test that "asserts release mode compiles" is impossible — by definition, if release mode doesn't compile, the test doesn't run. The verification is necessarily out-of-band (a CI lane or a manual `cargo test --release` invocation). This TEST-SPEC documents that out-of-band protocol so it is reproducible and reviewer-checkable.

---

## Per-protocol specifications

### VP-0617-01 — `cargo test --release` end-to-end smoke

**Purpose.** Verify that, post-fix, `cargo test --release` (a) compiles cleanly with zero errors and (b) runs to completion with all tests passing.

**Pre-fix state (HEAD `b079cdc`).** `cargo test --release` FAILS at compile time with:
1. Resolution errors for symbols inside `relativist-core/src/net/debug.rs:282-319` (the `mod tests` module references methods like `debug_assert_*` that vanish in release because the parent `impl Net` block is gated `#[cfg(debug_assertions)]`).
2. Non-exhaustive `match` error in `relativist-core/src/coordinator.rs:1871-1873` — the `match err.unwrap_err()` arm covers only `PullCoordinatorError::UnexpectedEvent { .. }` but the enum has a `WorkerIdMismatch { .. }` variant added by QA-D010-002 (commit `7fca43e`).

**Post-fix state.** Both errors are eliminated by:
1. Changing `#[cfg(test)]` to `#[cfg(all(test, debug_assertions))]` on the `mod tests` declaration at `net/debug.rs:282`.
2. Extending the `match` at `coordinator.rs:1871-1873` to include a `PullCoordinatorError::WorkerIdMismatch { .. } => panic!(...)` arm with a diagnostic message.

**Verification command (executed by developer, captured in commit body):**
```
cargo test --release --workspace 2>&1 | tee target/release-test-output.txt
```

**Acceptance.**
1. **Compile.** No `error[E0xxx]` lines in stderr.
2. **Test result.** Final line of stdout matches `test result: ok. N passed; 0 failed; M ignored; ...` for every test binary. `0 failed` is mandatory; `M` ignored is acceptable (some tests may be `#[ignore]`-gated for slow/release-incompatible cases).
3. **Numeric record (mandatory in commit body).** The developer MUST capture the exact `N passed` count from each test binary's output and report the workspace-level total in the commit message body. This becomes the new `cargo test --release` floor row in `docs/next-steps.md` and `codigo/relativist/CLAUDE.md` "Build & Test" section.

**Documented expected count (rough estimate, for reviewer sanity check):** `default debug` = 1784 minus the 12 `mod tests` functions in `net/debug.rs:282-319` that are now correctly gated out of release = **≥ 1772 release**. The exact post-fix count must be captured empirically; if it differs by more than ±20 from this estimate, the developer MUST investigate (could indicate other release-only `#[cfg]` gates in the codebase).

**Failure modes caught.**
- Either of the two known compile errors not fixed → step 1 fails.
- New release-only compile error introduced by future code → step 1 fails (good — TEST-SPEC-0617 catches it once integrated into CI lane).
- Test passes in debug but fails in release (e.g., a `debug_assert!` that masks a real bug) → step 2 fails.

---

### VP-0617-02 — Test-count parity vs default profile

**Purpose.** Defend against accidental loss of test coverage in release mode beyond the expected 12 `net/debug.rs` tests. If a developer accidentally adds a `#[cfg(debug_assertions)]` gate to an unrelated test module, the release count drops further and this VP catches it.

**Verification commands (paired):**
```
cargo test --workspace        2>&1 | tee target/debug-test-output.txt
cargo test --release --workspace 2>&1 | tee target/release-test-output.txt
```

**Acceptance.**
1. Both invocations report `0 failed`.
2. `count(debug) - count(release) ∈ [10, 16]` — i.e., the release count is exactly the 12 expected gated-out tests, with ±2 tolerance for any other intentional release-mode gates that may exist.
3. The developer documents the per-binary counts in a small table in the commit body, e.g.:
   ```
   | binary | debug | release | delta |
   |---|---|---|---|
   | relativist-core (lib)        | 1234 | 1222 | -12 |
   | relativist-core (integration) | ...  | ...  | 0   |
   | total                         | 1784 | 1772 | -12 |
   ```

**Why ±2 tolerance.** A future task may legitimately add a `#[cfg(all(test, debug_assertions))]` gate to a test that depends on debug-only invariants; one such test = ±1. Two is tight enough to catch broad over-gating, loose enough to absorb routine maintenance.

**Failure modes caught.**
- Over-gating: developer accidentally adds `debug_assertions` to a release-relevant test → delta exceeds 16.
- Under-gating: developer fixes only one of the two known errors and somehow another release-only test starts running where it didn't before → delta drops below 10.

---

### VP-0617-03 — `cargo clippy --all-targets --all-features --release -- -D warnings` clean

**Purpose.** TASK-0617 acceptance criterion 8: "release path lints cleanly too — pre-existing lint debt is in scope to surface, not to fix; if any release-only lint surfaces, file as a separate follow-up." The fix may resurface lints that only fire under `cfg(not(debug_assertions))` (e.g., a `dead_code` warning on a `#[cfg(debug_assertions)]`-only function whose only caller was in the now-correctly-gated `mod tests`).

**Verification command:**
```
cargo clippy --all-targets --all-features --release -- -D warnings 2>&1 | tee target/release-clippy-output.txt
```

**Acceptance.**
1. **Zero warnings under -D warnings.** If any new release-only warning fires, the developer MUST:
   - File a separate follow-up TASK-0619 (or next available number) documenting the warning + its file:line.
   - In TASK-0617's commit body, list the deferred warnings with their file:line and the new TASK number.
   - Apply targeted `#[allow(...)]` with a `// SPEC-comment-justification: deferred to TASK-0619` line if the warning is benign (e.g., dead code in test infrastructure). If the warning is non-trivial, escalate before landing TASK-0617.
2. **Note for reviewer.** The reviewer MUST run this command independently and confirm the output matches the commit body's claim. A clean stdout with `Finished` line is the only acceptance signal.

**Failure modes caught.**
- New release-only lint pre-existing the fix and surfaced by the fix → file as deferral.
- New release-only lint introduced BY the fix (e.g., the new `panic!` arm in coordinator.rs flagged by `clippy::panic_in_result_fn` if applicable) → must be fixed in TASK-0617 itself, not deferred.

---

## Notes

### Why this is a "verification protocol" not a Rust test

Adding `#[test] fn release_mode_compiles() {}` to a Rust file is a no-op: the test only runs if compilation succeeds, and compilation fails before any test runs. The only way to gate "does release mode compile" is at the build-orchestration layer — i.e., a CI lane that runs `cargo test --release` and fails the build on non-zero exit. This TEST-SPEC documents the protocol so it can be reviewed and (separately, OUT of D-012 scope) added to `.github/workflows/` as a release-test lane by the CICD agent.

### CI lane (recommended follow-up — OUT of D-012 scope)

The CICD agent should add a release-test lane to `.github/workflows/release-test.yml` post-TASK-0617:
```yaml
- name: cargo test --release
  run: cargo test --release --workspace --no-fail-fast
```
This is **explicitly out of scope** for TASK-0617 (per `docs/backlog/TASK-0617-d011-fu-release-tests-fix-cargo-test-release.md` "Files explicitly OUT of scope": "Adding a CI release lane — that's a separate CICD task (out of D-012 scope)"). Documented here so the next CICD pass picks it up.

### Coverage of constraints from the operator prompt

| Constraint | Where |
|---|---|
| Verifies `cargo test --release` runs to completion at HEAD with no compile errors and all tests pass | VP-0617-01 (load-bearing) |
| No NEW Rust tests required | Confirmed — 0 UT, 0 IT, 0 PT |
| Document the verification protocol (e.g., "CI lane added") | VP-0617-01/02/03 (the protocol itself); §"CI lane (recommended follow-up)" notes that the CI lane is OUT of D-012 scope and routes to a future CICD task |

### Cfg gates

The fix's first edit IS a `cfg` change: `#[cfg(test)]` → `#[cfg(all(test, debug_assertions))]` at `net/debug.rs:282`. The TEST-SPEC has no Rust-level cfg gates of its own.

---

## Cross-references

- **Source task:** `docs/backlog/TASK-0617-d011-fu-release-tests-fix-cargo-test-release.md`.
- **Bundle handoff:** `docs/handoffs/2026-05-05-D012-instrumentation-restore-handoff.md` §2 row 3, §3 D-011-FU-RELEASE-TESTS subsection (verbatim diff for both edits).
- **Origin log:** `docs/next-steps.md` 2026-05-05 entry, "`cargo test --release` is **broken pre-D-011** …".
- **MANIFEST citation:** `results/locked/v2_d011_final_baseline_2026-05-04/MANIFEST.md` "Test-floor status" section.
- **Edit targets (verbatim):**
  - `relativist-core/src/net/debug.rs:282` — change `#[cfg(test)]` to `#[cfg(all(test, debug_assertions))]`.
  - `relativist-core/src/coordinator.rs:1871-1873` — extend `match` to include `PullCoordinatorError::WorkerIdMismatch { .. } => panic!("unexpected WorkerIdMismatch from DispatchingFirst + RequestWork; expected UnexpectedEvent");`.

---

## Coverage matrix

| protocol_id | Compile (release) | Test count parity | Lint cleanliness |
|---|---|---|---|
| VP-0617-01 | ✅ (load-bearing) | partial (counts captured) | — |
| VP-0617-02 | — | ✅ (load-bearing, ±2 band) | — |
| VP-0617-03 | — | — | ✅ (load-bearing, deferral path documented) |

---

## Out-of-scope (explicitly NOT specified here)

- A CI release-test lane (`.github/workflows/release-test.yml`) — separate CICD task post-D-012.
- Network time / compute time / MIPS — TEST-SPECs 0615 / 0616 / 0618.
- Any change to the semantics of `WorkerIdMismatch` — that variant was added by QA-D010-002 and is not in scope here.
- Frozen baselines under `results/locked/`.
- Lint debt that is pre-existing and surfaces under release — DEFERRED to TASK-0619+ per VP-0617-03 acceptance.
