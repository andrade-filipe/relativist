# TEST-SPEC-0598 — Tests for TASK-0598 — Eliminate `debug_assertions`-gated counter ABI drift

**Task:** TASK-0598 (Phase B-3, P1)
**Spec:** SPEC-22 §3.x (Net/SparseNet ABI stability under feature-gated counters); SPEC-21 §3 (streaming counter discipline).
**Origin:** QA-D010-014 — counter fields gated behind `#[cfg(debug_assertions)]` cause structurally different Debug/Release builds (silent ABI drift).
**Strategy chosen:** **(b) always-present counter fields with debug-only writes** (user-decided default per dispatch brief).
**Test floor delta:** **+4 default** (no zero-copy gating).
**Prerequisites:** None.

---

## Test inventory

| test_id | level | target file | prerequisite TASK | cfg gates |
|---|---|---|---|---|
| UT-0598-01 | unit | `relativist-core/src/partition/streaming.rs::tests::counter_fields_present_on_debug_build` | none | `#[cfg(debug_assertions)]` |
| UT-0598-02 | unit | `relativist-core/src/partition/streaming.rs::tests::counter_fields_present_on_release_build` | none | `#[cfg(not(debug_assertions))]` |
| UT-0598-03 | unit | `relativist-core/src/partition/streaming.rs::tests::counter_writes_active_in_debug` | none | `#[cfg(debug_assertions)]` |
| UT-0598-04 | unit | `relativist-core/src/partition/streaming.rs::tests::counter_writes_inert_in_release` | none | `#[cfg(not(debug_assertions))]` |
| IT-0598-05 | integration | `relativist-core/tests/spec22_debug_release_abi_parity.rs::struct_field_count_identical_across_profiles` | none | none |

Total: **4 default tests + 1 integration** (note: UT-0598-01..04 are mutually exclusive by `cfg` so only 2 of 4 execute on a given build profile, but ALL 4 source-compile and the floor count is +4 because cargo test counts compiled tests on the runner. IT-0598-05 is unconditional, +1 unconditional).

Effective floor delta: **+4 default** (3 cfg-gated unit tests + 1 unconditional integration test, accounting for the 2-of-4 mutual exclusion → 2 active unit + 1 IT = +3 active per run, but +4 lines in the test source — this spec uses the conservative "active per profile" count).

---

## Per-test specifications

### UT-0598-01 — `counter_fields_present_on_debug_build`

**Purpose.** Verify that on a debug build, every counter field on `PendingStore`, `BorderClean`, and any other structure refactored by this task is **present** in the struct definition (compile-time check).
**Setup.** None — this is a structural test; instantiate the struct via `Default::default()` or its constructor.
**Action.** Construct the value; access each counter field by name in a way the compiler must accept (e.g. `let _ = store.pop_attempts;`).
**Assertions.**
- The struct compiles with all counter fields accessible (compilation IS the assertion).
- `store.pop_attempts == 0` immediately after construction (counter starts at zero).
- All counter fields enumerated in the production code are accessed at least once in the test body — if a field is removed, the test fails to compile (regression fence).
**Boundary case coverage.** Catches a future regression that re-introduces `#[cfg(debug_assertions)]` on the field declaration.
**cfg gate.** `#[cfg(debug_assertions)]`.
**Why it must exist.** Acceptance criterion #1 of TASK-0598 (no public-facing struct has a field whose presence depends on `cfg(debug_assertions)`).

---

### UT-0598-02 — `counter_fields_present_on_release_build`

**Purpose.** The mirror of UT-0598-01 on a release build. Field presence MUST be identical to debug.
**Setup.** None.
**Action.** Construct the value; access each counter field by name.
**Assertions.**
- Compilation succeeds (the field exists on release).
- `store.pop_attempts == 0` at construction time.
- The set of fields accessed in this test body is **byte-for-byte identical** to the set accessed in UT-0598-01 (enforce by literally copying the same access list — diff between the two test bodies should be only the `cfg` line).
**Boundary case coverage.** Catches a half-fix that removes `cfg(debug_assertions)` from one struct but not another.
**cfg gate.** `#[cfg(not(debug_assertions))]`.
**Why it must exist.** Acceptance criterion #1 — release-side companion of UT-0598-01. This pair forms the headline regression for ABI drift.

---

### UT-0598-03 — `counter_writes_active_in_debug`

