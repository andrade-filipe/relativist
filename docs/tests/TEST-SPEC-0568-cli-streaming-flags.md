# TEST-SPEC-0568: CLI streaming flags (`--chunk-size`, `--streaming-strategy`, `--fennel-alpha`, `--dispatch-mode`, optional `--max-pending-lifetime`)

**SPEC-21 §7 ID:** plumbing only (CLI surface for SPEC-21 GridConfig fields).
**Owning task:** TASK-0568.
**Parent spec:** SPEC-21 §3.3 R24, §3.4 R25, §3.6 R34, §3.7 R37g (CLI parity for GridConfig fields per SPEC-07 R-N convention); §3.8 A3 (consumer of TASK-0512 / TASK-0565).
**Type:** unit (clap parser + value-enum mapping + conflict rules).
**Theory anchor:** None direct (CLI surface).

---

## Inputs / Fixtures

- The `clap` derive struct from `relativist-cli/src/main.rs` post-TASK-0568 landing.
- Argv vectors constructed as `Vec<&str>` for each scenario.
- The `try_parse_from(...)` clap entry point (returns `Result<Args, clap::Error>`).
- The `GridConfig` mapping helper that converts parsed `StreamingArgs` to `GridConfig` field values.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0568-01 | `default_chunk_size_is_10000` | `try_parse_from(["bin"])` (no flags) | inspect parsed `chunk_size` | `== 10_000` (R24 default mirrored from GridConfig). |
| UT-0568-02 | `default_streaming_strategy_is_round_robin` | same | inspect `streaming_strategy` | parses to `StreamingStrategyArg::RoundRobin`; maps to `StreamingStrategyConfig::RoundRobin`. |
| UT-0568-03 | `default_dispatch_mode_is_auto` | same | inspect `dispatch_mode` | `DispatchModeArg::Auto`; maps to `DispatchMode::Auto`. |
| UT-0568-04 | `default_max_pending_lifetime_is_16` (CONDITIONAL on TASK-0565 shipping the optional fourth field) | same | inspect `max_pending_lifetime` | `== 16`. If TASK-0565 omits the field, this test is REMOVED and the test list updated per TASK-0568 NOTE line 79-80. |
| UT-0568-05 | `chunk_size_flag_parses_explicit_value` | `["bin", "--chunk-size", "256"]` | `try_parse_from(...)` | `Ok`; `chunk_size == 256`. |
| UT-0568-06 | `chunk_size_flag_accepts_u32_max` | `["bin", "--chunk-size", "4294967295"]` | parse | `Ok`; `chunk_size == u32::MAX`. (Joint with TEST-SPEC-0567 R26 short-circuit at runtime.) |
| UT-0568-07 | `streaming_strategy_round_robin_parses` | `["bin", "--streaming-strategy", "round-robin"]` | parse | `Ok`; maps to `StreamingStrategyConfig::RoundRobin` (note kebab-case ↔ PascalCase mapping per TASK-0568 NOTE line 80). |
| UT-0568-08 | `streaming_strategy_fennel_with_alpha_parses` | `["bin", "--streaming-strategy", "fennel", "--fennel-alpha", "1.5"]` | parse | `Ok`; maps to `StreamingStrategyConfig::Fennel { alpha: 1.5 }`. |
| UT-0568-09 | `streaming_strategy_fennel_without_alpha_uses_default` | `["bin", "--streaming-strategy", "fennel"]` | parse | depends on clap rule: either rejects (if `--fennel-alpha` is required-when-fennel) OR maps to `Fennel { alpha: 1.0 }` (REF-TBD default per SC-020). The TASK-0568 acceptance criterion uses `requires` semantics. Test asserts whichever is documented; default behavior is REJECTION until SC-020 closes. |
| UT-0568-10 | `streaming_strategy_round_robin_with_fennel_alpha_rejected` | `["bin", "--streaming-strategy", "round-robin", "--fennel-alpha", "1.0"]` | parse | `Err(clap::Error)` with `ErrorKind::ArgumentConflict` (clap `conflicts_with` rule). (R25 acceptance criterion.) |
| UT-0568-11 | `dispatch_mode_auto_parses` | `["bin", "--dispatch-mode", "auto"]` | parse | `Ok`; `DispatchMode::Auto`. |
| UT-0568-12 | `dispatch_mode_push_parses` | `["bin", "--dispatch-mode", "push"]` | parse | `Ok`; `DispatchMode::Push`. |
| UT-0568-13 | `dispatch_mode_pull_parses` | `["bin", "--dispatch-mode", "pull"]` | parse | `Ok`; `DispatchMode::Pull`. |
| UT-0568-14 | `max_pending_lifetime_flag_parses` (CONDITIONAL) | `["bin", "--max-pending-lifetime", "32"]` | parse | `Ok`; `max_pending_lifetime == 32`. |
| UT-0568-15 | `help_output_documents_all_flags` | `["bin", "--help"]` | parse | `Err(clap::Error)` with `ErrorKind::DisplayHelp`; the rendered help text contains `--chunk-size`, `--streaming-strategy`, `--fennel-alpha`, `--dispatch-mode` (and `--max-pending-lifetime` if shipped). |
| UT-0568-16 | `help_output_documents_chunk_size_sc024_caveat` | `["bin", "--help"]` | parse | rendered help contains the substring "benchmark calibration" or "Q2" or "TBD" near the `--chunk-size` line per TASK-0568 acceptance criterion. (R24 doc-tag transitively enforced at the CLI surface.) |
| UT-0568-17 | `cli_args_map_to_gridconfig` | `["bin", "--chunk-size", "256", "--streaming-strategy", "fennel", "--fennel-alpha", "1.5", "--dispatch-mode", "pull"]` | parse + `to_grid_config(args)` | `cfg.chunk_size == 256`; `cfg.streaming_strategy == Fennel { alpha: 1.5 }`; `cfg.dispatch_mode == Pull`. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | `--chunk-size 0` | clap accepts; runtime `GridConfig::validate()` rejects (per TEST-SPEC-0565 EC-1). The CLI parser DOES NOT validate value semantics. |
| EC-2 | `--streaming-strategy round_robin` (snake_case) | clap rejects (kebab-case is the value-enum spelling per TASK-0568 NOTE line 80). |
| EC-3 | `--dispatch-mode AUTO` (uppercase) | clap rejects with case-sensitivity (clap value_enum is case-sensitive by default). Document the lowercase contract explicitly. |
| EC-4 | Negative `--fennel-alpha -1.0` | clap accepts (f32 supports negative); semantic validation is downstream (FennelStreamingStrategy may reject). |
| EC-5 | `--chunk-size 4294967296` (above u32) | clap rejects with `ErrorKind::InvalidValue`. |
| EC-6 | Conflicting `--chunk-size 4294967295 --dispatch-mode pull` | both parse cleanly; runtime warning emitted at orchestrator entry per TEST-SPEC-0512 EC-3. CLI does NOT reject. |

