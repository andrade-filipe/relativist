# TASK-0597 — Thread `GridConfig.max_pending_lifetime` through legacy callers (QA-D010-009 residual)

**Phase:** B-2 (D-011 hardening)
**Bundle:** D-011 — Tier 3 Hardening + Bench Enablement
**Status:** PENDING
**Priority:** P1 (HIGH — biases memory measurement; required clean before Phase F-2 bench rodada)
**Spec:** SPEC-21 §3.7 R37g (`MAX_PENDING_LIFETIME` pending-store memory bound, closes SC-016).
**Origin:** QA-D010-009 residual (Stage 6 wired the new `_with_lifetime` wrapper end-to-end in `5a54111`; legacy callers still pass `u32::MAX`).
**Estimated complexity:** S (~40 LoC production + ~30 LoC tests)
**Estimated stages duration:** Stages 2→3→4→5→6 over ~0.5 day.

---

## Context

`5a54111` introduced `merge::generate_and_partition_chunked_with_chunk_size_and_lifetime` (the lifetime-aware orchestrator) but the legacy entrypoint `generate_and_partition_chunked_with_delta` (still consumed by call sites) hard-codes `max_pending_lifetime = u32::MAX`. That hard-code defeats the SC-016 memory bound: pending-references accumulate without bound during streaming generation, biasing M5 (peak coordinator memory) measurement and contradicting SPEC-21 R37g.

The fix is to thread the configured `GridConfig.max_pending_lifetime` from outer callers down through `generate_and_partition_chunked_with_delta` (and any other surviving wrapper), eliminating the `u32::MAX` constant.

## Dependencies

- TASK-0596 (Phase B-1) — independent, but recommended landing order is B-1 then B-2 (B-1 is CRITICAL).
- No spec commit dependency (SPEC-21 R37g already in tree).

## Files in scope

| File | Change |
|------|--------|
| `relativist-core/src/merge/helpers.rs` | Replace `u32::MAX` constants with the propagated `max_pending_lifetime` argument; signature change on legacy wrappers if needed. |
| `relativist-core/src/merge/mod.rs` (if re-exports surface the wrappers) | Match new signatures. |
| Call sites — to be enumerated by a `grep` for `generate_and_partition_chunked_with_delta(` and `u32::MAX` near pending-lifetime contexts | Pass `config.max_pending_lifetime` through. |
| `relativist-core/tests/spec21_pending_lifetime_legacy_caller.rs` (new) | Regression: legacy entrypoint with non-MAX lifetime correctly bounds pending-store size. |

## Files explicitly OUT of scope

- `relativist-core/src/partition/streaming.rs` — already correct as of `5a54111`.
- `relativist-core/src/bench/mod.rs` — bench wiring is Phase C-1.
- The `_with_chunk_size_and_lifetime` wrapper itself — already correct.

## Acceptance criteria

1. No surviving `u32::MAX` literal for `max_pending_lifetime` in any legacy caller of streaming generation.
2. `GridConfig.max_pending_lifetime` is the single source of truth, propagated down to the pending-store eviction site.
3. Regression test demonstrates that with `max_pending_lifetime = N`, pending-store size never exceeds N during streaming (SC-016 / R37g).
4. Existing tests (≥1683 default / ≥1726 zero-copy) pass with zero regression.
5. Lint clean (`cargo clippy --workspace --all-targets --all-features -- -D warnings`).

## Test floor delta expected

**+2 to +4 tests** added. New floor target after this task: **≥ previous + 2**.

## Notes

- Coordinate signature changes carefully: if `generate_and_partition_chunked_with_delta` is called from multiple test files, prefer adding a *new* parameter with default in a wrapper rather than breaking every test signature. But: zero stale `u32::MAX` constants must remain after this task closes.
- Verify by grepping: `rg "u32::MAX" relativist-core/src/merge/ relativist-core/src/partition/streaming.rs` should return zero matches in `max_pending_lifetime` contexts after fix.
