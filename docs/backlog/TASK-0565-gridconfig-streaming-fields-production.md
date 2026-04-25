# TASK-0565: `GridConfig` streaming fields production (chunk_size, streaming_strategy, dispatch_mode)

**Spec:** SPEC-21 §3.3 R24 (chunk_size configurable + default placeholder), §3.4 R25 (streaming_strategy selector), §3.6 R34 (dispatch_mode); SPEC-21 §3.8 A3 (consumer of TASK-0512).
**Requirements:** R24, R25, R34 (production wiring of the three new GridConfig fields plus optional R37g `max_pending_lifetime`).
**Priority:** P0 (blocker for TASK-0568 CLI surface, TASK-0554 orchestrator wiring, TASK-0577/0578 FSM gating).
**Status:** TODO
**Depends on:** TASK-0512 (SPEC-07 amendment A3 landed in spec text), TASK-0524 (`StreamingPartitionStrategy` trait), TASK-0530 (RoundRobinStreamingStrategy default), TASK-0531 (FennelStreamingStrategy opt).
**Blocked by:** none
**Estimated complexity:** S (~80 LoC field additions + serde defaults + enum derives + 1 strategy-selector helper; ~60 LoC tests).
**Bundle:** SPEC-21 Streaming Generation — Phase F (regression / polish / late-binding).

## Context

Per SPEC-21 §3.8 A3 (closes SC-001 part 2), `GridConfig` (in `relativist-core/src/config.rs`) MUST gain three additive fields:

```rust
pub chunk_size: u32,                          // R24, default 10_000 (placeholder, see Q2 / SC-024)
pub streaming_strategy: StreamingStrategyConfig, // R25, default RoundRobin
pub dispatch_mode: DispatchMode,              // R34, default Auto
```

A fourth field `pub max_pending_lifetime: u32` (R37g, default 16) MAY ship in the same patch (closes SC-016).

`StreamingStrategyConfig` and `DispatchMode` enums are declared in SPEC-21 §4.x; this task wires them onto `GridConfig` with proper `serde(default)` annotations and re-exports them from `src/config.rs` for downstream consumers (CLI in TASK-0568; orchestrator TASK-0554; FSMs TASK-0577/0578).

R10b/R10c interaction: this task does NOT touch `recycle_under_delta` (owned by SPEC-22 TASK-0482). Field-ordering coordination with SPEC-20 `GridConfig` elastic fields (TASK-0415) MUST avoid serde tag collisions — the patch is purely additive at the end of the struct.

## Acceptance Criteria

- [ ] `GridConfig` gains `chunk_size: u32`, `streaming_strategy: StreamingStrategyConfig`, `dispatch_mode: DispatchMode` (and optionally `max_pending_lifetime: u32`) per SPEC-21 §3.8 A3 verbatim defaults.
- [ ] `StreamingStrategyConfig` (variants `RoundRobin`, `Fennel { alpha: f32 }`) and `DispatchMode` (variants `Auto`, `Push`, `Pull`) declared in `src/config.rs` with `Debug, Clone, PartialEq, Eq, Serialize, Deserialize` derives; `serde(default)` set on every new field so old config files load unchanged.
- [ ] `chunk_size = u32::MAX` sentinel triggers R26 short-circuit path (verified jointly with TASK-0567).
- [ ] `streaming_strategy.build()` factory helper returns `Box<dyn StreamingPartitionStrategy>` with the configured variant; default-construction returns `RoundRobinStreamingStrategy::new(num_workers)`.
- [ ] All 1181 default / 1224 zero-copy baseline tests pass unchanged (purely additive — no semantic break for non-streaming paths).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/config.rs` | modify | Add three (or four) fields to `GridConfig`; declare `StreamingStrategyConfig` and `DispatchMode` enums; expose `streaming_strategy.build(num_workers)` factory. |

## Key Types / Signatures

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamingStrategyConfig {
    RoundRobin,
    Fennel { alpha: f32 }, // R5; gated on TASK-0531
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DispatchMode { Auto, Push, Pull }

impl Default for StreamingStrategyConfig { fn default() -> Self { Self::RoundRobin } }
impl Default for DispatchMode            { fn default() -> Self { Self::Auto } }
```

## Test Expectations (forward-ref)

Reuse coverage from TEST-SPEC-0512 (amendment-level). Production-level coverage:
- Round-trip serialize/deserialize a `GridConfig` JSON missing the new fields → defaults applied (additive-compat regression).
- `streaming_strategy.build(4)` returns the correct concrete strategy (RoundRobin vs Fennel).
- `chunk_size == u32::MAX` triggers short-circuit (joint with TASK-0567).

## Invariants Touched

- None (configuration surface only). Default values preserve every pre-SPEC-21 semantic.

## Notes

- Field ordering: append after SPEC-20 elastic fields per coordination note in TASK-0512 line 60. Bincode is ordering-sensitive, but `serde(default)` covers JSON / TOML.
- The placeholder default `chunk_size = 10_000` is gated on SC-024 (post-implementation benchmark calibration). Doc comment MUST cite Q2 / SC-024.
- Consumed by TASK-0567 (R26 short-circuit), TASK-0568 (CLI), TASK-0554 (orchestrator wiring path), TASK-0577 / TASK-0578 (FSM gating on `dispatch_mode`).

## DAG Links

- **Predecessors:** TASK-0512, TASK-0524, TASK-0530, TASK-0531.
- **Successors:** TASK-0567, TASK-0568, TASK-0554 (consumer), TASK-0577, TASK-0578.
