# TEST-SPEC-0416: CLI flags for elastic-grid configuration (R34)

**SPEC-20 §7 ID:** none direct (TASK-0416 is plumbing; runtime behaviour is gated by EG-I5).
**Owning task:** TASK-0416.
**Parent spec:** SPEC-07 CLI; SPEC-20 R34.
**Type:** unit (CLI parsing).

---

## Inputs / Fixtures

- `clap`-derived `Args` struct from `relativist-cli`.
- Test argv vectors representing different invocations of `relativist run`.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0416-01 | `cli_defaults_match_r33a` | argv `["relativist", "run"]` (no elastic flags) | parse → map to `GridConfig` → normalize → validate | resulting `GridConfig == GridConfig::default()` after normalize. |
| UT-0416-02 | `cli_hybrid_flag_sets_field` | argv `["relativist", "run", "--hybrid"]` | parse → map → normalize | `c.hybrid_coordinator == true`; `c.elastic_join == true` (derived). |
| UT-0416-03 | `cli_no_hybrid_flag_unsets_field` | argv `["relativist", "run", "--no-hybrid"]` | parse → map | `c.hybrid_coordinator == false`. |
| UT-0416-04 | `cli_elastic_departure_flag_sets_field` | argv `["relativist", "run", "--elastic-departure"]` | parse → map → normalize | `c.elastic_departure == true`; `c.retain_partitions == true` (derived); `c.elastic_join == true` (derived). |
| UT-0416-05 | `cli_initial_wait_timeout_seconds` | argv `[..., "--initial-wait-timeout", "60"]` | parse → map | `c.initial_wait_timeout == Duration::from_secs(60)`. |
| UT-0416-06 | `cli_join_window_min_max_ms` | argv `[..., "--join-window-min-ms", "100", "--join-window-max-ms", "1000"]` | parse → map | `c.join_window_min == Duration::from_millis(100)`; `c.join_window_max == Duration::from_millis(1000)`. |
| UT-0416-07 | `cli_solo_budget_value` | argv `[..., "--solo-budget", "5000"]` | parse → map | `c.solo_budget == 5000`. |
| UT-0416-08 | `cli_validation_rejects_inverted_join_window` | argv `[..., "--join-window-min-ms", "500", "--join-window-max-ms", "50"]` | parse → map → validate | validate returns `Err(ConfigError::JoinWindowOrdering)`. The CLI front-end then exits with a non-zero code; assert via the function's exit-code path or an early `Result::Err` propagation. |
| UT-0416-09 | `cli_validation_rejects_explicit_no_retain_with_elastic_departure` | argv `[..., "--elastic-departure", "--no-retain-partitions"]` | parse → map → normalize → validate | validate returns `Err(ConfigError::RetainRequiredForDeparture)` because the explicit `--no-retain-partitions` overrides the derive. |
| UT-0416-10 | `cli_help_mentions_each_flag` | run `clap` `--help` rendering | inspect generated help text | All 9 flags appear; each shows its default value; SPEC-20 short citation present (e.g., `"R34"`). |
| UT-0416-11 | `cli_v1_baseline_invocation_is_byte_identical_to_default` | argv `["relativist", "run"]` and argv `[..., "--no-hybrid", "--no-elastic-departure", "--no-elastic-join", "--no-retain-partitions", "--no-checkpoint-partitions"]` | parse both, normalize both | Both produce `GridConfig::default()`. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | `--initial-wait-timeout 0` | Parses; downstream `validate` may reject (see TEST-SPEC-0415 EC-2). |
| EC-2 | `--solo-budget 0` | Parses; `validate` returns `Err(ConfigError::SoloBudgetZero)`. |
| EC-3 | A boolean flag passed twice (e.g., `--hybrid --no-hybrid`) | `clap` resolves with last-wins or rejects (depends on `ArgAction`); test the chosen behaviour explicitly. |

## Invariants asserted

None.

## ARG/DISC/REF citation

None.

## Determinism notes

Pure synchronous CLI parse; deterministic.

## Cross-test dependencies

- Mirrors TEST-SPEC-0415 (every default tested at the GridConfig layer is also tested here at the CLI layer).
