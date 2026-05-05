# TASK-0603 — Add Tier 3 CLI flags to `BenchArgs` (Phase C-3)

**Phase:** C-3 (D-011 bench harness wiring — CLI surface)
**Bundle:** D-011 — Tier 3 Hardening + Bench Enablement
**Status:** PENDING
**Priority:** P0 (required for user-facing bench rodada in Phase F-2)
**Spec:** SPEC-09 R18a–R18g (committed `82b2d27`); SPEC-21 §3.8 A3 (GridConfig fields surfaced via CLI).
**Origin:** D-011 plan §C-3.
**Estimated complexity:** S (~40 LoC production + ~40 LoC tests)
**Estimated stages duration:** Stages 2→3→4→5→6 over ~0.5 day.

---

## Context

Per D-011 plan: `BenchArgs` in `relativist-core/src/config.rs:571-629` has 12 fields today, none for Tier 3. To run the bench rodada in Phase F-2 with the streaming path active, four flags must be surfaced via clap.

This task adds the flags + parses them into the new `BenchmarkSuiteConfig` fields landed by TASK-0602. It is a thin glue task with mostly mechanical clap derive + mapping logic.

## Dependencies

- **TASK-0602 (Phase C-1)** — REQUIRED. Provides the destination fields and enum types.
- **SPEC commit `82b2d27`** — already landed.

## Files in scope

| File | Change |
|------|--------|
| `relativist-core/src/config.rs:571-629` (`BenchArgs` struct) | Add 4 clap flags. |
| `relativist-core/src/config.rs` (`BenchArgs::to_suite_config()` or equivalent mapper) | Wire flags → `BenchmarkSuiteConfig` fields. |
| `relativist-core/tests/bench_cli_tier3_flags.rs` (new) | Parse-from-args round-trip tests for each flag, defaults, and invalid value rejection. |

## Files explicitly OUT of scope

- The downstream `BenchmarkSuiteConfig` definition — TASK-0602.
- Path selection in `bench/suite.rs` — TASK-0604.
- Memory probe — TASK-0605.

## Key flags

```
--chunk-size <N>             clap default: <none> (eager path)
--max-pending-lifetime <N>   clap default: 16 (matches coordinator CLI default)
--recycle-policy <POLICY>    enum: disable-under-delta | border-clean ; default: disable-under-delta
--representation <MODE>      enum: dense | sparse ; default: dense
```

## Acceptance criteria

1. Four new flags wired via clap derive with the exact names, types, and defaults above.
2. Enum flags use clap `value_enum` (kebab-case rendering: `disable-under-delta`, `border-clean`, `dense`, `sparse`).
3. Mapping function populates `BenchmarkSuiteConfig` fields from `BenchArgs` 1:1.
4. `relativist bench --help` displays the 4 new flags.
5. Invalid enum values produce a clap error (not a panic).
6. New parse-from-args tests cover: defaults, each flag set explicitly, each enum value, and invalid-value rejection.
7. All existing tests pass with zero regression.

## Test floor delta expected

**+5 to +8 tests** added (one per flag × default-set-invalid + a few combinatorial).

## Notes

- Pure additive on the CLI surface; backward-compat preserved for users who don't pass any new flag.
- After this lands, the smoke test `cargo run --bin relativist -- bench --benchmark ep_annihilation --sizes 1000 --workers 2 --chunk-size 100 --representation dense` runs end-to-end, hitting the eager path with the streaming config plumbed but inactive (`chunk_size = None` is still the default). To actually exercise the streaming path requires TASK-0604.
