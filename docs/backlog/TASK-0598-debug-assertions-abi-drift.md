# TASK-0598 — Eliminate `debug_assertions`-gated counter ABI drift (QA-D010-014)

**Phase:** B-3 (D-011 hardening)
**Bundle:** D-011 — Tier 3 Hardening + Bench Enablement
**Status:** PENDING
**Priority:** P1 (MEDIUM — silent debug/release mismatch on instrumented structs)
**Spec:** SPEC-22 §3.x (Net/SparseNet ABI stability under feature-gated counters); SPEC-21 §3.x (streaming counter discipline).
**Origin:** QA-D010-014 — counter fields gated behind `#[cfg(debug_assertions)]` cause structurally different Debug/Release builds.
**Estimated complexity:** S (~30 LoC production + ~40 LoC tests)
**Estimated stages duration:** Stages 2→3→4→5→6 over ~0.5 day.

---

## Context

Several diagnostic counter fields in streaming and arena structs are gated by `#[cfg(debug_assertions)]`, which means the struct layout / field count differs between debug and release builds. This is a silent ABI drift: serializing a struct in a debug-built coordinator and deserializing in a release-built worker (or vice-versa) corrupts data — relevant if heterogeneous binaries are ever co-deployed, and harmful for invariants comparing observed metrics across build profiles.

The fix is one of:
- **(a)** Move counter fields out from `debug_assertions` gating and into a dedicated cargo feature (`streaming-debug-counters`) so the build profile no longer changes ABI.
- **(b)** Replace counter-field gating with conditional *use* (always-present field; debug-only writes / verifies).

Option (b) is simpler and sufficient for SPEC-22 R9a serde stability — preferred unless an explicit request to add a new feature flag emerges.

## Dependencies

- None on D-011 spec amendments.
- Independent of TASK-0596/0597 — can run in parallel.

## Files in scope

| File | Change |
|------|--------|
| `relativist-core/src/partition/streaming.rs` | Audit all `#[cfg(debug_assertions)]`-gated fields on `PendingStore` / `BorderClean` / counter structs; move gating from field definition to write-site. |
| `relativist-core/src/net/free_list.rs` (if applicable) | Same audit. |
| `relativist-core/src/net/sparse.rs` (if applicable) | Same audit. |
| `relativist-core/tests/spec22_debug_release_abi_parity.rs` (new) | Regression: serialize struct in debug-built test, snapshot field count; verify serde stability is identical to a release-built reference snapshot. |

## Files explicitly OUT of scope

- The `streaming-no-recycle` feature gate (TASK-0591) — orthogonal.
- Any change to the counter *semantics* (the values written) — only their gating.

## Acceptance criteria

1. No public-facing struct in `partition/streaming.rs`, `net/free_list.rs`, `net/sparse.rs` has a field whose presence depends on `#[cfg(debug_assertions)]`.
2. Debug-only writes (assertions / counter increments) MAY remain `#[cfg(debug_assertions)]`-gated *at the use site*, but the field itself is always present.
3. New ABI parity test validates the field set is identical across debug/release simulated boundaries.
4. All existing tests pass with zero regression on both debug AND release profiles.
5. `cargo build --release --workspace` and `cargo build --workspace` both succeed without conflicting `unused_variable` warnings (gate counter writes appropriately).

## Test floor delta expected

**+2 to +4 tests** added.

## Notes

- This is a "fix the foundations" task — no functional behaviour change for users; benchmark output unchanged.
- Light Stage 5 QA — adversarial focus is "find any remaining `cfg(debug_assertions)` field-gates".
