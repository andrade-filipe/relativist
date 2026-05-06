# TASK-0568: CLI streaming flags (`--chunk-size`, `--streaming-strategy`, `--dispatch-mode`)

**Spec:** SPEC-21 §3.3 R24, §3.4 R25, §3.6 R34 (CLI surface for the three GridConfig streaming fields); SPEC-21 §3.8 A3 (consumer of TASK-0512 / TASK-0565).
**Requirements:** R24 / R25 / R34 CLI parity (every GridConfig field with a typed value MUST be settable from the CLI per SPEC-07 R-N convention).
**Priority:** P1 (operability — clap surface; not on the critical reduction path but required for benchmark scripts).
**Status:** TODO
**Depends on:** TASK-0512 (SPEC-07 amendment landed), TASK-0565 (GridConfig fields exist in `src/config.rs`).
**Blocked by:** none
**Estimated complexity:** S (~70 LoC clap derive additions + arg-parser tests).
**Bundle:** SPEC-21 Streaming Generation — Phase F (regression / polish / late-binding).

## Context

SPEC-07 convention (per R-N) is that every `GridConfig` field with a typed value is settable from the CLI via `clap` derives. SPEC-21 §3.8 A3 added three (optionally four) new fields:

- `chunk_size: u32` (R24, default 10_000)
- `streaming_strategy: StreamingStrategyConfig` (R25, default RoundRobin)
- `dispatch_mode: DispatchMode` (R34, default Auto)
- `max_pending_lifetime: u32` (R37g, default 16; gated on TASK-0565 also shipping the optional fourth field)

Per the SPEC-07 convention, this task adds the matching CLI flags:

```
--chunk-size <N>                 (default 10000)
--streaming-strategy <NAME>      (round-robin | fennel; default round-robin)
--fennel-alpha <FLOAT>           (only valid when streaming-strategy=fennel)
--dispatch-mode <MODE>           (auto | push | pull; default auto)
--max-pending-lifetime <N>       (default 16; gated on TASK-0565 fourth field)
```

The CLI parser MUST reject `--fennel-alpha` when `--streaming-strategy != fennel` (clap `requires`/`conflicts_with`).

## Acceptance Criteria

- [ ] `clap` derive struct in `relativist-cli/src/main.rs` (or wherever the CLI lives) gains the four flags above with `#[arg]` annotations and matching default values from SPEC-21 §3.8 A3.
- [ ] CLI argument parsing maps the flags onto `GridConfig.chunk_size`, `streaming_strategy`, `dispatch_mode`, and `max_pending_lifetime`.
- [ ] `--streaming-strategy fennel --fennel-alpha 1.5` constructs `StreamingStrategyConfig::Fennel { alpha: 1.5 }`.
- [ ] `--streaming-strategy round-robin --fennel-alpha 1.5` rejects with a clap conflict error.
- [ ] `--help` output documents all four flags with their defaults and the SC-024 caveat for `chunk_size`.
- [ ] All existing CLI integration tests pass unchanged.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-cli/src/main.rs` | modify | Add four `clap` derive args with conflict rules and defaults. |

## Key Types / Signatures

```rust
#[derive(clap::Args)]
struct StreamingArgs {
    #[arg(long, default_value_t = 10_000)]
    chunk_size: u32,
    #[arg(long, value_enum, default_value_t = StreamingStrategyArg::RoundRobin)]
    streaming_strategy: StreamingStrategyArg,
    #[arg(long, requires = "streaming_strategy_fennel")]
    fennel_alpha: Option<f32>,
    #[arg(long, value_enum, default_value_t = DispatchModeArg::Auto)]
    dispatch_mode: DispatchModeArg,
    #[arg(long, default_value_t = 16)]
    max_pending_lifetime: u32, // gated on TASK-0565 fourth field
}
```

## Test Expectations (forward-ref)

Reuse coverage from TEST-SPEC-0512 (CLI surface explicitly listed there). Add:
- Unit test: `--streaming-strategy fennel --fennel-alpha 1.5` parses to `StreamingStrategyConfig::Fennel { alpha: 1.5 }`.
- Unit test: `--streaming-strategy round-robin --fennel-alpha 1.0` exits with clap parse error.
- Unit test: `--chunk-size 4294967295` (u32::MAX) maps cleanly and triggers R26 short-circuit at runtime (joint with TASK-0567).

## Invariants Touched

- None (pure CLI surface).

## Notes

- If TASK-0565 ships only three fields (skipping `max_pending_lifetime`), drop `--max-pending-lifetime` from this task and amend the test list accordingly.
- Cross-coordinate value-enum names with the SPEC-21 §4.x enum spelling (`RoundRobin` ↔ `round-robin`).
- Consumed by ad-hoc bench scripts and `relativist-cli/scripts/run_*.sh` — not on the test critical path.

## DAG Links

- **Predecessors:** TASK-0512, TASK-0565.
- **Successors:** none (terminal CLI leaf).
