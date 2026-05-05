# TASK-0416: CLI flags for elastic-grid configuration (R34)

**Spec:** SPEC-20 §3.4 R34.
**Requirements:** R34 (9 CLI flags exposing the new `GridConfig` fields).
**Priority:** P0.
**Status:** TODO
**Depends on:** TASK-0415 (GridConfig fields must exist).
**Blocked by:** TASK-0415.
**Estimated complexity:** S (~60-90 LoC CLI arg defs + 40 LoC tests)
**Bundle:** SPEC-20 Elastic Grid — config + wire foundations.

## Context

R34 mandates that the SPEC-07 CLI expose the 9 new `GridConfig` fields as clap arguments. Defaults mirror R33a. The flag names follow the existing `--flag / --no-flag` pattern for booleans (compatible with `clap`'s `ArgAction::Set`).

## Acceptance Criteria

- [ ] Add clap arguments per R34:
  - `--hybrid` / `--no-hybrid`
  - `--elastic-departure` / `--no-elastic-departure`
  - `--elastic-join` / `--no-elastic-join`
  - `--retain-partitions` / `--no-retain-partitions`
  - `--checkpoint-partitions` / `--no-checkpoint-partitions`
  - `--initial-wait-timeout <SECONDS>` (default 30)
  - `--join-window-min-ms <MS>` (default 50)
  - `--join-window-max-ms <MS>` (default 500)
  - `--solo-budget <N>` (default 10000)
- [ ] CLI-to-`GridConfig` mapping function updated (TASK-0102 extension).
- [ ] After mapping, `GridConfig::normalize(&mut config)` is called (TASK-0415) to apply derived defaults.
- [ ] `GridConfig::validate(&config)?` is called before the grid starts; validation errors exit with non-zero code and a clear message.
- [ ] Help text (`--help`) shows each flag with its default and a short SPEC-20 citation.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-cli/src/args.rs` *(or wherever clap Args live)* | modify | Add 9 argument definitions with `#[arg(long, default_value = ...)]`. |
| `relativist-cli/src/lib.rs` | modify | Extend `cli_to_grid_config` to propagate new fields into `GridConfig`. |

## Test Expectations (forward-ref)

TEST-SPEC-0416:

- `cli_defaults_match_r33a` — parsing `relativist run` (no flags) yields `GridConfig::default()`.
- `cli_hybrid_flag_sets_field` — `--hybrid` flips `hybrid_coordinator` to true.
- `cli_validation_rejects_inverted_join_window` — `--join-window-min-ms 500 --join-window-max-ms 50` exits non-zero.
- `cli_derived_elastic_join_when_hybrid` — `--hybrid` without `--elastic-join` still ends up with `elastic_join=true` after normalize.

## Invariants Touched

- None.

## Notes

- Every new flag is opt-in / backward compatible. v1 benchmark reproduction requires zero flag changes.

## DAG Links

- **Predecessors:** TASK-0415.
- **Successors:** all runtime SPEC-20 behavior is gated via these flags; EG-I5 verifies the v1-compatibility mode.
