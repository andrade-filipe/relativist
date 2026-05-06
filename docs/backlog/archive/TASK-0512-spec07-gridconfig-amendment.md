# TASK-0512: [SPEC-07 amendment A3] `GridConfig` gains `chunk_size`, `streaming_strategy`, `dispatch_mode` (+ optional `max_pending_lifetime`)

**Spec:** SPEC-21 §3.8 A3 (closes SC-001 part 2); SPEC-21 §3.4 R24, R25; SPEC-21 §3.6 R34; SPEC-21 §3.7 R37g.
**Requirements:** A3 (formal SPEC-07 / SPEC-05 GridConfig amendment).
**Priority:** P0 (blocker for TASK-0565 GridConfig field production and TASK-0568 CLI surface).
**Status:** TODO
**Depends on:** none (Phase A).
**Blocked by:** none
**Estimated complexity:** S (~30 LoC spec text — combination of SPEC-07 R11 / SPEC-05 §4.1 GridConfig surface).
**Bundle:** SPEC-21 Streaming Generation — predecessor-spec amendment cluster (Phase A).
**Tag:** `[SPEC-07 amendment]`

## Context

GridConfig currently lives in SPEC-05 §4.1 (the canonical struct definition); SPEC-07 R11 maps CLI arguments to this struct. SPEC-21 R24 / R25 / R34 require three new configuration fields plus an optional fourth for R37g:

```rust
pub chunk_size: u32,                              // R24, default 10_000 (placeholder, see Q2 / SC-024)
pub streaming_strategy: StreamingStrategyConfig,  // R25, default RoundRobin
pub dispatch_mode: DispatchMode,                  // R34, default Auto
pub max_pending_lifetime: u32,                    // R37g, default 16 (optional in same patch)
```

with `StreamingStrategyConfig` and `DispatchMode` enums declared in SPEC-21 §4.x and re-exported from `src/config.rs` (per CLAUDE.md "module Structure" — config.rs surface).

The amendment is **additive**: existing fields and SPEC-05 R-N requirements are unchanged.

## Acceptance Criteria

- [ ] ESPECIALISTA EM SPECS lands the SPEC-07 next-revision diff amending R11 (CLI → GridConfig mapping) to enumerate the four new fields.
- [ ] ESPECIALISTA EM SPECS lands the SPEC-05 next-revision diff amending §4.1 (GridConfig struct definition) with the four new fields plus the supporting enums (`StreamingStrategyConfig`, `DispatchMode`).
- [ ] R24 default (`chunk_size = 10_000`) annotated as benchmark-TBD per SC-024; the comment "MUST be re-evaluated and either confirmed or replaced before v2 release" propagates into the spec text.
- [ ] R25 enumerates the two normative strategy options (`round_robin` default, `fennel` if implemented).
- [ ] R34 enumerates the three `DispatchMode` variants (`Push`, `Pull`, `Auto`) with `Auto` default.
- [ ] R37g `max_pending_lifetime` optional fourth field marked as default-16 with the same benchmark-calibration disposition.
- [ ] Cross-references SPEC-21 R24, R25, R34, R37g; preserves additive-change posture (no break to SPEC-05 R-N MUSTs).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `codigo/relativist/specs/SPEC-07-deployment.md` | modify (by ESPECIALISTA EM SPECS only) | R11 amended — adds four fields to the CLI-to-GridConfig mapping. |
| `codigo/relativist/specs/SPEC-05-merge.md` | modify (by ESPECIALISTA EM SPECS only) | §4.1 GridConfig struct amended — adds the four fields plus supporting enum declarations. |

## Test Expectations (forward-ref)

TEST-SPEC-0512 — covered by:
- TEST-SPEC-0565 GridConfig field production round-trip.
- TEST-SPEC-0568 CLI flags surface.
- TEST-SPEC-0567 short-circuit when `chunk_size = u32::MAX` (T6 isomorphism).

## Invariants Touched

- None directly (configuration surface only).

## Notes

- This is a spec-text-only task (no production code).
- The `StreamingStrategyConfig` and `DispatchMode` enums are SPEC-21-defined; both are re-exported from `src/config.rs` so the SPEC-05 / SPEC-07 amendment stays additive.
- Coordinate with TASK-0415 (SPEC-20 GridConfig elastic fields) on field ordering and serde-default attributes — landing order matters; this task documents the SPEC-21 surface, TASK-0565 implements it, and TASK-0500 (SPEC-22) / TASK-0455 (SPEC-20) regression gates verify zero break.
- Consumed by TASK-0565 (production), TASK-0568 (CLI), TASK-0566 (max_pending_lifetime field), TASK-0567 (chunk_size short-circuit).

## DAG Links

- **Predecessors:** none.
- **Successors:** TASK-0565, TASK-0566, TASK-0567, TASK-0568.
