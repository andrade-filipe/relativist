# TEST-SPEC-0603 — Tests for TASK-0603 — Tier 3 CLI flags on `BenchArgs`

**Task:** TASK-0603 (Phase C-3, P0)
**Spec:** SPEC-09 R18a–R18g (commit `82b2d27`); SPEC-21 §3.8 A3.
**Test floor delta:** **+8 default**.
**Prerequisites:** TASK-0602 (provides destination fields + enums).

---

## Test inventory

| test_id | level | target file | prerequisite TASK | cfg gates |
|---|---|---|---|---|
| UT-0603-01 | unit | `relativist-core/tests/bench_cli_tier3_flags.rs::parse_chunk_size_some_value` | TASK-0602 | none |
| UT-0603-02 | unit | `relativist-core/tests/bench_cli_tier3_flags.rs::parse_max_pending_lifetime_explicit_value` | TASK-0602 | none |
| UT-0603-03 | unit | `relativist-core/tests/bench_cli_tier3_flags.rs::parse_recycle_policy_each_variant` | TASK-0602 | none |
| UT-0603-04 | unit | `relativist-core/tests/bench_cli_tier3_flags.rs::parse_representation_each_variant` | TASK-0602 | none |
| UT-0603-05 | unit | `relativist-core/tests/bench_cli_tier3_flags.rs::defaults_when_flags_omitted` | TASK-0602 | none |
| UT-0603-06 | unit | `relativist-core/tests/bench_cli_tier3_flags.rs::invalid_recycle_policy_yields_clap_error` | TASK-0602 | none |
| UT-0603-07 | unit | `relativist-core/tests/bench_cli_tier3_flags.rs::invalid_representation_yields_clap_error` | TASK-0602 | none |
| UT-0603-08 | unit | `relativist-core/tests/bench_cli_tier3_flags.rs::bench_args_to_suite_config_mapping_is_one_to_one` | TASK-0602 | none |
| IT-0603-09 | integration | `relativist-core/tests/bench_cli_tier3_flags.rs::cli_smoke_chunk_size_100_workers_2_completes_zero` | TASK-0602, TASK-0604 (recommended) | `#[ignore = "smoke; requires bench harness wiring"]` if TASK-0604 not yet landed |

Total: **8 default tests** (+1 ignored smoke; not counted toward floor until TASK-0604 lands).

---

## Per-test specifications

### UT-0603-01 — `parse_chunk_size_some_value`

**Purpose.** `--chunk-size 1000` parses to `Some(1000)`.
**Setup.** Build a clap `try_parse_from` invocation:
`["relativist", "bench", "--benchmark", "ep_annihilation", "--sizes", "1000", "--workers", "2", "--chunk-size", "1000"]`.
**Action.** Parse via the project's CLI entry struct.
**Assertions.**
- `args.chunk_size == Some(1000_u32)`.
- `args.benchmark == "ep_annihilation"` (sanity — surrounding flags still work).
**Boundary case coverage.** Catches a buggy clap derive that types `chunk_size: u32` instead of `Option<u32>` and silently defaults to `0`.
**Why it must exist.** Acceptance criterion #1 of TASK-0603.

---

### UT-0603-02 — `parse_max_pending_lifetime_explicit_value`

**Purpose.** `--max-pending-lifetime 32` parses to `32`.
**Setup.** `["relativist", "bench", ..., "--max-pending-lifetime", "32"]`.
**Action.** Parse.
**Assertions.**
- `args.max_pending_lifetime == 32_u32`.
**Boundary case coverage.** Distinct from default-test (UT-0603-05); catches a buggy default that ignores the flag.
**Why it must exist.** Acceptance criterion #1.

---

### UT-0603-03 — `parse_recycle_policy_each_variant`

**Purpose.** Both kebab-case enum variants parse correctly via clap `value_enum`.
**Setup.** Two parses:
- `["relativist", "bench", ..., "--recycle-policy", "disable-under-delta"]`
- `["relativist", "bench", ..., "--recycle-policy", "border-clean"]`
**Action.** Parse each.
**Assertions.**
- First parse: `args.recycle_policy == RecyclePolicy::DisableUnderDelta`.
- Second parse: `args.recycle_policy == RecyclePolicy::BorderClean`.
**Boundary case coverage.** Catches a buggy clap derive that uses snake_case (`disable_under_delta`) instead of kebab-case rendering.
**Why it must exist.** Acceptance criterion #2 of TASK-0603.

---

### UT-0603-04 — `parse_representation_each_variant`

**Purpose.** Both `NetRepresentation` variants parse via kebab-case.
**Setup.** Two parses:
- `["relativist", "bench", ..., "--representation", "dense"]`
- `["relativist", "bench", ..., "--representation", "sparse"]`
**Action.** Parse.
**Assertions.**
- `args.representation == NetRepresentation::Dense` and `NetRepresentation::Sparse` respectively.
**Boundary case coverage.** Symmetric to UT-0603-03.
**Why it must exist.** Acceptance criterion #2.

---

### UT-0603-05 — `defaults_when_flags_omitted`

