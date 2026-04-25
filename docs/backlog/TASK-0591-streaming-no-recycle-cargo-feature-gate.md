# TASK-0591: `streaming-no-recycle` cargo feature gate (alternative one-liner closure of R37b)

**Spec:** SPEC-21 §3.7 R37b (G1 free-list interaction; closes SC-007 — alternative closure path); §3.8 A6 (consumer of TASK-0515).
**Requirements:** R37b alternative closure — an implementation MAY use a cargo feature gate `streaming-no-recycle` that disables the worker free-list outright during streaming; in that case, the contract degenerates to "no recycling occurs during streaming, full stop." This is a valid one-liner closure of the G1 threat per §3.7 R37b line 259.
**Priority:** P2 (alternative path; ships orthogonally to TASK-0589 / TASK-0590 as a SAFETY-NET feature flag).
**Status:** TODO
**Depends on:** TASK-0515 (SPEC-22 R10b broadening amendment landed in spec text — establishes the "alternative" disposition).
**Blocked by:** TASK-0589 / TASK-0590 SHOULD land first — the feature-gate is an EXTRA safety net, not a replacement; the runtime gates MUST already be correct for non-feature-flag builds.
**Estimated complexity:** S (~50 LoC: cargo feature declaration + cfg-gated empty stub for the free-list pop site + cross-feature-flag CI matrix).
**Bundle:** SPEC-21 Streaming Generation — Phase F (regression / polish / late-binding).

## Context

Per SPEC-21 R37b verbatim (line 259):

> Equivalently, an implementation MAY explicitly **disable free-list recycling** for the entire generation+accumulation phase via a `feature = "streaming-no-recycle"` cargo feature gate; in that case, the contract degenerates to "no recycling occurs during streaming, full stop." This is a valid one-liner closure of the G1 threat.

This task ships the cargo feature gate as a build-time alternative for users who want maximum safety with zero runtime overhead. When `--features streaming-no-recycle` is enabled:

1. The free-list pop site (TASK-0472) is wrapped in `#[cfg(not(feature = "streaming-no-recycle"))]` for the streaming-active code path; under the feature, pop returns `None` unconditionally during streaming regardless of `recycle_under_delta` policy.
2. Documentation and CHANGELOG note the trade-off: zero recycling under streaming → full v1-style memory growth → suitable for small / short-lived runs.
3. CI matrix gains a `streaming-no-recycle` feature column to verify the build compiles and the existing 1181 default tests pass when the feature is enabled (pure additive — no semantic break for non-streaming paths).

The feature gate is **orthogonal** to `RecyclePolicy::DisableUnderDelta` (Strategy A) and `RecyclePolicy::BorderClean` (Strategy B): when the feature is enabled, both runtime strategies behave identically to "no recycling under streaming" (the runtime gate becomes redundant but MUST remain present and CORRECT for builds that DO NOT enable the feature).

## Acceptance Criteria

- [ ] `relativist-core/Cargo.toml` declares `[features] streaming-no-recycle = []` (empty deps; pure code-path gate).
- [ ] Free-list pop site (extending TASK-0589's gate) is wrapped: when feature is enabled AND streaming is active, pop returns `None` unconditionally; when feature is disabled, the runtime gate from TASK-0589/0590 applies.
- [ ] Cargo-test matrix gains a `streaming-no-recycle` job: `cargo test --features streaming-no-recycle` MUST pass (1181 tests minimum; the streaming-active subset MAY have different pop-counter values per the gate, but functional outputs MUST be isomorphic to the non-feature build).
- [ ] CHANGELOG / SPEC-21 doc note: feature is opt-in, ships disabled by default, cited in the v1-backward-compat regression task as a documented escape hatch.
- [ ] Cross-feature isomorphism test: same input run with `streaming-no-recycle` enabled vs disabled produces isomorphic merged results (G1 / ARG-005 preserved by both paths).
- [ ] Feature-gate compile check: a CI lint verifies `#[cfg(feature = "streaming-no-recycle")]` annotations are syntactically present at the documented pop sites (defense against accidental feature-flag drift).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/Cargo.toml` | modify | Add `streaming-no-recycle` feature declaration. |
| `relativist-core/src/net/free_list.rs` | modify | Add `#[cfg(feature = "streaming-no-recycle")]` short-circuit at the pop site. |
| `.github/workflows/ci.yml` (or equivalent) | modify | Add `cargo test --features streaming-no-recycle` matrix column. |
| `relativist-core/tests/spec21_streaming_no_recycle_feature.rs` | create | Cross-feature isomorphism + zero-pop-during-streaming verification. |

## Key Types / Signatures

```rust
// In Net::create_agent, extending TASK-0589/0590:
#[cfg(feature = "streaming-no-recycle")]
{
    if worker_state.streaming_active {
        // unconditionally fall through to next_id increment
        return self.allocate_fresh();
    }
}
// ... TASK-0589/0590 runtime gate logic unchanged ...
```

## Test Expectations (forward-ref)

Reuse coverage from TEST-SPEC-0515 (which exercises BOTH gate states per INDEX line 279). Production-level:
- UT-0591-01: build succeeds with `--features streaming-no-recycle`.
- UT-0591-02: 1181 baseline tests pass under the feature (additive-compat).
- UT-0591-03: streaming run under the feature → zero free-list pops during streaming phase (verified via debug counter).
- UT-0591-04: cross-feature isomorphism — same input with/without feature → merged results isomorphic via `nets_isomorphic`.

## Invariants Touched

- G1 (BSP determinism under streaming) — preserved trivially under the feature (no recycling = no slot-id reuse = no border violation possible).
- ARG-005 delta-recoverability — preserved trivially.

## Notes

- This is the "one-liner closure" path for users who want compile-time guarantees over runtime gating. It trades memory efficiency for safety simplicity.
- The feature MUST NOT be enabled in the default CI matrix; it ships as an optional column.
- Documentation: the `streaming-no-recycle` feature is documented in `relativist-core/src/lib.rs` doc-comment and in the SPEC-21 closure note for R37b.
- Consumed by NO downstream task (terminal alternative path); cited in v1-backward-compat regression context as a documented escape hatch.

## DAG Links

- **Predecessors:** TASK-0515, TASK-0589, TASK-0590.
- **Successors:** none (terminal alternative-path leaf).