## Invariants asserted

- R24 / R25 / R34 (CLI parity per SPEC-07 R-N convention).
- R37g (CLI parity for max_pending_lifetime; conditional).
- §3.8 A3 (SPEC-07 amendment consumed at the CLI surface).

## ARG/DISC/REF citation

- None direct (CLI surface).

## Determinism notes

Pure synchronous clap parsing; no tokio, no RNG. All tests are `#[test]` plain.

The `--fennel-alpha` defaulting policy (UT-0568-09) is GATED on SC-020 closure. Until SC-020 closes (REF-TBD for FENNEL/LDG references at TCC root), the default for `--streaming-strategy fennel` without `--fennel-alpha` is REJECTION (clap `requires` rule); document this explicitly in the test docstring.

The `max_pending_lifetime` field (UT-0568-04, UT-0568-14) is CONDITIONAL on TASK-0565 shipping the optional fourth field. If TASK-0565 omits it, the developer MUST remove these two tests AND the `--max-pending-lifetime` flag from the clap derive. Test-list maintenance is the developer's responsibility per TASK-0568 NOTE line 79-80.

## Cross-test dependencies

- TEST-SPEC-0512 (SPEC-07 amendment-level coverage) — establishes the GridConfig field semantics that the CLI mirrors.
- TEST-SPEC-0565 (GridConfig production fields) — predecessor; CLI maps onto these fields.
- TEST-SPEC-0567 (R26 short-circuit) — joint coverage for `--chunk-size 4294967295` runtime behavior (UT-0568-06 + EC-6).
- TEST-SPEC-0531 (FennelStreamingStrategy) — provides the alpha-default behavior consumed by UT-0568-09.
- TEST-SPEC-0577 / TEST-SPEC-0578 (FSM gating on `dispatch_mode`) — consumer; CLI's `--dispatch-mode` flag drives the FSM branch.