**Purpose.** When the 4 new flags are NOT passed, the parsed `BenchArgs` carries the spec-defaults (matching the `BenchmarkSuiteConfig` defaults from TASK-0602).
**Setup.** `["relativist", "bench", "--benchmark", "ep_annihilation", "--sizes", "1000", "--workers", "2"]` (no Tier 3 flags).
**Action.** Parse.
**Assertions.**
- `args.chunk_size == None`.
- `args.max_pending_lifetime == 16`.
- `args.recycle_policy == RecyclePolicy::DisableUnderDelta`.
- `args.representation == NetRepresentation::Dense`.
**Boundary case coverage.** Catches a buggy clap derive that requires the new flags (regression: existing bench scripts break).
**Why it must exist.** TASK-0603 §Notes: "backward-compat preserved for users who don't pass any new flag" — this is the regression guard.

---

### UT-0603-06 — `invalid_recycle_policy_yields_clap_error`

**Purpose.** An unrecognized enum value produces a clap parse error, NOT a panic.
**Setup.** `["relativist", "bench", ..., "--recycle-policy", "destroy-everything"]`.
**Action.** `let result = Cli::try_parse_from(args);` (note: `try_parse_from`, not `parse_from`).
**Assertions.**
- `result.is_err()`.
- `result.unwrap_err().kind() == clap::error::ErrorKind::InvalidValue` (or the project's clap version equivalent).
- The error string contains the offending value `"destroy-everything"` AND lists the valid alternatives `disable-under-delta`, `border-clean`.
**Boundary case coverage.** Catches a buggy fallback (`unwrap_or(RecyclePolicy::DisableUnderDelta)`) that swallows invalid input.
**Why it must exist.** Acceptance criterion #5 of TASK-0603.

---

### UT-0603-07 — `invalid_representation_yields_clap_error`

**Purpose.** Symmetric to UT-0603-06 for `--representation`.
**Setup.** `["relativist", "bench", ..., "--representation", "quantum"]`.
**Action.** `try_parse_from`.
**Assertions.**
- `result.is_err()`.
- Error mentions `"quantum"` and lists valid: `dense`, `sparse`.
**Boundary case coverage.** Same.
**Why it must exist.** Same.

---

### UT-0603-08 — `bench_args_to_suite_config_mapping_is_one_to_one`

**Purpose.** Verify the mapping function `BenchArgs::to_suite_config()` (or the equivalent `From<&BenchArgs>` impl) populates `BenchmarkSuiteConfig` 1:1 from the 4 new flags.
**Setup.** Construct `BenchArgs` with all 4 fields explicitly set:
- `chunk_size = Some(500)`, `max_pending_lifetime = 64`, `recycle_policy = BorderClean`, `representation = Sparse`.
**Action.** Invoke the mapping fn.
**Assertions.**
- `suite_config.chunk_size == Some(500)`.
- `suite_config.max_pending_lifetime == 64`.
- `suite_config.recycle_policy == RecyclePolicy::BorderClean`.
- `suite_config.representation == NetRepresentation::Sparse`.
- `suite_config` other (existing) fields unchanged from `BenchArgs::to_suite_config` semantics — sanity guard via `cloned == derived` on a baseline.
**Boundary case coverage.** Catches a buggy mapping that swaps two fields (e.g., assigns `chunk_size` from `max_pending_lifetime`).
**Why it must exist.** Acceptance criterion #3 of TASK-0603 ("Mapping function populates `BenchmarkSuiteConfig` fields from `BenchArgs` 1:1").

---

### IT-0603-09 — `cli_smoke_chunk_size_100_workers_2_completes_zero` (smoke; gated)

**Purpose.** End-to-end CLI smoke from the D-011 plan.
**Setup.** None (uses `assert_cmd::Command` or `std::process::Command` to exec the workspace bin).
**Action.** Run: `cargo run --bin relativist -- bench --benchmark ep_annihilation --sizes 1000 --workers 2 --chunk-size 100`. Capture exit code and stdout.
**Assertions.**
- Exit code == 0.
- Stdout (or generated CSV) contains the literal `"ep_annihilation"`.
- Run completes in < 60 s wall-clock.
**Boundary case coverage.** Catches a wiring break between CLI parsing and the bench harness (the failure mode where the flag parses but is silently ignored).
**cfg gate.** Mark `#[ignore]` until TASK-0604 lands and the streaming path is live. After TASK-0604, this becomes part of the smoke floor.
**Why it must exist.** Acceptance criterion #4 of TASK-0603 ("`relativist bench --help` displays the 4 new flags") + plan §C-3 smoke.

---

## Coverage matrix

| test_id | TASK §AC-1 | §AC-2 | §AC-3 | §AC-4 | §AC-5 | §AC-6 |
|---|---|---|---|---|---|---|
| UT-0603-01 | ✅ | | | | | ✅ |
| UT-0603-02 | ✅ | | | | | ✅ |
| UT-0603-03 | ✅ | ✅ | | | | ✅ |
| UT-0603-04 | ✅ | ✅ | | | | ✅ |
| UT-0603-05 | ✅ | | | | | ✅ |
| UT-0603-06 | | | | | ✅ | ✅ |
| UT-0603-07 | | | | | ✅ | ✅ |
| UT-0603-08 | | | ✅ | | | ✅ |
| IT-0603-09 | | | | ✅ | | |

Every acceptance criterion has ≥1 test.

---

## Out-of-scope tests (deferred to other tasks)

- Tests of the underlying `BenchmarkSuiteConfig` defaults → **TASK-0602**.
- Path selection between eager/streaming based on parsed flags → **TASK-0604**.
- CSV column emission for new metrics → **TASK-0605** (R18d) and existing CSV writer infra.
- Memory probe correctness → **TASK-0605**.