**Purpose.** On debug, the counter writes (still gated by `#[cfg(debug_assertions)]` at the *use site*, per strategy (b)) actually fire — i.e. the counter increments when the gated code path runs.
**Setup.** Construct a `PendingStore` (or equivalent), push and pop one item.
**Action.** Trigger the code path that increments `pop_attempts` (or the equivalent counter); invoke the operation N=3 times.
**Assertions.**
- `store.pop_attempts == 3` (or whichever counter; expected value follows the spec's counter semantics).
- All other counters reflect the operations performed in their semantic order (e.g. `push_count == 3`, `pop_success == 3`).
**Boundary case coverage.** Catches a buggy refactor that moves the field out of `cfg(debug_assertions)` but **also** removes the increment (counter would always read 0 even on debug).
**cfg gate.** `#[cfg(debug_assertions)]`.
**Why it must exist.** Acceptance criterion #2 (debug-only writes MAY remain `cfg(debug_assertions)`-gated at the use site, but the field itself is always present) — this test validates the "debug-only writes are still active on debug" half.

---

### UT-0598-04 — `counter_writes_inert_in_release`

**Purpose.** On release, the counter writes (gated at the use site) do NOT fire — the field stays at its initial value (zero) regardless of how many times the gated code path runs.
**Setup.** Construct a `PendingStore`, push and pop items.
**Action.** Same as UT-0598-03 — N=3 push/pop pairs.
**Assertions.**
- `store.pop_attempts == 0` AFTER the 3 operations (the counter was never written on release).
- `store.push_count == 0` after 3 pushes (same reasoning).
- The struct's externally observable behavior (the actual queue contents, sizes, etc.) is identical to debug — only the diagnostic counter values differ.
**Boundary case coverage.** Catches a buggy refactor that forgets to re-gate the *write* sites once the field is unconditional, leaving release builds paying the cost of unused counter writes (a soft perf regression that the test makes hard).
**cfg gate.** `#[cfg(not(debug_assertions))]`.
**Why it must exist.** Acceptance criterion #2 (release builds: writes ARE gated; field is present but its value is the default). This is the key behavioral contract of strategy (b).

---

### IT-0598-05 — `struct_field_count_identical_across_profiles`

**Purpose.** Headline ABI parity test: serialize a `PendingStore` (or equivalent counter-bearing struct) via bincode in the current build profile, snapshot the byte length, and assert the snapshot is independent of the build profile by comparing against a reference snapshot file checked into the repo.
**Setup.**
- A reference snapshot file at `relativist-core/tests/fixtures/spec22_pending_store_release_v1.bin` (or equivalent) generated by a release build at task-implementation time and committed.
- Construct a `PendingStore` with the *same* deterministic state in this test (e.g. push 3 items with fixed payloads, pop 1).
**Action.** `let bytes = bincode::serialize(&store)?; let ref_bytes = std::fs::read(FIXTURE_PATH)?;`
**Assertions.**
- `bytes.len() == ref_bytes.len()` — same byte length, on both debug AND release.
- `bytes == ref_bytes` — byte-for-byte equality (because the counter values are zero-initialized AND counter writes don't fire OR fire identically with deterministic input — the test uses an input that triggers no counter writes if the strategy is "writes still gated by `cfg(debug_assertions)`"; if a future strategy unifies writes too, the test must be updated).
- A doc-comment in the test cites SPEC-22 §3.x (ABI stability) and the strategy (b) decision.
**Boundary case coverage.** Catches the silent debug/release size mismatch that is the headline failure mode of QA-D010-014. Without this fixture-based equality, a divergence could ship undetected.
**cfg gate.** None — runs on every profile and platform.
**Why it must exist.** Acceptance criterion #3 (new ABI parity test validates the field set is identical across debug/release simulated boundaries) — the only test that actually *compares* a debug-emitted byte stream against a release-emitted one.

**Implementation note for the developer.** If maintaining a checked-in binary fixture is undesirable, an alternative is to assert that `std::mem::size_of::<PendingStore>()` is non-zero AND constant across `cfg!(debug_assertions)` branches by computing it twice via a `const fn` and a runtime call — but bincode-byte equality is the stronger contract and matches SPEC-22 R9a wire stability semantics. **Stage 3 (developer) MUST pick one and document.**

---

## Coverage matrix

| test_id | AC-1 (no cfg-gated fields) | AC-2 (writes gated, fields not) | AC-3 (parity test) | AC-4 (no regression) | AC-5 (no warnings) |
|---|---|---|---|---|---|
| UT-0598-01 | ✅ | | | ✅ | |
| UT-0598-02 | ✅ | | | ✅ | |
| UT-0598-03 | | ✅ | | ✅ | |
| UT-0598-04 | | ✅ | | ✅ | |
| IT-0598-05 | ✅ | | ✅ | | |

Every acceptance criterion 1-3 has ≥1 test. AC-5 is a build-system gate (lint), not a unit test — verified by `cargo build` and `cargo build --release` both completing without warnings.

---

## Out-of-scope tests (deferred to other tasks)

- Tests on counter *semantics* (the values written, beyond zero/non-zero) → orthogonal; counter semantics are not changed by this task.
- Tests for the `streaming-no-recycle` feature gate → TASK-0599.
- Cross-version ABI tests (v3 ↔ v4) → TASK-0596 already covers this.

---

## Known spec ambiguity (adversarial flag)

- SPEC-22 §3.x does not enumerate which counter fields exist on `PendingStore` / `BorderClean`. The test must reference the production struct field list at implementation time. If the developer adds a NEW counter field not in the current production code, both UT-0598-01 and UT-0598-02 must enumerate it identically — flag this as a test-source-coupling fence (the two tests must be kept in sync diff-wise).
- The "always-present field with debug-only writes" strategy (b) is documented as the user's default but is NOT mandated by the spec text. If the user later opts for strategy (a) (dedicated cargo feature), UT-0598-03 / UT-0598-04 must be regenerated with the new feature gate, not `cfg(debug_assertions)`.
